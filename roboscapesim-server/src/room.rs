use std::time::Duration;
use std::time::Instant;

use chrono::Utc;
use dashmap::{DashMap, DashSet};
use derivative::Derivative;
use log::{error, info};
use nalgebra::point;
use nalgebra::{vector, Vector3};
use rand::Rng;
use rapier3d::prelude::{
    BroadPhase, CCDSolver, ColliderBuilder, ColliderSet, ImpulseJointSet, IntegrationParameters,
    IslandManager, MultibodyJointSet, NarrowPhase, PhysicsPipeline, RigidBodyBuilder, RigidBodySet, QueryPipeline, RigidBodyHandle,
};
use roboscapesim_common::*;
use serde::Serialize;

use crate::util::extra_rand::UpperHexadecimal;

use crate::CLIENTS;
use crate::robot::RobotData;
use crate::util::traits::Resettable;

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
}

/// Holds rapier-related structs together
pub struct Simulation {
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,
    pub gravity: Vector3<f32>,
    pub integration_parameters: IntegrationParameters,
    pub physics_pipeline: PhysicsPipeline,
    pub island_manager: IslandManager,
    pub broad_phase: BroadPhase,
    pub narrow_phase: NarrowPhase,
    pub impulse_joint_set: ImpulseJointSet,
    pub multibody_joint_set: MultibodyJointSet,
    pub ccd_solver: CCDSolver,
    pub query_pipeline: QueryPipeline,
    pub physics_hooks: (),
    pub event_handler: (),
    pub rigid_body_labels: DashMap<String, RigidBodyHandle>,
}

impl Simulation {
    /// Instantiate the simulation objects with default settings
    fn new() -> Simulation {
        Simulation {
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            gravity: vector![0.0, -9.81, 0.0],
            integration_parameters: IntegrationParameters { max_ccd_substeps: 8, max_stabilization_iterations: 2, ..Default::default() },
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: BroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            query_pipeline: QueryPipeline::new(),
            physics_hooks: (),
            event_handler: (),
            rigid_body_labels: DashMap::new(),
        }
    }

    /// Run an update of the simulation with the given delta time (in seconds)
    fn update(&mut self, delta_time: f64) {
        // Update dt
        self.integration_parameters.dt = delta_time as f32;
        
        // Run physics
        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            None,
            &self.physics_hooks,
            &self.event_handler,
          );    
          self.query_pipeline.update(&self.rigid_body_set, &self.collider_set);
    }
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
        };

        info!("Room {} created", obj.name);

        /* Create the ground. */
        let rigid_body = RigidBodyBuilder::fixed().translation(vector![0.0, -0.1, 0.0]);
        let floor_handle = obj.sim.rigid_body_set.insert(rigid_body);
        let collider = ColliderBuilder::cuboid(100.0, 0.1, 100.0);
        obj.sim.collider_set.insert_with_parent(collider, floor_handle, &mut obj.sim.rigid_body_set);
        obj.sim.rigid_body_labels.insert("ground".into(), floor_handle);
        
        let rigid_body = RigidBodyBuilder::dynamic()
            .ccd_enabled(true)
            .translation(vector![1.2, 2.5, 0.0])
            .rotation(vector![3.14159 / 3.0, 3.14159 / 3.0, 3.14159 / 3.0])
            .build();
        let collider = ColliderBuilder::cuboid(0.5, 0.5, 0.5).restitution(0.3).density(0.1).build();
        let cube_body_handle = obj.sim.rigid_body_set.insert(rigid_body);
        obj.sim.collider_set.insert_with_parent(collider, cube_body_handle, &mut obj.sim.rigid_body_set);
        obj.sim.rigid_body_labels.insert("cube".into(), cube_body_handle);
        obj.objects.insert("cube".into(), ObjectData {
            name: "cube".into(),
            transform: Transform { ..Default::default() },
            visual_info: Some(VisualInfo::Color(1.0, 1.0, 1.0)),
            is_kinematic: false,
            updated: true,
        });

        // Setup test room

        // Create robot
        let mut robot = RobotData::create_robot_body(&mut obj.sim);
        obj.sim.rigid_body_labels.insert("robot".into(), robot.body_handle);
        obj.objects.insert("robot".into(), ObjectData {
            name: "robot".into(),
            transform: Transform {scaling: vector![3.0,3.0,3.0], ..Default::default() },
            visual_info: Some(VisualInfo::Mesh("parallax_robot.glb".into())),
            is_kinematic: false,
            updated: true,
        });
        RobotData::setup_robot_socket(&mut robot);
        
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

        obj.robots.insert("robot".to_string(), robot);

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
        let time = Utc::now().timestamp();

        for mut robot in self.robots.iter_mut() {
            RobotData::robot_update(robot.value_mut(), &mut self.sim, &self.sockets, delta_time).await;
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

    pub fn reset(&mut self){
        // Reset robots
        for mut r in self.robots.iter_mut() {
            r.reset(&mut self.sim);
        }

        // TODO: reset others
    }
}
