use derivative::Derivative;
use log::info;
use tokio::{net::TcpStream, sync::{broadcast::{Receiver, Sender, self}, Mutex}};
use tokio_websockets::WebsocketStream;
use roboscapesim_common::{ClientMessage, UpdateMessage};
use std::sync::Arc;

use crate::CLIENTS;

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
    pub stream: Arc<Mutex<WebsocketStream<TcpStream>>>,
}

pub async fn accept_connection(stream: TcpStream) -> u128 {
    let addr = stream.peer_addr().expect("connected streams should have a peer address");
    info!("Peer address: {}", addr);

    let ws_stream = tokio_websockets::ServerBuilder::new().accept(stream)
        .await
        .expect("Error during the websocket handshake occurred");
    
    let id = rand::random();
    info!("New WebSocket connection id {} ({})", id, addr);
    
    let (tx, rx1) = broadcast::channel(16);
    let (tx1, rx) = broadcast::channel(16);
    CLIENTS.insert(id, SocketInfo { 
        tx: Arc::new(Mutex::new(tx)), 
        tx1: Arc::new(Mutex::new(tx1)), 
        rx: Arc::new(Mutex::new(rx)), 
        rx1: Arc::new(Mutex::new(rx1)), 
        stream: Arc::new(Mutex::new(ws_stream))
    });
    id
}