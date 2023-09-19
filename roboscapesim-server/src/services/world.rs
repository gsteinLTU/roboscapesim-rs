use std::{collections::BTreeMap, time::{Instant, Duration}};

use dashmap::DashMap;
use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, MethodParam, EventDescription, Request};
use log::info;
use nalgebra::{vector, UnitQuaternion, Vector3};
use rapier3d::prelude::AngVector;
use roboscapesim_common::{UpdateMessage, VisualInfo};
use serde_json::{json, Number};

use crate::{room::RoomData, vm::Intermediate, util::util::{num_val, bool_val}};

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
                MethodParam {
                    name: "kinematic".to_owned(),
                    documentation: Some("Should the block be unaffected by physics".to_owned()),
                    r#type: "boolean".to_owned(),
                    optional: true,
                },
                MethodParam {
                    name: "visualInfo".to_owned(),
                    documentation: Some("Visual appearance of the object, hex color or texture".to_owned()),
                    r#type: "string".to_owned(),
                    optional: true,
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

pub fn handle_world_msg(room: &mut RoomData, msg: Request) -> Result<Intermediate, String> {
    let mut response = vec![];

    match msg.function.as_str() {
        "reset" => {
            room.reset();
        },
        "showText" => {
            let id = msg.params[0].as_str().unwrap().to_owned();
            let text = msg.params[1].as_str().unwrap().to_owned();
            let timeout = msg.params[2].as_f64();
            RoomData::send_to_clients(&UpdateMessage::DisplayText(id, text, timeout), room.sockets.iter().map(|p| p.value().clone()));
        },
        "clearText" => {
            RoomData::send_to_clients(&UpdateMessage::ClearText, room.sockets.iter().map(|p| p.value().clone()));
        },
        "addBlock" => {
            let x = num_val(&msg.params[0]);
            let y = num_val(&msg.params[1]);
            let z = num_val(&msg.params[2]);
            let heading = num_val(&msg.params[3]);
            let name = "block".to_string() + &room.objects.len().to_string();
            let width = num_val(&msg.params[4]);
            let height = num_val(&msg.params[5]);
            let depth = num_val(&msg.params[6]);
            let kinematic = bool_val(&msg.params.get(7).unwrap_or(&serde_json::Value::Bool(false)));
            let visualinfo = msg.params.get(8).unwrap_or(&serde_json::Value::Null);

            let mut parsed_visualinfo = VisualInfo::default();

            if !visualinfo.is_null() {
                match visualinfo {
                    serde_json::Value::String(s) => { 
                        if s.len() > 0 {
                            if s.starts_with('#') || s.starts_with("rgb") {
                                // attempt to parse as hex/CSS color
                                let r: Result<colorsys::Rgb, _> = s.parse();

                                if let Ok(color) = r {
                                    parsed_visualinfo = VisualInfo::Color(color.red() as f32, color.green() as f32, color.blue() as f32, roboscapesim_common::Shape::Box);
                                } else if let Err(e) = r {
                                    let r = colorsys::Rgb::from_hex_str(s);
                                    if let Ok(color) = r {
                                        parsed_visualinfo = VisualInfo::Color(color.red() as f32, color.green() as f32, color.blue() as f32, roboscapesim_common::Shape::Box);
                                    } else if let Err(e) = r {
                                        info!("Failed to parse {s} as color");
                                    }
                                }
                            } else {
                                // attempt to parse as color name
                                let color = color_name::Color::val().by_string(s.to_owned());

                                if let Ok(color) = color {
                                    parsed_visualinfo = VisualInfo::Color(color[0] as f32 / 255.0 , color[1] as f32 / 255.0 , color[2] as f32 / 255.0 , roboscapesim_common::Shape::Box);
                                }
                            }
                        }
                    },
                    serde_json::Value::Array(a) =>  { 
                        // Complex visual info, allows setting texture, shape, etc
                    },
                    _ => {
                        info!("Received invalid visualinfo");
                    }
                }
            }
            info!("{:?}", visualinfo);

            let id = RoomData::add_shape(room, &name, vector![x, y, z], AngVector::new(0.0, heading, 0.0), Some(parsed_visualinfo), Some(vector![width, height, depth]), kinematic);
            response = vec![id];            
        },
        "addRobot" => {
            let x = num_val(&msg.params[0]);
            let y = num_val(&msg.params[1]);
            let z = num_val(&msg.params[2]);
            let heading = num_val(&msg.params.get(3).unwrap_or(&serde_json::Value::Number(Number::from(0))));
            
            let id = RoomData::add_robot(room, vector![x, y, z], UnitQuaternion::from_axis_angle(&Vector3::y_axis(), heading), false);
            response = vec![id];
        },
        f => {
            info!("Unrecognized function {}", f);

        }
    };
    
    let lock = &room.services.lock().unwrap();
    let s = lock.iter().find(|serv| serv.id == msg.device && serv.service_type == ServiceType::World);
    if let Some(s) = s {
        s.service.lock().unwrap().enqueue_response_to(msg, Ok(response.clone()));      
    }

    Ok(Intermediate::Json(serde_json::to_value(response).unwrap()))
}
