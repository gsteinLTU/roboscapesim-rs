use anyhow::Result;
use axum::{response::Html, response::IntoResponse, routing::get, routing::post, Json, Router, http::{Method, header}};
use cyberdeck::*;
use rapier3d::{na::Vector3, prelude::vector};
use roboscapesim_common::*;
mod room;
mod util;
use room::RoomData;
use simple_logger::SimpleLogger;
use std::{net::SocketAddr, cell::RefCell, sync::{Arc, RwLock, Mutex}};
use tokio::time::{sleep, Duration};
use tower_http::cors::{Any, CorsLayer};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use log::{info, trace};

static ROOMS: Lazy<DashMap<String, Arc<RwLock<RoomData>>>> = Lazy::new(|| {
    DashMap::new()
});

#[tokio::main]
async fn main() {
    // Setup logger
    SimpleLogger::new().with_level(log::LevelFilter::Warn).with_module_level("roboscapesim_server", log::LevelFilter::Info).env().init().unwrap();
    info!("Starting RoboScape Online Server...");

    // build our application with a route
    let app = Router::new()
        .route("/connect", post(connect))
	.layer(CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods([Method::GET, Method::POST])
        // allow requests from any origin
        .allow_origin(Any)
	.allow_headers([header::CONTENT_TYPE]));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("Running server on http://localhost:3000 ...");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn connect(Json(offer): Json<String>) -> impl IntoResponse {
    match start_peer_connection(offer).await {
        Ok(answer) => Ok(Json(answer)),
        Err(_) => Err("failed to connect"),
    }
}

async fn start_peer_connection(offer: String) -> Result<String> {
    let room = Arc::new(RwLock::new(RoomData::new(None, None)));
    let room_id = room.read().unwrap().name.clone();
    
    ROOMS.insert(room_id.to_string(), room.clone());

    let mut peer = Peer::new(move |peer_id, e| {
        let room_id = room_id.clone();
        async move {
            let room = ROOMS.get(&(room_id.to_string())).unwrap();
            match e {
                PeerEvent::DataChannelMessage(c, m) => {
                    trace!(
                        "{}::Recieved a message from channel {} with id {}!",
                        peer_id,
                        c.label(),
                        c.id()
                    );
                    let msg_str = String::from_utf8(m.data.to_vec()).unwrap();
                    trace!(
                        "{}::Message from DataChannel '{}': {}",
                        peer_id,
                        c.label(),
                        msg_str
                    );
                    //c.send_text(format!("Echo {}", msg_str)).await.unwrap();
                }
                PeerEvent::DataChannelStateChange(c) => {
                    if c.ready_state() == RTCDataChannelState::Open {
                        trace!("{}::DataChannel '{}'", peer_id, c.label());
                        let msg = serde_json::to_string(&room.read().unwrap().objects.iter().map(|e| e.value().to_owned()).collect::<Vec<_>>()).unwrap();
                        c.send_text(msg)
                            .await
                            .unwrap();
                    } else if c.ready_state() == RTCDataChannelState::Closed {
                        trace!("{}::DataChannel '{}'", peer_id, c.label());
                    }
                }
                PeerEvent::PeerConnectionStateChange(s) => {
                    trace!("{}::Peer connection state: {} ", peer_id, s)
                }
            }
        }
    })
    .await?;
    let answer = peer.receive_offer(&offer).await?;

    // move cyberdeck to another thread to keep it alive
    tokio::spawn(async move {
        while peer.connection_state() != RTCPeerConnectionState::Closed
            && peer.connection_state() != RTCPeerConnectionState::Disconnected
            && peer.connection_state() != RTCPeerConnectionState::Failed
        {
            // keep the connection alive while not in invalid state
            sleep(Duration::from_millis(1000)).await;
        }
        // because we moved cyberdeck ownership into here gets dropped here and will stop all channels
    });

    Ok(answer)
}