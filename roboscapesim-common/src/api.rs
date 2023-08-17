use serde::{Deserialize, Serialize};


#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateRoomRequestData {
    pub username: String,
    pub password: Option<String>,
    pub offer: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateRoomResponseData {
    pub server: String,
    pub room_id: String,
    pub answer: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IceData {
    pub peer_id: u128,
    pub server: String,
    pub candidate: String,
}
