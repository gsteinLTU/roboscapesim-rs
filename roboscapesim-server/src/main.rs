use anyhow::Result;
use axum::{routing::{post, get}, Router, http::{Method, header}};
use chrono::Utc;
use room::RoomData;
use simple_logger::SimpleLogger;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::{time::{sleep, Duration, self}, task, sync::Mutex, net::{TcpStream, TcpListener}};
use tower_http::cors::{Any, CorsLayer};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use log::{info, trace, error};
use tokio_websockets::WebsocketStream;

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

static ROOMS: Lazy<DashMap<String, Arc<Mutex<RoomData>>>> = Lazy::new(|| {
    DashMap::new()
});

pub static CLIENTS: Lazy<DashMap<u128, Arc<Mutex<WebsocketStream<TcpStream>>>>> = Lazy::new(|| {
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

    let _ws_loop = task::spawn(async move {
        let listener = TcpListener::bind("0.0.0.0:5000").await.unwrap();

        loop {
            let (conn, _) = listener.accept().await.unwrap();
            accept_connection(conn).await;
        }
    });

    let _update_loop = task::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(1000 / update_fps));

        loop {
            interval.tick().await;

            let update_time = Utc::now();

            // Perform updates
            for kvp in ROOMS.iter() {
                let mut lock = kvp.value().lock().await;
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

            // Get client updates
            for client in CLIENTS.iter() {
                if let Some(msg) = client.value().lock().await.next().await {
                    info!("Websocket message from {}: {:?}", client.key(), msg);
                }
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

    let mut ws_stream = tokio_websockets::ServerBuilder::new().accept(stream)
        .await
        .expect("Error during the websocket handshake occurred");

    
    let id = rand::random();
    info!("New WebSocket connection id {} ({})", id, addr);
    
    CLIENTS.insert(id, Arc::new(Mutex::new(ws_stream)));
    id
}

// async fn start_peer_connection(offer: String) -> Result<String> {
//     let room_id: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    
//     let peer = Arc::new(Peer::new(move |peer_id, e| {
//         let room_id = room_id.clone();
//         async move {

//             //room.lock().await.visitors.insert(username);
//             match e {
//                 PeerEvent::DataChannelMessage(c, m) => {
//                     trace!(
//                         "{}::Recieved a message from channel {} with id {}!",
//                         peer_id,
//                         c.label(),
//                         c.id()
//                     );
//                     let msg_str = String::from_utf8(m.data.to_vec()).unwrap();
//                     trace!(
//                         "{}::Message from DataChannel '{}': {}",
//                         peer_id,
//                         c.label(),
//                         msg_str
//                     );

//                     if let Ok(c) = serde_json::from_str::<ClientMessage>(&msg_str) {
//                         trace!("Client message: {:?}", c);

//                         if room_id.lock().await.is_none() {
//                             // Not in a room, so only joining a room makes sense
//                             if let ClientMessage::JoinRoom(new_room_id, username, password) = c {
//                                 info!("User {} (peer id {}), attempting to join room {}", username, peer_id, new_room_id);

//                                 match join_room(&username, &password.unwrap_or_default(), peer_id, &new_room_id).await {
//                                     Ok(_) => {
//                                         let _ = room_id.lock().await.insert(new_room_id.clone());
//                                     }
//                                     Err(e) => error!("{}", e),
//                                 }
//                             }
//                         } else {
//                             let room = ROOMS.get(&(room_id.clone().lock().await.clone().unwrap())).unwrap();

//                             match c {
//                                 ClientMessage::Heartbeat => {},
//                                 ClientMessage::ResetAll => {
//                                     room.lock().await.reset();
//                                 },
//                                 ClientMessage::ResetRobot(r) => {
//                                     room.lock().await.reset_robot(r.as_str());
//                                 },
//                                 ClientMessage::ClaimRobot(_) => todo!(),
//                                 ClientMessage::JoinRoom(new_room_id, username, password) => {
//                                     // For changing rooms if new room will be on same server

//                                     if room_id.lock().await.is_some() {
//                                         // TODO: leave old room
//                                     }
                                    
//                                     match join_room(&username, &password.unwrap_or_default(), peer_id, &new_room_id).await {
//                                         Ok(_) => {
//                                             let _ = room_id.lock().await.insert(new_room_id.clone());
//                                         }
//                                         Err(e) => error!("{}", e),
//                                     } 
//                                 },
//                             }
//                         }
//                     }
//                     //c.send_text(format!("Echo {}", msg_str)).await.unwrap();
//                 }
//                 PeerEvent::DataChannelStateChange(c) => {
//                     trace!("{}::Data Channel {} state: {} ", peer_id, c.label(), c.ready_state());

//                     if c.ready_state() == RTCDataChannelState::Open {
//                         CLIENTS.insert(peer_id, c.clone());
//                         info!("{}::DataChannel '{}' opened", peer_id, c.label());
//                         c.send_text(serde_json::to_string(&UpdateMessage::Heartbeat).unwrap()).await.unwrap();
//                     } else if c.ready_state() == RTCDataChannelState::Closed {
//                         info!("{}::DataChannel '{}' closed", peer_id, c.label());
//                         CLIENTS.remove(&peer_id);

//                         let r = room_id.lock().await;
//                         if r.is_some() {
//                             let room = ROOMS.get(&(r.clone().unwrap())).unwrap();
//                             room.lock().await.sockets.remove(&peer_id.to_string());
//                         }
//                     }
//                 }
//                 PeerEvent::PeerConnectionStateChange(s) => {
//                     trace!("{}::Peer connection state: {} ", peer_id, s);

//                     // Remove bad peers
//                     if s == RTCPeerConnectionState::Disconnected || s == RTCPeerConnectionState::Closed || s == RTCPeerConnectionState::Failed {
                        
//                     }
//                 }
//             }
//         }
//     },
//     Some(vec![
//         "stun:stun.l.google.com:19302".to_owned(),
//         "stun:stun1.l.google.com:19302".to_owned(),
//         "stun:stun2.l.google.com:19302".to_owned(),
//         "stun:stun3.l.google.com:19302".to_owned(),
//         "stun:stun4.l.google.com:19302".to_owned(),
//     ]))
//     .await?);
//     let pc = peer.peer_connection.clone();
//     pc.set_remote_description(RTCSessionDescription::offer(serde_json::from_str::<HashMap<String, String>>(&offer).unwrap().get("sdp").unwrap().to_owned()).unwrap()).await.unwrap();
//     let answer = pc.create_answer(None).await?;
//     let mut gather_complete = pc.gathering_complete_promise().await;
//     pc.set_local_description(answer).await?;
//     let _ = gather_complete.recv().await;

//     // move cyberdeck to another thread to keep it alive
//     tokio::spawn(async move {
//         while peer.connection_state() != RTCPeerConnectionState::Closed
//             && peer.connection_state() != RTCPeerConnectionState::Disconnected
//             && peer.connection_state() != RTCPeerConnectionState::Failed
//         {
//             // keep the connection alive while not in invalid state
//             sleep(Duration::from_millis(1000)).await;
//         }

//         info!("Peer {} dropping", peer.peer_id);
        
//         // because we moved cyberdeck ownership into here gets dropped here and will stop all channels
//     });

//     let answer = pc.local_description().await.unwrap();
//     Ok(answer.sdp)
// }

async fn join_room(username: &str, password: &str, peer_id: u128, room_id: &str) -> Result<(), String> {
    info!("User {} (peer id {}), attempting to join room {}", username, peer_id, room_id);

    if !ROOMS.contains_key(room_id) {
        return Err(format!("Room {} does not exist!", room_id));
    }

    let room = ROOMS.get(room_id).unwrap();
    let room = room.lock().await;
    
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
    let room = Arc::new(Mutex::new(RoomData::new(None, password)));
    
    // Set last interaction to creation time
    room.lock().await.last_interaction_time = Utc::now().timestamp();

    let room_id = room.lock().await.name.clone();
    ROOMS.insert(room_id.to_string(), room.clone());
    room_id
}