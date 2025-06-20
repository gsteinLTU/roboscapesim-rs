use async_once_cell::OnceCell;
use axum::{body::Body, extract::Query, http::{HeaderValue, Request}, middleware::Next, response::{IntoResponse, Response}, routing::{get, post, put}, Json, Router};
use dashmap::DashMap;
use log::{debug, error, info, trace};
use once_cell::sync::Lazy;
use roboscapesim_common::api::{
    CreateRoomRequestData, CreateRoomResponseData, EnvironmentInfo, RoomInfo, ServerStatus, ServerInfo
};
use tower_http::cors::CorsLayer;
use simple_logger::SimpleLogger;
use rand::Rng;

use std::{collections::HashMap, net::SocketAddr, time::SystemTime};

/// Known servers
static SERVERS: Lazy<DashMap<String, ServerInfo>> = Lazy::new(|| DashMap::new());

/// Known environments, should be available on all servers (being NetsBlox projects)
static ENVIRONMENTS: Lazy<DashMap<String, EnvironmentInfo>> = Lazy::new(|| DashMap::new());

/// Known rooms
static ROOMS: Lazy<DashMap<String, RoomInfo>> = Lazy::new(|| DashMap::new());

/// External IP address
static EXTERNAL_IP: OnceCell<String> = OnceCell::new();

/// Shared reqwest client for making HTTP requests
pub(crate) static REQWEST_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| 
    reqwest::ClientBuilder::new()
        .timeout(std::time::Duration::from_secs(3))
        .connect_timeout(std::time::Duration::from_secs(2))
        .pool_idle_timeout(std::time::Duration::from_secs(5))
        .pool_max_idle_per_host(2)
        .user_agent("roboscapesim-api/1.0")
        .tcp_keepalive(std::time::Duration::from_secs(30))
        .build()
        .unwrap()
);

#[tokio::main]
async fn main() {
    // Setup logger
    SimpleLogger::new()
        .with_level(log::LevelFilter::Error)
        .with_module_level("roboscapesim_api", log::LevelFilter::Info)
        .env()
        .init()
        .expect("Failed to initialize logger");
    
    async fn additional_cors(req: Request<Body>, next: Next) -> Result<Response, axum::http::StatusCode> {
        let mut response = next.run(req).await;
        
        // Overwrite or insert the PNA header unconditionally
        response.headers_mut().insert(
            "Access-Control-Allow-Private-Network",
            HeaderValue::from_static("true"),
        );
        Ok(response)
    }

    let app = Router::new()
        .route("/server/status", get(get_server_status))
        .route("/rooms/list", get(get_rooms_list))
        .route("/rooms/create", post(post_create))
        .route("/rooms/info", get(get_room_info))
        .route("/server/announce", post(post_server_announce))
        .route("/server/rooms", put(put_server_rooms))
        .route("/server/environments", put(put_server_environments))
        .route("/environments/list", get(get_environments_list))
        .layer(
            CorsLayer::very_permissive()
        )
        .layer(axum::middleware::from_fn(additional_cors))
        .layer(tower_http::timeout::TimeoutLayer::new(std::time::Duration::from_secs(10)));

    let addr = SocketAddr::from(([0, 0, 0, 0], 5001));
    let listener = tokio::net::TcpListener::bind(addr).await.expect("Failed to bind port");
    let server = axum::serve(listener, app.into_make_service());
    debug!("listening on {}", addr);

    // Clean up servers not updated in 6 minutes
    tokio::spawn(async move {
        loop {
            let mut servers_to_remove = Vec::new();
            
            // Collect servers that need to be checked (only those that are old enough)
            for server in SERVERS.iter() {
                if let Ok(elapsed) = server.value().last_update.elapsed() {
                    if elapsed.as_secs() > 360 {
                        servers_to_remove.push(server.key().clone());
                    }
                }
            }
            
            // Check each old server with a timeout
            for server_addr in servers_to_remove {
                // Use a shorter timeout for health checks to avoid blocking
                let health_check = async {
                    let res = REQWEST_CLIENT
                        .get(format!("{}/server/status", server_addr))
                        .timeout(std::time::Duration::from_secs(2))
                        .send()
                        .await;

                    match res {
                        Ok(response) => {
                            // Consume the response body to avoid connection issues
                            let _ = response.bytes().await;
                            false // Server is alive, don't remove it
                        }
                        Err(_) => true // Server is dead, remove it
                    }
                };
                
                // Execute health check with timeout
                let should_remove = tokio::time::timeout(
                    std::time::Duration::from_secs(3),
                    health_check
                ).await.unwrap_or(true); // If timeout, assume server is dead
                
                if should_remove {
                    info!("Removing server {} due to timeout", server_addr);
                    SERVERS.remove(&server_addr);

                    // Remove rooms on server more efficiently
                    ROOMS.retain(|_, room| room.server != server_addr);
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
        address: EXTERNAL_IP.get_or_init(async { get_external_ip().await.unwrap().trim().to_owned() }).await.to_owned(),
        max_rooms: SERVERS.iter().map(|x| x.value().max_rooms).sum(),
        last_update: SystemTime::now(),
    }).unwrap()
}

/// Get list of rooms, optionally filtering to a specific user and/or hibernating status
async fn get_rooms_list(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    let user = params.get("user");
    let not_hibernating = params.get("notHibernating");
    let mut rooms = ROOMS.iter().map(|x| x.value().clone()).collect::<Vec<_>>();
    if let Some(user) = user {
        rooms = rooms
            .into_iter()
            .filter(|x| x.visitors.contains(&user))
            .collect::<Vec<_>>();
    }

    if let Some(not_hibernating) = not_hibernating {
        let not_hibernating = not_hibernating.parse::<bool>().unwrap_or(false);

        if not_hibernating {
            rooms = rooms
                .into_iter()
                .filter(|x| x.is_hibernating == false)
                .collect::<Vec<_>>();
        }
    }

    serde_json::to_string(&rooms).unwrap()
}

/// Create a new room
async fn post_create(Json(data): Json<CreateRoomRequestData>) -> impl IntoResponse {
    info!("Request to create room for user {} with environment {:?} (edit mode: {})", data.username, data.environment, data.edit_mode);

    // Pick server to forward request to
    let server = get_best_server();

    // Return error when no servers available
    if server.is_none() {
        info!("No servers available");
        return (axum::http::StatusCode::SERVICE_UNAVAILABLE, Json(None));
    }

    // Forward request to server
    let server = server.unwrap();
    let response = REQWEST_CLIENT
        .post(format!("{}/rooms/create", server))
        .json(&data)
        .send()
        .await;

    // If error, return error
    let response = match response {
        Ok(resp) => resp,
        Err(e) => {
            error!("Error sending request to server: {:?}", e);
            // Remove server from list
            SERVERS.remove(&server);
            return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(None));
        }
    };

    info!("Response from server: {:?}", response.status());

    // Parse as JSON
    let parsed_response = response.json::<CreateRoomResponseData>().await;
    
    // If error, return error
    let parsed_response = match parsed_response {
        Ok(data) => data,
        Err(e) => {
            error!("Error parsing response from server: {:?}", e);
            return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(None));
        }
    };

    // If success, return created room's info
    (axum::http::StatusCode::OK, Json(Some(parsed_response)))
}

/// Get the best server to create a room on
/// This is the server with the fewest active rooms
/// If multiple servers have the same number of active rooms, one is picked at random
fn get_best_server() -> Option<String> {
    let active_rooms_per_server = get_active_rooms_per_server();
    
    if active_rooms_per_server.is_empty() {
        return None;
    }
    
    // Find minimum number of active rooms
    let min_rooms = active_rooms_per_server.values().min().copied().unwrap_or(0);
    
    // Collect servers with minimum rooms
    let best_servers: Vec<&String> = active_rooms_per_server
        .iter()
        .filter(|(_, &count)| count == min_rooms)
        .map(|(server, _)| server)
        .collect();
    
    // Pick one randomly
    if !best_servers.is_empty() {
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..best_servers.len());
        Some(best_servers[index].clone())
    } else {
        None
    }
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
        address: ip.clone(),
        max_rooms: data.max_rooms,
        last_update: SystemTime::now(),
    };

    // Check if server has been reset
    if data.active_rooms + data.hibernating_rooms == 0 && SERVERS.contains_key(&server.address){
        info!("Server {} reset", ip);
        // Remove rooms that no longer exist on server more efficiently
        ROOMS.retain(|_, room| room.server != server.address);
    }

    SERVERS.insert(server.address.clone(), server);
    info!("Server {} announced", ip);
    
    (axum::http::StatusCode::OK, Json(()))
}

/// Update list of rooms on server
async fn put_server_rooms(Json(data): Json<Vec<RoomInfo>>) -> impl IntoResponse {
    for room in data {
        ROOMS.insert(room.id.clone(), room);
    }
    
    (axum::http::StatusCode::OK, Json(()))
}

/// Update list of environments on server
async fn put_server_environments(Json(data): Json<Vec<EnvironmentInfo>>) -> impl IntoResponse {
    trace!("Environments: {:?}", data);
    for environment in data {
        ENVIRONMENTS.insert(environment.id.clone(), environment);
    }
    
    (axum::http::StatusCode::OK, Json(()))
}

/// Get list of environments
async fn get_environments_list() -> impl IntoResponse {
    serde_json::to_string(&ENVIRONMENTS.iter().map(|x| x.value().clone()).collect::<Vec<_>>()).unwrap_or(
        "[]".to_owned()
    )
}

/// Get number of non-hibernating rooms per server
fn get_active_rooms_per_server() -> HashMap<String, usize> {
    let mut active_rooms_per_server = HashMap::new();

    // Initialize all servers with 0 count
    for server in SERVERS.iter() {
        active_rooms_per_server.insert(server.key().clone(), 0);
    }

    // Count active rooms per server
    for room in ROOMS.iter() {
        if !room.value().is_hibernating {
            let server = &room.value().server;
            *active_rooms_per_server.entry(server.clone()).or_insert(0) += 1;
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
        REQWEST_CLIENT.get(url).send().await?.text().await
    }
}
