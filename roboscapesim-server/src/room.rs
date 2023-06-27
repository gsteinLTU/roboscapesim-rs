use chrono::Utc;
use dashmap::{DashMap, DashSet};
use nalgebra::vector;
use rand::{Rng};
use roboscapesim_common::*;
use log::{info, error};
use serde::Serialize;

#[path ="./util/mod.rs"]
mod util;
use util::extra_rand::UpperHexadecimal;

use crate::CLIENTS;


#[derive(Debug)]
/// Holds the data for a single room
pub struct RoomData {
    pub objects: DashMap<String, ObjectData>,
    pub name: String,
    pub password: Option<String>,
    pub timeout: i64,
    pub last_interaction_time: i64,
    pub hibernating: bool,
    pub sockets: DashMap<String, u128>,
    pub visitors: DashSet<String>,
    pub last_full_update: i64,
    pub roomtime: f64,
}

impl RoomData {
    pub fn new(name: Option<String>, password: Option<String>) -> RoomData {
        let obj = RoomData {
            objects: DashMap::new(),
            name: name.unwrap_or(Self::generate_room_id(None)),
            password,
            timeout: 60 * 15,
            last_interaction_time: Utc::now().timestamp(),
            hibernating: false,
            sockets: DashMap::new(),
            visitors: DashSet::new(),
            last_full_update: 0,
            roomtime: 0.0
        };

        info!("Room {} created", obj.name);


        // Setup test room
        obj.objects.insert("robot".into(), ObjectData { 
            name: "robot".into(),
            transform: Transform { ..Default::default() }, 
            visual_info: VisualInfo::Mesh("parallax_robot.glb".into()),
            is_kinematic: false,
            updated: true, 
        });
        obj.objects.insert("ground".into(), ObjectData { 
            name: "ground".into(),
            transform: Transform { scaling: vector![100.0, 0.05, 100.0], position: vector![0.0, -0.095, 0.0], ..Default::default() }, 
            visual_info: VisualInfo::Color(0.8, 0.6, 0.45) ,
            is_kinematic: true,
            updated: true, 
        });

        obj
    }

    /// Send a serialized object of type T to the client
    pub async fn send_to_client<T: Serialize>(&self, val: &T, client_id: u128) -> usize {
        let msg = serde_json::to_string(val).unwrap();
        let client = CLIENTS.get(&client_id);

        if let Some(client) = client { 
            return client.value().send_text(msg).await.unwrap_or_default();
        } else {
            error!("Client {} not found!", client_id);
            return 0;
        }
    }

    pub async fn send_state_to_client(&self, full_update: bool, client: u128) {
        if full_update {
            self.send_to_client(&UpdateMessage::Update(self.roomtime, true, self.objects.clone()), client).await;
        } else {
            self.send_to_client(&UpdateMessage::Update(self.roomtime, false, self.objects.iter().filter(|mvp| mvp.value().updated).map(|mvp| (mvp.key().clone(), mvp.value().clone())).collect::<DashMap<String, ObjectData>>()), client).await;
        }
    }
    
    pub async fn send_state_to_all_clients(&self, full_update: bool) {
        for client in &self.sockets {
            self.send_state_to_client(full_update, client.value().to_owned()).await;
        }
    }

    fn generate_room_id(length: Option<usize>) -> String {
        let s: String = rand::thread_rng()
            .sample_iter(&UpperHexadecimal)
            .take(length.unwrap_or(5))
            .map(char::from)
            .collect();
        ("Room".to_owned() + &s).to_owned()
    }

    pub async fn update(&mut self, delta_time: f64) {
        for mut obj in self.objects.iter_mut() {
            if let Orientation::Euler(mut angles) = obj.value().transform.rotation {
                angles[1] = angles[1] + (1.0 * delta_time) % 360.0;
                obj.value_mut().transform.rotation = Orientation::Euler(angles);
                obj.updated = true;
            }
        }

        let time = Utc::now().timestamp();

        self.roomtime += delta_time;

        if time - self.last_full_update < 60 {
            self.send_state_to_all_clients(false).await;
        } else {
            self.send_state_to_all_clients(true).await;
        }
    }
}