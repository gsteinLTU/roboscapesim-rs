use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateRoomRequestData {
    pub username: String,
    pub password: Option<String>,
    pub edit_mode: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateRoomResponseData {
    pub server: String,
    pub room_id: String
}
