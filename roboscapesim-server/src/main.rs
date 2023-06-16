use anyhow::Result;
use axum::{response::Html, response::IntoResponse, routing::get, routing::post, Json, Router, http::{Method, header}};
use cyberdeck::*;
use rapier3d::{na::Vector3, prelude::vector};
use roboscapesim_common::*;
use std::{net::SocketAddr, cell::RefCell, sync::Arc};
use tokio::time::{sleep, Duration};
use tower_http::cors::{Any, CorsLayer};
use dashmap::{DashMap, DashSet};
use once_cell::sync::Lazy;

static ROOM: Lazy<DashMap<String, ObjectData>> = Lazy::new(|| {
    DashMap::new()
});

#[tokio::main]
async fn main() {
    // Setup test room
    ROOM.insert("robot".into(), ObjectData { 
        name: "robot".into(),
        transform: Transform { ..Default::default() }, 
        visual_info: VisualInfo::Mesh("parallax_robot.glb".into()) 
    });
    ROOM.insert("ground".into(), ObjectData { 
        name: "ground".into(),
        transform: Transform { scaling: vector![100.0, 0.05, 100.0], position: vector![0.0, -0.095, 0.0], ..Default::default() }, 
        visual_info: VisualInfo::Color(1.0, 1.0, 1.0) 
    });

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
    println!("Running server on http://localhost:3000 ...");
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
    let mut peer = Peer::new(|peer_id, e| async move {
        match e {
            PeerEvent::DataChannelMessage(c, m) => {
                println!(
                    "{}::Recieved a message from channel {} with id {}!",
                    peer_id,
                    c.label(),
                    c.id()
                );
                let msg_str = String::from_utf8(m.data.to_vec()).unwrap();
                println!(
                    "{}::Message from DataChannel '{}': {}",
                    peer_id,
                    c.label(),
                    msg_str
                );
                //c.send_text(format!("Echo {}", msg_str)).await.unwrap();
            }
            PeerEvent::DataChannelStateChange(c) => {
                if c.ready_state() == RTCDataChannelState::Open {
                    println!("{}::DataChannel '{}'", peer_id, c.label());
                    c.send_text(serde_json::to_string(&ROOM.iter().map(|e| e.value().to_owned()).collect::<Vec<_>>()).unwrap())
                        .await
                        .unwrap();
                } else if c.ready_state() == RTCDataChannelState::Closed {
                    println!("{}::DataChannel '{}'", peer_id, c.label());
                }
            }
            PeerEvent::PeerConnectionStateChange(s) => {
                println!("{}::Peer connection state: {} ", peer_id, s)
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