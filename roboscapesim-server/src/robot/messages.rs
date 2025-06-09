use std::sync::Arc;
use std::time::SystemTime;

use dashmap::{DashMap, DashSet};
use log::{error, trace};
use roboscapesim_common::UpdateMessage;
use rapier3d::prelude::*;

use crate::robot::motor::{DriveState, SET_DISTANCE_DRIVE_SPEED};
use crate::robot::RobotData;
use crate::room::clients::ClientsManager;
use crate::simulation::Simulation;
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessageType {
    Drive,           // b'D'
    SetSpeed,        // b'S'
    Beep,           // b'B'
    SetLED,         // b'L'
    GetRange,       // b'R'
    GetTicks,       // b'T'
    SetNumeric,     // b'n'
    ButtonPress,    // b'P'
    Initialize,     // b'I'
}

impl MessageType {
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            b'D' => Some(Self::Drive),
            b'S' => Some(Self::SetSpeed),
            b'B' => Some(Self::Beep),
            b'L' => Some(Self::SetLED),
            b'R' => Some(Self::GetRange),
            b'T' => Some(Self::GetTicks),
            b'n' => Some(Self::SetNumeric),
            b'P' => Some(Self::ButtonPress),
            b'I' => Some(Self::Initialize),
            _ => None,
        }
    }

    pub fn to_byte(self) -> u8 {
        match self {
            Self::Drive => b'D',
            Self::SetSpeed => b'S',
            Self::Beep => b'B',
            Self::SetLED => b'L',
            Self::GetRange => b'R',
            Self::GetTicks => b'T',
            Self::SetNumeric => b'n',
            Self::ButtonPress => b'P',
            Self::Initialize => b'I',
        }
    }

    pub fn requires_timing_check(self) -> bool {
        match self {
            Self::SetNumeric | Self::SetLED | Self::Initialize | Self::GetRange | Self::GetTicks => false,
            _ => true,
        }
    }
}

pub fn process_roboscape_message(robot: &mut RobotData, buf: [u8; 512], had_messages: &mut bool, clients: &DashMap<String, DashSet<u128>>, sim: &Arc<Simulation>, msg: &mut Option<UpdateMessage>, size: usize) {
    let msg_type = MessageType::from_byte(buf[0]);

    if msg_type.is_none() {
        trace!("Unknown message type: {}", buf[0]);
        return;
    }

    let msg_type = msg_type.unwrap();

    if msg_type.requires_timing_check() {
        if robot.min_message_spacing > 0 && robot.last_message_time.elapsed().unwrap().as_millis() < robot.min_message_spacing {
            // Reject message if too soon after last message
            trace!("Rejecting message due to timing");
            return;
        }
    }

    match msg_type {
        MessageType::Drive => process_drive_message(robot, buf, had_messages),
        MessageType::SetSpeed => process_set_speed_message(robot, buf, had_messages),
        MessageType::Beep => process_beep_message(robot, buf, had_messages, clients),
        MessageType::SetLED => {
            trace!("OnSetLED");
            *had_messages = true;
        },
        MessageType::GetRange => process_get_range_message(robot, had_messages, sim),
        MessageType::GetTicks => process_get_ticks_message(robot, had_messages),
        MessageType::SetNumeric => {
            trace!("OnSetNumeric");
            // TODO: Decide on supporting this better, for now show encrypt numbers
            *had_messages = true;
            *msg = Some(UpdateMessage::DisplayText(robot.id.clone(), buf[1].to_string(), Some(1.0)));
        },
        MessageType::ButtonPress => {
            trace!("OnButtonPress");
        },
        _ => {
            trace!("Unhandled message type: {:?}", msg_type);
        },
    }

    robot.last_message_time = SystemTime::now();
    
    // Return to sender
    send_roboscape_message(robot, &buf[0..size]).unwrap();
}

fn process_get_ticks_message(robot: &mut RobotData, had_messages: &mut bool) {
    trace!("OnGetTicks");
    *had_messages = true;
    let left_ticks = (robot.motor_data.ticks[0] as i32).to_le_bytes();
    let right_ticks = (robot.motor_data.ticks[1] as i32).to_le_bytes();
    let mut message: [u8; 9] = [0; 9];

    // Create message
    message[0] = b'T';
    message[1..5].copy_from_slice(&right_ticks);
    message[5..9].copy_from_slice(&left_ticks);

    if let Err(e) = send_roboscape_message(robot, &message) {
        error!("{}", e);
    }
}

fn process_get_range_message(robot: &mut RobotData, had_messages: &mut bool, sim: &Arc<Simulation>) {
    trace!("OnGetRange");
    *had_messages = true;

    // Setup raycast
    let rigid_body_set = &sim.rigid_body_set.read().unwrap();
    let body = rigid_body_set.get(robot.physics.body_handle).unwrap();
    let body_pos = body.translation();
    let offset = body.rotation() * vector![0.17, 0.05, 0.0];
    let start_point = point![body_pos.x + offset.x, body_pos.y + offset.y, body_pos.z + offset.z];
    let ray = Ray::new(start_point, body.rotation() * vector![1.0, 0.0, 0.0]);
    let max_toi = 3.0;
    let solid = true;
    let filter = QueryFilter::default().exclude_sensors().exclude_rigid_body(robot.physics.body_handle);

    let mut distance = (max_toi * 100.0) as u16;
    if let Some((handle, toi)) = sim.query_pipeline.lock().unwrap().cast_ray(rigid_body_set,
        &sim.collider_set.read().unwrap(), &ray, max_toi, solid, filter
    ) {
        // The first collider hit has the handle `handle` and it hit after
        // the ray travelled a distance equal to `ray.dir * toi`.
        let hit_point = ray.point_at(toi); // Same as: `ray.origin + ray.dir * toi`
        distance = (toi * 100.0) as u16;
        trace!("Collider {:?} hit at point {}", handle, hit_point);
    }

    // Send result message
    let dist_bytes = u16::to_le_bytes(distance);
    if let Err(e) = send_roboscape_message(robot, &[b'R', dist_bytes[0], dist_bytes[1]]) {
        error!("{}", e);
    }
}

fn process_beep_message(robot: &mut RobotData, buf: [u8; 512], had_messages: &mut bool, clients: &DashMap<String, DashSet<u128>>) {
    trace!("OnBeep");
    *had_messages = true;
        
    if buf.len() > 4 {
        let freq = u16::from_le_bytes([buf[1], buf[2]]);
        let duration = u16::from_le_bytes([buf[3], buf[4]]);

        // Beep is only on client-side
        ClientsManager::send_to_clients(&UpdateMessage::Beep(robot.id.clone(), freq, duration), clients.iter().map(|c| c.value().clone().into_iter()).flatten());
    }
}

fn process_set_speed_message(robot: &mut RobotData, buf: [u8; 512], had_messages: &mut bool) {
    trace!("OnSetSpeed");
    robot.motor_data.drive_state = DriveState::SetSpeed;
    *had_messages = true;

    if buf.len() > 4 {
        let s1 = i16::from_le_bytes([buf[1], buf[2]]);
        let s2 = i16::from_le_bytes([buf[3], buf[4]]);

        robot.motor_data.speed_l = -s2 as f32 * robot.motor_data.speed_scale / 32.0;
        robot.motor_data.speed_r = -s1 as f32 * robot.motor_data.speed_scale / 32.0;
    }
}    

fn process_drive_message(robot: &mut RobotData, buf: [u8; 512], had_messages: &mut bool) {
    trace!("OnDrive");
    *had_messages = true;
    
    if buf.len() > 4 {
        robot.motor_data.drive_state = DriveState::SetDistance;
    
        let d1 = i16::from_le_bytes([buf[1], buf[2]]);
        let d2 = i16::from_le_bytes([buf[3], buf[4]]);
    
        robot.motor_data.distance_l = d2 as f64;
        robot.motor_data.distance_r = d1 as f64;

        trace!("OnDrive {} {}", d1, d2);
    
        // Check prevents robots from inching forwards from "drive 0 0"
        if f64::abs(robot.motor_data.distance_l) > f64::EPSILON {
            robot.motor_data.speed_l = f64::signum(robot.motor_data.distance_l) as f32 * SET_DISTANCE_DRIVE_SPEED * robot.motor_data.speed_scale;
        }

        if f64::abs(robot.motor_data.distance_r) > f64::EPSILON {
            robot.motor_data.speed_r = f64::signum(robot.motor_data.distance_r) as f32 * SET_DISTANCE_DRIVE_SPEED * robot.motor_data.speed_scale;
        }
    }
}

/// Send a RoboScape message to NetsBlox server
pub fn send_roboscape_message(robot: &mut RobotData, message: &[u8]) -> Result<usize, std::io::Error> {
    if robot.socket.is_none() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "Socket not connected"));
    }

    let mut buf = Vec::<u8>::new();

    // MAC address
    let mut mac = Vec::from(robot.mac);
    buf.append(&mut mac);

    // Timestamp
    let time = SystemTime::now().duration_since(robot.start_time).unwrap().as_secs() as u32;
    buf.append(&mut Vec::from(time.to_be_bytes()));

    // Message
    buf.append(&mut Vec::from(message));

    robot.socket.as_mut().unwrap().send(buf.as_slice())
}