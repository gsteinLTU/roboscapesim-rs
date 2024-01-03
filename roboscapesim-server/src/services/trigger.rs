use std::collections::BTreeMap;

use iotscape::{ServiceDefinition, IoTScapeServiceDescription, Request, EventDescription};
use log::info;
use netsblox_vm::runtime::SimpleValue;
use rapier3d::prelude::RigidBodyHandle;

use crate::room::RoomData;

use super::{service_struct::{Service, ServiceType, ServiceInfo}, HandleMessageResult};

pub struct TriggerService {
    pub service_info: ServiceInfo,
    pub rigid_body: RigidBodyHandle,
}

pub fn create_trigger_service(id: &str, rigid_body: &RigidBodyHandle) -> Box<dyn Service + Sync + Send> {
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

    definition.events.insert("triggerEnter".into(), EventDescription{
        params: vec!["object".into()],
    });

    definition.events.insert("triggerExit".into(), EventDescription{
        params: vec!["object".into()],
    });

    Box::new(TriggerService {
        service_info: ServiceInfo::new(id, definition, ServiceType::Trigger),
        rigid_body: *rigid_body,
    }) as Box<dyn Service + Sync + Send>
}

impl Service for TriggerService {
    fn update(&self) -> usize {
        self.service_info.update()
    }

    fn get_service_info(&self) -> &ServiceInfo {
        &self.service_info
    }

    fn handle_message(& self, _room: &mut RoomData, msg: &Request) -> HandleMessageResult {
        let response = vec![];

        info!("{:?}", msg);
    
        match msg.function.as_str() {
            f => {
                info!("Unrecognized function {}", f);
            }
        };

        self.get_service_info().enqueue_response_to(msg, Ok(response.clone()));      

        (Ok(SimpleValue::from_json(serde_json::to_value(response).unwrap()).unwrap()), None)
    }
}