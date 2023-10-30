
use std::time::{SystemTime, Duration, Instant};

use async_tungstenite::tungstenite::Message;
use clap::Parser;
use roboscapesim_common::{ClientMessage, UpdateMessage};
use serde::{Deserialize, Serialize};
use log::{info, trace, warn};
use futures::{prelude::*, future::join_all};
use tokio::{task, select};

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


    // Wait on rx task
    let mut tasks = vec![];
    for i in 0..args.num_clients {
        tasks.push(run_test_client(&args, i));
    }

    join_all(tasks).await;
}

async fn run_test_client(args: &Args, id: usize) {
    let client = reqwest::Client::new();
    
    // Get configuration from NetsBlox cloud server
    let config = client.get(format!("{}/configuration", args.netsblox_cloud_server.clone().unwrap()))
        .send()
        .await
        .expect("Failed to get NetsBlox cloud config")
        .json::<NetsBloxCloudConfig>().await.expect("Failed to parse NetsBlox cloud config");

    trace!("Client {}: NetsBlox cloud config: {:?}", id, config);

    // Send request to API server
    let username = config.username.unwrap_or(config.client_id.clone()).to_owned();
    let room = client.post(format!("{}/rooms/create", args.roboscape_online_server.clone().unwrap()))
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
    info!("Client {}: Connected to simulation server", id);

    // Send join message
    ws_stream.send(Message::Binary(rmp_serde::to_vec(&ClientMessage::JoinRoom(room.room_id.clone(), username.clone(), None)).unwrap())).await.expect("Failed to send join message");

    let ws_stream = std::sync::Arc::new(tokio::sync::Mutex::new(ws_stream));

    let ws_rx = ws_stream.clone();
    
    let robots = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new()));

    // Read incoming
    let rx_robots = robots.clone();
    let rx_task = task::spawn(async move {
        loop {
            let incoming = ws_rx.lock().await.next().await.ok_or("didn't receive anything");
            
            if incoming.is_ok() {
                let incoming = incoming.unwrap().unwrap().into_data();
                let msg: UpdateMessage = rmp_serde::from_slice(incoming.as_slice()).unwrap();

                if let UpdateMessage::Update(_, _, objects) = &msg  {
                    for o in objects {
                        let robot_id = o.0.clone().replace("robot_", "");
                        if o.0.starts_with("robot_") && !rx_robots.lock().await.contains(&robot_id) {
                            rx_robots.lock().await.push(robot_id.clone());
                            info!("Client {}: Robot {} seen", id, robot_id.clone());
                        }
                    }    
                }

                trace!("Client {}: Received: {:?}", id, msg);
            }

            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    });

    // Send IoTScape requests to services server
    let client_id = config.client_id.clone();
    let robots = robots.clone();
    let services_server = args.netsblox_services_server.clone().unwrap();
    let iotscape_task = task::spawn(async move {
        let client = reqwest::Client::new();
        let mut count = 0;
        let start = Instant::now();
        let mut last_stat = Instant::now();
        loop {
            if robots.lock().await.len() > 0 {
                let iotscape_request = client.post(format!("{}/ProximitySensor/getIntensity?clientId={}&t={}", services_server, &client_id, SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs()))
                    .json(&serde_json::json!({
                        "id": robots.lock().await[0].clone(),
                    }))
                    .timeout(Duration::from_secs(1))
                    .send()
                    .await;

                if let Ok(iotscape_request) = iotscape_request {
                    trace!("Client {}: IoTScape request: {:?}", id, iotscape_request);
                    count += 1;
                } else if let Err(e) = iotscape_request {
                    warn!("Client {}: IoTScape request error: {:?}", id, e);
                }

            }

            if last_stat.elapsed() > Duration::from_secs(1) {
                info!("Client {}: {} requests in {} seconds ({} per second)", id, count, start.elapsed().as_secs(), count as f64 / start.elapsed().as_secs() as f64);
                last_stat = Instant::now();
            }

            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    });

    select! {
        _ = rx_task => (),
        _ = iotscape_task => (),
    }
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
