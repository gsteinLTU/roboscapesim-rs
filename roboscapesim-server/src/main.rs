use std::sync::Arc;
use std::sync::atomic::Ordering;

use dashmap::DashMap;
use dashmap::DashSet;
use log::info;
use log::trace;
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

use crate::api::EXTERNAL_IP;
use crate::api::create_api;
use crate::api::get_external_ip;
use crate::socket::{ws_accept, ws_rx, ws_tx};

mod api;
mod robot;
mod room;
mod simulation;
mod vm;
mod scenarios;

#[path = "./util/mod.rs"]
mod util;

#[path = "./services/mod.rs"]
mod services;
mod socket;

pub const MAX_ROOMS: usize = 64;

pub static ROOMS: Lazy<DashMap<String, Arc<RoomData>>> = Lazy::new(|| DashMap::new());

pub static CLIENTS: Lazy<DashMap<u128, SocketInfo>> = Lazy::new(|| DashMap::new());

#[tokio::main]
async fn main() {
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

    // Announce to master server
    let _announce_api = task::spawn(api::announce_api());

    // Start API server
    let api = create_api();
    api.await;
}

pub const UPDATE_FPS: f64 = 60.0;

async fn update_fn() {
    let mut interval = time::interval(Duration::from_millis((1000.0 / UPDATE_FPS) as u64));

    interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);


    let updating = Arc::new(DashSet::new());
    
    loop {
        interval.tick().await;

        let update_time = get_timestamp();
        SHARED_CLOCK.update();
        
        // Perform updates
        for kvp in ROOMS.iter() {
            trace!("Updating room {}", kvp.key());
            let m = kvp.value().clone();
            if !m.hibernating.load(Ordering::Relaxed) {
                // Check timeout
                if update_time - m.last_interaction_time.load(Ordering::Relaxed) > m.timeout {
                    m.hibernating.store(true, Ordering::Relaxed);
                    m.hibernating_since.store(get_timestamp(), Ordering::Relaxed);

                    // Kick all users out
                    m.send_to_all_clients(&roboscapesim_common::UpdateMessage::Hibernating);
                    m.sockets.clear();
                    info!("{} is now hibernating", kvp.key());
                }
            }

            let updating = updating.clone();
            task::spawn(async move {
                if updating.contains(&m.name) {
                    return;
                }

                updating.insert(m.name.clone());
                m.update();
                updating.remove(&m.name);
            });
        }
    }
}
