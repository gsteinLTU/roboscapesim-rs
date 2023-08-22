use std::f32::consts::FRAC_PI_2;
use std::time::{Duration, Instant};

use chrono::Utc;
use dashmap::{DashMap, DashSet};
use derivative::Derivative;
use log::{error, info, trace};
use nalgebra::{point,vector};
use rand::Rng;
use rapier3d::prelude::{ColliderBuilder, RigidBodyBuilder};
use roboscapesim_common::*;
use serde::Serialize;

use crate::services::entity::{create_entity_service, handle_entity_message};
use crate::services::lidar::{handle_lidar_message, LIDARConfig, create_lidar_service};
use crate::services::position::{handle_position_sensor_message, create_position_service};
use crate::services::service_struct::{Service, ServiceType};
use crate::services::world::{self, handle_world_msg};
use crate::simulation::Simulation;
use crate::util::extra_rand::UpperHexadecimal;

use crate::CLIENTS;
use crate::robot::RobotData;
use crate::util::traits::resettable::{Resettable, RigidBodyResetter};

#[derive(Derivative)]
#[derivative(Debug)]
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
    pub last_update: Instant,
    pub last_full_update: i64,
    pub roomtime: f64,
    pub robots: DashMap<String, RobotData>,
    #[derivative(Debug = "ignore")]
    pub sim: Simulation,
    #[derivative(Debug = "ignore")]
    pub reseters: DashMap<String, Box<dyn Resettable + Send + Sync>>,
    #[derivative(Debug = "ignore")]
    pub services: Vec<Service>,
    #[derivative(Debug = "ignore")]
    pub lidar_configs: DashMap<String, LIDARConfig>,
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
            last_full_update: 0,
            roomtime: 0.0,
            sim: Simulation::new(),
            last_update: Instant::now(),
            robots: DashMap::new(),
            reseters: DashMap::new(),
            services: vec![],
            lidar_configs: DashMap::new(),
        };

        info!("Room {} created", obj.name);

        // Setup test room
        // Create IoTScape service
        let service = world::create_world_service(obj.name.as_str());
        obj.services.push(service);

        // Ground
        let rigid_body = RigidBodyBuilder::fixed().translation(vector![0.0, -0.1, 0.0]);
        let floor_handle = obj.sim.rigid_body_set.insert(rigid_body);
        let collider = ColliderBuilder::cuboid(100.0, 0.1, 100.0);
        obj.sim.collider_set.insert_with_parent(collider, floor_handle, &mut obj.sim.rigid_body_set);
        obj.sim.rigid_body_labels.insert("ground".into(), floor_handle);
        

        // Test cube
        let body_name = obj.name.to_owned() + &"_" + &"cube";
        let rigid_body = RigidBodyBuilder::dynamic()
            .ccd_enabled(true)
            .translation(vector![1.2, 2.5, 0.0])
            .rotation(vector![3.14159 / 3.0, 3.14159 / 3.0, 3.14159 / 3.0])
            .build();
        let collider = ColliderBuilder::cuboid(0.5, 0.5, 0.5).restitution(0.3).density(0.1).build();
        let cube_body_handle = obj.sim.rigid_body_set.insert(rigid_body);
        obj.sim.collider_set.insert_with_parent(collider, cube_body_handle, &mut obj.sim.rigid_body_set);
        obj.sim.rigid_body_labels.insert(body_name.clone(), cube_body_handle);
        obj.objects.insert(body_name.clone(), ObjectData {
            name: body_name.clone(),
            transform: Transform { ..Default::default() },
            visual_info: Some(VisualInfo::Color(1.0, 1.0, 1.0)),
            is_kinematic: false,
            updated: true,
        });
        obj.reseters.insert(body_name.clone(), Box::new(RigidBodyResetter::new(cube_body_handle, &obj.sim)));

        let service = create_entity_service(&body_name, &cube_body_handle);
        obj.services.push(service);


        // Create robot 1
        let mut robot = RobotData::create_robot_body(&mut obj.sim, None, None, None);
        let robot_id: String = ("robot_".to_string() + robot.id.as_str()).into();
        obj.sim.rigid_body_labels.insert(robot_id.clone(), robot.body_handle);
        obj.objects.insert(robot_id.clone(), ObjectData {
            name: robot_id.clone(),
            transform: Transform {scaling: vector![3.0,3.0,3.0], ..Default::default() },
            visual_info: Some(VisualInfo::Mesh("parallax_robot.glb".into())),
            is_kinematic: false,
            updated: true,
        });
        RobotData::setup_robot_socket(&mut robot);
        
        let service = create_position_service(&robot.id, &robot.body_handle);
        obj.services.push(service);
        
        let service = create_lidar_service(&robot.id, &robot.body_handle);
        obj.services.push(service);
        obj.lidar_configs.insert(robot.id.clone(), LIDARConfig { num_beams: 16, start_angle: -FRAC_PI_2, end_angle: FRAC_PI_2, offset_pos: vector![0.17,0.1,0.0], max_distance: 3.0 });
        
        obj.robots.insert(robot.id.to_string(), robot);

        // Create robot 2
        let mut robot2 = RobotData::create_robot_body(&mut obj.sim, None, Some(vector![1.0, 1.0, 1.0]), None);
        let robot2_id: String = ("robot_".to_string() + robot2.id.as_str()).into();
        obj.sim.rigid_body_labels.insert(robot2_id.clone(), robot2.body_handle);
        obj.objects.insert(robot2_id.clone(), ObjectData {
            name: robot2_id.clone(),
            transform: Transform {scaling: vector![3.0,3.0,3.0], ..Default::default() },
            visual_info: Some(VisualInfo::Mesh("parallax_robot.glb".into())),
            is_kinematic: false,
            updated: true,
        });
        RobotData::setup_robot_socket(&mut robot2);
        
        let service = create_position_service(&robot2.id, &robot2.body_handle);
        obj.services.push(service);

        let service = create_lidar_service(&robot2.id, &robot2.body_handle);
        obj.services.push(service);

        obj.robots.insert(robot2.id.to_string(), robot2);

        // Wheel debug
        // let mut i = 0;
        // for wheel in &robot.wheel_bodies {
        //     obj.sim.rigid_body_labels.insert(format!("wheel_{}", i).into(), wheel.clone());
        //     obj.objects.insert(format!("wheel_{}", i).into(), ObjectData {
        //         name: format!("wheel_{}", i).into(),
        //         transform: Transform { scaling: vector![0.18,0.03,0.18], ..Default::default() },
        //         visual_info: VisualInfo::Color(1.0, 1.0, 1.0),
        //         is_kinematic: false,
        //         updated: true,
        //     });
        //     i += 1;
        // }


        obj.objects.insert("ground".into(), ObjectData {
            name: "ground".into(),
            transform: Transform { scaling: vector![100.0, 0.4, 100.0], position: point![0.0, 0.1, 0.0], ..Default::default() },
            visual_info: Some(VisualInfo::Color(0.8, 0.6, 0.45)),
            is_kinematic: false,
            updated: true,
        });

        obj
    }

    /// Send a serialized object of type T to the client
    pub async fn send_to_client<T: Serialize>(val: &T, client_id: u128) -> usize {
        let msg = serde_json::to_string(val).unwrap();
        let client = CLIENTS.get(&client_id);

        if let Some(client) = client {
            return client.value().send_text(msg).await.unwrap_or_default();
        } else {
            error!("Client {} not found!", client_id);
            return 0;
        }
    }

    /// Send a serialized object of type T to all clients in list
    pub async fn send_to_clients<T: Serialize>(val: &T, clients: impl Iterator<Item = u128>) {
        let msg = serde_json::to_string(val).unwrap();

        for client_id in clients {
            let client = CLIENTS.get(&client_id);
            
            if let Some(client) = client {
                client.value().send_text(&msg).await.unwrap_or_default();
            } else {
                error!("Client {} not found!", client_id);
            }
        }
    }

    /// Send the room's current state data to a specific client
    pub async fn send_info_to_client(&self, client: u128) {
        let mut users = vec![];

        for user in self.visitors.iter() {
            users.push(user.clone());
        }

        Self::send_to_client(
            &UpdateMessage::RoomInfo(
                RoomState { name: self.name.clone(), roomtime: self.roomtime, users }
            ),
            client,
        )
        .await;
    }

    /// Send the room's current state data to a specific client
    pub async fn send_state_to_client(&self, full_update: bool, client: u128) {
        if full_update {
            Self::send_to_client(
                &UpdateMessage::Update(self.roomtime, true, self.objects.clone()),
                client,
            )
            .await;
        } else {
            Self::send_to_client(
                &UpdateMessage::Update(
                    self.roomtime,
                    false,
                    self.objects
                        .iter()
                        .filter(|mvp| mvp.value().updated)
                        .map(|mvp| {
                            let mut val = mvp.value().clone();
                            val.visual_info = None;
                            (mvp.key().clone(), val)
                        })
                        .collect::<DashMap<String, ObjectData>>(),
                ),
                client,
            )
            .await;
        }
    }

    pub async fn send_state_to_all_clients(&self, full_update: bool) {
        for client in &self.sockets {
            self.send_state_to_client(full_update, client.value().to_owned())
                .await;
        }

        for mut obj in self.objects.iter_mut() {
            obj.value_mut().updated = false;
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
        // Check if room empty/not empty
        if !self.hibernating && self.sockets.len() == 0 {
            self.hibernating = true;
            return;
        } else if self.hibernating && self.sockets.len() > 0 {
            self.hibernating = false;
        }

        if self.hibernating {
            return;
        }

        trace!("Updating {}", self.name);

        let time = Utc::now().timestamp();

        for mut robot in self.robots.iter_mut() {
            if RobotData::robot_update(robot.value_mut(), &mut self.sim, &self.sockets, delta_time).await {
                self.last_interaction_time = Utc::now().timestamp();
            }
        }

        let mut msgs = vec![];
        for service in self.services.iter_mut() {
            // Handle messages
            if service.update() > 0 {
                loop {
                    if service.service.lock().unwrap().rx_queue.len() == 0 {
                        break;
                    }

                    let msg = service.service.lock().unwrap().rx_queue.pop_front().unwrap();

                    msgs.push((service.service_type, msg));
                }
            }
        }

        if msgs.len() > 0 {
            if msgs.iter().filter(|msg| msg.1.function != "heartbeat").count() > 0 {
                self.last_interaction_time = Utc::now().timestamp();
            }
        }
        
        for (service_type, msg) in msgs {
            info!("{:?}", msg);
            match service_type {
                ServiceType::World => handle_world_msg(self, msg),
                ServiceType::Entity => handle_entity_message(self, msg),
                ServiceType::PositionSensor => handle_position_sensor_message(self, msg),
                ServiceType::LIDAR => handle_lidar_message(self, msg),
                t => {
                    info!("Service type {:?} not yet implemented.", t);
                }
            }
        }
        
        self.sim.update(delta_time);

        // Update data before send
        for mut o in self.objects.iter_mut()  {
            if self.sim.rigid_body_labels.contains_key(o.key()) {
                let get = &self.sim.rigid_body_labels.get(o.key()).unwrap();
                let handle = get.value();
                let body = self.sim.rigid_body_set.get(*handle).unwrap();
                let old_transform = o.value().transform;
                o.value_mut().transform = Transform { position: body.translation().clone().into(), rotation: Orientation::Quaternion(body.rotation().quaternion().clone()), scaling: old_transform.scaling };

                if old_transform != o.value().transform {
                    o.value_mut().updated = true;
                }
            }
        }

        self.roomtime += delta_time;

        if time - self.last_full_update < 60 {
            if (Instant::now() - self.last_update) > Duration::from_millis(100) {
                self.send_state_to_all_clients(false).await;
                self.last_update = Instant::now();
            }
        } else {
            self.send_state_to_all_clients(true).await;
            self.last_update = Instant::now();
            self.last_full_update = time;
        }
    }

    /// Reset entire room
    pub fn reset(&mut self){
        // Reset robots
        for mut r in self.robots.iter_mut() {
            r.reset(&mut self.sim);
        }

        for mut resetter in self.reseters.iter_mut() {
            resetter.value_mut().reset(&mut self.sim);
        }

        self.last_interaction_time = Utc::now().timestamp();
    }

    /// Reset single robot
    pub fn reset_robot(&mut self, id: &str){
        if self.robots.contains_key(&id.to_string()) {
            self.robots.get_mut(&id.to_string()).unwrap().reset(&mut self.sim);
        } else {
            info!("Request to reset non-existing robot {}", id);
        }

        self.last_interaction_time = Utc::now().timestamp();
    }
}