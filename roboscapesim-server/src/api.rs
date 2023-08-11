use axum::{Json, response::IntoResponse};
use once_cell::unsync::Lazy;
use serde::Serialize;
use async_std::task::block_on;
use std::{sync::Mutex, cell::RefCell};

use crate::{ROOMS, MAX_ROOMS};

static EXTERNAL_IP: Mutex<Option<String>> = Mutex::new(None);

#[derive(Debug, Serialize)]
struct ServerStatus {
    #[serde(rename = "activeRooms")]
    active_rooms: usize,
    #[serde(rename = "hibernatingRooms")]
    hibernating_rooms: usize,
    #[serde(rename = "maxRooms")]
    max_rooms: usize,
}

#[derive(Debug, Serialize)]
struct RoomInfo {
    id: String,
    environment: String,
    server: String,  
}

pub(crate) async fn server_status() -> impl IntoResponse {
    let mut hibernating_rooms: usize = 0;

    for r in ROOMS.iter() {
        if r.lock().await.hibernating {
            hibernating_rooms += 1;
        }
    }

    Json(ServerStatus {
        active_rooms: ROOMS.len(),
        hibernating_rooms,
        max_rooms: MAX_ROOMS,
    })
}

pub(crate) async fn rooms_list() -> impl IntoResponse {
    let mut rooms = vec![];

    let lock = &mut EXTERNAL_IP.lock().unwrap();
    if lock.is_none() {
        lock.insert("hi".to_string());
    }
    
    for r in ROOMS.iter() {
        rooms.push(RoomInfo{
            id: r.lock().await.name.clone(),
            environment: "rust".to_string(),
            server: lock.clone().unwrap(),
        });
    }

    Json(rooms)
}