use std::sync::atomic::Ordering;
use std::sync::Arc;

use dashmap::DashMap;
use log::info;
use once_cell::sync::Lazy;
use room::RoomData;
use room::SHARED_CLOCK;
use simple_logger::SimpleLogger;
use socket::SocketInfo;
use tokio::{
    task,
    time::{self, Duration},
};
use util::util::get_timestamp;

use crate::api::{create_api, get_external_ip, EXTERNAL_IP};
use crate::socket::{ws_accept, ws_rx, ws_tx};

mod api;
mod robot;
mod room;
mod simulation;
mod vm;
mod scenarios;
mod socket;

#[path = "./util/mod.rs"]
mod util;

#[path = "./services/mod.rs"]
mod services;

pub const MAX_ROOMS: usize = 64;
pub const UPDATE_FPS: f64 = 60.0;
const UPDATE_INTERVAL_MILLIS: u64 = (1000.0 / UPDATE_FPS) as u64;
const ROOM_CLEANUP_INTERVAL_SECS: u64 = 120; // 2 minutes

pub static ROOMS: Lazy<DashMap<String, Arc<RoomData>>> = Lazy::new(|| DashMap::new());
pub static CLIENTS: Lazy<DashMap<u128, SocketInfo>> = Lazy::new(|| DashMap::new());

#[tokio::main]
async fn main() {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Setup logger
    SimpleLogger::new()
        .with_level(log::LevelFilter::Error)
        .with_module_level("roboscapesim_server", log::LevelFilter::Info)
        .with_module_level("iotscape", log::LevelFilter::Info)
        .env()
        .init()
        .unwrap();
    info!("Starting RoboScape Online Server...");
    
    if let Ok(ip) = get_external_ip().await {
        let _ = EXTERNAL_IP.lock().unwrap().insert(ip.trim().into());
    }

    // Loop listening for new WS connections
    let _ws_loop = task::spawn(ws_accept());

    // Loop sending/receiving and adding to channels
    let _ws_update_loop_tx = task::spawn(ws_rx());
    let _ws_update_loop_rx = task::spawn(ws_tx());

    // Update simulations
    let _update_loop = task::spawn(update_fn());

    // Cleanup dead rooms
    let _cleanup_loop = task::spawn(cleanup_dead_rooms());

    // Announce to master server
    let _announce_api = task::spawn(api::announce_api());

    // Start API server
    let api = create_api();
    api.await;
}

async fn update_fn() {
    let mut interval = time::interval(Duration::from_millis(UPDATE_INTERVAL_MILLIS));
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
    
    loop {
        interval.tick().await;
        SHARED_CLOCK.update();
    }
}

async fn cleanup_dead_rooms() {
    let mut interval = time::interval(Duration::from_secs(ROOM_CLEANUP_INTERVAL_SECS));
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
    
    loop {
        interval.tick().await;
        
        let mut dead_rooms = vec![];
        let timestamp = get_timestamp();
        for kvp in ROOMS.iter() {
            let room = kvp.value();

            if timestamp - room.last_interaction_time.load(Ordering::Relaxed) > room.metadata.full_timeout {
                dead_rooms.push(kvp.key().clone());
                room.is_alive.store(false, Ordering::Relaxed);
            }
        }

        for room in dead_rooms {
            info!("Room {} has timed out and will be removed", room);
            ROOMS.remove(&room);
        }
    }
}
