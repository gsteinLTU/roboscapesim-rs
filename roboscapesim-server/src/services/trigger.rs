use std::{collections::{BTreeMap, HashSet}, sync::Arc};

use iotscape::{EventDescription, IoTScapeServiceDescription, MethodDescription, MethodReturns, Request, ServiceDefinition};
use log::info;
use netsblox_vm::runtime::SimpleValue;
use rapier3d::{geometry::ColliderHandle, prelude::RigidBodyHandle};
use serde_json::Value;

use crate::room::RoomData;

use super::{service_struct::{Service, ServiceType, ServiceInfo}, HandleMessageResult};

pub struct TriggerService {
    pub service_info: Arc<ServiceInfo>,
    pub collider: ColliderHandle,
}

impl TriggerService {
    pub async fn create(id: &str, collider: &ColliderHandle) -> Box<dyn Service> {
        // Create definition struct
        let mut definition = ServiceDefinition {
            id: id.to_owned(),
            methods: BTreeMap::new(),
            events: BTreeMap::new(),
            description: IoTScapeServiceDescription {
                description: Some("Service for listening to trigger events in a RoboScape Online simulation".to_owned()),
                externalDocumentation: None,
                termsOfService: None,
                contact: Some("gstein@ltu.edu".to_owned()),
                license: None,
                version: "1".to_owned(),
            },
        };

        definition.methods.insert("entitiesInside".into(), MethodDescription {
            documentation: Some("Get a list of entities inside the trigger".into()),
            params: vec![],
            returns: MethodReturns {
                documentation: Some("List of entities inside the trigger".into()),
                r#type: vec!["string".into(), "string".into()],
            },
        });

        definition.events.insert("triggerEnter".into(), EventDescription{
            params: vec!["entity".into(), "trigger".into()],
        });

        definition.events.insert("triggerExit".into(), EventDescription{
            params: vec!["entity".into(), "trigger".into()],
        });

        Box::new(TriggerService {
            service_info: Arc::new(ServiceInfo::new(id, definition, ServiceType::Trigger).await),
            collider: *collider,
        }) as Box<dyn Service>
    }
}

impl Service for TriggerService {
    fn update(&self) {
        
    }

    fn get_service_info(&self) -> Arc<ServiceInfo> {
        self.service_info.clone()
    }

    fn handle_message(&self, room: &RoomData, msg: &Request) -> HandleMessageResult {
        let mut response: Vec<Value> = vec![];

        info!("{:?}", msg);
    
        match msg.function.as_str() {
            "entitiesInside" => {
                response = room.sim.sensors.get(&(self.service_info.id.clone(), self.collider)).unwrap().value().iter().map(|name| Value::from(name.clone())).collect();
            }
            f => {
                info!("Unrecognized function {}", f);
            }
        };

        self.get_service_info().enqueue_response_to(msg, Ok(response.clone()));      

        (Ok(SimpleValue::from_json(serde_json::to_value(response).unwrap()).unwrap()), None)
    }
}