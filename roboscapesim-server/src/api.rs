use axum::{Json, response::IntoResponse, extract::Query};
use log::{info, error};
use roboscapesim_common::api::{CreateRoomRequestData, CreateRoomResponseData, ServerStatus, RoomInfo};
use std::{sync::Mutex, net::SocketAddr, collections::HashMap};
use axum_macros::debug_handler;
use axum::{routing::{post, get}, Router, http::{Method, header}};
use tower_http::cors::{Any, CorsLayer};

use crate::{ROOMS, MAX_ROOMS, room::{create_room, LOCAL_SCENARIOS, DEFAULT_SCENARIOS_FILE}};

pub(crate) static EXTERNAL_IP: Mutex<Option<String>> = Mutex::new(None);

/// Create API server with routes
pub async fn create_api(addr: SocketAddr) {
    let app = Router::new()
    .route("/server/status", get(server_status))
    .route("/rooms/list", get(get_rooms_list))
    .route("/rooms/create", post(post_create))
    .route("/rooms/info", get(get_room_info))
    .route("/environments/list", get(get_environments_list))
	.layer(CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods([Method::GET, Method::POST])
        // allow requests from any origin
        .allow_origin(Any)
	    .allow_headers([header::CONTENT_TYPE]));
    
    let server = axum::Server::bind(&addr)
        .serve(app.into_make_service());

    if let Err(err) = server.await {
        error!("server error: {}", err);
    }
}

/// Get status of rooms on server
pub(crate) async fn server_status() -> impl IntoResponse {
    let mut hibernating_rooms: usize = 0;

    for r in ROOMS.iter() {
        if r.lock().unwrap().hibernating.load(std::sync::atomic::Ordering::Relaxed) {
            hibernating_rooms += 1;
        }
    }

    Json(ServerStatus {
        active_rooms: ROOMS.len(),
        hibernating_rooms,
        max_rooms: MAX_ROOMS,
    })
}

#[debug_handler]
/// Get list of rooms, optionally filtering to a specific user
pub(crate) async fn get_rooms_list(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    let rooms = get_rooms(params.get("user").cloned().or(Some("INVALID".to_owned())), true);
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

    let room = room.unwrap().clone();
    let room_data = room.lock().unwrap();

    let visitors = room_data.visitors.lock().unwrap().clone();
    
    (axum::http::StatusCode::OK, Json(Some(RoomInfo{
        id: room_data.name.clone(),
        environment: "rust".to_string(),
        server: get_server(),
        creator: "TODO".to_owned(),
        has_password: room_data.password.is_some(),
        is_hibernating: room_data.hibernating.load(std::sync::atomic::Ordering::Relaxed),
        visitors,
    })))
}

/// Get list of rooms, optionally filtering to a specific user
fn get_rooms(user_filter: Option<String>, include_hibernating: bool) -> Vec<RoomInfo> {
    let mut rooms = vec![];
    
    let user_filter = user_filter.unwrap_or_default();

    for r in ROOMS.iter() {
        let room_data = r.lock().unwrap();
        // Skip if user not in visitors
        if !user_filter.is_empty() && !room_data.visitors.lock().unwrap().contains(&user_filter) {
            continue;
        }

        if !include_hibernating && room_data.hibernating.load(std::sync::atomic::Ordering::Relaxed) {
            continue;
        }

        let id = room_data.name.clone();

        rooms.push(RoomInfo{
            id,
            environment: "rust".to_string(),
            server: get_server(),
            creator: "TODO".to_owned(),
            has_password: room_data.password.is_some(),
            is_hibernating: room_data.hibernating.load(std::sync::atomic::Ordering::Relaxed),
            visitors: room_data.visitors.lock().unwrap().clone(),
        });
    }
    rooms
}

#[debug_handler]
pub(crate) async fn post_create(Json(request): Json<CreateRoomRequestData>) -> impl IntoResponse {
    let room_id = create_room(request.environment, request.password, request.edit_mode).await;

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

pub(crate) async fn get_external_ip() -> Result<String, reqwest::Error> {
    // Final deployment is expected to be to AWS, although this URL currently works on other networks
    Ok("127.0.0.1".into())
    //let url = "http://checkip.amazonaws.com";
    //reqwest::get(url).await.unwrap().text().await
}

pub(crate) fn get_server() -> String {
    let ip = EXTERNAL_IP.lock().unwrap().clone().unwrap().replace(".", "-");
    if ip == "127-0-0-1" {"ws"} else {"wss"}.to_owned() + "://" + &ip + ".roboscapeonlineservers.netsblox.org:5000"
}