use std::{collections::BTreeMap, time::{Instant, Duration}};

use dashmap::DashMap;
use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, MethodParam, EventDescription, Request};
use log::info;
use nalgebra::{vector, UnitQuaternion, Vector3};
use rapier3d::prelude::AngVector;
use roboscapesim_common::UpdateMessage;

use crate::room::RoomData;

use super::service_struct::{Service, ServiceType, setup_service};

pub fn create_world_service(id: &str) -> Service {
    // Create definition struct
    let mut definition = ServiceDefinition {
        id: id.to_owned(),
        methods: BTreeMap::new(),
        events: BTreeMap::new(),
        description: IoTScapeServiceDescription {
            description: Some("Service for managing the RoboScape Online simulation".to_owned()),
            externalDocumentation: None,
            termsOfService: None,
            contact: Some("gstein@ltu.edu".to_owned()),
            license: None,
            version: "1".to_owned(),
        },
    };

    // Define methods
    definition.methods.insert(
        "addRobot".to_owned(),
        MethodDescription {
            documentation: Some("Add a robot to the World".to_owned()),
            params: vec![
                MethodParam {
                    name: "x".to_owned(),
                    documentation: Some("X position".to_owned()),
                    r#type: "number".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "y".to_owned(),
                    documentation: Some("Y position".to_owned()),
                    r#type: "number".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "z".to_owned(),
                    documentation: Some("Z position".to_owned()),
                    r#type: "number".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "heading".to_owned(),
                    documentation: Some("Direction".to_owned()),
                    r#type: "number".to_owned(),
                    optional: false,
                },
            ],
            returns: MethodReturns {
                documentation: None,
                r#type: vec!["string".to_owned()],
            },
        },
    );

    definition.methods.insert(
        "addBlock".to_owned(),
        MethodDescription {
            documentation: Some("Add a block to the World".to_owned()),
            params: vec![
                MethodParam {
                    name: "x".to_owned(),
                    documentation: Some("X position".to_owned()),
                    r#type: "number".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "y".to_owned(),
                    documentation: Some("Y position".to_owned()),
                    r#type: "number".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "z".to_owned(),
                    documentation: Some("Z position".to_owned()),
                    r#type: "number".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "heading".to_owned(),
                    documentation: Some("Direction".to_owned()),
                    r#type: "number".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "width".to_owned(),
                    documentation: Some("X-axis size".to_owned()),
                    r#type: "number".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "height".to_owned(),
                    documentation: Some("Y-axis size".to_owned()),
                    r#type: "number".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "depth".to_owned(),
                    documentation: Some("Z-axis size".to_owned()),
                    r#type: "number".to_owned(),
                    optional: false,
                },
            ],
            returns: MethodReturns {
                documentation: None,
                r#type: vec!["string".to_owned()],
            },
        },
    );

    definition.methods.insert(
        "reset".to_owned(),
        MethodDescription {
            documentation: Some("Reset conditions of World".to_owned()),
            params: vec![],
            returns: MethodReturns {
                documentation: None,
                r#type: vec![],
            },
        },
    );

    definition.methods.insert(
        "clearText".to_owned(),
        MethodDescription {
            documentation: Some("Clear text messages on the client display".to_owned()),
            params: vec![],
            returns: MethodReturns {
                documentation: None,
                r#type: vec![],
            },
        },
    );

    definition.methods.insert(
        "showText".to_owned(),
        MethodDescription {
            documentation: Some("Show a text message on the client displays".to_owned()),
            params: vec![
                MethodParam {
                    name: "textbox_id".to_owned(),
                    documentation: Some("ID of text box to update/create".to_owned()),
                    r#type: "string".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "text".to_owned(),
                    documentation: Some("Text to display".to_owned()),
                    r#type: "string".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "timeout".to_owned(),
                    documentation: Some("Time (in s) to keep message around for".to_owned()),
                    r#type: "number".to_owned(),
                    optional: true,
                },
            ],
            returns: MethodReturns {
                documentation: None,
                r#type: vec![],
            },
        },
    );



    definition.events.insert(
        "userJoined".to_owned(),
        EventDescription { params: vec![] },
    );

    definition.events.insert(
        "userLeft".to_owned(),
        EventDescription { params: vec![] },
    );

    let service = setup_service(definition, ServiceType::World, None);

    service
        .lock()
        .unwrap()
        .announce()
        .expect("Could not announce to server");

    let last_announce = Instant::now();
    let announce_period = Duration::from_secs(30);

    Service {
        id: id.to_string(),
        service_type: ServiceType::World,
        service,
        last_announce,
        announce_period,
        attached_rigid_bodies: DashMap::new(),
    }
}

pub async fn handle_world_msg(room: &mut RoomData, msg: Request) {
    match msg.function.as_str() {
        "reset" => {
            room.reset();
        },
        "showText" => {
            let id = msg.params[0].as_str().unwrap().to_owned();
            let text = msg.params[1].as_str().unwrap().to_owned();
            let timeout = msg.params[2].as_f64();
            RoomData::send_to_clients(&UpdateMessage::DisplayText(id, text, timeout), room.sockets.iter().map(|p| p.value().clone())).await;
        },
        "clearText" => {
            RoomData::send_to_clients(&UpdateMessage::ClearText, room.sockets.iter().map(|p| p.value().clone())).await;
        },
        "addBlock" => {
            let x = msg.params[0].as_f64().unwrap() as f32;
            let y = msg.params[1].as_f64().unwrap() as f32;
            let z = msg.params[2].as_f64().unwrap() as f32;
            let heading = msg.params[3].as_f64().unwrap() as f32;
            let name = "block".to_string() + &room.objects.len().to_string();
            let width = msg.params[4].as_f64().unwrap() as f32;
            let height = msg.params[5].as_f64().unwrap() as f32;
            let depth = msg.params[6].as_f64().unwrap() as f32;
            RoomData::add_shape(room, &name, vector![x, y, z], AngVector::new(0.0, heading, 0.0), None, Some(vector![width, height, depth]), false);
            let s = room.services.iter().find(|serv| serv.id == msg.device && serv.service_type == ServiceType::PositionSensor);
            if let Some(s) = s {
                s.service.lock().unwrap().enqueue_response_to(msg, Ok(vec![name]));      
            }
        },
        "addRobot" => {
            let x = msg.params[0].as_f64().unwrap() as f32;
            let y = msg.params[1].as_f64().unwrap() as f32;
            let z = msg.params[2].as_f64().unwrap() as f32;
            let heading = msg.params[3].as_f64().unwrap() as f32;

            let id = RoomData::add_robot(room, vector![x, y, z], UnitQuaternion::from_axis_angle(&Vector3::y_axis(), heading), false);
            let s = room.services.iter().find(|serv| serv.id == msg.device && serv.service_type == ServiceType::PositionSensor);
            if let Some(s) = s {
                s.service.lock().unwrap().enqueue_response_to(msg, Ok(vec![id]));      
            }
        },
        f => {
            info!("Unrecognized function {}", f);
        }
    };
}