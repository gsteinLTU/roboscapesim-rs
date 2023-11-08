use std::collections::BTreeMap;

use atomic_instant::AtomicInstant;
use dashmap::DashMap;
use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, Request, EventDescription};
use log::info;
use nalgebra::Vector3;
use netsblox_vm::runtime::SimpleValue;
use rapier3d::prelude::{RigidBodyHandle, Real};

use crate::room::RoomData;

use super::{service_struct::{setup_service, ServiceType, Service, DEFAULT_ANNOUNCE_PERIOD}, HandleMessageResult};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProximityConfig {
    pub target: Vector3<Real>,
    pub multiplier: f32,
    pub offset: f32,
}

impl Default for ProximityConfig {
    fn default() -> Self {
        Self {
            target: Vector3::new(0.0, 0.0, 0.0),
            multiplier: 1.0,
            offset: 0.0,
        }
    }
}

pub fn create_proximity_service(id: &str, rigid_body: &RigidBodyHandle) -> Service {
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
                r#type: vec!["number".to_owned()],
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
    
    let service = setup_service(definition, ServiceType::ProximitySensor, None);

    service
        .lock()
        .unwrap()
        .announce()
        .expect("Could not announce to server");

    let last_announce = AtomicInstant::now();
    let announce_period = DEFAULT_ANNOUNCE_PERIOD;

    let attached_rigid_bodies = DashMap::new();
    attached_rigid_bodies.insert("main".into(), *rigid_body);

    Service {
        id: id.to_string(),
        service_type: ServiceType::ProximitySensor,
        service,
        last_announce,
        announce_period,
        attached_rigid_bodies,
    }
}

pub fn handle_proximity_sensor_message(room: &mut RoomData, msg: Request) -> HandleMessageResult {
    let mut response = vec![];
    let mut message_response = None;

    let s = room.services.get(&(msg.device.clone(), ServiceType::ProximitySensor));
    if let Some(s) = s {
        let service = s.value();
        if let Some(body) = service.attached_rigid_bodies.get("main") {
            let simulation = &mut room.sim.lock().unwrap();
            
            if let Some(o) = simulation.rigid_body_set.lock().unwrap().get(*body) {
                if let Some(t) = room.proximity_configs.get(&msg.device) {
                    match msg.function.as_str() {
                        "getIntensity" => {
                            // TODO: apply some function definable through some config setting
                            let dist = ((t.target.to_owned() - o.translation()).norm() * t.multiplier) + t.offset;
                            response = vec![dist.into()];
                        },
                        "dig" => {
                            // TODO: Something better than this?
                            // For now, sending a message to the project that a dig was attempted
                            message_response.replace(((service.id.to_owned(), ServiceType::ProximitySensor), "dig".to_owned(), BTreeMap::new()));
                        },
                        f => {
                            info!("Unrecognized function {}", f);
                        }
                    };
                } else {
                    info!("No target defined for {}", msg.device);
                }
            } else {
                info!("Unrecognized object {}", msg.device);
            };
        } else {
            info!("No main rigid body found for {}", msg.device);
        }

        service.enqueue_response_to(msg, Ok(response.clone()));      

    } else {
        info!("No service found for {}", msg.device);
    }

    if response.len() == 1 {
        return (Ok(SimpleValue::from_json(response[0].clone()).unwrap()), message_response);
    }
    (Ok(SimpleValue::from_json(serde_json::to_value(response).unwrap()).unwrap()), message_response)
}