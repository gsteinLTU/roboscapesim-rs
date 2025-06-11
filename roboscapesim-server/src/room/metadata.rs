use crate::util::util::get_timestamp;
use crate::room::clients::ClientsManager;
use crate::api::get_server;

use roboscapesim_common::api::RoomInfo;

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;

use dashmap::DashSet;
use log::info;

#[derive(Debug)]
pub struct RoomMetadata {
    pub name: String,
    pub environment: String,
    pub password: Option<String>,
    pub hibernate_timeout: i64,
    pub full_timeout: i64,
    /// List of usernames of users who have visited the room
    pub visitors: DashSet<String>, 
    /// Whether the room is in edit mode, if so, IoTScape messages are sent to NetsBlox server instead of being handled locally by VM
    pub edit_mode: bool,
    pub hibernating: Arc<AtomicBool>,
    pub hibernating_since: Arc<AtomicI64>,
    /// Last time the room was announced to the API server
    pub last_announce_time: Arc<AtomicI64>,
}

impl RoomMetadata {
    pub fn new(name: String, environment: String, password: Option<String>, hibernate_timeout: i64, full_timeout: i64, edit_mode: bool) -> Self {
        Self {
            name,
            environment,
            password,
            hibernate_timeout,
            full_timeout,
            visitors: DashSet::new(),
            edit_mode,
            hibernating: Arc::new(AtomicBool::new(false)),
            hibernating_since: Arc::new(AtomicI64::default()),
            last_announce_time: Arc::new(AtomicI64::new(0)),
        }
    }


    pub(crate) fn get_room_info(&self) -> RoomInfo {
        RoomInfo{
            id: self.name.clone(),
            environment: self.environment.clone(),
            server: get_server().to_owned(),
            creator: "TODO".to_owned(),
            has_password: self.password.is_some(),
            is_hibernating: self.hibernating.load(std::sync::atomic::Ordering::Relaxed),
            visitors: self.visitors.clone().into_iter().collect(),
        }
    }

    /// Check if the room should hibernate or wake up
    pub(crate) fn check_hibernation_state(&self, clients_manager: &ClientsManager) {
        if !self.hibernating.load(Ordering::Relaxed) && clients_manager.sockets.is_empty() {
            self.hibernating.store(true, Ordering::Relaxed);
            self.hibernating_since.store(get_timestamp(), Ordering::Relaxed);
            info!("{} is now hibernating", self.name);
        } else if self.hibernating.load(Ordering::Relaxed) && !clients_manager.sockets.is_empty() {
            self.hibernating.store(false, Ordering::Relaxed);
            info!("{} is no longer hibernating", self.name);
        }    
    }
}
