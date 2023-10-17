use std::{collections::BTreeMap, time::{Instant, Duration}};

use dashmap::DashMap;
use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, Request};
use log::info;
use rapier3d::prelude::RigidBodyHandle;

use crate::{room::RoomData, vm::Intermediate};

use super::service_struct::{setup_service, ServiceType, Service};


pub fn create_position_service(id: &str, rigid_body: &RigidBodyHandle) -> Service {
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
        "getX".to_owned(),
        MethodDescription {
            documentation: Some("Get X coordinate position of object".to_owned()),
            params: vec![],
            returns: MethodReturns {
                documentation: None,
                r#type: vec!["number".to_owned()],
            },
        },
    );


    definition.methods.insert(
        "getY".to_owned(),
        MethodDescription {
            documentation: Some("Get Y coordinate position of object".to_owned()),
            params: vec![],
            returns: MethodReturns {
                documentation: None,
                r#type: vec!["number".to_owned()],
            },
        },
    );


    definition.methods.insert(
        "getZ".to_owned(),
        MethodDescription {
            documentation: Some("Get Z coordinate position of object".to_owned()),
            params: vec![],
            returns: MethodReturns {
                documentation: None,
                r#type: vec!["number".to_owned()],
            },
        },
    );

    definition.methods.insert(
        "getHeading".to_owned(),
        MethodDescription {
            documentation: Some("Get heading direction (yaw) of object".to_owned()),
            params: vec![],
            returns: MethodReturns {
                documentation: None,
                r#type: vec!["number".to_owned()],
            },
        },
    );
    
    let service = setup_service(definition, ServiceType::PositionSensor, None);

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
        service_type: ServiceType::PositionSensor,
        service,
        last_announce,
        announce_period,
        attached_rigid_bodies,
    }
}

pub fn handle_position_sensor_message(room: &mut RoomData, msg: Request) -> Result<Intermediate, String>  {
    let mut response = vec![];
    
    let s = room.services.get(&(msg.device.clone(), ServiceType::PositionSensor));
    if let Some(s) = s {
        if let Some(body) = s.value().lock().unwrap().attached_rigid_bodies.get("main") {
            let simulation = &mut room.sim.lock().unwrap();

            if let Some(o) = simulation.rigid_body_set.lock().unwrap().get(*body) {
                match msg.function.as_str() {
                    "getX" => {
                            response = vec![o.translation().x.into()];
                    },
                    "getY" => {
                            response = vec![o.translation().y.into()];
                    },
                    "getZ" => {
                            response = vec![o.translation().z.into()];
                    },
                    "getPosition" => {
                            response = vec![o.translation().x.into(), o.translation().y.into(), o.translation().z.into()];
                    },
                    "getHeading" => {
                            response = vec![o.rotation().euler_angles().1.into()];
                    },
                    f => {
                        info!("Unrecognized function {}", f);
                    }
                };
            } else {
                info!("Unrecognized object {}", msg.device);
            };
        }
        
        s.value().lock().unwrap().service.lock().unwrap().enqueue_response_to(msg, Ok(response.clone()));      

    } else {
        info!("No service found for {}", msg.device);
    }

    Ok(Intermediate::Json(serde_json::to_value(response).unwrap()))
}