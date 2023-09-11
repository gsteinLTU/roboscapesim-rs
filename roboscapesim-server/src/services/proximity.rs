use std::{collections::BTreeMap, time::{Instant, Duration}};

use dashmap::DashMap;
use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, Request};
use log::info;
use rapier3d::prelude::RigidBodyHandle;

use crate::room::RoomData;

use super::service_struct::{setup_service, ServiceType, Service};

pub fn create_proximity_service(id: &str, rigid_body: &RigidBodyHandle, target: &RigidBodyHandle, override_name: Option<&str>) -> Service {
    // Create definition struct
    let mut definition = ServiceDefinition {
        id: id.to_owned(),
        methods: BTreeMap::new(),
        events: BTreeMap::new(),
        description: IoTScapeServiceDescription {
            description: Some("Get the position and orientation of an object".to_owned()),
            externalDocumentation: None,
            termsOfService: None,
            contact: Some("gstein@ltu.edu".to_owned()),
            license: None,
            version: "1".to_owned(),
        },
    };

    // Define methods
    definition.methods.insert(
        "getIntensity".to_owned(),
        MethodDescription {
            documentation: Some("Get sensor reading at current position".to_owned()),
            params: vec![],
            returns: MethodReturns {
                documentation: None,
                r#type: vec!["number".to_owned(), "number".to_owned(), "number".to_owned()],
            },
        },
    );

    definition.methods.insert(
        "dig".to_owned(),
        MethodDescription {
            documentation: Some("Get heading direction (yaw) of object".to_owned()),
            params: vec![],
            returns: MethodReturns {
                documentation: None,
                r#type: vec![],
            },
        },
    );
    
    let service = setup_service(definition, ServiceType::ProximitySensor, override_name);

    service
        .lock()
        .unwrap()
        .announce()
        .expect("Could not announce to server");

    let last_announce = Instant::now();
    let announce_period = Duration::from_secs(30);

    let attached_rigid_bodies = DashMap::new();
    attached_rigid_bodies.insert("main".into(), rigid_body.clone());
    attached_rigid_bodies.insert("target".into(), target.clone());

    Service {
        id: id.to_string(),
        service_type: ServiceType::PositionSensor,
        service,
        last_announce,
        announce_period,
        attached_rigid_bodies,
    }
}

pub fn handle_proximity_sensor_message(room: &mut RoomData, msg: Request) {
    let s = room.services.iter().find(|serv| serv.id == msg.device && serv.service_type == ServiceType::ProximitySensor);
    if let Some(s) = s {
        if let Some(body) = s.attached_rigid_bodies.get("main") {
            if let Some(target_body) = s.attached_rigid_bodies.get("target") {
                if let Some(o) = room.sim.rigid_body_set.get(body.clone()) {
                    if let Some(t) = room.sim.rigid_body_set.get(target_body.clone()) {
                        match msg.function.as_str() {
                            "getIntensity" => {
                                // TODO: apply some function
                                let dist = (t.translation() - o.translation()).norm();
                                s.service.lock().unwrap().enqueue_response_to(msg, Ok(vec![dist.to_string()]));                   
                            },
                            "dig" => {
                                // TODO:
                            },
                            f => {
                                info!("Unrecognized function {}", f);
                            }
                        };
                    }
                } else {
                    info!("Unrecognized object {}", msg.device);
                }
            }
        }
    } else {
        info!("No service found for {}", msg.device);
    }
}