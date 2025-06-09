use crate::robot::physics::RobotPhysics;

use super::*;

impl RoomData {
    /// Add a robot to a room
    pub(crate) fn add_robot(room: &RoomData, position: Vector3<Real>, orientation: UnitQuaternion<f32>, wheel_debug: bool, speed_mult: Option<f32>, scale: Option<f32>) -> String {
        let speed_mult = speed_mult.unwrap_or(1.0).clamp(-10.0, 10.0);
        let scale: f32 = scale.unwrap_or(1.0).clamp(1.0, 5.0);

        let mut robot = RobotPhysics::create_robot_body(room.sim.clone(), None, Some(position), Some(orientation), Some(scale));
        robot.speed_scale = speed_mult;
        let robot_id: String = "robot_".to_string() + robot.id.as_str();
        room.sim.rigid_body_labels.insert(robot_id.clone(), robot.physics.body_handle);
        room.objects.insert(robot_id.clone(), ObjectData {
            name: robot_id.clone(),
            transform: Transform {scaling: vector![scale * SCALE, scale * SCALE, scale * SCALE], ..Default::default() },
            visual_info: Some(VisualInfo::Mesh("parallax_robot.glb".into())),
            is_kinematic: false,
            updated: true,
        });
        RobotData::setup_robot_socket(&mut robot);

        // Wheel debug
        if wheel_debug {
            let mut i = 0;
            for wheel in &robot.physics.wheel_bodies {
                room.sim.rigid_body_labels.insert(format!("wheel_{}", i), *wheel);
                room.objects.insert(format!("wheel_{}", i), ObjectData {
                    name: format!("wheel_{}", i),
                    transform: Transform { scaling: vector![0.18,0.03,0.18], ..Default::default() },
                    visual_info: Some(VisualInfo::default()),
                    is_kinematic: false,
                    updated: true,
                });
                i += 1;
            }
        }

        let id = robot.id.to_string();
        room.robots.insert(robot.id.to_string(), robot);
        room.last_full_update_sent.store(0, Ordering::Relaxed);
        id
    }

    /// Add a physics object to the room
    pub(crate) fn add_shape(room: &RoomData, name: &str, position: Vector3<Real>, rotation: AngVector<Real>, visual_info: Option<VisualInfo>, size: Option<Vector3<Real>>, is_kinematic: bool, visual_only: bool) -> String {
        let is_kinematic = is_kinematic || visual_only;
        let body_name = room.metadata.name.to_owned() + "_" + name;
        let mut position = position;

        // Apply jitter with extra objects to prevent lag from overlap
        let count_non_robots = room.count_non_robots();
        if !visual_only && count_non_robots > 10 {
            let mut rng = rand::thread_rng();
            let mult = if count_non_robots > 40 { 2.0 } else if count_non_robots > 20 { 1.5 } else { 1.0 };
            let jitter = vector![rng.gen_range(-0.0015..0.0015) * mult, rng.gen_range(-0.0025..0.0025) * mult, rng.gen_range(-0.0015..0.0015) * mult];
            position += jitter;
        }
        
        let mut rigid_body = if is_kinematic { RigidBodyBuilder::kinematic_position_based() } else { RigidBodyBuilder::dynamic() }
            .ccd_enabled(true)
            .translation(position)
            .build();

        rigid_body.set_rotation(UnitQuaternion::from_euler_angles(rotation.x, rotation.y, rotation.z), false);
        
        let mut size = size.unwrap_or_else(|| vector![1.0, 1.0, 1.0]);

        let visual_info = visual_info.unwrap_or_default();

        let shape = match visual_info {
            VisualInfo::Color(_, _, _, s) => {
                s
            },
            VisualInfo::Texture(_, _, _, s) => {
                s
            },
            _ => Shape::Box
        };

        let rigid_body_set = room.sim.rigid_body_set.clone();
        let cube_body_handle = rigid_body_set.write().unwrap().insert(rigid_body);

        if !visual_only {
            let collider = match shape {
                Shape::Box => ColliderBuilder::cuboid(size.x / 2.0, size.y / 2.0, size.z / 2.0),
                Shape::Sphere => {
                    size.y = size.x;
                    size.z = size.x;
                    ColliderBuilder::ball(size.x / 2.0)
                },
                Shape::Cylinder => {
                    size.z = size.x;
                    ColliderBuilder::cylinder(size.y / 2.0, size.x / 2.0)
                },
                Shape::Capsule => {
                    size.z = size.x;
                    ColliderBuilder::capsule_y(size.y / 2.0, size.x / 2.0)
                },
            };

            let collider = collider.restitution(0.3).density(0.045).friction(0.6).build();
            room.sim.collider_set.write().unwrap().insert_with_parent(collider, cube_body_handle, &mut rigid_body_set.write().unwrap());
        }

        room.sim.rigid_body_labels.insert(body_name.clone(), cube_body_handle);

        room.objects.insert(body_name.clone(), ObjectData {
            name: body_name.clone(),
            transform: Transform { position: position.into(), scaling: size, rotation: Orientation::Euler(rotation), ..Default::default() },
            visual_info: Some(visual_info),
            is_kinematic,
            updated: true,
        });

        room.reseters.insert(body_name.clone(), Box::new(RigidBodyResetter::new(cube_body_handle, room.sim.clone())));
        
        room.last_full_update_sent.store(0, Ordering::Relaxed);
        body_name
    }

    /// Add a service to the room
    pub(crate) async fn add_sensor<'a, T: ServiceFactory>(&self, id: &'a str, config: T::Config) -> &'a str {
        let service = Arc::new(T::create(id, config).await);
        self.services.insert((id.into(), service.get_service_info().service_type), service);
        id
    }

    /// Specialized add_shape for triggers
    pub(crate) async fn add_trigger(room: &RoomData, name: &str, position: Vector3<Real>, rotation: AngVector<Real>, size: Option<Vector3<Real>>) -> String {
        let body_name = room.metadata.name.to_owned() + "_" + name;
        let rigid_body =  RigidBodyBuilder::kinematic_position_based()
            .ccd_enabled(true)
            .translation(position)
            .rotation(rotation)
            .build();

        let size = size.unwrap_or_else(|| vector![1.0, 1.0, 1.0]);

        let collider = ColliderBuilder::cuboid(size.x / 2.0, size.y / 2.0, size.z / 2.0).sensor(true).build();

        let cube_body_handle = room.sim.rigid_body_set.write().unwrap().insert(rigid_body);
        let rigid_body_set = room.sim.rigid_body_set.clone();
        let collider_handle = room.sim.collider_set.write().unwrap().insert_with_parent(collider, cube_body_handle, &mut rigid_body_set.write().unwrap());
        room.sim.rigid_body_labels.insert(body_name.clone(), cube_body_handle);

        room.objects.insert(body_name.clone(), ObjectData {
            name: body_name.clone(),
            transform: Transform { position: position.into(), scaling: size, rotation: Orientation::Euler(rotation), ..Default::default() },
            visual_info: Some(VisualInfo::None),
            is_kinematic: true,
            updated: true,
        });

        room.reseters.insert(body_name.clone(), Box::new(RigidBodyResetter::new(cube_body_handle, room.sim.clone())));

        let service = Arc::new(TriggerService::create(&body_name, &collider_handle).await);
        let service_id = service.get_service_info().id.clone();
        room.services.insert((service_id.clone(), ServiceType::Trigger), service);
        room.sim.sensors.insert((service_id, collider_handle), DashSet::new());
        room.last_full_update_sent.store(0, Ordering::Relaxed);
        body_name
    }
}