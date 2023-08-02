use std::net::UdpSocket;
use std::time::{SystemTime, UNIX_EPOCH, Duration};

use chrono::Utc;
use dashmap::DashMap;
use derivative::Derivative;
use log::{info, error};
use nalgebra::{Point3,UnitQuaternion};
use rapier3d::prelude::*;
use roboscapesim_common::{UpdateMessage, Transform, Orientation};

use crate::room::{Simulation, RoomData};
use crate::util::traits::resettable::Resettable;
#[derive(Derivative)]
#[derivative(Debug)]
pub struct RobotData {
    pub body_handle: RigidBodyHandle,
    pub wheel_joints: Vec<MultibodyJointHandle>,
    pub wheel_bodies: Vec<RigidBodyHandle>,
    pub socket: Option<UdpSocket>,
    pub speed_l: f32,
    pub speed_r: f32,
    pub last_heartbeat: i64,
    pub id: String,
    pub whisker_l: ColliderHandle,
    pub whisker_r: ColliderHandle,
    pub whisker_states: [bool; 2],
    pub ticks: [f64; 2],
    pub drive_state: DriveState,
    pub distance_l: f64,
    pub distance_r: f64,
    pub initial_transform: Transform,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DriveState {
    SetSpeed,
    SetDistance
}

const SET_DISTANCE_DRIVE_SPEED: f32 = 75.0 / -32.0;

impl RobotData {
    pub fn send_roboscape_message(&mut self, message: &[u8]) -> Result<usize, std::io::Error> {
        if self.socket.is_none() {
            return Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "Socket not connected"));
        }

        let mut buf = Vec::<u8>::new();

        // MAC address
        let mut mac: Vec<u8> = vec![1,2,3,4,5,6];
        buf.append(&mut mac);

        // Timestamp
        let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as u32;
        buf.append(&mut Vec::from(time.to_be_bytes()));

        // Message
        buf.append(&mut Vec::from(message));

        self.socket.as_mut().unwrap().send(&buf.as_slice())
    }

    pub fn create_robot_body(sim: &mut Simulation) -> RobotData {
            
        /*
        * Vehicle we will control manually.
        */
        let scale = 3.0;
        let hw = 0.07 * scale;
        let hh = 0.027 * scale;
        let hd = 0.03 * scale;

        let box_center: Point3<f32> = Point3::new(0.0, 1.0 + hh * 2.0, 0.0);
        let box_rotation = UnitQuaternion::from_euler_angles(0.0, 0.0, 0.0);
        // TODO: use rotation

        let rigid_body = RigidBodyBuilder::dynamic()
            .translation(vector![box_center.x * scale, box_center.y * scale, box_center.z * scale])
            .linear_damping(2.0)
            .angular_damping(2.0)
            .ccd_enabled(true)
            .can_sleep(false);
        
        let vehicle_handle = sim.rigid_body_set.insert(rigid_body);
        
        let collider = ColliderBuilder::cuboid(hw, hh, hd).density(25.0);
        sim.collider_set.insert_with_parent(collider, vehicle_handle, &mut sim.rigid_body_set);

        //let mut vehicle = DynamicRayCastVehicleController::new(vehicle_handle);
        let wheel_positions = [
            point![hw * 0.5, -hh + 0.015 * scale, hd + 0.01  * scale],
            point![hw * 0.5, -hh + 0.015 * scale, -hd - 0.01  * scale],
        ];

        let ball_wheel_radius = 0.015 * scale;
        let ball_wheel_positions = [
            point![-hw * 0.75, -hh, 0.0]
        ];


        let mut wheel_bodies: Vec<RigidBodyHandle> = vec![];
        let mut wheel_joints: Vec<MultibodyJointHandle> = vec![];

        for pos in wheel_positions {
            //vehicle.add_wheel(pos, -Vector::y(), Vector::z(), hh, hh / 4.0, &tuning);
            
            let wheel_pos_in_world = Point3::new(box_center.x + pos.x, box_center.y + pos.y, box_center.z + pos.z);

            let wheel_rb = sim.rigid_body_set.insert(
                RigidBodyBuilder::dynamic()
                    .translation(vector![
                        wheel_pos_in_world.x,
                        wheel_pos_in_world.y,
                        wheel_pos_in_world.z
                    ]).rotation(vector![3.14159 / 2.0, 0.0, 0.0]).ccd_enabled(true).can_sleep(false)
            );

            let collider = ColliderBuilder::cylinder(0.01  * scale, 0.03  * scale).friction(0.8).density(10.0);
            //let collider = ColliderBuilder::ball(0.03 * scale).friction(0.8).density(40.0);
            sim.collider_set.insert_with_parent(collider, wheel_rb, &mut sim.rigid_body_set);

            let joint = rapier3d::dynamics::GenericJointBuilder::new(JointAxesMask::X | JointAxesMask::Y | JointAxesMask::Z | JointAxesMask::ANG_X | JointAxesMask::ANG_Y )
                .local_anchor1(pos)
                .local_anchor2(point![0.0, 0.01 * if pos.z > 0.0 { -1.0 } else { 1.0 }, 0.0])
                .local_frame2(Isometry::new(vector![0.0, 0.0, 0.0], vector![3.14159 / 2.0, 0.0, 0.0]))
                .motor_max_force(JointAxis::AngZ, 1000.0)
                .motor_model(JointAxis::AngZ, MotorModel::ForceBased)
                .motor_velocity(JointAxis::AngZ, 0.0, 4.0)
                .build();

            wheel_joints.push(sim.multibody_joint_set.insert(vehicle_handle, wheel_rb, joint, true).unwrap());
            wheel_bodies.push(wheel_rb);
        }


        for pos in ball_wheel_positions {        
            let wheel_pos_in_world = Point3::new(box_center.x + pos.x, box_center.y + pos.y, box_center.z + pos.z);

            let wheel_rb = sim.rigid_body_set.insert(
                RigidBodyBuilder::dynamic()
                    .translation(vector![
                        wheel_pos_in_world.x,
                        wheel_pos_in_world.y,
                        wheel_pos_in_world.z
                    ]).ccd_enabled(true)
                    .can_sleep(false)
            );

            let collider = ColliderBuilder::ball(ball_wheel_radius).density(5.0).friction(0.2);
            sim.collider_set.insert_with_parent(collider, wheel_rb, &mut sim.rigid_body_set);

            let joint = rapier3d::dynamics::GenericJointBuilder::new(JointAxesMask::X | JointAxesMask::Y | JointAxesMask::Z )
                .local_anchor1(pos)
                .local_anchor2(point![0.0, 0.0, 0.0])
                .build();

            sim.multibody_joint_set.insert(vehicle_handle, wheel_rb, joint, true);
        }

        // Create whiskers
        let whisker_l = ColliderBuilder::cuboid(hw * 0.4, 0.025, hd * 0.8).sensor(true).translation(vector![hw * 1.25, 0.05, hd * -0.4]);
        let whisker_l = sim.collider_set.insert_with_parent(whisker_l, vehicle_handle, &mut sim.rigid_body_set);
        let whisker_r = ColliderBuilder::cuboid(hw * 0.4, 0.025, hd * 0.8).sensor(true).translation(vector![hw * 1.25, 0.05, hd * 0.4]);
        let whisker_r = sim.collider_set.insert_with_parent(whisker_r, vehicle_handle, &mut sim.rigid_body_set);

        RobotData { 
            body_handle: vehicle_handle,
            wheel_joints,
            wheel_bodies,
            socket: None,
            speed_l: 0.0,
            speed_r: 0.0,
            last_heartbeat: 0,
            id: "010203040506".into(),
            whisker_l,
            whisker_r,
            whisker_states: [false, false],
            ticks: [0.0, 0.0],
            drive_state: DriveState::SetSpeed,
            distance_l: 0.0,
            distance_r: 0.0,
            initial_transform: Transform { position: box_center.to_owned(), rotation: roboscapesim_common::Orientation::Quaternion(box_rotation.quaternion().to_owned()), ..Default::default() },
        }
    }

    pub fn setup_robot_socket(robot: &mut RobotData) {
        //let server = "127.0.0.1";
        let server = "52.73.65.98";
        let socket = UdpSocket::bind("0.0.0.0:0").unwrap();

        socket.connect(server.to_owned() + ":1973").expect("Failed to connect");

        socket.set_read_timeout(Some(Duration::from_millis(1))).expect("Failed to set timeout");
        socket.set_write_timeout(Some(Duration::from_millis(1))).expect("Failed to set timeout");

        robot.last_heartbeat = Utc::now().timestamp();
        robot.socket = Some(socket);
        
        // Send initial message
        if let Err(e) = robot.send_roboscape_message(b"I") {
            error!("{}", e);
        }
    }

    pub async fn robot_update(robot: &mut RobotData, sim: &mut Simulation, clients: &DashMap<String, u128>, dt: f64){
        if robot.socket.is_none() {
            return;
        }

        if Utc::now().timestamp() - robot.last_heartbeat > 50 {
            if let Err(e) = robot.send_roboscape_message(b"I") {
                panic!("{}", e);
            }
        }

        // Update ticks
        robot.ticks[0] += (robot.speed_l * -32.0) as f64 * dt;
        robot.ticks[1] += (robot.speed_r * -32.0) as f64 * dt;

        if robot.drive_state == DriveState::SetDistance {
            robot.distance_l -= (robot.speed_l * -32.0) as f64 * dt;
            robot.distance_r -= (robot.speed_r * -32.0) as f64 * dt;
            info!("{} {}", robot.distance_l, robot.distance_r);

            // Stop robot if distance reached
            if f64::abs(robot.distance_l) < f64::abs(robot.speed_l as f64 * -32.0 * dt) {
                info!("Distance reached L");
                robot.speed_l = 0.0;
            }
            if f64::abs(robot.distance_r) < f64::abs(robot.speed_r as f64 * -32.0 * dt) {
                info!("Distance reached R");
                robot.speed_r = 0.0;
            }

            if robot.speed_l == 0.0 && robot.speed_r == 0.0 {
                robot.drive_state = DriveState::SetSpeed;
            }
        }

        let mut buf = [0u8; 512];
        let size = robot.socket.as_mut().unwrap().recv(&mut buf).unwrap_or_default();

        if size > 0 {
            match &buf[0] {
                b'D' => { 
                    info!("OnDrive");

                    if buf.len() > 4 {
                        robot.drive_state = DriveState::SetDistance;

                        let d1 = i16::from_le_bytes([buf[1], buf[2]]);
                        let d2 = i16::from_le_bytes([buf[3], buf[4]]);

                        robot.distance_l = d2 as f64;
                        robot.distance_r = d1 as f64;

                        info!("OnDrive {} {}", d1, d2);

                        robot.speed_l = f64::signum(robot.distance_l) as f32 * SET_DISTANCE_DRIVE_SPEED;
                        robot.speed_r = f64::signum(robot.distance_r) as f32 * SET_DISTANCE_DRIVE_SPEED;                    
                    }
                },
                b'S' => { 
                    info!("OnSetSpeed");
                    robot.drive_state = DriveState::SetSpeed;

                    if buf.len() > 4 {
                        let s1 = i16::from_le_bytes([buf[1], buf[2]]);
                        let s2 = i16::from_le_bytes([buf[3], buf[4]]);

                        robot.speed_l = -s2 as f32 / 32.0;
                        robot.speed_r = -s1 as f32 / 32.0;
                    }
                },
                b'B' => { 
                    info!("OnBeep");
                    
                    if buf.len() > 4 {
                        let freq = u16::from_le_bytes([buf[1], buf[2]]);
                        let duration = u16::from_le_bytes([buf[3], buf[4]]);

                        // Beep is only on client-side
                        RoomData::send_to_clients(&UpdateMessage::Beep(robot.id.clone(), freq, duration), clients.iter().map(|kvp| kvp.value().clone())).await;
                    }
                },
                b'L' => { 
                    info!("OnSetLED");
                },
                b'R' => { 
                    info!("OnGetRange");

                    // Setup raycast
                    let body = sim.rigid_body_set.get(robot.body_handle).unwrap();
                    let body_pos = body.translation();
                    let offset = body.rotation() * vector![0.18, 0.05, 0.0];
                    let start_point = point![body_pos.x + offset.x, body_pos.y + offset.y, body_pos.z + offset.z];
                    let ray = Ray::new(start_point, body.rotation() * vector![1.0, 0.0, 0.0]);
                    let max_toi = 3.0;
                    let solid = true;
                    let filter = QueryFilter::default().exclude_sensors();

                    let mut distance = 0u16;
                    if let Some((handle, toi)) = sim.query_pipeline.cast_ray(&sim.rigid_body_set,
                        &sim.collider_set, &ray, max_toi, solid, filter
                    ) {
                        // The first collider hit has the handle `handle` and it hit after
                        // the ray travelled a distance equal to `ray.dir * toi`.
                        let hit_point = ray.point_at(toi); // Same as: `ray.origin + ray.dir * toi`
                        distance = (toi * 100.0) as u16;
                        println!("Collider {:?} hit at point {}", handle, hit_point);
                    }

                    // Send result message
                    let dist_bytes = u16::to_le_bytes(distance);
                    if let Err(e) = robot.send_roboscape_message(&[b'R', dist_bytes[0], dist_bytes[1]] ) {
                        error!("{}", e);
                    }
                },
                b'T' => { 
                    info!("OnGetTicks");
                    let left_ticks = (robot.ticks[0] as i32).to_le_bytes();
                    let right_ticks = (robot.ticks[1] as i32).to_le_bytes();
                    let mut message: [u8; 9] = [0; 9];

                    // Create message
                    message[0] = b'T';
                    message[1..5].copy_from_slice(&left_ticks);
                    message[5..9].copy_from_slice(&right_ticks);

                    if let Err(e) = robot.send_roboscape_message(&message) {
                        error!("{}", e);
                    }
                },
                b'n' => { 
                    info!("OnSetNumeric");
                },
                b'P' => {
                    info!("OnButtonPress");                    
                },
                _ => {}
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
            
            if let Err(e) = robot.send_roboscape_message(&message) {
                error!("{}", e);
            }
        }
    }
}

impl Resettable for RobotData {
    fn reset(&mut self, sim: &mut Simulation) {
        // Reset position
        let body = sim.rigid_body_set.get_mut(self.body_handle).unwrap();
        body.set_translation(self.initial_transform.position - point![0.0, 0.0, 0.0], true);

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

        self.last_heartbeat = Utc::now().timestamp();
        
        // Send initial message
        if let Err(e) = self.send_roboscape_message(b"I") {
            error!("{}", e);
        }
    }
}