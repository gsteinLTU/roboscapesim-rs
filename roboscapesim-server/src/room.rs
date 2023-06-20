use std::sync::Arc;

use chrono::Utc;
use dashmap::{DashMap, DashSet};
use nalgebra::vector;
use rand::{Rng};
use roboscapesim_common::*;
use cyberdeck::{RTCDataChannel};
use log::info;

#[path ="./util/mod.rs"]
mod util;
use util::extra_rand::UpperHexadecimal;


/// Holds the data for a single room
pub struct RoomData {
    pub objects: DashMap<String, ObjectData>,
    pub name: String,
    pub password: Option<String>,
    pub timeout: i64,
    pub last_interaction_time: i64,
    pub hibernating: bool,
    pub sockets: DashMap<String, Arc<RTCDataChannel>>,
    pub visitors: DashSet<String>,
}

impl RoomData {
    pub fn new(name: Option<String>, password: Option<String>) -> RoomData {
        let mut obj = RoomData {
            objects: DashMap::new(),
            name: name.unwrap_or(Self::generate_room_id(None)),
            password,
            timeout: 60 * 15,
            last_interaction_time: Utc::now().timestamp(),
            hibernating: false,
            sockets: DashMap::new(),
            visitors: DashSet::new(),
        };

        info!("Room {} created", obj.name);


        // Setup test room
        obj.objects.insert("robot".into(), ObjectData { 
            name: "robot".into(),
            transform: Transform { ..Default::default() }, 
            visual_info: VisualInfo::Mesh("parallax_robot.glb".into()) 
        });
        obj.objects.insert("ground".into(), ObjectData { 
            name: "ground".into(),
            transform: Transform { scaling: vector![100.0, 0.05, 100.0], position: vector![0.0, -0.095, 0.0], ..Default::default() }, 
            visual_info: VisualInfo::Color(0.8, 0.6, 0.45) 
        });

        obj
    }

    fn generate_room_id(length: Option<usize>) -> String {
        let s: String = rand::thread_rng()
            .sample_iter(&UpperHexadecimal)
            .take(length.unwrap_or(5))
            .map(char::from)
            .collect();
        ("Room".to_owned() + &s).to_owned()
    }
}