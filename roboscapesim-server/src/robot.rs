use std::net::UdpSocket;
use std::time::{SystemTime, Duration};
use std::f32::consts::FRAC_PI_2;

use dashmap::{DashMap, DashSet};
use derivative::Derivative;
use log::{info, error, trace};
use nalgebra::{Point3,UnitQuaternion, Vector3};
use roboscapesim_common::{UpdateMessage, Transform, Orientation};
use rapier3d::prelude::*;

use crate::room::RoomData;
use crate::simulation::{Simulation, SCALE};
use crate::util::extra_rand::generate_random_mac_address;
use crate::util::traits::resettable::Resettable;
use crate::util::util::{bytes_to_hex_string, get_timestamp};

/// Represents a robot in the simulation
#[derive(Derivative)]
#[derivative(Debug)]
pub struct RobotData {
    /// Main body of robot
    pub body_handle: RigidBodyHandle,
    /// Joints connecting wheels to body
    pub wheel_joints: Vec<MultibodyJointHandle>,
    /// Physics bodies for wheels
    pub wheel_bodies: Vec<RigidBodyHandle>,
    /// Socket to NetsBlox server, or None if not connected
    pub socket: Option<UdpSocket>,
    /// Desired speed of left wheel
    pub speed_l: f32,
    /// Desired speed of right wheel
    pub speed_r: f32,
    /// Last time a heartbeat was sent
    pub last_heartbeat: i64,
    /// String representation of MAC address
    pub id: String,
    /// MAC address as bytes
    pub mac: [u8; 6],
    pub whisker_l: ColliderHandle,
    pub whisker_r: ColliderHandle,
    pub whisker_states: [bool; 2],
    /// Distance traveled by each wheel
    pub ticks: [f64; 2],
    pub drive_state: DriveState,
    /// Distance to travel for SetDistance
    pub distance_l: f64,
    /// Distance to travel for SetDistance
    pub distance_r: f64,
    pub initial_transform: Transform,
    /// Username of user who claimed this robot, or None if unclaimed
    pub claimed_by: Option<String>,
    /// Whether this robot can be claimed, non-claimable robots are intended for scenario controlled robots
    pub claimable: bool,
    pub start_time: SystemTime,
}

/// Possible drive modes
#[derive(Debug, PartialEq, Eq)]
pub enum DriveState {
    /// Run wheels at requested speed
    SetSpeed,
    /// Drive until distance reached
    SetDistance
}

/// Speed used when using SetDistance
const SET_DISTANCE_DRIVE_SPEED: f32 = 75.0 / -32.0;

impl RobotData {
    /// Send a RoboScape message to NetsBlox server
    pub fn send_roboscape_message(&mut self, message: &[u8]) -> Result<usize, std::io::Error> {
        if self.socket.is_none() {
            return Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "Socket not connected"));
        }

        let mut buf = Vec::<u8>::new();

        // MAC address
        let mut mac = Vec::from(self.mac);
        buf.append(&mut mac);

        // Timestamp
        let time = SystemTime::now().duration_since(self.start_time).unwrap().as_secs() as u32;
        buf.append(&mut Vec::from(time.to_be_bytes()));

        // Message
        buf.append(&mut Vec::from(message));

        self.socket.as_mut().unwrap().send(buf.as_slice())
    }

    /// Create physics body for robot, returns RobotData for the robot
    pub fn create_robot_body(sim: &mut Simulation, mac: Option<[u8; 6]>, position: Option<Vector3<Real>>, orientation: Option<UnitQuaternion<Real>>) -> RobotData {
        let mac = mac.unwrap_or_else(generate_random_mac_address);
        let id = bytes_to_hex_string(&mac).to_owned();
        info!("Creating robot {}", id);

        // Size of robot
        const HW: f32 = 0.07 * SCALE;
        const HH: f32 = 0.03 * SCALE;
        const HD: f32 = 0.03 * SCALE;

        let mut box_center: Point3<f32> = Point3::new(0.0, 1.0 + HH * 2.0, 0.0);
        let mut box_rotation = UnitQuaternion::from_euler_angles(0.0, 0.0, 0.0);

        let rigid_body = RigidBodyBuilder::dynamic()
            .translation(vector![box_center.x * SCALE, box_center.y * SCALE, box_center.z * SCALE])
            .angular_damping(5.0)
            .linear_damping(5.0)
            .ccd_enabled(true)
            .can_sleep(false);
        
        let vehicle_handle = sim.rigid_body_set.lock().unwrap().insert(rigid_body);
        
        let collider = ColliderBuilder::cuboid(HW, HH, HD).density(25.0);
        sim.collider_set.insert_with_parent(collider, vehicle_handle, &mut sim.rigid_body_set.lock().unwrap());

        //let mut vehicle = DynamicRayCastVehicleController::new(vehicle_handle);
        let wheel_half_width = 0.01;
        let wheel_positions = [
            point![HW * 0.5, -HH + 0.015 * SCALE, HD + wheel_half_width * SCALE],
            point![HW * 0.5, -HH + 0.015 * SCALE, -HD - wheel_half_width * SCALE],
        ];

        const BALL_WHEEL_RADIUS: f32 = 0.015 * SCALE;
        let ball_wheel_positions = [
            point![-HW * 0.75, -HH, 0.0]
        ];


        let mut wheel_bodies: Vec<RigidBodyHandle> = Vec::with_capacity(2);
        let mut wheel_joints: Vec<MultibodyJointHandle> = Vec::with_capacity(2);

        for pos in wheel_positions {
            //vehicle.add_wheel(pos, -Vector::y(), Vector::z(), hh, hh / 4.0, &tuning);
            
            let wheel_pos_in_world = Point3::new(box_center.x + pos.x, box_center.y + pos.y, box_center.z + pos.z);

            let wheel_rb = sim.rigid_body_set.lock().unwrap().insert(
                RigidBodyBuilder::dynamic()
                    .translation(vector![
                        wheel_pos_in_world.x,
                        wheel_pos_in_world.y,
                        wheel_pos_in_world.z
                    ]).rotation(vector![FRAC_PI_2, 0.0, 0.0]).ccd_enabled(true).can_sleep(false)
                    .angular_damping(500.0).linear_damping(50.0)
                    .enabled_rotations(false, false, true)
                    .enabled_translations(false, false, false)
            );

            let collider = ColliderBuilder::cylinder(wheel_half_width * SCALE, 0.03  * SCALE).friction(0.8).density(10.0);
            //let collider = ColliderBuilder::ball(0.03 * scale).friction(0.8).density(40.0);
            sim.collider_set.insert_with_parent(collider, wheel_rb, &mut sim.rigid_body_set.lock().unwrap());

            let joint = rapier3d::dynamics::GenericJointBuilder::new(JointAxesMask::X | JointAxesMask::Y | JointAxesMask::Z | JointAxesMask::ANG_X | JointAxesMask::ANG_Y )
                .local_anchor1(pos)
                .local_anchor2(point![0.0, 0.01 * SCALE * if pos.z > 0.0 { -1.0 } else { 1.0 }, 0.0])
                .local_frame2(Isometry::new(vector![0.0, 0.0, 0.0], vector![FRAC_PI_2, 0.0, 0.0]))
                .motor_max_force(JointAxis::AngZ, 750.0)
                .motor_model(JointAxis::AngZ, MotorModel::ForceBased)
                .motor_velocity(JointAxis::AngZ, 0.0, 4.0)
                .build();

            wheel_joints.push(sim.multibody_joint_set.insert(vehicle_handle, wheel_rb, joint, true).unwrap());
            wheel_bodies.push(wheel_rb);
        }


        for pos in ball_wheel_positions {        
            let wheel_pos_in_world = Point3::new(box_center.x + pos.x, box_center.y + pos.y, box_center.z + pos.z);

            let wheel_rb = sim.rigid_body_set.lock().unwrap().insert(
                RigidBodyBuilder::dynamic()
                    .translation(vector![
                        wheel_pos_in_world.x,
                        wheel_pos_in_world.y,
                        wheel_pos_in_world.z
                    ]).ccd_enabled(true)
                    .can_sleep(false).angular_damping(15.0).linear_damping(5.0)
                    .enabled_translations(false, false, false)
            );

            let collider = ColliderBuilder::ball(BALL_WHEEL_RADIUS).density(5.0).friction(0.25);
            sim.collider_set.insert_with_parent(collider, wheel_rb, &mut sim.rigid_body_set.lock().unwrap());

            let joint = rapier3d::dynamics::GenericJointBuilder::new(JointAxesMask::X | JointAxesMask::Y | JointAxesMask::Z )
                .local_anchor1(pos)
                .local_anchor2(point![0.0, 0.0, 0.0])
                .build();
            
            wheel_bodies.push(wheel_rb);

            sim.multibody_joint_set.insert(vehicle_handle, wheel_rb, joint, true);
        }

        // Create whiskers
        let whisker_l = ColliderBuilder::cuboid(HW * 0.4, 0.025, HD * 0.8).sensor(true).translation(vector![HW * 1.25, 0.05, HD * -0.4]);
        let whisker_l = sim.collider_set.insert_with_parent(whisker_l, vehicle_handle, &mut sim.rigid_body_set.lock().unwrap());
        let whisker_r = ColliderBuilder::cuboid(HW * 0.4, 0.025, HD * 0.8).sensor(true).translation(vector![HW * 1.25, 0.05, HD * 0.4]);
        let whisker_r = sim.collider_set.insert_with_parent(whisker_r, vehicle_handle, &mut sim.rigid_body_set.lock().unwrap());

        // Apply position and orientation
        if let Some(p) = position {
            sim.rigid_body_set.lock().unwrap().get_mut(vehicle_handle).unwrap().set_translation(p, true);
            box_center = p.into();
        }

        if let Some(o) = orientation {
            sim.rigid_body_set.lock().unwrap().get_mut(vehicle_handle).unwrap().set_rotation(o, true);
            box_rotation = o;
        }

        RobotData { 
            body_handle: vehicle_handle,
            wheel_joints,
            wheel_bodies,
            socket: None,
            speed_l: 0.0,
            speed_r: 0.0,
            last_heartbeat: 0,
            mac,
            id,
            whisker_l,
            whisker_r,
            whisker_states: [false, false],
            ticks: [0.0, 0.0],
            drive_state: DriveState::SetSpeed,
            distance_l: 0.0,
            distance_r: 0.0,
            initial_transform: Transform { position: box_center.to_owned(), rotation: roboscapesim_common::Orientation::Quaternion(box_rotation.quaternion().to_owned()), ..Default::default() },
            claimed_by: None,
            claimable: true,
            start_time: SystemTime::now(),
        }
    }

    pub fn setup_robot_socket(robot: &mut RobotData) {
        let server = std::env::var("ROBOSCAPE_SERVER").unwrap_or("52.73.65.98".to_string());
        let port = std::env::var("ROBOSCAPE_PORT").unwrap_or("1973".to_string());
        let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
        socket.set_nonblocking(true).unwrap();

        socket.connect(server.to_owned() + ":" + &port).expect("Failed to connect");

        socket.set_read_timeout(Some(Duration::from_millis(1))).expect("Failed to set timeout");
        socket.set_write_timeout(Some(Duration::from_millis(1))).expect("Failed to set timeout");

        robot.last_heartbeat = get_timestamp();
        robot.socket = Some(socket);
        
        // Send initial message
        if let Err(e) = robot.send_roboscape_message(b"I") {
            error!("{}", e);
        }
    }

    pub fn robot_update(robot: &mut RobotData, sim: &mut Simulation, clients: &DashMap<String, DashSet<u128>>, dt: f64) -> (bool, Option<UpdateMessage>) {
        if robot.socket.is_none() {
            return (false, None);
        }

        let mut had_messages = false;

        if get_timestamp() - robot.last_heartbeat > 50 {
            if let Err(e) = robot.send_roboscape_message(b"I") {
                error!("{}", e);
            }
        }

        if robot.drive_state == DriveState::SetDistance {

            // Stop robot if distance reached
            if f64::abs(robot.distance_l) < f64::abs(robot.speed_l as f64 * -32.0 * dt) {
                trace!("Distance reached L");
                robot.speed_l = 0.0;
            } else {
                robot.distance_l -= (robot.speed_l * -32.0) as f64 * dt;
            }

            if f64::abs(robot.distance_r) < f64::abs(robot.speed_r as f64 * -32.0 * dt) {
                trace!("Distance reached R");
                robot.speed_r = 0.0;
            } else {
                robot.distance_r -= (robot.speed_r * -32.0) as f64 * dt;
            }

            if robot.speed_l == 0.0 && robot.speed_r == 0.0 {
                robot.drive_state = DriveState::SetSpeed;
            }
        }

        // Update ticks
        robot.ticks[0] += (robot.speed_l * -32.0) as f64 * dt;
        robot.ticks[1] += (robot.speed_r * -32.0) as f64 * dt;

        let mut msg = None;
        
        let mut buf = [0u8; 512];
        let size = robot.socket.as_mut().unwrap().recv(&mut buf);

        if let Ok(size) = size {
            if size > 0 {
                had_messages = true;
                match &buf[0] {
                    b'D' => { 
                        trace!("OnDrive");

                        if buf.len() > 4 {
                            robot.drive_state = DriveState::SetDistance;

                            let d1 = i16::from_le_bytes([buf[1], buf[2]]);
                            let d2 = i16::from_le_bytes([buf[3], buf[4]]);

                            robot.distance_l = d2 as f64;
                            robot.distance_r = d1 as f64;

                            trace!("OnDrive {} {}", d1, d2);

                            // Check prevents robots from inching forwards from "drive 0 0"
                            if f64::abs(robot.distance_l) > f64::EPSILON {
                                robot.speed_l = f64::signum(robot.distance_l) as f32 * SET_DISTANCE_DRIVE_SPEED;
                            }

                            if f64::abs(robot.distance_r) > f64::EPSILON {
                                robot.speed_r = f64::signum(robot.distance_r) as f32 * SET_DISTANCE_DRIVE_SPEED;
                            }                    
                        }
                    },
                    b'S' => { 
                        trace!("OnSetSpeed");
                        robot.drive_state = DriveState::SetSpeed;

                        if buf.len() > 4 {
                            let s1 = i16::from_le_bytes([buf[1], buf[2]]);
                            let s2 = i16::from_le_bytes([buf[3], buf[4]]);

                            robot.speed_l = -s2 as f32 / 32.0;
                            robot.speed_r = -s1 as f32 / 32.0;
                        }
                    },
                    b'B' => { 
                        trace!("OnBeep");
                        
                        if buf.len() > 4 {
                            let freq = u16::from_le_bytes([buf[1], buf[2]]);
                            let duration = u16::from_le_bytes([buf[3], buf[4]]);

                            // Beep is only on client-side
                            RoomData::send_to_clients(&UpdateMessage::Beep(robot.id.clone(), freq, duration), clients.iter().map(|c| c.value().clone().into_iter()).flatten());
                        }
                    },
                    b'L' => { 
                        trace!("OnSetLED");
                    },
                    b'R' => { 
                        trace!("OnGetRange");

                        // Setup raycast
                        let rigid_body_set = &sim.rigid_body_set.lock().unwrap();
                        let body = rigid_body_set.get(robot.body_handle).unwrap();
                        let body_pos = body.translation();
                        let offset = body.rotation() * vector![0.17, 0.05, 0.0];
                        let start_point = point![body_pos.x + offset.x, body_pos.y + offset.y, body_pos.z + offset.z];
                        let ray = Ray::new(start_point, body.rotation() * vector![1.0, 0.0, 0.0]);
                        let max_toi = 3.0;
                        let solid = true;
                        let filter = QueryFilter::default().exclude_sensors().exclude_rigid_body(robot.body_handle);

                        let mut distance = (max_toi * 100.0) as u16;
                        if let Some((handle, toi)) = sim.query_pipeline.cast_ray(rigid_body_set,
                            &sim.collider_set, &ray, max_toi, solid, filter
                        ) {
                            // The first collider hit has the handle `handle` and it hit after
                            // the ray travelled a distance equal to `ray.dir * toi`.
                            let hit_point = ray.point_at(toi); // Same as: `ray.origin + ray.dir * toi`
                            distance = (toi * 100.0) as u16;
                            trace!("Collider {:?} hit at point {}", handle, hit_point);
                        }

                        // Send result message
                        let dist_bytes = u16::to_le_bytes(distance);
                        if let Err(e) = robot.send_roboscape_message(&[b'R', dist_bytes[0], dist_bytes[1]] ) {
                            error!("{}", e);
                        }
                    },
                    b'T' => { 
                        trace!("OnGetTicks");
                        let left_ticks = (robot.ticks[0] as i32).to_le_bytes();
                        let right_ticks = (robot.ticks[1] as i32).to_le_bytes();
                        let mut message: [u8; 9] = [0; 9];

                        // Create message
                        message[0] = b'T';
                        message[1..5].copy_from_slice(&right_ticks);
                        message[5..9].copy_from_slice(&left_ticks);

                        if let Err(e) = robot.send_roboscape_message(&message) {
                            error!("{}", e);
                        }
                    },
                    b'n' => { 
                        trace!("OnSetNumeric");
                        // TODO: Decide on supporting this better, for now show encrypt numbers
                        msg = Some(UpdateMessage::DisplayText(robot.id.clone(), buf[1].to_string(), Some(1.0)));
                    },
                    b'P' => {
                        trace!("OnButtonPress");         
                    },
                    _ => {}
                }
                // Return to sender
                robot.send_roboscape_message(&buf[0..size]).unwrap();
            }
        }

        // Apply calculated speeds to wheels
        let joint1 = sim.multibody_joint_set.get_mut(robot.wheel_joints[0]).unwrap().0.link_mut(2).unwrap();
        joint1.joint.data.set_motor_velocity(JointAxis::AngZ, robot.speed_l, 4.0);

        let joint2 = sim.multibody_joint_set.get_mut(robot.wheel_joints[1]).unwrap().0.link_mut(1).unwrap();
        joint2.joint.data.set_motor_velocity(JointAxis::AngZ, robot.speed_r, 4.0);
        
        let mut new_whisker_states = [false, false];

        // Check whiskers
        if sim.narrow_phase.intersections_with(robot.whisker_l).count() > 0 {
            for c in sim.narrow_phase.intersections_with(robot.whisker_l) {
                // Ignore non-intersections 
                if !c.2 {
                    continue;
                } 

                new_whisker_states[0] = true;
            }
        }

        if sim.narrow_phase.intersections_with(robot.whisker_r).count() > 0 {
            for c in sim.narrow_phase.intersections_with(robot.whisker_r) {
                // Ignore non-intersections 
                if !c.2 {
                    continue;
                } 

                new_whisker_states[1] = true;
            }
        }

        // Send message if whisker changed
        if new_whisker_states != robot.whisker_states {
            robot.whisker_states = new_whisker_states;
            // Whiskers in message are inverted
            let message: [u8; 2] = [b'W', if robot.whisker_states[1] { 0 } else { 1 } + if robot.whisker_states[0] { 0 } else { 2 } ];

            trace!("Whisker states: {:?}", robot.whisker_states);
            
            if let Err(e) = robot.send_roboscape_message(&message) {
                error!("{}", e);
            }
        }

        (had_messages, msg)
    }
}

impl Resettable for RobotData {
    fn reset(&mut self, sim: &mut Simulation) {
        // Reset position
        let rigid_body_set = &mut sim.rigid_body_set.lock().unwrap();
        for wheel in &self.wheel_bodies {
            let body = rigid_body_set.get_mut(*wheel).unwrap();
            body.set_linvel(vector![0.0, 0.0, 0.0], false);
            body.set_angvel(vector![0.0, 0.0, 0.0], false);
        }

        // Reset position
        let body = rigid_body_set.get_mut(self.body_handle).unwrap();
        body.set_translation(self.initial_transform.position - point![0.0, 0.0, 0.0], false);

        // Reset velocity
        body.set_linvel(vector![0.0, 0.0, 0.0], false);
        body.set_angvel(vector![0.0, 0.0, 0.0], false);

        // Reset rotation
        match self.initial_transform.rotation {
            Orientation::Quaternion(q) => {
                body.set_rotation(UnitQuaternion::new_unchecked(q), true);
            }
            Orientation::Euler(e) => {
                body.set_rotation(UnitQuaternion::from_euler_angles(e.x, e.y, e.z), true);
            }
        }

        // Reset state
        self.drive_state = DriveState::SetSpeed;
        self.speed_l = 0.0;
        self.speed_r = 0.0;
        self.whisker_states = [false, false];
        self.ticks = [0.0, 0.0];
        self.start_time = SystemTime::now();

        self.last_heartbeat = get_timestamp();
        
        // Send initial message
        if let Err(e) = self.send_roboscape_message(b"I") {
            error!("{}", e);
        }
    }
}

impl PartialEq for RobotData {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}