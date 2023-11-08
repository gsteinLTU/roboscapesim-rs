use std::collections::BTreeMap;

use atomic_instant::AtomicInstant;
use dashmap::DashMap;
use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, Request};
use log::info;
use nalgebra::Vector3;
use netsblox_vm::runtime::SimpleValue;
use rapier3d::prelude::{RigidBodyHandle, Real};

use crate::room::RoomData;

use super::{service_struct::{setup_service, ServiceType, Service, DEFAULT_ANNOUNCE_PERIOD}, HandleMessageResult};

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

pub fn create_waypoint_service(id: &str, rigid_body: &RigidBodyHandle) -> Service {
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
    
    let service = setup_service(definition, ServiceType::WaypointList, None);

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
        service_type: ServiceType::WaypointList,
        service,
        last_announce,
        announce_period,
        attached_rigid_bodies,
    }
}

pub fn handle_waypoint_message(room: &mut RoomData, msg: Request) -> HandleMessageResult {
    let mut response = vec![];
    let message_response = None;

    let s = room.services.get(&(msg.device.clone(), ServiceType::WaypointList));
    if let Some(s) = s {
        let service = s.value();            
        if let Some(t) = room.waypoint_configs.get(&msg.device) {
            match msg.function.as_str() {
                "getNextWaypoint" => {
                    // TODO: apply some function definable through some config setting
                    let t = t.target.to_owned();
                    response = vec![t.x.into(), t.y.into(), t.z.into()];
                },
                f => {
                    info!("Unrecognized function {}", f);
                }
            };
        } else {
            info!("No target defined for {}", msg.device);
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