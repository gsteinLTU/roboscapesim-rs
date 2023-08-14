use std::{collections::BTreeMap, time::{Instant, Duration}};

use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, Request};
use log::info;
use rapier3d::prelude::RigidBodyHandle;

use crate::room::RoomData;

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
    let announce_period = Duration::from_secs(30);

    Service {
        id: id.to_string(),
        service_type: ServiceType::PositionSensor,
        service,
        last_announce,
        announce_period,
        attached_rigid_body: Some(rigid_body.to_owned()),
    }
}

pub fn handle_position_sensor_message(room: &mut RoomData, msg: Request) {
    let s = room.services.iter().find(|serv| serv.id == msg.device && serv.service_type == ServiceType::PositionSensor);
    if let Some(s) = s {
        if s.attached_rigid_body.is_some() {
            if let Some(o) = room.sim.rigid_body_set.get(s.attached_rigid_body.unwrap()) {
                match msg.function.as_str() {
                    "getX" => {
                            s.service.lock().unwrap().enqueue_response_to(msg, Ok(vec![o.translation().x.to_string()]));                   
                    },
                    "getY" => {
                            s.service.lock().unwrap().enqueue_response_to(msg, Ok(vec![o.translation().y.to_string()]));
                    },
                    "getZ" => {
                            s.service.lock().unwrap().enqueue_response_to(msg, Ok(vec![o.translation().z.to_string()]));        
                    },
                    "getPosition" => {
                            s.service.lock().unwrap().enqueue_response_to(msg, Ok(vec![o.translation().x.to_string(), o.translation().y.to_string(), o.translation().z.to_string()]));              
                    },
                    "getHeading" => {
                            s.service.lock().unwrap().enqueue_response_to(msg, Ok(vec![o.rotation().euler_angles().1.to_string()]));                          
                    },
                    f => {
                        info!("Unrecognized function {}", f);
                    }
                };
            } else {
                info!("Unrecognized object {}", msg.device);
            }
        }
    } else {
        info!("No service found for {}", msg.device);
    }
}