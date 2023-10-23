use std::{collections::BTreeMap, time::{Instant, Duration}};

use dashmap::DashMap;
use iotscape::{ServiceDefinition, IoTScapeServiceDescription, Request, EventDescription};
use log::info;
use rapier3d::prelude::RigidBodyHandle;

use crate::{room::RoomData, vm::Intermediate};

use super::{service_struct::{Service, ServiceType, setup_service}, HandleMessageResult};

pub fn create_trigger_service(id: &str, rigid_body: &RigidBodyHandle) -> Service {
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

    let service = setup_service(definition, ServiceType::Trigger, None);

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
        service_type: ServiceType::Trigger,
        service,
        last_announce,
        announce_period,
        attached_rigid_bodies,
    }
}

pub fn handle_trigger_message(room: &mut RoomData, msg: Request) -> HandleMessageResult {
    let mut response = vec![];

    info!("{:?}", msg);
    
    let s = room.services.get(&(msg.device.clone(), ServiceType::Entity));
    if let Some(s) = s {
        match msg.function.as_str() {
            f => {
                info!("Unrecognized function {}", f);
            }
        };

        s.value().lock().unwrap().service.lock().unwrap().enqueue_response_to(msg, Ok(response.clone()));      
    }

    (Ok(Intermediate::Json(serde_json::to_value(response).unwrap())), None)
}