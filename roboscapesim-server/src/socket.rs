use derivative::Derivative;
use futures::{StreamExt, stream::{SplitSink, SplitStream}};
use log::{info, trace};
use tokio::net::{TcpStream, TcpListener};
use tokio_tungstenite::{WebSocketStream, tungstenite::Message};
use roboscapesim_common::{ClientMessage, UpdateMessage};
use std::sync::{Arc, Mutex, mpsc::{Sender, Receiver, self}};

use tokio::time::{Duration, sleep};
use futures::{SinkExt, FutureExt};

use crate::{CLIENTS, room::join_room};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct SocketInfo {
    /// To client
    pub tx: Arc<Mutex<Sender<UpdateMessage>>>, 
    /// To server, internal use
    pub tx1: Arc<Mutex<Sender<ClientMessage>>>, 
    /// From client
    pub rx: Arc<Mutex<Receiver<ClientMessage>>>, 
    /// From client, internal use
    pub rx1: Arc<Mutex<Receiver<UpdateMessage>>>, 
    #[derivative(Debug = "ignore")]
    pub sink: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
    #[derivative(Debug = "ignore")]
    pub stream: Arc<Mutex<SplitStream<WebSocketStream<TcpStream>>>>,
}

pub async fn accept_connection(tcp_stream: TcpStream) -> Result<u128, String> {
    let addr = tcp_stream.peer_addr().expect("connected streams should have a peer address");
    info!("Peer address: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(tcp_stream)
        .await;

    if let Err(e) = ws_stream {
        return Err(format!("Error accepting websocket connection: {:?}", e));
    }

    let ws_stream = ws_stream.unwrap();
    
    let (sink, stream) = ws_stream.split();

    let id = rand::random();
    info!("New WebSocket connection id {} ({})", id, addr);
    
    let (tx, rx1) = mpsc::channel();
    let (tx1, rx) = mpsc::channel();
    CLIENTS.insert(id, SocketInfo { 
        tx: Arc::new(Mutex::new(tx)), 
        tx1: Arc::new(Mutex::new(tx1)), 
        rx: Arc::new(Mutex::new(rx)), 
        rx1: Arc::new(Mutex::new(rx1)), 
        sink: Arc::new(Mutex::new(sink)),
        stream: Arc::new(Mutex::new(stream)),
    });
    Ok(id)
}

pub async fn ws_rx() {
    loop {
        let mut disconnected = vec![];
        // Get client updates
        for client in CLIENTS.iter() {
            // RX
            while let Some(Some(msg)) = client.value().stream.lock().unwrap().next().now_or_never() {
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
                                match msg {
                                    ClientMessage::JoinRoom(id, username, password) => {
                                        join_room(&username, &(password.unwrap_or_default()), client.key().to_owned(), &id).unwrap();
                                    },
                                    _ => {
                                        client.tx1.lock().unwrap().send(msg.to_owned()).unwrap();
                                    }
                                }
                            } 
                        }
                        _ => {}
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

        sleep(Duration::from_nanos(50)).await;
    }
}

pub async fn ws_tx() {
    loop {
        // Get client updates
        for client in CLIENTS.iter() {                
            // TX
            let receiver = client.rx1.lock().unwrap();
            while let Ok(msg) = receiver.recv_timeout(Duration::default()) {
                let msg = serde_json::to_string(&msg).unwrap();
                client.sink.lock().unwrap().send(Message::Text(msg)).now_or_never();
            }
        }
        
        sleep(Duration::from_nanos(25)).await;
    }
}

pub async fn ws_accept() {
    let listener = TcpListener::bind("0.0.0.0:5000").await.unwrap();

    loop {
        let (conn, _) = listener.accept().await.unwrap();
        accept_connection(conn).await;
    }
}

