use std::{sync::Arc, time::SystemTime};

use log::{error, info, trace};
use nalgebra::{Point3, UnitQuaternion, Vector3};
use rapier3d::prelude::*;
use roboscapesim_common::{Transform, Orientation};
use std::f32::consts::FRAC_PI_2;

use crate::{robot::{messages::send_roboscape_message, RobotData, RobotMotorData}, simulation::{Simulation, SCALE}, util::{extra_rand::generate_random_mac_address, util::bytes_to_hex_string}};


/// Physics data for the robot, used for simulation
#[derive(Debug)]
pub struct RobotPhysics {
    /// Handle to the robot's rigid body
    pub body_handle: RigidBodyHandle,
    /// Handles to the robot's wheel joints
    pub wheel_joints: Vec<MultibodyJointHandle>,
    /// Handles to the robot's wheel bodies
    pub wheel_bodies: Vec<RigidBodyHandle>,
}

impl RobotPhysics {
    /// Create physics body for robot, returns RobotData for the robot
    pub fn create_robot_body(sim: Arc<Simulation>, mac: Option<[u8; 6]>, position: Option<Vector3<Real>>, orientation: Option<UnitQuaternion<Real>>, scale: Option<Real>) -> RobotData {
        let mut robot = {
            let mac = mac.unwrap_or_else(generate_random_mac_address);
            let id = bytes_to_hex_string(&mac).to_owned();
            info!("Creating robot {}", id);

            let scale = scale.unwrap_or(1.0) * SCALE;

            // Size of robot
            let hw: f32 = 0.07 * scale;
            let hh: f32 = 0.03 * scale;
            let hd: f32 = 0.03 * scale;

            let box_center: Point3<f32> = Point3::new(0.0, 1.0 + hh * 2.0, 0.0);
            let box_rotation = UnitQuaternion::from_euler_angles(0.0, 0.0, 0.0);

            let rigid_body = RigidBodyBuilder::dynamic()
                .translation(vector![box_center.x * scale, box_center.y * scale, box_center.z * scale])
                .angular_damping(5.0)
                .linear_damping(5.0)
                .ccd_enabled(true)
                .can_sleep(false);
            
            let bodies = &mut sim.rigid_body_set.write().unwrap();
            let vehicle_handle = bodies.insert(rigid_body);
            
            let collider = ColliderBuilder::cuboid(hw, hh, hd).density(25.0);
            sim.collider_set.write().unwrap().insert_with_parent(collider, vehicle_handle, bodies);

            let wheel_half_width = 0.01;
            let wheel_positions = [
                point![hw * 0.5, -hh + 0.015 * scale, hd + wheel_half_width * scale],
                point![hw * 0.5, -hh + 0.015 * scale, -hd - wheel_half_width * scale],
            ];

            let ball_wheel_radius: f32 = 0.015 * scale;
            let ball_wheel_positions = [
                point![-hw * 0.75, -hh, 0.0]
            ];

            let mut wheel_bodies: Vec<RigidBodyHandle> = Vec::with_capacity(2);
            let mut wheel_joints: Vec<MultibodyJointHandle> = Vec::with_capacity(2);

            for pos in wheel_positions {
                //vehicle.add_wheel(pos, -Vector::y(), Vector::z(), hh, hh / 4.0, &tuning);
                
                let wheel_pos_in_world = Point3::new(box_center.x + pos.x, box_center.y + pos.y, box_center.z + pos.z);

                let wheel_rb = bodies.insert(
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

                let collider = ColliderBuilder::cylinder(wheel_half_width * scale, 0.03  * scale).friction(0.8).density(10.0);
                //let collider = ColliderBuilder::ball(0.03 * scale).friction(0.8).density(40.0);
                sim.collider_set.write().unwrap().insert_with_parent(collider, wheel_rb, bodies);

                let joint = rapier3d::dynamics::GenericJointBuilder::new(JointAxesMask::LIN_AXES | JointAxesMask::ANG_X | JointAxesMask::ANG_Y )
                    .local_anchor1(pos)
                    .local_anchor2(point![0.0, 0.01 * scale * if pos.z > 0.0 { -1.0 } else { 1.0 }, 0.0])
                    .local_frame2(Isometry::new(vector![0.0, 0.0, 0.0], vector![FRAC_PI_2, 0.0, 0.0]))
                    .motor_max_force(JointAxis::AngZ, 300.0 * scale * scale)
                    .motor_model(JointAxis::AngZ, MotorModel::ForceBased)
                    .motor_velocity(JointAxis::AngZ, 0.0, 0.0)
                    .build();

                wheel_joints.push(sim.multibody_joint_set.write().unwrap().insert(vehicle_handle, wheel_rb, joint, true).unwrap());
                wheel_bodies.push(wheel_rb);
            }


            for pos in ball_wheel_positions {        
                let wheel_pos_in_world = Point3::new(box_center.x + pos.x, box_center.y + pos.y, box_center.z + pos.z);

                let wheel_rb = bodies.insert(
                    RigidBodyBuilder::dynamic()
                        .translation(vector![
                            wheel_pos_in_world.x,
                            wheel_pos_in_world.y,
                            wheel_pos_in_world.z
                        ]).ccd_enabled(true)
                        .can_sleep(false).angular_damping(15.0).linear_damping(5.0)
                        .enabled_translations(false, false, false)
                );

                let collider = ColliderBuilder::ball(ball_wheel_radius).density(5.0).friction(0.25);
                sim.collider_set.write().unwrap().insert_with_parent(collider, wheel_rb, bodies);

                let joint = rapier3d::dynamics::GenericJointBuilder::new(JointAxesMask::LIN_AXES)
                    .local_anchor1(pos)
                    .local_anchor2(point![0.0, 0.0, 0.0])
                    .build();
                
                wheel_bodies.push(wheel_rb);

                sim.multibody_joint_set.write().unwrap().insert(vehicle_handle, wheel_rb, joint, true);
            }

            // Create whiskers
            let whisker_l = ColliderBuilder::cuboid(hw * 0.4, 0.025, hd * 0.8).sensor(true).mass(0.0).translation(vector![hw * 1.25, 0.05, hd * -0.4]);
            let whisker_l = sim.collider_set.write().unwrap().insert_with_parent(whisker_l, vehicle_handle, bodies);
            let whisker_r = ColliderBuilder::cuboid(hw * 0.4, 0.025, hd * 0.8).sensor(true).mass(0.0).translation(vector![hw * 1.25, 0.05, hd * 0.4]);
            let whisker_r = sim.collider_set.write().unwrap().insert_with_parent(whisker_r, vehicle_handle, bodies);

            RobotData { 
                physics: RobotPhysics {
                    body_handle: vehicle_handle,
                    wheel_joints,
                    wheel_bodies,
                },
                socket: None,
                last_heartbeat: 0,
                mac,
                id,
                whisker_l,
                whisker_r,
                whisker_states: [false, false],
                motor_data: RobotMotorData::default(),
                initial_transform: Transform { position: position.unwrap_or(box_center.to_owned().coords).into(), rotation: orientation.unwrap_or(box_rotation).into(), ..Default::default() },
                claimed_by: None,
                claimable: true,
                start_time: SystemTime::now(),
                last_message_time: SystemTime::UNIX_EPOCH,
                min_message_spacing: 1000 / 25, // 25 messages per second
            }
        };

        RobotPhysics::update_transform(&mut robot, sim, position, orientation.and_then(|o| Some(o.into())), true);

        robot
    }   


    pub fn update_transform(robot: &mut RobotData, sim: Arc<Simulation>, position: Option<Vector3<Real>>, rotation: Option<Orientation>, reset_velocity: bool) {
        if let Some(position) = position {
            // Reset position
            {
                let rigid_body_set = &mut sim.rigid_body_set.write().unwrap();
                for wheel in &robot.physics.wheel_bodies {
                    let body = rigid_body_set.get_mut(*wheel).unwrap();
                    body.set_linvel(vector![0.0, 0.0, 0.0], true);
                    body.set_angvel(vector![0.0, 0.0, 0.0], true);
                }
                
                // Reset position
                let body = rigid_body_set.get_mut(robot.physics.body_handle).unwrap();
                body.set_translation(position, false);
                body.set_locked_axes(LockedAxes::all(), true);
            }
            
            // // Update simulation a bit
            // sim.update(1.0 / (UPDATE_FPS / 4.0));
        }

        let rigid_body_set = &mut sim.rigid_body_set.write().unwrap();
        let body = rigid_body_set.get_mut(robot.physics.body_handle).unwrap();
        body.set_locked_axes(LockedAxes::empty(), true);

        // Reset velocity
        if reset_velocity {
            body.set_linvel(vector![0.0, -0.01, 0.0], true);
            body.set_angvel(vector![0.0, 0.0, 0.0], true);
        }

        if let Some(rotation) = rotation {
            // Set rotation
            match rotation {
                Orientation::Quaternion(q) => {
                    body.set_rotation(UnitQuaternion::new_unchecked(q), true);
                }
                Orientation::Euler(e) => {
                    body.set_rotation(UnitQuaternion::from_euler_angles(e.x, e.y, e.z), true);
                }
            }
        }
    }

    pub fn set_wheel_speeds(robot: &mut RobotData, sim: &Arc<Simulation>, speed_l: f32, speed_r: f32) {
        let jointset = &mut sim.multibody_joint_set.write().unwrap();
        let joint1 = jointset.get_mut(robot.physics.wheel_joints[0]).unwrap().0.link_mut(2).unwrap();
        joint1.joint.data.set_motor_velocity(JointAxis::AngZ, speed_l, 4.0);

        let joint2 = jointset.get_mut(robot.physics.wheel_joints[1]).unwrap().0.link_mut(1).unwrap();
        joint2.joint.data.set_motor_velocity(JointAxis::AngZ, speed_r, 4.0);
    }

    pub fn check_whiskers(robot: &mut RobotData, sim: Arc<Simulation>) {
        let mut new_whisker_states = [false, false];

        // Check whiskers
        for c in sim.narrow_phase.lock().unwrap().intersection_pairs_with(robot.whisker_l) {
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
            
        for c in sim.narrow_phase.lock().unwrap().intersection_pairs_with(robot.whisker_r) {
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
    }

}