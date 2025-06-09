use std::net::UdpSocket;
use std::sync::Arc;
use std::time::{SystemTime, Duration};

use dashmap::{DashMap, DashSet};
use derivative::Derivative;
use log::error;
use roboscapesim_common::{UpdateMessage, Transform};
use rapier3d::prelude::*;

use crate::robot::messages::send_roboscape_message;
use crate::robot::motor::RobotMotorData;
use crate::robot::physics::RobotPhysics;
use crate::simulation::Simulation;
use crate::util::traits::resettable::Resettable;
use crate::util::util::get_timestamp;

pub mod messages;
pub mod physics;
pub mod motor;

/// Represents a robot in the simulation
#[derive(Derivative)]
#[derivative(Debug)]
pub struct RobotData {
    /// Physics data for the robot
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
    pub last_message_time: SystemTime,
    pub min_message_spacing: u128,
}

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

        robot.motor_data.update_wheel_state(dt);

        let mut msg = None;
        
        let mut buf = [0u8; 512];
        let size = robot.socket.as_mut().unwrap().recv(&mut buf);

        if let Ok(size) = size {
            if size > 0 {
                messages::process_roboscape_message(robot, buf, &mut had_messages, clients, &sim, &mut msg, size);
            }
        }

        RobotPhysics::set_wheel_speeds(robot, &sim, robot.motor_data.speed_l, robot.motor_data.speed_r);
        RobotPhysics::check_whiskers(robot, sim);

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