use std::{collections::BTreeMap, time::{Instant, Duration}, f32::consts::FRAC_PI_2};

use dashmap::DashMap;
use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, Request};

use log::trace;
use nalgebra::{UnitQuaternion, Vector3, vector, Rotation3};
use rapier3d::prelude::{RigidBodyHandle, Real, Ray, QueryFilter};

use crate::{room::RoomData, simulation::SCALE, vm::Intermediate};

use super::service_struct::{setup_service, ServiceType, Service};

pub struct LIDARConfig {
    pub num_beams: u8, 
    pub start_angle: Real, 
    pub end_angle: Real, 
    pub offset_pos: Vector3<Real>,
    pub max_distance: Real,
}

impl Default for LIDARConfig {
    fn default() -> Self {
        Self { num_beams: 3, start_angle: -FRAC_PI_2, end_angle: FRAC_PI_2, offset_pos: Vector3::zeros(), max_distance: 3.0 }
    }
}

pub fn create_lidar_service(id: &str, rigid_body: &RigidBodyHandle) -> Service {
    // Create definition struct
    let mut definition = ServiceDefinition {
        id: id.to_owned(),
        methods: BTreeMap::new(),
        events: BTreeMap::new(),
        description: IoTScapeServiceDescription {
            description: Some("Get distances at multiple angles".to_owned()),
            externalDocumentation: None,
            termsOfService: None,
            contact: Some("gstein@ltu.edu".to_owned()),
            license: None,
            version: "1".to_owned(),
        },
    };

    // Define methods
    definition.methods.insert(
        "getRange".to_owned(),
        MethodDescription {
            documentation: Some("Get list of distances around the sensor".to_owned()),
            params: vec![],
            returns: MethodReturns {
                documentation: None,
                r#type: vec!["number".to_owned(), "number".to_owned()],
            },
        },
    );

    let service = setup_service(definition, ServiceType::LIDAR, None);

    service
        .lock()
        .unwrap()
        .announce()
        .expect("Could not announce to server");

    let last_announce = Instant::now();
    let announce_period = Duration::from_secs(50);

    let attached_rigid_bodies = DashMap::new();
    attached_rigid_bodies.insert("main".into(), *rigid_body);

    Service {
        id: id.to_string(),
        service_type: ServiceType::LIDAR,
        service,
        last_announce,
        announce_period,
        attached_rigid_bodies,
    }
}

pub fn calculate_rays(config: &LIDARConfig, orientation: &UnitQuaternion<Real>, body_pos: &Vector3<Real>) -> Vec<Ray> {
    let num_beams = config.num_beams;
    let start_angle = config.start_angle;
    let end_angle = config.end_angle;
    let offset_pos = config.offset_pos;

    let mut rays = vec![];
    let angle_delta = (end_angle - start_angle) / f32::max(1.0, num_beams as f32 - 1.0);
    let origin = nalgebra::OPoint { coords: body_pos + orientation * offset_pos };

    for i in 0..num_beams {
        let angle = -angle_delta * i as f32 - start_angle;
        let direction = orientation * Rotation3::from_axis_angle(&Vector3::y_axis(), angle);
        let direction = direction * vector![1.0, 0.0, 0.0];
        rays.push(Ray::new(origin, direction));
    }

    rays
}

pub fn handle_lidar_message(room: &mut RoomData, msg: Request) -> Result<Intermediate, String> {
    let mut response = vec![];

    let s = room.services.get(&(msg.device.clone(), ServiceType::LIDAR));
    if let Some(s) = s {
        let service = s.value().lock().unwrap();
        if let Some(body) = service.attached_rigid_bodies.get("main") {
            let simulation = room.sim.lock().unwrap();

            if let Some(o) = simulation.rigid_body_set.lock().unwrap().get(*body) {
                if !room.lidar_configs.contains_key(&service.id) {
                    room.lidar_configs.insert(service.id.clone(), LIDARConfig::default());
                }

                let get = room.lidar_configs.get(&service.id).unwrap();
                let config = get;
                let rays = calculate_rays(config, o.rotation(), o.translation());
                
                // Raycast each ray
                let solid = true;
                let filter = QueryFilter::default().exclude_sensors().exclude_rigid_body(*body);

                let mut distances = vec![];
                for ray in rays {
                    let mut distance = config.max_distance * 100.0;
                    if let Some((handle, toi)) = simulation.query_pipeline.cast_ray(&simulation.rigid_body_set.lock().unwrap(),
                        &simulation.collider_set, &ray, config.max_distance * SCALE, solid, filter
                    ) {
                        // The first collider hit has the handle `handle` and it hit after
                        // the ray travelled a distance equal to `ray.dir * toi`.
                        let hit_point = ray.point_at(toi); // Same as: `ray.origin + ray.dir * toi`
                        distance = toi * 100.0 / SCALE;
                        trace!("Collider {:?} hit at point {}", handle, hit_point);
                    }
                    distances.push(distance);
                }

                response = distances.iter().map(|f| (*f).into() ).collect();     
            };
        }
        service.service.lock().unwrap().enqueue_response_to(msg, Ok(response.clone()));
    }

    Ok(Intermediate::Json(serde_json::to_value(response).unwrap()))
}

#[cfg(test)]
use std::f32::consts::{PI, FRAC_PI_4};

#[test]
fn test_calculate_rays() {
    let mut config = LIDARConfig {
        num_beams: 3,
        start_angle: 0.0,
        end_angle: FRAC_PI_2,
        ..Default::default()
    };

    // Test some angles
    let rays = calculate_rays(&config, &UnitQuaternion::identity(), &Vector3::zeros());
    assert_eq!(rays.len(), 3);
    float_cmp::assert_approx_eq!(f32, rays[0].dir.x, 1.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[0].dir.z, 0.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].dir.x, 0.7071068, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].dir.z, 0.7071068, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[2].dir.x, 0.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[2].dir.z, 1.0, epsilon = 0.0000003, ulps = 5);

    config.start_angle = FRAC_PI_2;
    config.end_angle = 0.0;
    let rays = calculate_rays(&config, &UnitQuaternion::identity(), &Vector3::zeros());
    assert_eq!(rays.len(), 3);
    float_cmp::assert_approx_eq!(f32, rays[0].dir.x, 0.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[0].dir.z, 1.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].dir.x, 0.7071068, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].dir.z, 0.7071068, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[2].dir.x, 1.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[2].dir.z, 0.0, epsilon = 0.0000003, ulps = 5);

    config.start_angle = -FRAC_PI_4;
    config.end_angle = FRAC_PI_4;
    let rays = calculate_rays(&config, &UnitQuaternion::identity(), &Vector3::zeros());
    assert_eq!(rays.len(), 3);
    float_cmp::assert_approx_eq!(f32, rays[0].dir.x, 0.7071068, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[0].dir.z, -0.7071068, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].dir.x, 1.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].dir.z, 0.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[2].dir.x, 0.7071068, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[2].dir.z, 0.7071068, epsilon = 0.0000003, ulps = 5);

    // Test change of origin
    config.start_angle = 0.0;
    config.end_angle = FRAC_PI_2;
    let rays = calculate_rays(&config, &UnitQuaternion::identity(), &vector![1.0,2.0,3.0]);
    float_cmp::assert_approx_eq!(f32, rays[0].origin.x, 1.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[0].origin.y, 2.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[0].origin.z, 3.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].origin.x, 1.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].origin.y, 2.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].origin.z, 3.0, epsilon = 0.0000003, ulps = 5);

    // Test offset
    config.offset_pos = vector![1.0,2.0,3.0];
    let rays = calculate_rays(&config, &UnitQuaternion::identity(), &Vector3::zeros());
    float_cmp::assert_approx_eq!(f32, rays[0].dir.x, 1.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[0].dir.z, 0.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].dir.x, 0.7071068, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].dir.z, 0.7071068, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[2].dir.x, 0.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[2].dir.z, 1.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[0].origin.x, 1.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[0].origin.y, 2.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[0].origin.z, 3.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].origin.x, 1.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].origin.y, 2.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].origin.z, 3.0, epsilon = 0.0000003, ulps = 5);

    // Test orientation
    // Flipped upside down
    config.offset_pos = vector![0.0,0.0,0.0];
    let rays = calculate_rays(&config, &UnitQuaternion::from_euler_angles(PI, 0.0, 0.0), &Vector3::zeros());
    assert_eq!(rays.len(), 3);
    float_cmp::assert_approx_eq!(f32, rays[0].dir.x, 1.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[0].dir.z, 0.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].dir.x, 0.7071068, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].dir.z, -0.7071068, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[2].dir.x, 0.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[2].dir.z, -1.0, epsilon = 0.0000003, ulps = 5);

    // Pointing up with offset
    config.offset_pos = vector![1.0,0.0,0.0];
    let rays = calculate_rays(&config, &UnitQuaternion::from_euler_angles(0.0, 0.0, -FRAC_PI_2), &Vector3::zeros());
    assert_eq!(rays.len(), 3);
    float_cmp::assert_approx_eq!(f32, rays[0].origin.x, 0.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[0].origin.y, -1.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[0].origin.z, 0.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[0].dir.x, 0.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[0].dir.y, -1.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[0].dir.z, 0.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].dir.y, -0.7071068, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].dir.z, 0.7071068, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[2].dir.y, 0.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[2].dir.z, 1.0, epsilon = 0.0000003, ulps = 5);
}