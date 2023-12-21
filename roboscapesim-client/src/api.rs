use roboscapesim_common::api::*;

use crate::{REQWEST_CLIENT, ui::set_title};

#[cfg(debug_assertions)]
pub const API_SERVER: &str = "http://localhost:5001/";
#[cfg(not(debug_assertions))]
pub const API_SERVER: &str = "https://roboscapeonlineapi2.netsblox.org/";

/// Request a new room from the main API server
pub async fn request_room(username: String, password: Option<String>, edit_mode: bool, environment: Option<String>) -> Result<CreateRoomResponseData, reqwest::Error> {
    set_title("Connecting...");

    let mut client_clone = Default::default();
    REQWEST_CLIENT.with(|client| {
        client_clone = client.clone();
    });

    // TODO: get API URL through env var for deployed version
    let response = client_clone.post(format!("{}rooms/create", API_SERVER)).json(&CreateRoomRequestData {
        username,
        password,
        edit_mode,
        environment
    }).send().await?;

    response.json().await
}

/// Query the main API server for room info for a given room ID
pub async fn request_room_info(id: &String) -> Result<RoomInfo, reqwest::Error> {
    let mut client_clone = Default::default();
    REQWEST_CLIENT.with(|client| {
        client_clone = client.clone();
    });

    let response = client_clone.get(format!("{}rooms/info?id={}", API_SERVER, id)).send().await?;

    response.json().await
}

/// Query the main API server for a list of environments
pub async fn get_environments() -> Result<Vec<EnvironmentInfo>, reqwest::Error> {
    let mut client_clone = Default::default();
    REQWEST_CLIENT.with(|client| {
        client_clone = client.clone();
    });

    let get = client_clone.get(format!("{}environments/list", API_SERVER));

    let results = get.send().await?;
    let results = results.json::<Vec<EnvironmentInfo>>().await;
    results
}

/// Query the main API server for a list of rooms
pub async fn get_rooms_list(user: Option<String>, non_hibernating: Option<bool>) -> Result<Vec<RoomInfo>, reqwest::Error> {
    let mut client_clone = Default::default();
    REQWEST_CLIENT.with(|client| {
        client_clone = client.clone();
    });

    let mut query = if let Some(user) = user {
        format!("user={}", user)
    } else {
        "".to_string()
    };

    if let Some(non_hibernating) = non_hibernating {
        if !query.is_empty() {
            query.push('&');
        }
        query.push_str(format!("notHibernating={}", non_hibernating).as_str());
    }

    let get = client_clone.get(format!("{}rooms/list?{}", API_SERVER, query));

    let results = get.send().await?;
    let results = results.json::<Vec<RoomInfo>>().await;
    results
}