use std::{num::NonZeroUsize, sync::Arc};

#[cfg(feature = "no_deadlocks")]
use no_deadlocks::{Mutex, RwLock};
#[cfg(not(feature = "no_deadlocks"))]
use std::sync::{Mutex, RwLock};

use dashmap::{DashMap, DashSet};
use nalgebra::Vector3;
use rapier3d::prelude::*;

use crate::robot::RobotData;

/// Holds rapier-related structs together
pub struct Simulation {
    pub rigid_body_set: Arc<RwLock<RigidBodySet>>,
    pub collider_set: Arc<RwLock<ColliderSet>>,
    pub gravity: Vector3<f32>,
    pub integration_parameters: Arc<RwLock<IntegrationParameters>>,
    pub physics_pipeline: Arc<Mutex<PhysicsPipeline>>,
    pub island_manager: Arc<Mutex<IslandManager>>,
    pub broad_phase: Arc<RwLock<DefaultBroadPhase>>,
    pub narrow_phase: Arc<Mutex<NarrowPhase>>,
    pub impulse_joint_set: Arc<RwLock<ImpulseJointSet>>,
    pub multibody_joint_set: Arc<RwLock<MultibodyJointSet>>,
    pub ccd_solver: Arc<Mutex<CCDSolver>>,
    pub query_pipeline: Arc<Mutex<QueryPipeline>>,
    pub physics_hooks: (),
    pub event_handler: (),
    pub rigid_body_labels: DashMap<String, RigidBodyHandle>,
    pub sensors: DashMap<(String, ColliderHandle), DashSet<String>>,
}

pub const SCALE: f32 = 3.0;


impl Simulation {
    /// Instantiate the simulation objects with default settings
    pub fn new() -> Simulation {
        Simulation {
            rigid_body_set: Arc::new(RwLock::new(RigidBodySet::new())),
            collider_set: Arc::new(RwLock::new(ColliderSet::new())),
            gravity: vector![0.0, -9.81 * 3.0, 0.0],
            integration_parameters: Arc::new(RwLock::new(IntegrationParameters { 
                num_additional_friction_iterations: 4, 
                num_solver_iterations: NonZeroUsize::new(2).unwrap(), 
                num_internal_pgs_iterations: 10, 
                warmstart_coefficient: 1.0,
                ..Default::default() })),
            physics_pipeline: Arc::new(Mutex::new(PhysicsPipeline::new())),
            island_manager: Arc::new(Mutex::new(IslandManager::new())),
            broad_phase: Arc::new(RwLock::new(DefaultBroadPhase::new())),
            narrow_phase: Arc::new(Mutex::new(NarrowPhase::new())),
            impulse_joint_set: Arc::new(RwLock::new(ImpulseJointSet::new())),
            multibody_joint_set: Arc::new(RwLock::new(MultibodyJointSet::new())),
            ccd_solver: Arc::new(Mutex::new(CCDSolver::new())),
            query_pipeline: Arc::new(Mutex::new(QueryPipeline::new())),
            physics_hooks: (),
            event_handler: (),
            rigid_body_labels: DashMap::new(),
            sensors: DashMap::new(),
        }
    }

    /// Run an update of the simulation with the given delta time (in seconds)
    pub fn update(&self, delta_time: f64) {
        // Update dt
        self.integration_parameters.write().unwrap().dt = delta_time as f32;

        let broad_phase = &mut *self.broad_phase.write().unwrap();

        // Run physics
        self.physics_pipeline.lock().unwrap().step(
            &self.gravity,
            &self.integration_parameters.read().unwrap(),
            &mut self.island_manager.lock().unwrap(),
            broad_phase,
            &mut self.narrow_phase.lock().unwrap(),
            &mut self.rigid_body_set.write().unwrap(),
            &mut self.collider_set.write().unwrap(),
            &mut self.impulse_joint_set.write().unwrap(),
            &mut self.multibody_joint_set.write().unwrap(),
            &mut self.ccd_solver.lock().unwrap(),
            None,
            &self.physics_hooks,
            &self.event_handler,
          );    

          self.query_pipeline.lock().unwrap().update(&self.rigid_body_set.read().unwrap(), &self.collider_set.write().unwrap());
    }

    /// Remove all parts of a robot from the simulation
    pub fn cleanup_robot(&self, r: &RobotData) {
        // Clean up robot parts
        self.multibody_joint_set.write().unwrap().remove_multibody_articulations(r.body_handle, false);

        for handle in &r.wheel_bodies {
            self.multibody_joint_set.write().unwrap().remove_multibody_articulations(*handle, false);
            self.rigid_body_set.write().unwrap().remove(*handle, &mut self.island_manager.lock().unwrap(), &mut self.collider_set.write().unwrap(), &mut self.impulse_joint_set.write().unwrap(), &mut self.multibody_joint_set.write().unwrap(), true);
        }
    
        self.rigid_body_set.write().unwrap().remove(r.body_handle, &mut self.island_manager.lock().unwrap(), &mut self.collider_set.write().unwrap(), &mut self.impulse_joint_set.write().unwrap(), &mut self.multibody_joint_set.write().unwrap(), true);
    }

    pub fn remove_body(&self, handle: RigidBodyHandle) {
       self.rigid_body_set.write().unwrap().remove(handle, &mut self.island_manager.lock().unwrap(), &mut self.collider_set.write().unwrap(), &mut self.impulse_joint_set.write().unwrap(), &mut self.multibody_joint_set.write().unwrap(), true);
    }
}