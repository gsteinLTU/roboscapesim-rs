use axum::Json;

use axum::response::IntoResponse;
use serde::Serialize;

use crate::{ROOMS, MAX_ROOMS};

#[derive(Debug, Serialize)]
struct ServerStatus {
    activeRooms: usize,
    hibernatingRooms: usize,
    maxRooms: usize,
}

pub(crate) async fn server_status() -> impl IntoResponse {
    let mut hibernatingRooms: usize = 0;

    for r in ROOMS.iter() {
        if r.lock().await.hibernating {
            hibernatingRooms += 1;
        }
    }

    Json(ServerStatus {
        activeRooms: ROOMS.len(),
        hibernatingRooms: hibernatingRooms,
        maxRooms: MAX_ROOMS,
    })
}