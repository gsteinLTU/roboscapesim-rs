use axum::{Json, response::IntoResponse};
use log::info;
use roboscapesim_common::api::CreateRoomRequestData;
use serde::{Serialize, Deserialize};
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
    let rooms = get_rooms(None, true).await;
    Json(rooms)
}

/// Get list of rooms, optionally filtering to a specific user
async fn get_rooms(user_filter: Option<String>, include_hibernating: bool) -> Vec<RoomInfo> {
    let mut rooms = vec![];
    
    let server = EXTERNAL_IP.lock().unwrap().clone().unwrap_or_else(|| "127.0.0.1".into());

    let user_filter = user_filter.unwrap_or_default();

    for r in ROOMS.iter() {
        let room_data = r.lock().await;
        // Skip if user not in visitors
        if user_filter.len() > 0 && room_data.visitors.contains(&user_filter) {
            continue;
        }

        if !include_hibernating && room_data.hibernating {
            continue;
        }

        let id = room_data.name.clone();

        rooms.push(RoomInfo{
            id,
            environment: "rust".to_string(),
            server: server.clone(),
        });
    }
    rooms
}

pub(crate) async fn post_create(Json(request): Json<CreateRoomRequestData>) -> impl IntoResponse {
    info!("{:?}", request);
    ()
}

pub(crate) async fn get_external_ip() -> Result<String, reqwest::Error> {
    // Final deployment is expected to be to AWS, although this URL currently works on other networks
    let url = "http://checkip.amazonaws.com";
    reqwest::get(url).await.unwrap().text().await
}
