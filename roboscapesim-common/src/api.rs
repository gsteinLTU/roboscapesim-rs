use serde::{Deserialize, Serialize};


#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateRoomRequestData {
    pub username: String,
    pub password: Option<String>,
}