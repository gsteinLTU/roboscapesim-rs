use super::*;

#[derive(Debug)]
pub struct ClientsManager {
    pub(crate) sockets: DashMap<String, DashSet<u128>>,
}

impl ClientsManager {
    pub fn new() -> Self {
        ClientsManager {
            sockets: DashMap::new(),
        }
    }

    /// Send an UpdateMessage to all clients in the room
    pub fn send_to_all_clients(&self, msg: &UpdateMessage) {
        for client in &self.sockets {
            for client_id in client.iter() {
                Self::send_to_client(
                    msg,
                    client_id.to_owned(),
                );
            }
        }
    }

    /// Send UpdateMessage to all clients in list
    pub fn send_to_clients(msg: &UpdateMessage, clients: impl Iterator<Item = u128>) {
        for client_id in clients {
            let client = CLIENTS.get(&client_id);
            
            if let Some(client) = client {
                client.value().tx.send(msg.clone()).unwrap();
            } else {
                error!("Client {} not found!", client_id);
            }
        }
    }

    /// Send UpdateMessage to a client
    pub fn send_to_client(msg: &UpdateMessage, client_id: u128) {
        let client = CLIENTS.get(&client_id);

        if let Some(client) = client {
            client.value().tx.send(msg.clone()).unwrap();
        } else {
            error!("Client {} not found!", client_id);
        }
    }

    /// Send the room's current state data to a specific client
    pub fn send_info_to_client(&self, room: &RoomData, client: u128) {
        Self::send_to_client(
            &UpdateMessage::RoomInfo(
                RoomState { name: room.metadata.name.clone(), roomtime: room.roomtime.read().unwrap().clone(), users: room.metadata.visitors.clone().into_iter().collect() }
            ),
            client,
        );
    }


    /// Send the room's current state data to a specific client
    pub fn send_state_to_client(&self, room: &RoomData, full_update: bool, client: u128) {
        if full_update {
            Self::send_to_client(
                &UpdateMessage::Update(room.roomtime.read().unwrap().clone(), true, room.objects.iter().map(|kvp| (kvp.key().to_owned(), kvp.value().to_owned())).collect()),
                client,
            );
        } else {
            Self::send_to_client(
                &UpdateMessage::Update(
                    room.roomtime.read().unwrap().clone(),
                    false,
                    room.objects
                        .iter()
                        .filter(|mvp| mvp.value().updated)
                        .map(|mvp| {
                            let mut val = mvp.value().clone();
                            val.visual_info = None;
                            (mvp.key().clone(), val)
                        })
                        .collect::<HashMap<String, ObjectData>>(),
                ),
                client,
            );
        }
    }


    /// Send the room's current state data to all clients
    pub fn send_state_to_all_clients(&self, room: &RoomData, full_update: bool) {
        let update_msg: UpdateMessage;
        if full_update {
            update_msg = UpdateMessage::Update(room.roomtime.read().unwrap().clone(), true, room.objects.iter().map(|kvp| (kvp.key().to_owned(), kvp.value().to_owned())).collect());
        } else {
            update_msg = UpdateMessage::Update(
                room.roomtime.read().unwrap().clone(),
                false,
                room.objects
                    .iter()
                    .filter(|mvp| mvp.value().updated)
                    .map(|mvp| {
                        let mut val = mvp.value().clone();
                        val.visual_info = None;
                        (mvp.key().clone(), val)
                    })
                    .collect::<HashMap<String, ObjectData>>(),
            );
        }

        self.send_to_all_clients(
            &update_msg
        );

        for mut obj in room.objects.iter_mut() {
            obj.value_mut().updated = false;
        }
    }

    /// Get all messages from all clients
    pub fn get_messages(&self) -> Vec<(ClientMessage, String, u128)> {
        let mut msgs = vec![];
        for client in self.sockets.iter() {
            let client_username = client.key().to_owned();
    
            for client in client.value().iter() {
                let client = CLIENTS.get(&client);
    
                if let Some(client) = client {
                    while let Ok(msg) = client.rx.recv_timeout(Duration::ZERO) {
                        msgs.push((msg, client_username.clone(), client.key().to_owned()));
                    }
                }
            }
        }
        msgs
    }

    /// Clean up disconnected clients
    pub fn remove_disconnected_clients(&self, room: &RoomData) {
        let mut disconnected = vec![];
        for client_ids in self.sockets.iter() {
            for client_id in client_ids.value().iter() {
                if !CLIENTS.contains_key(&client_id) {
                    disconnected.push((client_ids.key().clone(), client_id.to_owned()));
                }
            }
        }
        
        // Remove disconnected clients from the room
        for (username, client_id) in disconnected {
            info!("Removing client {} from room {}", client_id, &room.metadata.name);
            self.sockets.get(&username).and_then(|c| c.value().remove(&client_id));
    
            if self.sockets.get(&username).unwrap().value().is_empty() {
                self.sockets.remove(&username);
            }
    
            // Send leave message to clients
            // TODO: handle multiple clients from one username better?
            let world_service_id = room.services.iter().find(|s| s.key().1 == ServiceType::World).unwrap().value().get_service_info().id.clone();
            room.netsblox_msg_tx.send(((world_service_id, ServiceType::World), "userLeft".to_string(), BTreeMap::from([("username".to_owned(), username.to_owned())]))).unwrap();
        }
    }
}