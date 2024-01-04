use std::collections::BTreeMap;

use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, Request};
use log::info;
use nalgebra::Vector3;
use netsblox_vm::runtime::SimpleValue;
use rapier3d::prelude::Real;

use crate::room::RoomData;

use super::{service_struct::{ServiceType, Service, ServiceInfo, ServiceFactory}, HandleMessageResult};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaypointConfig {
    pub target: Vector3<Real>,
}

impl Default for WaypointConfig {
    fn default() -> Self {
        Self {
            target: Vector3::new(0.0, 0.0, 0.0),
        }
    }
}

pub struct WaypointService {
    pub service_info: ServiceInfo,
    pub config: WaypointConfig,
}

impl ServiceFactory for WaypointService {
    type Config = WaypointConfig;

    fn create(id: &str, config: Self::Config) -> Box<dyn Service> {
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
            "getNextWaypoint".to_owned(),
            MethodDescription {
                documentation: Some("Get the next waypoint to navigate to".to_owned()),
                params: vec![],
                returns: MethodReturns {
                    documentation: None,
                    r#type: vec!["number".to_owned(), "number".to_owned(), "number".to_owned()],
                },
            },
        );
        Box::new(WaypointService {
            service_info: ServiceInfo::new(id, definition, ServiceType::WaypointList),
            config,
        }) as Box<dyn Service>
    }
}

impl Service for WaypointService {
    fn update(&self) -> usize {
        self.service_info.update()
    }

    fn get_service_info(&self) -> &ServiceInfo {
        &self.service_info
    }

    fn handle_message(&self, _room: &mut RoomData, msg: &Request) -> HandleMessageResult {
        let mut response = vec![];
        let message_response = None;

        let service = self.get_service_info();           
        match msg.function.as_str() {
            "getNextWaypoint" => {
                // TODO: apply some function definable through some config setting
                let t = self.config.target.to_owned();
                response = vec![t.x.into(), t.y.into(), t.z.into()];
            },
            f => {
                info!("Unrecognized function {}", f);
            }
        };

        service.enqueue_response_to(msg, Ok(response.clone()));      


        if response.len() == 1 {
            return (Ok(SimpleValue::from_json(response[0].clone()).unwrap()), message_response);
        }
        (Ok(SimpleValue::from_json(serde_json::to_value(response).unwrap()).unwrap()), message_response)
    }
}