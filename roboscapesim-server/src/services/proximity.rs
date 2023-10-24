use std::{collections::BTreeMap, time::Instant};

use dashmap::DashMap;
use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, Request, Response, EventResponse, EventDescription};
use log::info;
use nalgebra::Vector3;
use rapier3d::prelude::{RigidBodyHandle, Real};

use crate::{room::RoomData, vm::Intermediate};

use super::{service_struct::{setup_service, ServiceType, Service, DEFAULT_ANNOUNCE_PERIOD}, HandleMessageResult};

pub fn create_proximity_service(id: &str, rigid_body: &RigidBodyHandle, target: &Vector3<Real>, override_name: Option<String>) -> Service {
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

    definition.events.insert("dig".to_owned(),
    EventDescription {
        params: vec![],
    });
    
    let service = setup_service(definition, ServiceType::ProximitySensor, override_name.as_deref());

    service
        .lock()
        .unwrap()
        .announce()
        .expect("Could not announce to server");

    let last_announce = Instant::now();
    let announce_period = DEFAULT_ANNOUNCE_PERIOD;

    let attached_rigid_bodies = DashMap::new();
    attached_rigid_bodies.insert("main".into(), *rigid_body);
    
    let key_points = DashMap::new();
    key_points.insert("target".into(), *target);

    Service {
        id: id.to_string(),
        service_type: ServiceType::PositionSensor,
        service,
        last_announce,
        announce_period,
        attached_rigid_bodies,
        key_points,
    }
}

pub fn handle_proximity_sensor_message(room: &mut RoomData, msg: Request) -> HandleMessageResult {
    let mut response = vec![];
    let mut message_response = None;

    let s = room.services.get(&(msg.device.clone(), ServiceType::ProximitySensor));
    if let Some(s) = s {
        let service = s.value().lock().unwrap();
        if let Some(body) = service.attached_rigid_bodies.get("main") {
            let simulation = &mut room.sim.lock().unwrap();
            
            if let Some(o) = simulation.rigid_body_set.lock().unwrap().get(*body) {
                if let Some(t) = service.key_points.get("target") {
                    match msg.function.as_str() {
                        "getIntensity" => {
                            // TODO: apply some function definable through some config setting
                            let dist = (t.to_owned() - o.translation()).norm();
                            response = vec![dist.into()];
                        },
                        "dig" => {
                            // TODO: Something better than this?
                            // For now, sending a message to the project that a dig was attempted
                            message_response.replace(Response {
                                id: "".to_owned(),
                                request: "".to_owned(),
                                service: service.service.lock().unwrap().name.to_owned(),
                                response: None,
                                event: Some(EventResponse {
                                    r#type: Some("dig".to_owned()),
                                    args: Some(BTreeMap::new()),
                                }),
                                error: None,
                            });
                        },
                        f => {
                            info!("Unrecognized function {}", f);
                        }
                    };
                }
            } else {
                info!("Unrecognized object {}", msg.device);
            };
        }

        s.value().lock().unwrap().service.lock().unwrap().enqueue_response_to(msg, Ok(response.clone()));      

    } else {
        info!("No service found for {}", msg.device);
    }

    (Ok(Intermediate::Json(serde_json::to_value(response).unwrap())), message_response)
}