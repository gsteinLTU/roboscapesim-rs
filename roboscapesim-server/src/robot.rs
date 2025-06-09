use std::net::UdpSocket;
use std::sync::Arc;
use std::time::{SystemTime, Duration};

use dashmap::{DashMap, DashSet};
use derivative::Derivative;
use log::{error, trace};
use roboscapesim_common::{UpdateMessage, Transform};
use rapier3d::prelude::*;

use crate::robot::messages::send_roboscape_message;
use crate::robot::physics::RobotPhysics;
use crate::simulation::Simulation;
use crate::util::traits::resettable::Resettable;
use crate::util::util::get_timestamp;

pub mod messages;
pub mod physics;

/// Data for robot motors, used for controlling speed and distance
#[derive(Default, Debug)]
pub struct RobotMotorData {
    /// Speed of left wheel
    pub speed_l: f32,
    /// Speed of right wheel
    pub speed_r: f32,
    /// Ticks for left wheel
    pub ticks: [f64; 2],
    /// Current drive state
    pub drive_state: DriveState,
    /// Distance to travel for left wheel
    pub distance_l: f64,
    /// Distance to travel for right wheel
    pub distance_r: f64,
}

/// Represents a robot in the simulation
#[derive(Derivative)]
#[derivative(Debug)]
pub struct RobotData {
    /// Main body of robot
    pub physics: RobotPhysics,
    /// Socket to NetsBlox server, or None if not connected
    pub socket: Option<UdpSocket>,
    /// Last time a heartbeat was sent
    pub last_heartbeat: i64,
    /// String representation of MAC address
    pub id: String,
    /// MAC address as bytes
    pub mac: [u8; 6],
    pub whisker_l: ColliderHandle,
    pub whisker_r: ColliderHandle,
    pub whisker_states: [bool; 2],
    pub motor_data: RobotMotorData,
    pub initial_transform: Transform,
    /// Username of user who claimed this robot, or None if unclaimed
    pub claimed_by: Option<String>,
    /// Whether this robot can be claimed, non-claimable robots are intended for scenario controlled robots
    pub claimable: bool,
    pub start_time: SystemTime,
    pub speed_scale: f32,
    pub last_message_time: SystemTime,
    pub min_message_spacing: u128,
}

/// Possible drive modes
#[derive(Debug, PartialEq, Eq)]
pub enum DriveState {
    /// Run wheels at requested speed
    SetSpeed,
    /// Drive until distance reached
    SetDistance
}

impl Default for DriveState {
    fn default() -> Self {
        DriveState::SetSpeed
    }
}

/// Speed used when using SetDistance
const SET_DISTANCE_DRIVE_SPEED: f32 = 75.0 / -32.0;

impl RobotData {
    pub fn setup_robot_socket(robot: &mut RobotData) {
        let server = std::env::var("ROBOSCAPE_SERVER").unwrap_or("52.73.65.98".to_string());
        let port = std::env::var("ROBOSCAPE_PORT").unwrap_or("1973".to_string());
        let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
        socket.set_nonblocking(true).unwrap();

        socket.connect(server.to_owned() + ":" + &port).expect("Failed to connect");

        socket.set_read_timeout(Some(Duration::from_micros(1))).expect("Failed to set timeout");
        socket.set_write_timeout(Some(Duration::from_micros(1))).expect("Failed to set timeout");

        robot.last_heartbeat = get_timestamp();
        robot.socket = Some(socket);
        
        // Send initial message
        if let Err(e) = send_roboscape_message(robot, b"I") {
            error!("{}", e);
        }
    }

    pub fn robot_update(robot: &mut RobotData, sim: Arc<Simulation>, clients: &DashMap<String, DashSet<u128>>, dt: f64) -> (bool, Option<UpdateMessage>) {
        if robot.socket.is_none() {
            return (false, None);
        }

        let mut had_messages = false;

        if get_timestamp() - robot.last_heartbeat > 50 {
            if let Err(e) = send_roboscape_message(robot, b"I") { 
                error!("{}", e);
            }
        }

        if robot.motor_data.drive_state == DriveState::SetDistance {

            // Stop robot if distance reached
            if f64::abs(robot.motor_data.distance_l) < f64::abs(robot.motor_data.speed_l as f64 * -32.0 * dt) {
                trace!("Distance reached L");
                robot.motor_data.speed_l = 0.0;
            } else {
                robot.motor_data.distance_l -= (robot.motor_data.speed_l * -32.0) as f64 * dt;
            }

            if f64::abs(robot.motor_data.distance_r) < f64::abs(robot.motor_data.speed_r as f64 * -32.0 * dt) {
                trace!("Distance reached R");
                robot.motor_data.speed_r = 0.0;
            } else {
                robot.motor_data.distance_r -= (robot.motor_data.speed_r * -32.0) as f64 * dt;
            }

            if robot.motor_data.speed_l == 0.0 && robot.motor_data.speed_r == 0.0 {
                robot.motor_data.drive_state = DriveState::SetSpeed;
            }
        }

        // Update ticks
        robot.motor_data.ticks[0] += (robot.motor_data.speed_l * robot.speed_scale * -32.0) as f64 * dt;
        robot.motor_data.ticks[1] += (robot.motor_data.speed_r * robot.speed_scale * -32.0) as f64 * dt;

        let mut msg = None;
        
        let mut buf = [0u8; 512];
        let size = robot.socket.as_mut().unwrap().recv(&mut buf);

        if let Ok(size) = size {
            if size > 0 {
                messages::process_roboscape_message(robot, buf, &mut had_messages, clients, &sim, &mut msg, size);
            }
        }

        // Apply calculated speeds to wheels
        {
            let jointset = &mut sim.multibody_joint_set.write().unwrap();
            let joint1 = jointset.get_mut(robot.physics.wheel_joints[0]).unwrap().0.link_mut(2).unwrap();
            joint1.joint.data.set_motor_velocity(JointAxis::AngZ, robot.motor_data.speed_l, 4.0);

            let joint2 = jointset.get_mut(robot.physics.wheel_joints[1]).unwrap().0.link_mut(1).unwrap();
            joint2.joint.data.set_motor_velocity(JointAxis::AngZ, robot.motor_data.speed_r, 4.0);
        }
        
        let mut new_whisker_states = [false, false];

        // Check whiskers
        for c in sim.narrow_phase.lock().unwrap().intersections_with(robot.whisker_l) {
            // Ignore non-intersections 
            if !c.2 {
                continue;
            } 

            if let Some(other) = sim.collider_set.read().unwrap().get(c.0) {
                if !other.is_sensor() && other.is_enabled() {
                    new_whisker_states[0] = true;
                }
            }
        }
        
        for c in sim.narrow_phase.lock().unwrap().intersections_with(robot.whisker_r) {
            // Ignore non-intersections 
            if !c.2 {
                continue;
            } 

            if let Some(other) = sim.collider_set.read().unwrap().get(c.0) {
                if !other.is_sensor() && other.is_enabled() {
                    new_whisker_states[1] = true;
                }
            }
        }
        

        // Send message if whisker changed
        if new_whisker_states != robot.whisker_states {
            robot.whisker_states = new_whisker_states;
            // Whiskers in message are inverted
            let message: [u8; 2] = [b'W', if robot.whisker_states[1] { 0 } else { 1 } + if robot.whisker_states[0] { 0 } else { 2 } ];

            trace!("Whisker states: {:?}", robot.whisker_states);

            if let Err(e) = send_roboscape_message(robot, &message) {
                error!("{}", e);
            }
        }

        (had_messages, msg)
    }
}

impl Resettable for RobotData {
    fn reset(&mut self, sim: Arc<Simulation>) {
        let rotation = self.initial_transform.rotation.clone();
        let position = self.initial_transform.position - point![0.0, 0.0, 0.0];

        RobotPhysics::update_transform(self, sim.clone(), Some(position), Some(rotation), true);

        // Reset state
        self.motor_data = RobotMotorData::default();
        self.start_time = SystemTime::now();

        self.last_heartbeat = get_timestamp();

        RobotPhysics::update_transform(self, sim.clone(), Some(position), Some(rotation), true);

        // Send initial message
        if let Err(e) = send_roboscape_message(self, b"I") {
            error!("{}", e);
        }
    }
}

impl PartialEq for RobotData {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}