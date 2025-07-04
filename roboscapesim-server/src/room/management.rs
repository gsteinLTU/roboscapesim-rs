use std::collections::BTreeMap;

use super::RoomData;

use std::sync::atomic::Ordering;

use crate::{room::clients::ClientsManager, services::ServiceType};
use crate::util::util::get_timestamp;

use dashmap::DashSet;
use log::{info, error};
use roboscapesim_common::UpdateMessage;

use crate::ROOMS;

pub fn join_room(username: &str, password: &str, peer_id: u128, room_id: &str) -> Result<(), String> {
    info!("User {} (peer id {}), attempting to join room {}", username, peer_id, room_id);

    if !ROOMS.contains_key(room_id) {
        return Err(format!("Room {} does not exist!", room_id));
    }

    let room = ROOMS.get(room_id).unwrap();

    // Check password
    if room.metadata.password.clone().is_some_and(|pass| pass != password) {
        error!("User {} attempted to join room {} with wrong password", username, room_id);
        return Err("Wrong password!".to_owned());
    }

    // Setup connection to room
    if !room.metadata.visitors.contains(&username.to_owned()) {
        room.metadata.visitors.insert(username.to_owned());
    }

    if !room.clients_manager.sockets.contains_key(username) {
        room.clients_manager.sockets.insert(username.to_string(), DashSet::new());
    }

    room.clients_manager.sockets.get_mut(username).unwrap().insert(peer_id);
    room.last_interaction_time.store(get_timestamp(),Ordering::Relaxed);

    // Give client initial update
    room.clients_manager.send_info_to_client(&room, peer_id);
    room.clients_manager.send_state_to_client(&room, true, peer_id);

    // Send room info to API (force announcement when client joins)
    room.announce(true);

    // Initial robot claim data
    for robot in room.robots.iter() {
        if robot.value().claimed_by.is_some() {   
            ClientsManager::send_to_client(&UpdateMessage::RobotClaimed(robot.key().clone(), robot.value().claimed_by.clone().unwrap_or("".to_owned())), peer_id);
        }
    }

    // Send user join event
    let world_service_id = room.services.iter().find(|s| s.key().1 == ServiceType::World).unwrap().value().get_service_info().id.clone();
    room.netsblox_msg_tx.send(((world_service_id, ServiceType::World), "userJoined".to_string(), BTreeMap::from([("username".to_owned(), username.to_owned())]))).unwrap();

    Ok(())
}

pub async fn create_room(environment: Option<String>, password: Option<String>, edit_mode: bool) -> String {
    let room = RoomData::new(None, environment, password, edit_mode).await;

    // Set last interaction to creation time
    room.last_interaction_time.store(get_timestamp(),Ordering::Relaxed);

    let room_id = room.metadata.name.clone();
    ROOMS.insert(room_id.to_string(), room.clone());
    RoomData::launch(room);

    room_id
}
