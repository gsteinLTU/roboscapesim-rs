use std::{collections::BTreeMap, time::{Instant, Duration}};

use dashmap::DashMap;
use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, MethodParam, Request};
use log::info;
use nalgebra::{vector, UnitQuaternion};
use rapier3d::prelude::RigidBodyHandle;

use crate::{room::RoomData, vm::Intermediate, util::util::num_val};

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
        "setRotation".to_owned(),
        MethodDescription {
            documentation: Some("Set rotation".to_owned()),
            params: vec![
                MethodParam {
                    name: "pitch".to_owned(),
                    documentation: Some("X rotation".to_owned()),
                    r#type: "number".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "yaw".to_owned(),
                    documentation: Some("Y rotation".to_owned()),
                    r#type: "number".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "roll".to_owned(),
                    documentation: Some("Z rotation".to_owned()),
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

    definition.methods.insert(
        "getPosition".to_owned(),
        MethodDescription {
            documentation: Some("Get XYZ coordinate position of object".to_owned()),
            params: vec![],
            returns: MethodReturns {
                documentation: None,
                r#type: vec!["number".to_owned(), "number".to_owned(), "number".to_owned()],
            },
        },
    );

    definition.methods.insert(
        "getRotation".to_owned(),
        MethodDescription {
            documentation: Some("Get Euler angle rotation of object".to_owned()),
            params: vec![],
            returns: MethodReturns {
                documentation: None,
                r#type: vec!["number".to_owned(), "number".to_owned(), "number".to_owned()],
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
    let announce_period = Duration::from_secs(50);

    let attached_rigid_bodies = DashMap::new();
    attached_rigid_bodies.insert("main".into(), *rigid_body);

    Service {
        id: id.to_string(),
        service_type: ServiceType::Entity,
        service,
        last_announce,
        announce_period,
        attached_rigid_bodies,
    }
}

pub fn handle_entity_message(room: &mut RoomData, msg: Request) -> Result<Intermediate, String> {
    let mut response = vec![];

    info!("{:?}", msg);
    
    let s = room.services.get(&(msg.device.clone(), ServiceType::Entity));
    if let Some(s) = s {
        if let Some(body) = s.value().lock().unwrap().attached_rigid_bodies.get("main") {
            if let Some(o) = room.sim.lock().unwrap().rigid_body_set.lock().unwrap().get_mut(*body) {
                match msg.function.as_str() {
                    "reset" => {
                        if let Some(r) = room.reseters.get_mut(msg.device.as_str()) {
                            r.reset(&mut room.sim.lock().unwrap());
                        } else {
                            info!("Unrecognized device {}", msg.device);
                        }
                    },
                    "setPosition" => {
                        let x = num_val(&msg.params[0]);
                        let y = num_val(&msg.params[1]);
                        let z = num_val(&msg.params[2]);
                        o.set_translation(vector![x, y, z], true);
                    },
                    "setRotation" => {
                        let pitch = num_val(&msg.params[1]);
                        let yaw = num_val(&msg.params[2]);
                        let roll = num_val(&msg.params[0]);
                        o.set_rotation(UnitQuaternion::from_euler_angles(roll, pitch, yaw), true);
                    },
                    "getPosition" => {
                        response = vec![o.translation().x.into(), o.translation().y.into(), o.translation().z.into()];              
                    },
                    "getRotation" => {
                        let r = o.rotation().euler_angles();
                        response = vec![r.2.into(), r.0.into(), r.1.into()];              
                    },
                    f => {
                        info!("Unrecognized function {}", f);
                    }
                };
            }
        }

        s.value().lock().unwrap().service.lock().unwrap().enqueue_response_to(msg, Ok(response.clone()));      
    }

    Ok(Intermediate::Json(serde_json::to_value(response).unwrap()))
}