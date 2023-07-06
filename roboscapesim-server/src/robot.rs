use std::net::UdpSocket;
use std::time::{SystemTime, UNIX_EPOCH, Duration};

use derivative::Derivative;
use log::info;
use nalgebra::Point3;
use rapier3d::prelude::*;

use crate::room::Simulation;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct RobotData {
    pub body_handle: RigidBodyHandle,
    pub wheel_joints: Vec<MultibodyJointHandle>,
    pub socket: Option<UdpSocket>,
    pub speed_l: f32,
    pub speed_r: f32,
}

pub fn send_roboscape_message(socket: &mut UdpSocket, message: &[u8]) -> Result<usize, std::io::Error> {
    let mut buf = Vec::<u8>::new();

    // MAC address
    let mut mac: Vec<u8> = vec![1,2,3,4,5,6];
    buf.append(&mut mac);

    // Timestamp
    let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as u32;
    buf.append(&mut Vec::from(time.to_be_bytes()));

    // Message
    buf.append(&mut Vec::from(message));

    socket.send(&buf.as_slice())
}

pub fn create_robot_body(sim: &mut Simulation) -> RobotData {
        
    /*
    * Vehicle we will control manually.
    */
    let scale = 3.0;
    let hw = 0.05 * scale;
    let hh = 0.03 * scale;
    let hd = 0.04 * scale;

    let box_center: Point3<f32> = Point3::new(0.0, 1.0 + hh * 2.0, 0.0);
    let rigid_body = RigidBodyBuilder::dynamic()
        .translation(vector![box_center.x * scale, box_center.y * scale, box_center.z * scale])
        .linear_damping(2.0)
        .angular_damping(2.0)
        .ccd_enabled(true);
    
    let vehicle_handle = sim.rigid_body_set.insert(rigid_body);
    
    let collider = ColliderBuilder::cuboid(hw, hh, hd).density(25.0);
    sim.collider_set.insert_with_parent(collider, vehicle_handle, &mut sim.rigid_body_set);

    //let mut vehicle = DynamicRayCastVehicleController::new(vehicle_handle);
    let wheel_positions = [
        point![hw * 0.5, -hh + 0.015 * scale, hw],
        point![hw * 0.5, -hh + 0.015 * scale, -hw],
    ];

    let ball_wheel_radius = 0.015 * scale;
    let ball_wheel_positions = [
        point![-hw * 0.75, -hh, 0.0]
    ];


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
                ]).rotation(vector![3.14159 / 2.0, 0.0, 0.0]).ccd_enabled(true)
        );

        let collider = ColliderBuilder::cylinder(0.01  * scale, 0.03  * scale).friction(0.8).density(10.0);
        //let collider = ColliderBuilder::ball(0.03 * scale).friction(0.8).density(40.0);
        sim.collider_set.insert_with_parent(collider, wheel_rb, &mut sim.rigid_body_set);

        let mut joint = rapier3d::dynamics::GenericJointBuilder::new(JointAxesMask::X | JointAxesMask::Y | JointAxesMask::Z | JointAxesMask::ANG_X | JointAxesMask::ANG_Y )
            .local_anchor1(pos)
            .local_anchor2(point![0.0, 0.01 * if pos.z > 0.0 { -1.0 } else { 1.0 }, 0.0])
            .local_frame2(Isometry::new(vector![0.0, 0.0, 0.0], vector![3.14159 / 2.0, 0.0, 0.0]))
            .motor_max_force(JointAxis::AngZ, 1000.0)
            .motor_model(JointAxis::AngZ, MotorModel::ForceBased)
            .motor_velocity(JointAxis::AngZ, 0.0, 4.0)
            //.motor_velocity(JointAxis::AngZ, -25.0 * if pos.z > 0.0 { -1.0 } else { 1.0 }, 4.0)
            .build();

        // let joint = rapier3d::dynamics::RevoluteJointBuilder::new(UnitVector3::new_normalize(vector![0.0,1.0,0.0]))
        //     .local_anchor1(pos)
        //     .local_anchor2(point![0.0, 0.01 * if pos.z > 0.0 { -1.0 } else { 1.0 }, 0.0])
            // .motor_max_force(1000.0)
            // .motor_model(MotorModel::ForceBased)
            // .motor_velocity(-25.0 * if pos.z > 0.0 { -1.0 } else { 1.0 }, 4.0)
            // .build();
            
        //impulse_joints.insert(vehicle_handle, wheel_rb, joint, true);
        wheel_joints.push(sim.multibody_joint_set.insert(vehicle_handle, wheel_rb, joint, true).unwrap());
    }


    for pos in ball_wheel_positions {
        //vehicle.add_wheel(pos, -Vector::y(), Vector::z(), hh, hh / 4.0, &tuning);
        
        let wheel_pos_in_world = Point3::new(box_center.x + pos.x, box_center.y + pos.y, box_center.z + pos.z);

        let wheel_rb = sim.rigid_body_set.insert(
            RigidBodyBuilder::dynamic()
                .translation(vector![
                    wheel_pos_in_world.x,
                    wheel_pos_in_world.y,
                    wheel_pos_in_world.z
                ]).ccd_enabled(true)
        );

        let collider = ColliderBuilder::ball(ball_wheel_radius).density(5.0).friction(0.2);
        sim.collider_set.insert_with_parent(collider, wheel_rb, &mut sim.rigid_body_set);

        let joint = rapier3d::dynamics::GenericJointBuilder::new(JointAxesMask::X | JointAxesMask::Y | JointAxesMask::Z )
             .local_anchor1(pos)
             .local_anchor2(point![0.0, 0.0, 0.0])
             .build();

        sim.multibody_joint_set.insert(vehicle_handle, wheel_rb, joint, true);
    }

    RobotData { 
        body_handle: vehicle_handle,
        wheel_joints,
        socket: None,
        speed_l: 0.0,
        speed_r: 0.0,
    }
}

pub fn setup_robot_socket(robot: &mut RobotData) {
    //let server = "127.0.0.1";
    let server = "52.73.65.98";
    let mut socket = UdpSocket::bind("0.0.0.0:0").unwrap();

    socket.connect(server.to_owned() + ":1973");

    socket.set_read_timeout(Some(Duration::from_millis(1)));
    socket.set_write_timeout(Some(Duration::from_millis(1)));
    
    if let Err(e) = send_roboscape_message(&mut socket, b"I") {
        panic!("{}", e);
    }

    robot.socket = Some(socket);
}

pub fn robot_update(robot: &mut RobotData, sim: &mut Simulation, dt: f64){
    if robot.socket.is_none() {
        return;
    }

    let body = sim.rigid_body_set.get_mut(robot.body_handle).unwrap();
    let mut buf = [0 as u8; 512];
    
    //body.add_force_at_point(physics_state.integration_parameters.dt * (body.rotation() * vector![speed_l,0.0,0.0]), body.position().transform_point(&point![0.0,0.0,1.0]), true);
    //body.add_force_at_point(physics_state.integration_parameters.dt * (body.rotation() * vector![speed_r,0.0,0.0]), body.position().transform_point(&point![0.0,0.0,-1.0]), true);
    
    //body.set_translation(body.translation() + vector![1.0,0.0,0.0], true)
    //body.apply_impulse_at_point(body.position().transform_vector(&vector!(10.0,0.0,0.0)), Point3::from(body.position().transform_point(&point!(0.0,0.0,0.1))), true);
    let size = robot.socket.as_mut().unwrap().recv(&mut buf).unwrap_or_default();

    if size > 0 {
        //dbg!(&buf);
        match &buf[0] {
            b'D' => { 
                info!("OnDrive");
            },
            b'S' => { 
                info!("OnSetSpeed");
                let left = i16::from_le_bytes([buf[1], buf[2]]);
                let right = i16::from_le_bytes([buf[3], buf[4]]);

                robot.speed_l = -left as f32 / 32.0;
                robot.speed_r = -right as f32 / 32.0;
                
                info!("{:?}", robot);
                
                let joint1 = sim.multibody_joint_set.get_mut(robot.wheel_joints[0]).unwrap().0.link_mut(2).unwrap();
                joint1.joint.data.set_motor_velocity(JointAxis::AngZ, robot.speed_l, 4.0);
                
                let joint2 = sim.multibody_joint_set.get_mut(robot.wheel_joints[1]).unwrap().0.link_mut(1).unwrap();
                joint2.joint.data.set_motor_velocity(JointAxis::AngZ, robot.speed_r, 4.0);
                
            },
            b'B' => { 
                info!("OnBeep");
            },
            b'L' => { 
                info!("OnSetLED");
            },
            b'R' => { 
                info!("OnGetRange");
            },
            b'T' => { 
                info!("OnGetTicks");
            },
            b'n' => { 
                info!("OnSetNumeric");
            },
            _ => {}
        }
    }
}