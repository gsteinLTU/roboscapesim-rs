use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;
use std::time::{Duration, Instant};

use chrono::Utc;
use dashmap::{DashMap, DashSet};
use derivative::Derivative;
use log::{error, info, trace};
use nalgebra::{vector, Vector3, UnitQuaternion};
use rand::Rng;
use rapier3d::prelude::{ColliderBuilder, RigidBodyBuilder, AngVector, Real};
use roboscapesim_common::*;

use crate::services::entity::{create_entity_service, handle_entity_message};
use crate::services::lidar::{handle_lidar_message, LIDARConfig, create_lidar_service};
use crate::services::position::{handle_position_sensor_message, create_position_service};
use crate::services::proximity::handle_proximity_sensor_message;
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
    pub last_sim_update: Instant,
    pub roomtime: f64,
    pub robots: HashMap<String, RobotData>,
    #[derivative(Debug = "ignore")]
    pub sim: Simulation,
    #[derivative(Debug = "ignore")]
    pub reseters: HashMap<String, Box<dyn Resettable + Send + Sync>>,
    #[derivative(Debug = "ignore")]
    pub services: Vec<Service>,
    #[derivative(Debug = "ignore")]
    pub lidar_configs: HashMap<String, LIDARConfig>,
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
            robots: HashMap::new(),
            reseters: HashMap::new(),
            services: vec![],
            lidar_configs: HashMap::new(),
            last_sim_update: Instant::now(),
        };

        info!("Room {} created", obj.name);

        // Setup test room
        // Create IoTScape service
        let service = world::create_world_service(obj.name.as_str());
        obj.services.push(service);

        // Ground
        RoomData::add_shape(&mut obj, "ground", vector![0.0, -0.1, 0.0], AngVector::zeros(), Some(VisualInfo::Color(0.8, 0.6, 0.45, Shape::Box)), Some(vector![10.0, 0.1, 10.0]), true);

        // Test cube
        RoomData::add_shape(&mut obj, "cube", vector![1.2, 2.5, 0.0], vector![3.14159 / 3.0, 3.14159 / 3.0, 3.14159 / 3.0], None, None, false);

        // Create robot 1
        RoomData::add_robot(&mut obj, vector![0.0, 1.0, 0.0], UnitQuaternion::from_euler_angles(0.0, 3.14159 / 3.0, 0.0), false);

        // Create robot 2
        RoomData::add_robot(&mut obj, vector![1.0, 1.0, 1.0], UnitQuaternion::from_euler_angles(0.0, 3.14159 / 3.0, 0.0), false);

        obj
    }

    /// Send UpdateMessage to a client
    pub fn send_to_client(msg: &UpdateMessage, client_id: u128) {
        let client = CLIENTS.get(&client_id);

        if let Some(client) = client {
            client.value().tx.lock().unwrap().send(msg.clone()).unwrap();
        } else {
            error!("Client {} not found!", client_id);
        }
    }

    /// Send UpdateMessage to all clients in list
    pub fn send_to_clients(msg: &UpdateMessage, clients: impl Iterator<Item = u128>) {
        for client_id in clients {
            let client = CLIENTS.get(&client_id);
            
            if let Some(client) = client {
                client.value().tx.lock().unwrap().send(msg.clone()).unwrap();
            } else {
                error!("Client {} not found!", client_id);
            }
        }
    }

    /// Send the room's current state data to a specific client
    pub fn send_info_to_client(&self, client: u128) {
        let mut users = vec![];

        for user in self.visitors.iter() {
            users.push(user.clone());
        }

        Self::send_to_client(
            &UpdateMessage::RoomInfo(
                RoomState { name: self.name.clone(), roomtime: self.roomtime, users }
            ),
            client,
        );
    }

    /// Send the room's current state data to a specific client
    pub fn send_state_to_client(&self, full_update: bool, client: u128) {
        if full_update {
            Self::send_to_client(
                &UpdateMessage::Update(self.roomtime, true, self.objects.iter().map(|kvp| (kvp.key().to_owned(), kvp.value().to_owned())).collect()),
                client,
            );
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
                        .collect::<HashMap<String, ObjectData>>(),
                ),
                client,
            );
        }
    }

    pub fn send_to_all_clients(&self, msg: &UpdateMessage) {
        for client in &self.sockets {
            Self::send_to_client(
                msg,
                client.value().to_owned(),
            );
        }
    }

    pub fn send_state_to_all_clients(&self, full_update: bool) {
        let update_msg: UpdateMessage;
        if full_update {
            update_msg = UpdateMessage::Update(self.roomtime, true, self.objects.iter().map(|kvp| (kvp.key().to_owned(), kvp.value().to_owned())).collect());
        } else {
            update_msg = UpdateMessage::Update(
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
                    .collect::<HashMap<String, ObjectData>>(),
            );
        }

        self.send_to_all_clients(
            &update_msg
        );

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

    pub fn update(&mut self) {
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

        //let max_delta_time = 1.0 / 30.0;
        let now = Instant::now();
        /*let delta_time = (now - self.last_sim_update).as_secs_f64();
        let delta_time = f64::min(max_delta_time, delta_time);*/
        let delta_time = 1.0 / 60.0;
        self.last_sim_update = now;

        // Handle client messages
        let mut needs_reset = false;
        let mut robot_resets = vec![];
        for client in self.sockets.iter() {
            let client = CLIENTS.get(client.value());

            if let Some(client) = client {
                let receiver = &mut client.rx.lock().unwrap();
                while let Ok(msg) = receiver.recv_timeout(Duration::default()) {
                    match msg {
                        ClientMessage::ResetAll => { needs_reset = true; },
                        ClientMessage::ResetRobot(robot_id) => {
                            if self.is_authorized(client.key().clone(), &robot_id) {
                                robot_resets.push(robot_id);
                            }
                        },
                        ClientMessage::ClaimRobot(robot_id) => {
                            // TODO: Claim robot
                        },
                        ClientMessage::EncryptRobot(robot_id) => {
                            if let Some(robot) = self.robots.get_mut(&robot_id) {
                                robot.send_roboscape_message(&[b'P', 0]).unwrap();
                                robot.send_roboscape_message(&[b'P', 1]).unwrap();
                            }
                        },
                        _ => {}
                    }
                }
            }
        }

        if needs_reset {
            self.reset();
        } else {
            for robot in robot_resets {
                self.reset_robot(&robot);
            }
        }

        let time = Utc::now().timestamp();

        for robot in self.robots.iter_mut() {
            if RobotData::robot_update(robot.1, &mut self.sim, &self.sockets, delta_time) {
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
            match service_type {
                ServiceType::World => handle_world_msg(self, msg),
                ServiceType::Entity => handle_entity_message(self, msg),
                ServiceType::PositionSensor => handle_position_sensor_message(self, msg),
                ServiceType::LIDAR => handle_lidar_message(self, msg),
                ServiceType::ProximitySensor => handle_proximity_sensor_message(self, msg),
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
            if (now - self.last_update) > Duration::from_millis(120) {
                self.send_state_to_all_clients(false);
                self.last_update = now;
            }
        } else {
            self.send_state_to_all_clients(true);
            self.last_full_update = time;
            self.last_update = now;
        }
    }

    /// Reset entire room
    pub(crate) fn reset(&mut self){
        // Reset robots
        for r in self.robots.iter_mut() {
            r.1.reset(&mut self.sim);
        }

        for resetter in self.reseters.iter_mut() {
            resetter.1.reset(&mut self.sim);
        }

        self.last_interaction_time = Utc::now().timestamp();
    }

    /// Reset single robot
    pub(crate) fn reset_robot(&mut self, id: &str){
        if self.robots.contains_key(&id.to_string()) {
            self.robots.get_mut(&id.to_string()).unwrap().reset(&mut self.sim);
        } else {
            info!("Request to reset non-existing robot {}", id);
        }

        self.last_interaction_time = Utc::now().timestamp();
    }

    /// Test if a client is allowed to interact with a robot (for encrypt, reset)
    pub(crate) fn is_authorized(&self, client: u128, robot_id: &str) -> bool {
        // TODO: check robot claim
        // Make sure not only claim matches but also that claimant is still in-room
        true
    }

    /// Add a robot to a room
    pub(crate) fn add_robot(room: &mut RoomData, position: Vector3<Real>, orientation: UnitQuaternion<f32>, wheel_debug: bool) -> String {
        let mut robot = RobotData::create_robot_body(&mut room.sim, None, Some(position), Some(orientation));
        let robot_id: String = ("robot_".to_string() + robot.id.as_str()).into();
        room.sim.rigid_body_labels.insert(robot_id.clone(), robot.body_handle);
        room.objects.insert(robot_id.clone(), ObjectData {
            name: robot_id.clone(),
            transform: Transform {scaling: vector![3.0,3.0,3.0], ..Default::default() },
            visual_info: Some(VisualInfo::Mesh("parallax_robot.glb".into())),
            is_kinematic: false,
            updated: true,
        });
        RobotData::setup_robot_socket(&mut robot);
            
        let service = create_position_service(&robot.id, &robot.body_handle);
        room.services.push(service);
            
        let service = create_lidar_service(&robot.id, &robot.body_handle);
        room.services.push(service);
        room.lidar_configs.insert(robot.id.clone(), LIDARConfig { num_beams: 16, start_angle: -FRAC_PI_2, end_angle: FRAC_PI_2, offset_pos: vector![0.17,0.1,0.0], max_distance: 3.0 });
            
        // Wheel debug
        if wheel_debug {
            let mut i = 0;
            for wheel in &robot.wheel_bodies {
                room.sim.rigid_body_labels.insert(format!("wheel_{}", i).into(), wheel.clone());
                room.objects.insert(format!("wheel_{}", i).into(), ObjectData {
                    name: format!("wheel_{}", i).into(),
                    transform: Transform { scaling: vector![0.18,0.03,0.18], ..Default::default() },
                    visual_info: Some(VisualInfo::default()),
                    is_kinematic: false,
                    updated: true,
                });
                i += 1;
            }
        }

        let id = robot.id.to_string();
        room.robots.insert(robot.id.to_string(), robot);
        room.last_full_update = 0;
        id
    }

    /// Add a cuboid object to the room
    pub(crate) fn add_shape(room: &mut RoomData, name: &str, position: Vector3<Real>, rotation: AngVector<Real>, mut visual_info: Option<VisualInfo>, mut size: Option<Vector3<Real>>, is_kinematic: bool) {
        let body_name = room.name.to_owned() + &"_" + name;
        let rigid_body = if is_kinematic { RigidBodyBuilder::kinematic_position_based() } else { RigidBodyBuilder::dynamic() }
            .ccd_enabled(true)
            .translation(position)
            .rotation(rotation)
            .build();
        
        if size.is_none() {
            size = Some(vector![0.5, 0.5, 0.5]);
        }

        let mut size = size.unwrap();

        if visual_info.is_none() {
            visual_info = Some(VisualInfo::default());
        }
        
        let shape = match visual_info {
            Some(VisualInfo::Color(_, _, _, s)) => {
                s
            },
            Some(VisualInfo::Texture(_, s)) => {
                s
            },
            _ => Shape::Box
        };

        let collider = match shape {
            Shape::Box => ColliderBuilder::cuboid(size.x, size.y, size.z),
            Shape::Sphere => {
                size.y = size.x;
                size.z = size.x;
                ColliderBuilder::ball(size.x)
            },
            Shape::Cylinder => {
                size.z = size.x;
                ColliderBuilder::cylinder(size.y, size.x)
            },
            Shape::Capsule => {
                size.z = size.x;
                ColliderBuilder::capsule_y(size.y, size.x)
            },
        };

        let collider = collider.restitution(0.3).density(0.1).build();
        let cube_body_handle = room.sim.rigid_body_set.insert(rigid_body);
        room.sim.collider_set.insert_with_parent(collider, cube_body_handle, &mut room.sim.rigid_body_set);
        room.sim.rigid_body_labels.insert(body_name.clone(), cube_body_handle);

        room.objects.insert(body_name.clone(), ObjectData {
            name: body_name.clone(),
            transform: Transform { position: position.into(), scaling: size * 2.0, rotation: Orientation::Euler(rotation), ..Default::default() },
            visual_info,
            is_kinematic,
            updated: true,
        });

        room.reseters.insert(body_name.clone(), Box::new(RigidBodyResetter::new(cube_body_handle, &room.sim)));

        let service = create_entity_service(&body_name, &cube_body_handle);
        room.services.push(service);
        room.last_full_update = 0;
    }
}