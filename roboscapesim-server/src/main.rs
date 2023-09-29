use anyhow::Result;
use chrono::Utc;
use roboscapesim_common::ClientMessage;
use room::RoomData;
use simple_logger::SimpleLogger;
use socket::SocketInfo;
use tokio_tungstenite::tungstenite::Message;
use std::{net::SocketAddr, sync::Mutex};
use std::sync::Arc;
use tokio::{time::{Duration, self, sleep}, task, net::TcpListener};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use log::{info, trace, error};
use futures::{SinkExt, StreamExt, FutureExt};

use crate::{api::{get_external_ip, EXTERNAL_IP, create_api}, socket::accept_connection};

mod room;
mod robot;
mod simulation;
mod api;
mod vm;

#[path = "./util/mod.rs"]
mod util;

#[path = "./services/mod.rs"]
mod services;
mod socket;

const MAX_ROOMS: usize = 64;

static ROOMS: Lazy<DashMap<String, Arc<Mutex<RoomData>>>> = Lazy::new(|| {
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

    // Start API server
    create_api(SocketAddr::from(([0, 0, 0, 0], 3000))).await;
    info!("Running API server on port 3000 ...");

    // Loop listening for new WS connections
    let _ws_loop = task::spawn(ws_accept());

    // Loop sending/receiving and adding to channels
    let _ws_update_loop_tx = task::spawn(ws_rx());
    let _ws_update_loop_rx = task::spawn(ws_tx());

    // Update simulations
    let _update_loop = task::spawn(update_fn());
}

async fn update_fn() {
    let update_fps = 60;
    let mut interval = time::interval(Duration::from_millis(1000 / update_fps));

    loop {
        interval.tick().await;

        let update_time = Utc::now();
        // Perform updates
        for kvp in ROOMS.iter() {
            let m = kvp.value().clone();
            if !m.lock().unwrap().hibernating.load(std::sync::atomic::Ordering::Relaxed) {
                let lock = &mut m.lock().unwrap();
                // Check timeout
                if update_time.timestamp() - lock.last_interaction_time > lock.timeout {
                    lock.hibernating.store(true, std::sync::atomic::Ordering::Relaxed);
                    // Kick all users out
                    lock.send_to_all_clients(&roboscapesim_common::UpdateMessage::Hibernating);
                    lock.sockets.clear();
                    info!("{} is now hibernating", kvp.key());
                }
            }

            task::spawn(
                async move {
                    let room_data = &mut m.lock().unwrap();
                    room_data.update();
                }
            );
        }
    }
}

async fn ws_rx() {
    loop {
        // Get client updates
        for client in CLIENTS.iter() {
            // RX
            while let Some(Some(Ok(msg))) = client.value().stream.lock().unwrap().next().now_or_never() {
                trace!("Websocket message from {}: {:?}", client.key(), msg);
                if let Ok(msg) = msg.to_text() {

                    if let Ok(msg) = serde_json::from_str::<ClientMessage>(msg) {
                        match msg {
                            ClientMessage::JoinRoom(id, username, password) => {
                                join_room(&username, &(password.unwrap_or_default()), client.key().to_owned(), &id).unwrap();
                            },
                            _ => {
                                client.tx1.lock().unwrap().send(msg.to_owned()).unwrap();
                            }
                        }
                    }                    
                }
            }
        }

        sleep(Duration::from_nanos(50)).await;
    }
}

async fn ws_tx() {
    loop {
        // Get client updates
        for client in CLIENTS.iter() {                
            // TX
            let receiver = client.rx1.lock().unwrap();
            while let Ok(msg) = receiver.recv_timeout(Duration::default()) {
                let msg = serde_json::to_string(&msg).unwrap();
                client.sink.lock().unwrap().send(Message::Text(msg)).now_or_never();
            }
        }
        
        sleep(Duration::from_nanos(25)).await;
    }
}

async fn ws_accept() {
    let listener = TcpListener::bind("0.0.0.0:5000").await.unwrap();

    loop {
        let (conn, _) = listener.accept().await.unwrap();
        accept_connection(conn).await;
    }
}

fn join_room(username: &str, password: &str, peer_id: u128, room_id: &str) -> Result<(), String> {
    info!("User {} (peer id {}), attempting to join room {}", username, peer_id, room_id);

    if !ROOMS.contains_key(room_id) {
        return Err(format!("Room {} does not exist!", room_id));
    }

    let room = ROOMS.get(room_id).unwrap();
    let room = room.lock().unwrap();
    
    // Check password
    if room.password.clone().is_some_and(|pass| pass != password) {
        return Err("Wrong password!".to_owned());
    }
    
    // Setup connection to room
    room.visitors.insert(username.to_owned());
    room.sockets.insert(peer_id.to_string(), peer_id);
    room.send_info_to_client(peer_id);
    room.send_state_to_client(true, peer_id);
    Ok(())
}

async fn create_room(password: Option<String>, edit_mode: bool) -> String {
    let room = Arc::new(Mutex::new(RoomData::new(None, password, edit_mode)));
    
    // Set last interaction to creation time
    room.lock().unwrap().last_interaction_time = Utc::now().timestamp();

    let room_id = room.lock().unwrap().name.clone();
    ROOMS.insert(room_id.to_string(), room.clone());
    room_id
}