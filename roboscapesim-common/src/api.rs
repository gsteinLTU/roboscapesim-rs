use std::time::SystemTime;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateRoomRequestData {
    pub username: String,
    pub password: Option<String>,
    pub edit_mode: bool,
    pub environment: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateRoomResponseData {
    pub server: String,
    pub room_id: String
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ServerStatus {
    #[serde(rename = "activeRooms")]
    pub active_rooms: usize,
    #[serde(rename = "hibernatingRooms")]
    pub hibernating_rooms: usize,
    #[serde(rename = "maxRooms")]
    pub max_rooms: usize,
    pub address: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RoomInfo {
    pub id: String,
    pub environment: String,
    pub server: String,  
    #[serde(rename = "hasPassword")]
    pub has_password: bool,
    #[serde(rename = "isHibernating")]
    pub is_hibernating: bool,
    pub creator: String,  
    pub visitors: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EnvironmentInfo {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ServerInfo {
    pub address: String,
    pub max_rooms: usize,
    pub last_update: SystemTime,
}