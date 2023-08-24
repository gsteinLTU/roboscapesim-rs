use anyhow::Result;
use axum::{routing::{post, get}, Router, http::{Method, header}};
use chrono::Utc;
use roboscapesim_common::ClientMessage;
use room::RoomData;
use simple_logger::SimpleLogger;
use socket::SocketInfo;
use tokio_tungstenite::tungstenite::Message;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::{time::{Duration, self}, task, sync::RwLock, net::TcpListener};
use tower_http::cors::{Any, CorsLayer};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use log::{info, trace, error};
use futures::{SinkExt, StreamExt, FutureExt};

use crate::{api::{server_status, rooms_list, get_external_ip, EXTERNAL_IP, post_create}, socket::accept_connection};

mod room;
mod robot;
mod simulation;
mod api;

#[path = "./util/mod.rs"]
mod util;

#[path = "./services/mod.rs"]
mod services;
mod socket;

const MAX_ROOMS: usize = 64;

static ROOMS: Lazy<DashMap<String, Arc<RwLock<RoomData>>>> = Lazy::new(|| {
    DashMap::new()
});

pub static CLIENTS: Lazy<DashMap<u128, SocketInfo>> = Lazy::new(|| {
    DashMap::new()
});

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    // Setup logger
    SimpleLogger::new().with_level(log::LevelFilter::Error).with_module_level("roboscapesim_server", log::LevelFilter::Info).env().init().unwrap();
    info!("Starting RoboScape Online Server...");

    if let Ok(ip) = get_external_ip().await {
        let _ = EXTERNAL_IP.lock().unwrap().insert(ip.trim().into());
    }

    // build our application with a route
    let app = Router::new()
    .route("/server/status", get(server_status))
    .route("/rooms/list", get(rooms_list))
    .route("/rooms/create", post(post_create))
	.layer(CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods([Method::GET, Method::POST])
        // allow requests from any origin
        .allow_origin(Any)
	.allow_headers([header::CONTENT_TYPE]));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("Running server on port 3000 ...");

    let server = axum::Server::bind(&addr)
        .serve(app.into_make_service());

    let update_fps = 30;

    // Loop listening for new WS connections
    let _ws_loop = task::spawn(async move {
        let listener = TcpListener::bind("0.0.0.0:5000").await.unwrap();

        loop {
            let (conn, _) = listener.accept().await.unwrap();
            accept_connection(conn).await;
        }
    });

    // Loop sending/receiving and adding to channels
    let _ws_update_loop = task::spawn(async move {
        loop {
            // Get client updates
            for client in CLIENTS.iter() {

                // RX
                if let Some(Some(Ok(msg))) = client.value().stream.lock().await.next().now_or_never() {
                    trace!("Websocket message from {}: {:?}", client.key(), msg);
                    if let Ok(msg) = msg.to_text() {

                        if let Ok(msg) = serde_json::from_str::<ClientMessage>(msg) {
                            match msg {
                                ClientMessage::JoinRoom(id, username, password) => {
                                    join_room(&username, &(password.unwrap_or_default()), client.key().to_owned(), &id).await.unwrap();
                                },
                                _ => {
                                    client.tx1.lock().await.send(msg.to_owned()).unwrap();
                                }
                            }
                        }                    
                    }
                }
                
                // TX
                if client.rx1.lock().await.len() > 0 {
                    while client.rx1.lock().await.len() > 0 {
                        let recv = client.rx1.lock().await.try_recv();
                        
                        if let Ok(msg) = recv {
                            let msg = serde_json::to_string(&msg).unwrap();
                            client.sink.lock().await.send(Message::Text(msg)).now_or_never();
                        }
                    }
                }
            }
        }
    });

    let _update_loop = task::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(1000 / update_fps));

        loop {
            interval.tick().await;

            let update_time = Utc::now();
            
            // Perform updates
            for kvp in ROOMS.iter() {
                let mut lock = kvp.value().write().await;
                if !lock.hibernating {
                    // Check timeout
                    if update_time.timestamp() - lock.last_interaction_time > lock.timeout {
                        lock.hibernating = true;
                        info!("{} is now hibernating", kvp.key());
                        return;
                    }
                }

                // Perform update
                lock.update(1.0 / update_fps as f64).await;
            }
        }
    });

    if let Err(err) = server.await {
        error!("server error: {}", err);
    }
}

async fn join_room(username: &str, password: &str, peer_id: u128, room_id: &str) -> Result<(), String> {
    info!("User {} (peer id {}), attempting to join room {}", username, peer_id, room_id);

    if !ROOMS.contains_key(room_id) {
        return Err(format!("Room {} does not exist!", room_id));
    }

    let room = ROOMS.get(room_id).unwrap();
    let room = room.read().await;
    
    // Check password
    if room.password.clone().is_some_and(|pass| pass != password) {
        return Err("Wrong password!".to_owned());
    }
    
    // Setup connection to room
    room.visitors.insert(username.to_owned());
    room.sockets.insert(peer_id.to_string(), peer_id);
    room.send_info_to_client(peer_id).await;
    room.send_state_to_client(true, peer_id).await;
    Ok(())
}

async fn create_room(password: Option<String>) -> String {
    let room = Arc::new(RwLock::new(RoomData::new(None, password)));
    
    // Set last interaction to creation time
    room.write().await.last_interaction_time = Utc::now().timestamp();

    let room_id = room.read().await.name.clone();
    ROOMS.insert(room_id.to_string(), room.clone());
    room_id
}