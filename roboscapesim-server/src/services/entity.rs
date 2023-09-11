use std::{collections::BTreeMap, time::{Instant, Duration}};

use dashmap::DashMap;
use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, MethodParam, Request};
use log::info;
use rapier3d::prelude::RigidBodyHandle;

use crate::room::RoomData;

use super::service_struct::{Service, ServiceType, setup_service};

pub fn create_entity_service(id: &str, rigid_body: &RigidBodyHandle) -> Service {
    // Create definition struct
    let mut definition = ServiceDefinition {
        id: id.to_owned(),
        methods: BTreeMap::new(),
        events: BTreeMap::new(),
        description: IoTScapeServiceDescription {
            description: Some("Service for managing objects in a RoboScape Online simulation".to_owned()),
            externalDocumentation: None,
            termsOfService: None,
            contact: Some("gstein@ltu.edu".to_owned()),
            license: None,
            version: "1".to_owned(),
        },
    };

    // Define methods
    definition.methods.insert(
        "setPosition".to_owned(),
        MethodDescription {
            documentation: Some("Set position".to_owned()),
            params: vec![
                MethodParam {
                    name: "x".to_owned(),
                    documentation: Some("X position".to_owned()),
                    r#type: "number".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "y".to_owned(),
                    documentation: Some("Y position".to_owned()),
                    r#type: "number".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "z".to_owned(),
                    documentation: Some("Z position".to_owned()),
                    r#type: "number".to_owned(),
                    optional: false,
                },
            ],
            returns: MethodReturns {
                documentation: None,
                r#type: vec![],
            },
        },
    );

    definition.methods.insert(
        "reset".to_owned(),
        MethodDescription {
            documentation: Some("Reset conditions of Entity".to_owned()),
            params: vec![],
            returns: MethodReturns {
                documentation: None,
                r#type: vec![],
            },
        },
    );

    let service = setup_service(definition, ServiceType::Entity, None);

    service
        .lock()
        .unwrap()
        .announce()
        .expect("Could not announce to server");

    let last_announce = Instant::now();
    let announce_period = Duration::from_secs(30);

    let attached_rigid_bodies = DashMap::new();
    attached_rigid_bodies.insert("main".into(), rigid_body.clone());

    Service {
        id: id.to_string(),
        service_type: ServiceType::Entity,
        service,
        last_announce,
        announce_period,
        attached_rigid_bodies,
    }
}

pub fn handle_entity_message(room: &mut RoomData, msg: Request) {
    match msg.function.as_str() {
        "reset" => {
            if let Some(r) = room.reseters.get_mut(msg.device.as_str()) {
                r.reset(&mut room.sim);
            } else {
                info!("Unrecognized device {}", msg.device);
            }
        },
        f => {
            info!("Unrecognized function {}", f);
        }
    };
}