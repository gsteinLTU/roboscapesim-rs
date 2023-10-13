use axum::{extract::Query, response::IntoResponse, Json};
use axum::{
    http::{header, Method},
    routing::{get, post, put},
    Router,
};
use dashmap::DashMap;
use log::{debug, error};
use once_cell::sync::Lazy;
use roboscapesim_common::api::{
    CreateRoomRequestData, CreateRoomResponseData, EnvironmentInfo, RoomInfo, ServerStatus,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr, sync::Mutex, time::SystemTime};
use tower_http::cors::{Any, CorsLayer};
use simple_logger::SimpleLogger;

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ServerInfo {
    pub address: String,
    pub max_rooms: usize,
    pub last_update: SystemTime,
}

static SERVERS: Lazy<DashMap<String, ServerInfo>> = Lazy::new(|| DashMap::new());

static ENVIRONMENTS: Lazy<DashMap<String, EnvironmentInfo>> = Lazy::new(|| DashMap::new());

static ROOMS: Lazy<DashMap<String, RoomInfo>> = Lazy::new(|| DashMap::new());

static EXTERNAL_IP: Mutex<Option<String>> = Mutex::new(None);

#[tokio::main]
async fn main() {
    // Setup logger
    SimpleLogger::new()
        .with_level(log::LevelFilter::Error)
        .with_module_level("roboscapesim_api", log::LevelFilter::Info)
        .env()
        .init()
        .unwrap();

    EXTERNAL_IP
        .lock()
        .unwrap()
        .replace(get_external_ip().await.unwrap());

    let app = Router::new()
        .route("/server/status", get(get_server_status))
        .route("/rooms/list", get(get_rooms_list))
        .route("/rooms/create", post(post_create))
        .route("/rooms/info", get(get_room_info))
        .route("/server/announce", post(post_server_announce))
        .route("/server/rooms", put(put_server_rooms))
        .route("/environments/list", get(get_environments_list))
        .layer(
            CorsLayer::new()
                // allow `GET` and `POST` when accessing the resource
                .allow_methods([Method::GET, Method::POST])
                // allow requests from any origin
                .allow_origin(Any)
                .allow_headers([header::CONTENT_TYPE]),
        );

    let addr = SocketAddr::from(([127, 0, 0, 1], 5001));
    debug!("listening on {}", addr);
    let server = axum::Server::bind(&addr).serve(app.into_make_service());

    // Clean up servers not updated in 5 minutes
    tokio::spawn(async move {
        loop {
            let mut servers_to_remove = Vec::new();
            for server in SERVERS.iter() {
                if server.value().last_update.elapsed().unwrap().as_secs() > 300 {
                    servers_to_remove.push(server.key().clone());
                }
            }
            for server in servers_to_remove {
                SERVERS.remove(&server);

                // Remove rooms on server
                let mut rooms_to_remove = Vec::new();
                for room in ROOMS.iter() {
                    if room.value().server == server {
                        rooms_to_remove.push(room.key().clone());
                    }
                }
                for room in rooms_to_remove {
                    ROOMS.remove(&room);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    });

    if let Err(err) = server.await {
        error!("server error: {}", err);
    }
}

/// Get status of rooms on server
async fn get_server_status() -> impl IntoResponse {
    serde_json::to_string(&ServerInfo {
        address: EXTERNAL_IP.lock().unwrap().clone().unwrap(),
        max_rooms: SERVERS.iter().map(|x| x.value().max_rooms).sum(),
        last_update: SystemTime::now(),
    }).unwrap()
}

/// Get list of rooms, optionally filtering to a specific user
async fn get_rooms_list(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    let user = params.get("user");
    let mut rooms = ROOMS.iter().map(|x| x.value().clone()).collect::<Vec<_>>();
    if let Some(user) = user {
        rooms = rooms
            .into_iter()
            .filter(|x| x.visitors.contains(&user))
            .collect::<Vec<_>>();
    }
    serde_json::to_string(&rooms).unwrap()
}

/// Create a new room
async fn post_create(Json(data): Json<CreateRoomRequestData>) -> impl IntoResponse {
    // Pick server to forward request to
    let active_rooms_per_server = get_active_rooms_per_server();
    // Sort servers by number of active rooms
    let mut active_rooms_per_server = active_rooms_per_server.iter().map(|x| (x.0.clone(), x.1.clone())).collect::<Vec<_>>();
    active_rooms_per_server.sort_by(|a, b| a.1.cmp(&b.1));

    // Pick server with fewest active rooms
    let mut server = None;
    if active_rooms_per_server.len() > 0 {
        server = Some(active_rooms_per_server[0].0.clone());
    }

    // Return error when no servers available
    if server.is_none() {
        return (axum::http::StatusCode::SERVICE_UNAVAILABLE, Json(None));
    }

    // Forward request to server
    let server = server.unwrap();
    let response = reqwest::Client::new()
        .post(format!("http://{}/rooms/create", server))
        .json(&data)
        .send()
        .await;

    // If error, return error
    if response.is_err() {
        return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(None));
    }

    // Parse as JSON
    let response = response.unwrap().json::<CreateRoomResponseData>().await;
    
    // If error, return error
    if response.is_err() {
        error!("Error parsing response from server: {:?}", response);
        return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(None));
    }

    // If success, return created room's info
    let response: CreateRoomResponseData = response.unwrap();
    (axum::http::StatusCode::OK, Json(Some(response)))
}

/// Get info about a room
async fn get_room_info(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    let room_id = params.get("id").unwrap_or(&"INVALID".to_owned()).clone();
    let room = ROOMS.get(&room_id);
    
    if room.is_none() {
        return (axum::http::StatusCode::NOT_FOUND,Json(None));    
    }

    let room_data = room.unwrap().clone();
    let visitors = room_data.visitors.clone();
    
    (axum::http::StatusCode::OK, Json(Some(RoomInfo{
        id: room_data.id.clone(),
        environment: room_data.environment.clone(),
        server: room_data.server.clone(),
        creator: room_data.creator.clone(),
        has_password: room_data.has_password,
        is_hibernating: room_data.is_hibernating,
        visitors,
    })))
}

/// Receive announcement from server
async fn post_server_announce(Json(data): Json<(String, ServerStatus)>) -> impl IntoResponse {
    let (ip, data) = data;
    let server = ServerInfo {
        address: ip,
        max_rooms: data.max_rooms,
        last_update: SystemTime::now(),
    };
    SERVERS.insert(server.address.clone(), server);
}

/// Update list of rooms on server
async fn put_server_rooms(Json(data): Json<Vec<RoomInfo>>) -> impl IntoResponse {
    for room in data {
        ROOMS.insert(room.id.clone(), room);
    }
}

/// Get list of environments
async fn get_environments_list() -> impl IntoResponse {
    serde_json::to_string(&ENVIRONMENTS.iter().map(|x| x.value().clone()).collect::<Vec<_>>()).unwrap()
}

/// Get number of non-hibernating rooms per server
fn get_active_rooms_per_server() -> HashMap<String, usize> {
    let mut active_rooms_per_server = HashMap::new();

    for room in ROOMS.iter() {
        if !room.value().is_hibernating {
            let server = room.value().server.clone();
            let count = active_rooms_per_server.get(&server).unwrap_or(&0) + 1;
            active_rooms_per_server.insert(server, count);
        }
    }
    
    active_rooms_per_server
}

/// Get external IP address
pub(crate) async fn get_external_ip() -> Result<String, reqwest::Error> {
    // Final deployment is expected to be to AWS, although this URL currently works on other networks
    #[cfg(debug_assertions)]
    {
        Ok("127.0.0.1".into())
    }
    #[cfg(not(debug_assertions))]
    {
        let url = "http://checkip.amazonaws.com";
        reqwest::get(url).await?.text().await
    }
}