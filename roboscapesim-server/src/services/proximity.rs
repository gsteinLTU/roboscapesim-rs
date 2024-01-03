use std::collections::BTreeMap;

use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, Request, EventDescription};
use log::info;
use nalgebra::Vector3;
use netsblox_vm::runtime::SimpleValue;
use rapier3d::prelude::{RigidBodyHandle, Real};

use crate::room::RoomData;

use super::{service_struct::{ServiceType, Service, ServiceInfo}, HandleMessageResult};

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

pub struct ProximityService {
    pub service_info: ServiceInfo,
    pub rigid_body: RigidBodyHandle,
}

pub fn create_proximity_service(id: &str, rigid_body: &RigidBodyHandle) -> Box<dyn Service + Sync + Send> {
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

    // Define events
    definition.events.insert("dig".to_owned(),
    EventDescription {
        params: vec![],
    });
    
    Box::new(ProximityService{
        service_info: ServiceInfo::new(id, definition, ServiceType::ProximitySensor),
        rigid_body: *rigid_body,
    }) as Box<dyn Service + Sync + Send>
}

impl Service for ProximityService {
    fn update(&self) -> usize {
        self.service_info.update()
    }

    fn get_service_info(&self) -> &ServiceInfo {
        &self.service_info
    }

    fn handle_message(& self, room: &mut RoomData, msg: &Request) -> HandleMessageResult {

        let mut response = vec![];
        let mut message_response = None;

        let service = self.get_service_info();
        let simulation = &mut room.sim.lock().unwrap();
        
        if let Some(o) = simulation.rigid_body_set.lock().unwrap().get(self.rigid_body) {
            if let Some(t) = room.proximity_configs.get(&msg.device) {
                match msg.function.as_str() {
                    "getIntensity" => {
                        // TODO: apply some more complex function definable through some config setting?
                        let dist = ((t.target.to_owned() - o.translation()).norm() * t.multiplier) + t.offset;
                        response.push(dist.into());
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

        service.enqueue_response_to(&msg, Ok(response.clone()));      


        if response.len() == 1 {
            return (Ok(SimpleValue::from_json(response[0].clone()).unwrap()), message_response);
        }
        (Ok(SimpleValue::from_json(serde_json::to_value(response).unwrap()).unwrap()), message_response)
    }
}