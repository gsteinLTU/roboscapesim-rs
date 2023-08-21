use std::{collections::BTreeMap, time::{Instant, Duration}, f32::consts::{PI, FRAC_PI_2, FRAC_PI_4}};

use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns};

use nalgebra::{UnitQuaternion, Vector3};
use rapier3d::prelude::{RigidBodyHandle, Real, Ray};

use super::service_struct::{setup_service, ServiceType, Service};

pub struct LIDARData {
    pub num_beams: u8, 
    pub start_angle: UnitQuaternion<Real>, 
    pub end_angle: UnitQuaternion<Real>, 
    pub offset: Vector3<Real>
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
    let announce_period = Duration::from_secs(30);

    Service {
        id: id.to_string(),
        service_type: ServiceType::PositionSensor,
        service,
        last_announce,
        announce_period,
        attached_rigid_body: Some(rigid_body.to_owned()),
    }
}

pub fn calculate_rays(num_beams: u8, start_angle: f32, end_angle: f32, orientation: UnitQuaternion<Real>, body_pos: Vector3<Real>, offset_pos: Vector3<Real>) -> Vec<Ray> {
    let mut rays = vec![];
    let angle_delta = (end_angle - start_angle) / f32::max(1.0, num_beams as f32 - 1.0);

    for i in 0..num_beams {
        let angle = angle_delta * i as f32 + start_angle;
        let direction = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), angle);
        let direction = orientation * (direction * *Vector3::z_axis());
        rays.push(Ray::new(nalgebra::OPoint { coords: body_pos + orientation * offset_pos }, direction));
    }

    rays
}


#[test]
fn test_calculate_rays() {
    let rays = calculate_rays(3, 0.0, FRAC_PI_2, UnitQuaternion::identity(), Vector3::zeros(), Vector3::zeros());
    assert_eq!(rays.len(), 3);
    float_cmp::assert_approx_eq!(f32, rays[0].dir.x, 0.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[0].dir.z, 1.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].dir.x, 0.7071068, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].dir.z, 0.7071068, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[2].dir.x, 1.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[2].dir.z, 0.0, epsilon = 0.0000003, ulps = 5);

    let rays = calculate_rays(3, FRAC_PI_2, 0.0, UnitQuaternion::identity(), Vector3::zeros(), Vector3::zeros());
    assert_eq!(rays.len(), 3);
    float_cmp::assert_approx_eq!(f32, rays[0].dir.x, 1.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[0].dir.z, 0.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].dir.x, 0.7071068, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].dir.z, 0.7071068, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[2].dir.x, 0.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[2].dir.z, 1.0, epsilon = 0.0000003, ulps = 5);

    let rays = calculate_rays(3, -FRAC_PI_4, FRAC_PI_4, UnitQuaternion::identity(), Vector3::zeros(), Vector3::zeros());
    assert_eq!(rays.len(), 3);
    float_cmp::assert_approx_eq!(f32, rays[0].dir.x, -0.7071068, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[0].dir.z, 0.7071068, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].dir.x, 0.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[1].dir.z, 1.0, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[2].dir.x, 0.7071068, epsilon = 0.0000003, ulps = 5);
    float_cmp::assert_approx_eq!(f32, rays[2].dir.z, 0.7071068, epsilon = 0.0000003, ulps = 5);
}