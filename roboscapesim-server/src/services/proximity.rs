use std::{collections::BTreeMap, sync::Arc};

use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, Request, EventDescription};
use log::info;
use nalgebra::Vector3;
use netsblox_vm::runtime::SimpleValue;
use rapier3d::prelude::{RigidBodyHandle, Real};

use crate::room::RoomData;

use super::{service_struct::{ServiceType, Service, ServiceInfo, ServiceFactory}, HandleMessageResult};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProximityConfig {
    pub target: Vector3<Real>,
    pub multiplier: f32,
    pub offset: f32,
    pub body: RigidBodyHandle,
}

impl Default for ProximityConfig {
    fn default() -> Self {
        Self {
            target: Vector3::new(0.0, 0.0, 0.0),
            multiplier: 1.0,
            offset: 0.0,
            body: RigidBodyHandle::invalid(),
        }
    }
}

pub struct ProximityService {
    pub service_info: Arc<ServiceInfo>,
    pub config: ProximityConfig,
}

impl ServiceFactory for ProximityService {
    type Config = ProximityConfig;

    async fn create(id: &str, config: Self::Config) -> Box<dyn Service> {
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
            service_info: Arc::new(ServiceInfo::new(id, definition, ServiceType::ProximitySensor).await),
            config,
        }) as Box<dyn Service>
    }
}

impl Service for ProximityService {
    fn update(&self) {

    }

    fn get_service_info(&self) -> Arc<ServiceInfo> {
        self.service_info.clone()
    }

    fn handle_message(&self, room: &RoomData, msg: &Request) -> HandleMessageResult {

        let mut response = vec![];
        let mut message_response = None;

        let service = self.get_service_info();
        
        if let Some(o) = room.sim.rigid_body_set.read().unwrap().get(self.config.body) {
             match msg.function.as_str() {
                "getIntensity" => {
                    // TODO: apply some more complex function definable through some config setting?
                    let dist = ((self.config.target.to_owned() - o.translation()).norm() * self.config.multiplier) + self.config.offset;
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
            info!("Unrecognized object {}", msg.device);
        };

        service.enqueue_response_to(&msg, Ok(response.clone()));      


        if response.len() == 1 {
            return (Ok(SimpleValue::from_json(response[0].clone()).unwrap()), message_response);
        }
        (Ok(SimpleValue::from_json(serde_json::to_value(response).unwrap()).unwrap()), message_response)
    }
}