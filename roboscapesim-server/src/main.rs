use anyhow::Result;
use axum::{response::IntoResponse, routing::post, Json, Router, http::{Method, header}};
use chrono::Utc;
use cyberdeck::*;
mod room;
use roboscapesim_common::ClientMessage;
use room::RoomData;
mod robot;
use simple_logger::SimpleLogger;
use std::{net::SocketAddr, sync::Arc};
use tokio::{time::{sleep, Duration, self}, task, sync::Mutex};
use tower_http::cors::{Any, CorsLayer};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use log::{info, trace, error};

#[path = "./util/mod.rs"]
mod util;

static ROOMS: Lazy<DashMap<String, Arc<Mutex<RoomData>>>> = Lazy::new(|| {
    DashMap::new()
});

pub static CLIENTS: Lazy<DashMap<u128, Arc<RTCDataChannel>>> = Lazy::new(|| {
    DashMap::new()
});

#[tokio::main]
async fn main() {
    // Setup logger
    SimpleLogger::new().with_level(log::LevelFilter::Warn).with_module_level("roboscapesim_server", log::LevelFilter::Info).env().init().unwrap();
    info!("Starting RoboScape Online Server...");

    // build our application with a route
    let app = Router::new()
        .route("/connect", post(connect))
	.layer(CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods([Method::GET, Method::POST])
        // allow requests from any origin
        .allow_origin(Any)
	.allow_headers([header::CONTENT_TYPE]));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("Running server on http://localhost:3000 ...");

    let server = axum::Server::bind(&addr)
        .serve(app.into_make_service());

    let update_fps = 30;

    let _update_loop = task::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(1000 / update_fps));

        loop {
            interval.tick().await;

            let update_time = Utc::now();

            // Perform updates
            for kvp in ROOMS.iter() {
                let mut lock = kvp.value().lock().await;
                if !lock.hibernating {
                    trace!("Updating {}", kvp.key());

                    // Check timeout
                    if update_time.timestamp() - lock.last_interaction_time > lock.timeout {
                        lock.hibernating = true;
                        info!("{} is now hibernating", kvp.key());
                        return;
                    }

                    // Perform update
                    lock.update(1.0 / update_fps as f64).await;
                }
            }
        }
    });

    if let Err(err) = server.await {
        error!("server error: {}", err);
    }
}

async fn connect(Json(offer): Json<String>) -> impl IntoResponse {
    match start_peer_connection(offer).await {
        Ok(answer) => Ok(Json(answer)),
        Err(_) => Err("failed to connect"),
    }
}

async fn start_peer_connection(offer: String) -> Result<String> {
    // Temporarily, create a new room for each peer
    let room_id = create_room(None).await;

    let mut peer = Peer::new(move |peer_id, e| {
        let room_id = room_id.clone();
        
        async move {
            let room = ROOMS.get(&(room_id.to_string())).unwrap();
            match e {
                PeerEvent::DataChannelMessage(c, m) => {
                    trace!(
                        "{}::Recieved a message from channel {} with id {}!",
                        peer_id,
                        c.label(),
                        c.id()
                    );
                    let msg_str = String::from_utf8(m.data.to_vec()).unwrap();
                    trace!(
                        "{}::Message from DataChannel '{}': {}",
                        peer_id,
                        c.label(),
                        msg_str
                    );

                    if let Ok(c) = serde_json::from_str::<ClientMessage>(&msg_str) {
                        trace!("Client message: {:?}", c);
                        match c {
                            ClientMessage::Heartbeat => {},
                            ClientMessage::ResetAll => {
                                room.lock().await.reset();
                            },
                            ClientMessage::ResetRobot(r) => {
                                room.lock().await.reset_robot(r.as_str());
                            },
                            ClientMessage::ClaimRobot(_) => todo!(),
                        }
                    }
                    //c.send_text(format!("Echo {}", msg_str)).await.unwrap();
                }
                PeerEvent::DataChannelStateChange(c) => {
                    if c.ready_state() == RTCDataChannelState::Open {
                        CLIENTS.insert(peer_id, c.clone());
                        trace!("{}::DataChannel '{}'", peer_id, c.label());
                        room.lock().await.sockets.insert(peer_id.to_string(), peer_id);
                        room.lock().await.send_state_to_client(true, peer_id).await;
                    } else if c.ready_state() == RTCDataChannelState::Closed {
                        trace!("{}::DataChannel '{}'", peer_id, c.label());
                    }
                }
                PeerEvent::PeerConnectionStateChange(s) => {
                    trace!("{}::Peer connection state: {} ", peer_id, s);

                    // Remove bad peers
                    if s == RTCPeerConnectionState::Disconnected || s == RTCPeerConnectionState::Closed || s == RTCPeerConnectionState::Failed {
                        
                    }
                }
            }
        }
    })
    .await?;
    let answer = peer.receive_offer(&offer).await?;

    // move cyberdeck to another thread to keep it alive
    tokio::spawn(async move {
        while peer.connection_state() != RTCPeerConnectionState::Closed
            && peer.connection_state() != RTCPeerConnectionState::Disconnected
            && peer.connection_state() != RTCPeerConnectionState::Failed
        {
            // keep the connection alive while not in invalid state
            sleep(Duration::from_millis(1000)).await;
        }
        // because we moved cyberdeck ownership into here gets dropped here and will stop all channels
    });

    Ok(answer)
}

async fn create_room(password: Option<String>) -> String {
    let room = Arc::new(Mutex::new(RoomData::new(None, password)));
    
    // Set last interaction to creation time
    room.lock().await.last_interaction_time = Utc::now().timestamp();

    let room_id = room.lock().await.name.clone();
    ROOMS.insert(room_id.to_string(), room.clone());
    room_id
}