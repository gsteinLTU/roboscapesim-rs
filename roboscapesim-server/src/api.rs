use axum::{Json, response::IntoResponse, extract::Query};
use log::{error, info};
use once_cell::sync::Lazy;
use roboscapesim_common::api::{CreateRoomRequestData, CreateRoomResponseData, ServerStatus, RoomInfo, EnvironmentInfo};
use std::{net::SocketAddr, collections::HashMap, sync::Mutex};
use axum_macros::debug_handler;
use axum::{routing::{post, get}, Router, http::{Method, header}};
use tower_http::{cors::{Any, CorsLayer}, timeout::TimeoutLayer};

use crate::{ROOMS, MAX_ROOMS, room::management::create_room, scenarios::{DEFAULT_SCENARIOS_FILE, LOCAL_SCENARIOS}};

pub static EXTERNAL_IP: Mutex<Option<String>> = Mutex::new(None);

/// SystemTime when API server was started
pub static LAUNCH_TIME: Lazy<std::time::SystemTime> = Lazy::new(|| std::time::SystemTime::now());

const ANNOUNCEMENT_INTERVAL_SECS: u64 = 60 * 5;
const HEALTHCHECK_MINIMUM_UPTIME: u64 = 60 * 60 * 16;

pub static API_PORT: Lazy<u16> = Lazy::new(|| std::env::var("LOCAL_API_PORT")
    .unwrap_or_else(|_| "3000".to_string())
    .parse::<u16>()
    .expect("PORT must be a number")
);

/// Shared reqwest client for making HTTP requests
pub(crate) static REQWEST_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| 
    reqwest::ClientBuilder::new().timeout(std::time::Duration::from_secs(2)).build().unwrap()
);

/// Announce server to main API server
pub async fn announce_api() {
    // Every 5 minutes, announce to main server
    let url = format!("{}/server/announce", get_main_api_server());
    let server = get_local_api_server();
    let max_rooms = MAX_ROOMS;

    // Send initial announcement
    let data = (server.clone(), ServerStatus {
        active_rooms: 0,
        hibernating_rooms: 0,
        max_rooms,
        address: get_server(),
    });
    
    let res = REQWEST_CLIENT.post(&url).json(&data).send().await;
    match res {
        Ok(response) => {
            if !response.status().is_success() {
                error!("Failed to announce to main server: HTTP {}", response.status());
            }
        }
        Err(err) => error!("Error announcing to main server: {}", err),
    }

    // Send environment list
    let res = REQWEST_CLIENT.put(format!("{}/server/environments", get_main_api_server()))
        .json(&LOCAL_SCENARIOS.values().cloned().map(|s| s.into()).collect::<Vec<EnvironmentInfo>>())
        .send().await;
    match res {
        Ok(response) => {
            if !response.status().is_success() {
                error!("Failed to announce environments to main server: HTTP {}", response.status());
            }
        }
        Err(err) => error!("Error announcing environments to main server: {}", err),
    }

    // Loop sending announcement every 5 minutes
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(ANNOUNCEMENT_INTERVAL_SECS)).await;
        let data = (server.clone(), ServerStatus {
            active_rooms: ROOMS.len(),
            hibernating_rooms: ROOMS.iter().filter(|r| r.metadata.hibernating.load(std::sync::atomic::Ordering::Relaxed)).count(),
            max_rooms,
            address: get_server(),
        });
        let res = REQWEST_CLIENT.post(&url).json(&data).send().await;
        if let Err(err) = res {
            error!("Error announcing to main server: {}", err);
        }
    }
}

/// Create API server with routes
pub async fn create_api() {
    let addr = SocketAddr::from(([0, 0, 0, 0], API_PORT.clone()));

    let app = Router::new()
    .route("/server/status", get(server_status))
    .route("/rooms/list", get(get_rooms_list))
    .route("/rooms/create", post(post_create))
    .route("/rooms/info", get(get_room_info))
    .route("/environments/list", get(get_environments_list))
    .route("/server/healthcheck", get(get_healthcheck))
	.layer(CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods([Method::GET, Method::POST])
        // allow requests from any origin
        .allow_origin(Any)
	    .allow_headers([header::CONTENT_TYPE]))
    .layer(TimeoutLayer::with_status_code(axum::http::StatusCode::REQUEST_TIMEOUT, std::time::Duration::from_secs(5)));
    
    let listener = tokio::net::TcpListener::bind(addr).await.expect("Failed to bind port");
    let server = axum::serve(listener, app.into_make_service());

    info!("API server listening on {}", addr);

    // Make sure launch time is set
    let _ = LAUNCH_TIME.clone();
    info!("API server launched at {:?}", LAUNCH_TIME);

    if let Err(err) = server.await {
        error!("server error: {}", err);
    }
}

/// Get status of rooms on server
pub(crate) async fn server_status() -> impl IntoResponse {
    let mut hibernating_rooms: usize = 0;

    for r in ROOMS.iter() {
        if r.metadata.hibernating.load(std::sync::atomic::Ordering::Relaxed) {
            hibernating_rooms += 1;
        }
    }

    Json(ServerStatus {
        active_rooms: ROOMS.len(),
        hibernating_rooms,
        max_rooms: MAX_ROOMS,
        address: get_server(),
    })
}

#[debug_handler]
/// Get list of rooms, optionally filtering to a specific user
pub(crate) async fn get_rooms_list(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    let rooms = get_rooms(params.get("user").cloned().or(None), true);
    Json(rooms)
}

#[debug_handler]
/// Get info about a specific room
pub(crate) async fn get_room_info(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    let room_id = params.get("id").unwrap_or(&"INVALID".to_owned()).clone();
    let room = ROOMS.get(&room_id);
    
    if room.is_none() {
        return (axum::http::StatusCode::NOT_FOUND,Json(None));    
    }

    let server = get_server().to_owned();
    let room = room.unwrap().clone();

    let visitors = room.metadata.visitors.clone().into_iter().collect();
    
    
    (axum::http::StatusCode::OK, Json(Some(RoomInfo{
        id: room.metadata.name.clone(),
        environment: room.metadata.environment.clone(),
        server,
        creator: "TODO".to_owned(),
        has_password: room.metadata.password.is_some(),
        is_hibernating: room.metadata.hibernating.load(std::sync::atomic::Ordering::Relaxed),
        visitors,
    })))
}

/// Get list of rooms, optionally filtering to a specific user
fn get_rooms(user_filter: Option<String>, include_hibernating: bool) -> Vec<RoomInfo> {
    ROOMS.iter().filter(|r| {
        if let Some(ref user) = user_filter {
            // Skip if user not in visitors
            if !user.is_empty() && !r.metadata.visitors.contains(user) {
                return false;
            }
        }

        // Skip if hibernating and not requested
        include_hibernating || !r.metadata.hibernating.load(std::sync::atomic::Ordering::Relaxed)
    }).map(|r| r.metadata.get_room_info()).collect()
}

#[debug_handler]
pub(crate) async fn get_healthcheck() -> impl IntoResponse {
    // If uptime is less than 16 hours, return 200 OK
    if LAUNCH_TIME.elapsed().unwrap().as_secs() < HEALTHCHECK_MINIMUM_UPTIME {
        return (axum::http::StatusCode::OK, "Too new");
    }

    // Check if any rooms active
    if ROOMS.len() == 0 {
        return (axum::http::StatusCode::SERVICE_UNAVAILABLE, "Can restart, no rooms active");
    }

    // TODO: Create room and test WS connection, may need to be done in a separate program

    (axum::http::StatusCode::OK, "Rooms active, don't restart")
}

#[debug_handler]
pub(crate) async fn post_create(Json(request): Json<CreateRoomRequestData>) -> impl IntoResponse {
    let room_id = create_room(request.environment, request.password, request.edit_mode).await;

    // Send room info to API (force announcement when room is created)
    ROOMS.get(&room_id).unwrap().value().announce(true);

    Json(CreateRoomResponseData {
        server: get_server(),
        room_id
    })
}

#[debug_handler]
pub(crate) async fn get_environments_list() -> impl IntoResponse {
    // Return DEFAULT_SCENARIOS_FILE string with JSON content type
    ([(header::CONTENT_TYPE, "application/json"),], DEFAULT_SCENARIOS_FILE)
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
        let response = REQWEST_CLIENT.get(url).send().await?;
        let text = response.text().await?;
        Ok(text.trim().to_string())
    }
}

/// Get main API server URL
pub(crate) fn get_main_api_server() -> String {
    #[cfg(debug_assertions)]
    {
        "http://127.0.0.1:5001".to_owned()
    }
    #[cfg(not(debug_assertions))]
    {
        "https://roboscapeonlineapi2.netsblox.org".to_owned()
    }
}

pub(crate) fn get_server() -> String {
    let ip = EXTERNAL_IP.lock().unwrap().clone().unwrap().replace(".", "-");
    if ip == "127-0-0-1" {"ws"} else {"wss"}.to_owned() + "://" + &ip + ".roboscapeonlineservers.netsblox.org" + if ip == "127-0-0-1" {":5000"} else {""}
}

pub(crate) fn get_local_api_server() -> String {
    let ip = EXTERNAL_IP.lock().unwrap().clone().unwrap().replace(".", "-");
    // Port 3000 is what's exposed on deployed servers, even if locally running on a different port
    if ip == "127-0-0-1" {"http"} else {"https"}.to_owned() + "://" + &ip + ".roboscapeonlineservers.netsblox.org:3000"
}
