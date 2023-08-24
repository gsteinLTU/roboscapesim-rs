use anyhow::Result;
use axum::{routing::{post, get}, Router, http::{Method, header}};
use chrono::Utc;
use derivative::Derivative;
use futures::SinkExt;
use roboscapesim_common::{ClientMessage, UpdateMessage};
use room::RoomData;
use simple_logger::SimpleLogger;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::{time::{Duration, self}, task, sync::{Mutex, broadcast::{Sender, Receiver, self}, RwLock}, net::{TcpStream, TcpListener}};
use tower_http::cors::{Any, CorsLayer};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use log::{info, trace, error};
use tokio_websockets::{WebsocketStream, Message};

use crate::api::{server_status, rooms_list, get_external_ip, EXTERNAL_IP, post_create};

mod room;
mod robot;
mod simulation;
mod api;

#[path = "./util/mod.rs"]
mod util;

#[path = "./services/mod.rs"]
mod services;

const MAX_ROOMS: usize = 64;

static ROOMS: Lazy<DashMap<String, Arc<RwLock<RoomData>>>> = Lazy::new(|| {
    DashMap::new()
});

#[derive(Derivative)]
#[derivative(Debug)]
pub struct SocketInfo {
    /// To client
    pub tx: Arc<Mutex<Sender<UpdateMessage>>>, 
    /// To server, internal use
    pub tx1: Arc<Mutex<Sender<ClientMessage>>>, 
    /// From client
    pub rx: Arc<Mutex<Receiver<ClientMessage>>>, 
    /// From client, internal use
    pub rx1: Arc<Mutex<Receiver<UpdateMessage>>>, 
    #[derivative(Debug = "ignore")]
    pub stream: Arc<Mutex<WebsocketStream<TcpStream>>>,
}

pub static CLIENTS: Lazy<DashMap<u128, SocketInfo>> = Lazy::new(|| {
    DashMap::new()
});

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    // Setup logger
    SimpleLogger::new().with_level(log::LevelFilter::Error).with_module_level("roboscapesim_server", log::LevelFilter::Trace).env().init().unwrap();
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

    let _ws_loop = task::spawn(async move {
        let listener = TcpListener::bind("0.0.0.0:5000").await.unwrap();

        loop {
            let (conn, _) = listener.accept().await.unwrap();
            accept_connection(conn).await;
        }
    });

    let _ws_update_loop = task::spawn(async move {
        loop {
            // Get client updates
            for client in CLIENTS.iter() {
                // RX
                if let Some(Ok(msg)) = client.value().stream.lock().await.next().await {
                    trace!("Websocket message from {}: {:?}", client.key(), msg);
                    if let Ok(msg) = msg.as_text() {

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
                        if let Ok(msg) = client.rx1.lock().await.recv().await {
                            let msg = serde_json::to_string(&msg).unwrap();
                            client.stream.lock().await.send(Message::text(msg)).await.unwrap();
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

async fn accept_connection(stream: TcpStream) -> u128 {
    let addr = stream.peer_addr().expect("connected streams should have a peer address");
    info!("Peer address: {}", addr);

    let ws_stream = tokio_websockets::ServerBuilder::new().accept(stream)
        .await
        .expect("Error during the websocket handshake occurred");
    
    let id = rand::random();
    info!("New WebSocket connection id {} ({})", id, addr);
    
    let (tx, rx1) = broadcast::channel(16);
    let (tx1, rx) = broadcast::channel(16);
    CLIENTS.insert(id, SocketInfo { 
        tx: Arc::new(Mutex::new(tx)), 
        tx1: Arc::new(Mutex::new(tx1)), 
        rx: Arc::new(Mutex::new(rx)), 
        rx1: Arc::new(Mutex::new(rx1)), 
        stream: Arc::new(Mutex::new(ws_stream))
    });
    id
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