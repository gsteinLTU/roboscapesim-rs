use async_std::net::{TcpListener, TcpStream};
use derivative::Derivative;
use futures::StreamExt;
use log::{info, trace, error, warn};
use once_cell::sync::Lazy;

use async_tungstenite::{WebSocketReceiver, WebSocketSender, tungstenite::Message};
use async_listen::ListenExt;
use roboscapesim_common::{ClientMessage, UpdateMessage};
use std::sync::Arc;

#[cfg(feature = "no_deadlocks")]
use no_deadlocks::Mutex;
#[cfg(not(feature = "no_deadlocks"))]
use std::sync::Mutex;

use crossbeam_channel::{Sender, Receiver, self};

use tokio::time::{Duration, sleep};
use futures::{SinkExt, FutureExt};

use crate::{CLIENTS, room::management::join_room};

/// Local WebSocket port number
pub static LOCAL_WS_PORT: Lazy<u16> = Lazy::new(|| std::env::var("LOCAL_WS_PORT")
    .unwrap_or_else(|_| "5000".to_string())
    .parse::<u16>()
    .expect("PORT must be a number")
);

#[derive(Derivative)]
#[derivative(Debug)]
pub struct SocketInfo {
    /// To client
    pub tx: Sender<UpdateMessage>, 
    /// To server, internal use
    pub tx1: Sender<ClientMessage>, 
    /// From client
    pub rx: Receiver<ClientMessage>, 
    /// From client, internal use
    pub rx1: Receiver<UpdateMessage>, 
    #[derivative(Debug = "ignore")]
    pub sink: Arc<Mutex<WebSocketSender<TcpStream>>>,
    #[derivative(Debug = "ignore")]
    pub stream: Arc<Mutex<WebSocketReceiver<TcpStream>>>,
}

pub async fn accept_connection(tcp_stream: TcpStream) -> Result<u128, String> {
    let addr = tcp_stream.peer_addr();
    
    if let Err(e) = addr {
        return Err(format!("Error getting peer address: {:?}", e));
    }

    let addr = addr.unwrap();    
    info!("Peer address: {}", addr);

    let ws_stream = async_tungstenite::accept_async(tcp_stream).await;

    if let Err(e) = ws_stream {
        return Err(format!("Error accepting websocket connection: {:?}", e));
    }

    let ws_stream = ws_stream.unwrap();
    
    let (sink, stream) = ws_stream.split();

    let id = rand::random();
    info!("New WebSocket connection id {} ({})", id, addr);
    
    let (tx, rx1) = crossbeam_channel::unbounded();
    let (tx1, rx) = crossbeam_channel::unbounded();
    CLIENTS.insert(id, SocketInfo { 
        tx,
        tx1, 
        rx, 
        rx1, 
        sink: Arc::new(Mutex::new(sink)),
        stream: Arc::new(Mutex::new(stream)),
    });

    info!("Connected clients: {}", CLIENTS.len());
    
    Ok(id)
}

pub async fn ws_rx() {
    loop {
        let mut disconnected = vec![];
        // Get client updates
        for client in CLIENTS.iter() {
            // RX
            while let Some(Some(msg)) = client.value().stream.lock().unwrap().next().now_or_never() {

                let mut deserialized_msg = None;

                if let Ok(msg) = msg {
                    trace!("Websocket message from {}: {:?}", client.key(), msg);
                    match msg {
                        Message::Close(_) => {
                            info!("Client {} disconnected", client.key());
                            disconnected.push(client.key().to_owned());
                            break;
                        },
                        Message::Text(msg) => {
                            if let Ok(msg) = serde_json::from_str::<ClientMessage>(&msg) {
                                deserialized_msg = Some(msg);
                            } 
                        },
                        Message::Binary(msg) => {
                            if let Ok(msg) = rmp_serde::from_slice(msg.iter().as_slice()) {
                                deserialized_msg = Some(msg);
                            } 
                        },
                        _ => {}
                    }

                    if let Some(msg) = deserialized_msg {
                        match msg {
                            ClientMessage::JoinRoom(id, username, password) => {
                                if let Err(e) = join_room(&username, &(password.unwrap_or_default()), client.key().to_owned(), &id){
                                    error!("Error joining room: {:?}", e);

                                    // Send error message
                                    client.tx.send(UpdateMessage::FatalError(e.to_string())).unwrap();
                                }   
                            },
                            _ => {
                                client.tx1.send(msg.to_owned()).unwrap();
                            }
                        }
                    }
                       
                } else if let Err(e) = msg {
                    info!("Error receiving websocket message from {}: {:?}", client.key(), e);       
                }
            }
        }

        // Remove disconnected clients
        for disconnect in disconnected {
            CLIENTS.remove(&disconnect);
        }

        sleep(Duration::from_micros(15)).await;
    }
}

pub async fn ws_tx() {
    loop {        
        // Get client updates
        for client in CLIENTS.iter() {                
            // TX
            let mut to_send: Vec<UpdateMessage> = vec![];
            let mut msg_count = 0;
            while let Ok(msg) = client.rx1.recv_timeout(Duration::ZERO) {
                msg_count += 1;
                match msg {
                    UpdateMessage::Update(_, full_update, _) => {
                        if full_update {
                            to_send.push(msg);
                        } else {
                            if to_send.is_empty() {
                                to_send.push(msg);
                            } else {
                                // Replace existing non-full update
                                for i in (0..to_send.len()).rev() {
                                    if let UpdateMessage::Update(_, false, _) = to_send[i] {
                                        to_send.remove(i);
                                        break;
                                    }
                                }

                                to_send.push(msg);
                            }
                        }
                    },
                    _ => {
                        to_send.push(msg);
                    }
                }
            }

            if msg_count > 0 {
                //trace!("Sending {} messages to {}", msg_count, client.key());
            }

            let sink = &mut client.sink.lock().unwrap();
            for msg in to_send {
                let r = rmp_serde::to_vec(&msg);

                if let Ok(buf) = r {
                    sink.feed(Message::Binary(buf.into())).now_or_never();
                } else if let Err(e) = r {
                    info!("Error serializing message: {:?}", e);
                }
            }
            sink.flush().now_or_never();
        }
        
        sleep(Duration::from_micros(15)).await;
    }
}

/// Accept WebSocket connections
pub async fn ws_accept() {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", LOCAL_WS_PORT.clone())).await.expect("Failed to bind WS port");

    let mut incoming = listener.incoming()
        .log_warnings(|e| warn!("Warning accepting connection: {:?}", e))
        .handle_errors(Duration::from_millis(50));
    loop {
        while let Some(conn) = incoming.next().await {
            let conn = accept_connection(conn).await;

            if let Err(e) = conn {
                warn!("Error accepting connection: {:?}", e);
                continue;
            }
        }
        sleep(Duration::from_millis(2)).await;
    }
}

