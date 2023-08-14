use axum::{Json, response::IntoResponse};
use serde::Serialize;
use std::sync::Mutex;
use axum_macros::debug_handler;

use crate::{ROOMS, MAX_ROOMS};

pub(crate) static EXTERNAL_IP: Mutex<Option<String>> = Mutex::new(None);

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

#[debug_handler]
pub(crate) async fn rooms_list() -> impl IntoResponse {
    let rooms = get_rooms().await;
    Json(rooms)
}

async fn get_rooms() -> Vec<RoomInfo> {
    let mut rooms = vec![];
    
    let server = EXTERNAL_IP.lock().unwrap().clone().unwrap_or_else(|| "127.0.0.1".into());

    for r in ROOMS.iter() {
        let id = r.lock().await.name.clone();

        rooms.push(RoomInfo{
            id,
            environment: "rust".to_string(),
            server: server.clone(),
        });
    }
    rooms
}

pub(crate) async fn get_external_ip() -> Result<String, reqwest::Error> {
    let url = "http://checkip.amazonaws.com";
    reqwest::get(url).await.unwrap().text().await
}
