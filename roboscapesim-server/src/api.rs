use axum::Json;

use axum::response::IntoResponse;
use serde::Serialize;

use crate::{ROOMS, MAX_ROOMS};

#[derive(Debug, Serialize)]
struct ServerStatus {
    #[serde(rename = "activeRooms")]
    active_rooms: usize,
    #[serde(rename = "hibernatingRooms")]
    hibernating_rooms: usize,
    #[serde(rename = "maxRooms")]
    max_rooms: usize,
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