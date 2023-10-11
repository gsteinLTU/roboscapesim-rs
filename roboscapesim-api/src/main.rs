use axum::{Json, response::IntoResponse, extract::Query};
use log::{info, error, debug};
use roboscapesim_common::api::{CreateRoomRequestData, CreateRoomResponseData, ServerStatus, RoomInfo};
use std::{sync::Mutex, net::SocketAddr, collections::HashMap};
use axum::{routing::{post, get, put}, Router, http::{Method, header}};
use tower_http::cors::{Any, CorsLayer};

#[tokio::main]
async fn main() {

    let app = Router::new()
    .route("/server/status", get(get_server_status))
    .route("/rooms/list", get(get_rooms_list))
    .route("/rooms/create", post(post_create))
    .route("/rooms/info", get(get_room_info))
    .route("/server/announce", post(post_announce))
    .route("/server/rooms", put(put_server_rooms))
    .route("/environments/list", get(get_environments_list))
	.layer(CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods([Method::GET, Method::POST])
        // allow requests from any origin
        .allow_origin(Any)
	    .allow_headers([header::CONTENT_TYPE]));
    
    
    let addr = SocketAddr::from(([127, 0, 0, 1], 5000));
    debug!("listening on {}", addr);
    let server = axum::Server::bind(&addr)
        .serve(app.into_make_service());

    if let Err(err) = server.await {
        error!("server error: {}", err);
    }
}

/// Get status of rooms on server
async fn get_server_status() -> impl IntoResponse {

}

/// Get list of rooms, optionally filtering to a specific user
async fn get_rooms_list(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {

}

/// Create a new room
async fn post_create(Json(data): Json<CreateRoomRequestData>) -> impl IntoResponse {

}

/// Get info about a room
async fn get_room_info(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {

}

/// Receive announcement from server
async fn post_announce() -> impl IntoResponse {

}

/// Update list of rooms on server
async fn put_server_rooms() -> impl IntoResponse {

}

/// Get list of environments
async fn get_environments_list() -> impl IntoResponse {

}