use std::sync::{Arc, Mutex};

use dashmap::{DashMap, DashSet};
use nalgebra::Vector3;
use rapier3d::prelude::*;

use crate::robot::RobotData;

/// Holds rapier-related structs together
pub struct Simulation {
    pub rigid_body_set: Arc<Mutex<RigidBodySet>>,
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
    pub sensors: DashMap<(String, ColliderHandle), DashSet<ColliderHandle>>,
}

pub const SCALE: f32 = 3.0;


impl Simulation {
    /// Instantiate the simulation objects with default settings
    pub fn new() -> Simulation {
        Simulation {
            rigid_body_set: Arc::new(Mutex::new(RigidBodySet::new())),
            collider_set: ColliderSet::new(),
            gravity: vector![0.0, -9.81 * 3.0, 0.0],
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
            sensors: DashMap::new(),
        }
    }

    /// Run an update of the simulation with the given delta time (in seconds)
    pub fn update(&mut self, delta_time: f64) {
        // Update dt
        self.integration_parameters.dt = delta_time as f32;
        
        // Run physics
        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set.lock().unwrap(),
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            None,
            &self.physics_hooks,
            &self.event_handler,
          );    
          self.query_pipeline.update(&self.rigid_body_set.lock().unwrap(), &self.collider_set);
    }

    pub fn cleanup_robot(&mut self, r: &RobotData) {
        // Clean up robot parts
        self.multibody_joint_set.remove_multibody_articulations(r.body_handle, false);

        for handle in &r.wheel_bodies {
            self.multibody_joint_set.remove_multibody_articulations(*handle, false);
            self.rigid_body_set.lock().unwrap().remove(*handle, &mut self.island_manager, &mut self.collider_set, &mut self.impulse_joint_set, &mut self.multibody_joint_set, true);
        }
    
        self.rigid_body_set.lock().unwrap().remove(r.body_handle, &mut self.island_manager, &mut self.collider_set, &mut self.impulse_joint_set, &mut self.multibody_joint_set, true);
    }

    pub fn remove_body(&mut self, handle: RigidBodyHandle) {
       self.rigid_body_set.lock().unwrap().remove(handle, &mut self.island_manager, &mut self.collider_set, &mut self.impulse_joint_set, &mut self.multibody_joint_set, true);
    }
}