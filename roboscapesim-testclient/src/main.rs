
use async_tungstenite::tungstenite::Message;
use clap::{Parser, Subcommand};
use roboscapesim_common::{ClientMessage, UpdateMessage};
use serde::{Deserialize, Serialize};
use log::{info};
use futures::prelude::*;

#[derive(Parser, Debug, Clone)]
#[command(name="roboscapesim-testclient", version="0.1.0", about="Test client for RoboScape Online")]
struct Args {
    num_clients: usize,

    scenario: Option<String>,

    #[arg(short = 'r', long)]
    roboscape_online_server: Option<String>,

    #[arg(short = 'n', long)]
    netsblox_services_server: Option<String>,

    #[arg(short = 'c', long)]
    netsblox_cloud_server: Option<String>,
}


#[tokio::main]
async fn main() {
    let mut args = Args::parse();

    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .with_module_level("roboscapesim_testclient", log::LevelFilter::Info)
        .env()
        .init()
        .unwrap();

    if args.roboscape_online_server.is_none() {
        args.roboscape_online_server = Some("http://localhost:5001".to_owned());
    }

    if args.netsblox_services_server.is_none() {
        args.netsblox_services_server = Some("http://localhost:8080".to_owned());
    }

    if args.netsblox_cloud_server.is_none() {
        args.netsblox_cloud_server = Some("http://localhost:7777".to_owned());
    }

    if args.scenario.is_none() {
        args.scenario = Some("Default".to_owned());
    }

    let client = reqwest::Client::new();
    
    // Get configuration from NetsBlox cloud server
    let config = client.get(format!("{}/configuration", args.netsblox_cloud_server.unwrap()))
        .send()
        .await
        .expect("Failed to get NetsBlox cloud config")
        .json::<NetsBloxCloudConfig>().await.expect("Failed to parse NetsBlox cloud config");

    info!("NetsBlox cloud config: {:?}", config);

    // Send request to API server
    let username = config.username.unwrap_or(config.client_id).to_owned();
    let room = client.post(format!("{}/rooms/create", args.roboscape_online_server.unwrap()))
        .json(&roboscapesim_common::api::CreateRoomRequestData {
            environment: args.scenario.clone(),
            password: None,
            username: username.clone(),
            edit_mode: false,
        })
        .send()
        .await
        .expect("Failed to create room")
        .json::<roboscapesim_common::api::CreateRoomResponseData>().await.expect("Failed to parse response from server");

    // Create websocket connection to simulation server
    let (mut ws_stream, _) = async_tungstenite::tokio::connect_async(room.server).await.expect("Failed to connect to simulation server");
    info!("Connected to simulation server");

    // Send join message
    ws_stream.send(Message::Binary(rmp_serde::to_vec(&ClientMessage::JoinRoom(room.room_id.clone(), username.clone(), None)).unwrap())).await.expect("Failed to send join message");

    // Read incoming
    loop {
        let incoming = ws_stream.next().await.ok_or("didn't receive anything").unwrap().unwrap().into_data();
        let msg: UpdateMessage = rmp_serde::from_slice(incoming.as_slice()).unwrap();
        info!("Received: {:?}", msg);
    }
    //http://localhost:8080/PublicRoles/getUserVariable?clientId=_netsblox69d5c21e-629f-4f5e-af1a-3d8bd7196762&t=1698638825397
}


#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServiceHost {
    pub url: String,
    pub categories: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NetsBloxCloudConfig {
        pub client_id: String,
        pub username: Option<String>,
        pub services_hosts: Vec<ServiceHost>,
        pub cloud_url: String,
}
