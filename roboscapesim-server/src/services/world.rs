use std::{collections::BTreeMap, time::{Instant, Duration}};

use dashmap::DashMap;
use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, MethodParam, EventDescription, Request};
use log::info;
use nalgebra::{vector, UnitQuaternion, Vector3};
use rapier3d::prelude::{AngVector, Real};
use roboscapesim_common::{UpdateMessage, VisualInfo, Shape};
use serde_json::{Number, Value};

use crate::{room::RoomData, vm::Intermediate, util::util::{num_val, bool_val, str_val}};

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
                documentation: Some("ID of created entity".to_owned()),
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
                documentation: Some("ID of created entity".to_owned()),
                r#type: vec!["string".to_owned()],
            },
        },
    );

    definition.methods.insert(
        "addEntity".to_owned(),
        MethodDescription {
            documentation: Some("Add an entity to the World".to_owned()),
            params: vec![
                MethodParam {
                    name: "type".to_owned(),
                    documentation: Some("Type of entity (block, ball, trigger, robot)".to_owned()),
                    r#type: "string".to_owned(),
                    optional: false,
                },
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
                    name: "rotation".to_owned(),
                    documentation: Some("Yaw or list of pitch, yaw, roll".to_owned()),
                    r#type: "number".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "options".to_owned(),
                    documentation: Some("Two-dimensional list of options, e.g. visualInfo, size, isKinematic".to_owned()),
                    r#type: "string".to_owned(),
                    optional: true,
                },
            ],
            returns: MethodReturns {
                documentation: Some("ID of created entity".to_owned()),
                r#type: vec!["string".to_owned()],
            },
        },
    );

    
    definition.methods.insert(
        "instantiateEntities".to_owned(),
        MethodDescription {
            documentation: Some("Add a list of Entities to the World".to_owned()),
            params: vec![
                MethodParam {
                    name: "entities".to_owned(),
                    documentation: Some("List of entity data to instantiate".to_owned()),
                    r#type: "Array".to_owned(),
                    optional: false,
                },
            ],
            returns: MethodReturns {
                documentation: Some("ID of created entities".to_owned()),
                r#type: vec!["string".to_owned(), "string".to_owned()],
            },
        },
    );

    
    definition.methods.insert(
        "listEntities".to_owned(),
        MethodDescription {
            documentation: Some("List Entities in this World".to_owned()),
            params: vec![],
            returns: MethodReturns {
                documentation: Some("IDs of Entities in World".to_owned()),
                r#type: vec!["string".to_owned(), "string".to_owned()],
            },
        },
    );

    

    definition.methods.insert(
        "removeEntity".to_owned(),
        MethodDescription {
            documentation: Some("Remove an entity from the world".to_owned()),
            params: vec![
                MethodParam {
                    name: "entity".to_owned(),
                    documentation: Some("ID of entity to remove".to_owned()),
                    r#type: "string".to_owned(),
                    optional: false,
                },
            ],
            returns: MethodReturns {
                documentation: None,
                r#type: vec![],
            },
        },
    );

    definition.methods.insert(
        "removeAllEntities".to_owned(),
        MethodDescription {
            documentation: Some("Remove all entities from the world".to_owned()),
            params: vec![],
            returns: MethodReturns {
                documentation: None,
                r#type: vec![],
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
        "reset".to_owned(),
        EventDescription { params: vec![] },
    );

    definition.events.insert(
        "userJoined".to_owned(),
        EventDescription { params: vec!["string".into()] },
    );

    definition.events.insert(
        "userLeft".to_owned(),
        EventDescription { params: vec!["string".into()] },
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

const MAX_COORD: f32 = 10000.0;

pub fn handle_world_msg(room: &mut RoomData, msg: Request) -> Result<Intermediate, String> {
    let mut response: Vec<Value> = vec![];

    info!("{:?}", msg);

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
        "removeEntity" => {
            let id = str_val(&msg.params[0]).to_owned();
            if room.objects.contains_key(&id) {
                room.remove(&id);
            }
        },
        "removeAllEntities" => {
            room.remove_all();
        },
        "clearText" => {
            RoomData::send_to_clients(&UpdateMessage::ClearText, room.sockets.iter().map(|p| p.value().clone()));
        },
        "addEntity" => {
            add_entity(None, &msg.params, room);
        },
        "instantiateEntities" => {
            if msg.params[0].is_array() {
                let objs = msg.params[0].as_array().unwrap();
                response = objs.iter().filter_map(|obj| obj.as_array().and_then(|obj| add_entity(obj[0].as_str().and_then(|s| Some(s.to_owned())), &obj.iter().skip(1).map(|o| o.to_owned()).collect(), room))).collect();
            }
        },
        "listEntities" => {
            response = room.objects.iter().map(|e| { 
                let mut kind = "box".to_owned();
                let pos = e.value().transform.position;
                let rot: (f32, f32, f32) = e.value().transform.rotation.into();
                let rot = vec![rot.0, rot.1, rot.2];
                let scale = e.value().transform.scaling;
                let scale = vec![scale.x, scale.y, scale.z];

                let mut options: Vec<Vec<Value>> = vec![
                    vec!["kinematic".into(), e.is_kinematic.to_string().into()],
                    vec!["size".into(), scale.into()],
                ];

                match &e.value().visual_info {
                    Some(VisualInfo::Color(r, g, b, shape)) => {
                        kind = shape.to_string();
                        options.push(vec!["color".into(), vec![Value::from(r * 255.0), Value::from(g * 255.0), Value::from(b * 255.0)].into()]);
                    },
                    Some(VisualInfo::Texture(t, u, v, shape)) => {
                        kind = shape.to_string();
                        options.push(vec!["texture".into(), t.clone().into()]);
                        options.push(vec!["uscale".into(), u.clone().into()]);
                        options.push(vec!["vscale".into(), v.clone().into()]);
                    },
                    Some(VisualInfo::Mesh(m)) => {
                        // TODO: Implement mesh vis info
                    },
                    Some(VisualInfo::None) => {},
                    None => {},
                }
                vec![
                    Value::from(e.key().clone()),
                    kind.into(),
                    pos.x.into(),
                    pos.y.into(),
                    pos.z.into(),
                    rot.into(),
                    options.into(),
                ].into()
            }).collect::<Vec<Value>>();
        },
        "addBlock" => {
            let x = num_val(&msg.params[0]).clamp(-MAX_COORD, MAX_COORD);
            let y = num_val(&msg.params[1]).clamp(-MAX_COORD, MAX_COORD);
            let z = num_val(&msg.params[2]).clamp(-MAX_COORD, MAX_COORD);
            let heading = num_val(&msg.params[3]);
            let name = "block".to_string() + &room.objects.len().to_string();
            let width = num_val(&msg.params[4]);
            let height = num_val(&msg.params[5]);
            let depth = num_val(&msg.params[6]);
            let kinematic = bool_val(&msg.params.get(7).unwrap_or(&serde_json::Value::Bool(false)));
            let visualinfo = msg.params.get(8).unwrap_or(&serde_json::Value::Null);

            let parsed_visualinfo = parse_visual_info(visualinfo, Shape::Box);
            info!("{:?}", visualinfo);

            let id = RoomData::add_shape(room, &name, vector![x, y, z], AngVector::new(0.0, heading, 0.0), Some(parsed_visualinfo), Some(vector![width, height, depth]), kinematic);
            response = vec![id.into()];            
        },
        "addRobot" => {
            let x = num_val(&msg.params[0]);
            let y = num_val(&msg.params[1]);
            let z = num_val(&msg.params[2]);
            let heading = num_val(&msg.params.get(3).unwrap_or(&serde_json::Value::Number(Number::from(0))));
            
            let id = RoomData::add_robot(room, vector![x, y, z], UnitQuaternion::from_axis_angle(&Vector3::y_axis(), heading), false);
            response = vec![id.into()];
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

fn add_entity(desired_name: Option<String>, params: &Vec<Value>, room: &mut RoomData) -> Option<Value> {

    if params.len() < 6 {
        return None;
    }
    // TODO use ids to replace existing entities or recreate with same id (should it keep room part consistent?)

    let entity_type = str_val(&params[0]).to_lowercase();
    let x = num_val(&params[1]).clamp(-MAX_COORD, MAX_COORD);
    let y = num_val(&params[2]).clamp(-MAX_COORD, MAX_COORD);
    let z = num_val(&params[3]).clamp(-MAX_COORD, MAX_COORD);
    let rotation = &params[4];
    let options = &params[5];

    // Parse rotation
    let rotation = match rotation {
        serde_json::Value::Number(n) => AngVector::new(0.0, n.as_f64().unwrap() as f32, 0.0),
        serde_json::Value::String(s) => AngVector::new(0.0, s.parse().unwrap_or_default(), 0.0),
        serde_json::Value::Array(a) => {
            if a.len() >= 3 {
                AngVector::new(num_val(&a[0]), num_val(&a[1]), num_val(&a[2]))
            } else if a.len() > 0 {
                AngVector::new(0.0, num_val(&a[0]), 0.0)
            } else {
                AngVector::new(0.0, 0.0, 0.0)
            }
        },
        _ => AngVector::new(0.0, 0.0, 0.0)
    };

    if options.is_array() {
        // Parse options
        let options = options.as_array().unwrap();

        let shape = match entity_type.as_str() {
            "box" | "block" | "cube" | "cuboid" => Shape::Box,
            "ball" | "sphere" | "orb" | "spheroid" => Shape::Sphere,
            _ => Shape::Box
        };

        // Transform into dict
        let options = BTreeMap::from_iter(options.iter().filter_map(|option| { 
            if option.is_array() {
                let option = option.as_array().unwrap();

                if option.len() >= 2 {
                    if option[0].is_string() {
                        return Some((str_val(&option[0]).to_lowercase(), option[1].clone()));
                    }
                }
            }

            None
        }));

        // Check for each option
        let kinematic = options.get("kinematic").and_then(|v| Some(bool_val(v))).unwrap_or(false);
        let mut size = vec![];
    
        if options.contains_key("size") {
            match &options.get("size").unwrap() {
                serde_json::Value::Number(n) => {
                    size = vec![n.as_f64().unwrap_or(1.0).clamp(0.05, 100000.0) as f32];
                },
                serde_json::Value::Array(a) =>  {
                    size = a.iter().map(|n| num_val(&n).clamp(0.05, 100000.0)).collect();
                },
                _ => {}
            }
        }
    
        let mut parsed_visualinfo: Option<VisualInfo> = None;
    
        if options.contains_key("texture") {
            let mut uscale = 1.0;
            let mut vscale = 1.0;

            if options.contains_key("uscale") {
                uscale = num_val(options.get("uscale").unwrap());
            }

            if options.contains_key("vscale") {
                vscale = num_val(options.get("vscale").unwrap());
            }

            parsed_visualinfo = Some(VisualInfo::Texture(str_val(&options.get("texture").unwrap()), uscale, vscale, shape));
        } else if options.contains_key("color") {
            // Parse color data
            parsed_visualinfo = Some(parse_visual_info(options.get("color").unwrap(), shape));
        }

        let parsed_visualinfo = parsed_visualinfo.unwrap_or(VisualInfo::Color(1.0, 1.0, 1.0, shape));
    
        let id = match entity_type.as_str() {
            "robot" => {
                Some(RoomData::add_robot(room, vector![x, y, z], UnitQuaternion::from_axis_angle(&Vector3::y_axis(), rotation.y), false))
            },
            "box" | "block" | "cube" | "cuboid" => {
                let name = "block".to_string() + &room.objects.len().to_string();
            
                if size.len() == 1 {
                    size = vec![size[0], size[0], size[0]];
                } else if size.len() == 0 {
                    size = vec![1.0, 1.0, 1.0];
                }

                Some(RoomData::add_shape(room, &name, vector![x, y, z], rotation, Some(parsed_visualinfo), Some(vector![size[0], size[1], size[2]]), kinematic))
            },
            "ball" | "sphere" | "orb" | "spheroid" => {
                let name = "ball".to_string() + &room.objects.len().to_string();

                if size.len() == 0 {
                    size = vec![1.0];
                }

                Some(RoomData::add_shape(room, &name, vector![x, y, z], rotation, Some(parsed_visualinfo), Some(vector![size[0], size[0], size[0]]), kinematic))
            },
            _ => {
                info!("Unknown entity type requested: {entity_type}");
                None
            }
        };
        if let Some(id) = id {
            return Some(id.into());
        }
    } else {
        // TODO: IoTScape error
    }
    None
}

fn parse_visual_info(visualinfo: &serde_json::Value, shape: roboscapesim_common::Shape) -> VisualInfo {
    let mut parsed_visualinfo = VisualInfo::default();

    if !visualinfo.is_null() {
        match visualinfo {
            serde_json::Value::String(s) => { 
                if s.len() > 0 {
                    if s.starts_with('#') || s.starts_with("rgb") {
                        // attempt to parse as hex/CSS color
                        let r: Result<colorsys::Rgb, _> = s.parse();

                        if let Ok(color) = r {
                            parsed_visualinfo = VisualInfo::Color(color.red() as f32, color.green() as f32, color.blue() as f32, shape);
                        } else if let Err(_) = r {
                            let r = colorsys::Rgb::from_hex_str(s);
                            if let Ok(color) = r {
                                parsed_visualinfo = VisualInfo::Color(color.red() as f32, color.green() as f32, color.blue() as f32, shape);
                            } else if let Err(_) = r {
                                info!("Failed to parse {s} as color");
                            }
                        }
                    } else {
                        // attempt to parse as color name
                        let color = color_name::Color::val().by_string(s.to_owned());

                        if let Ok(color) = color {
                            parsed_visualinfo = VisualInfo::Color(color[0] as f32 / 255.0, color[1] as f32 / 255.0, color[2] as f32 / 255.0, shape);
                        }
                    }
                }
            },
            serde_json::Value::Array(a) =>  { 
                // Color as array
                if a.len() == 3 {
                    parsed_visualinfo = VisualInfo::Color(num_val(&a[0]) as f32 / 255.0, num_val(&a[1]) as f32 / 255.0, num_val(&a[2]) as f32 / 255.0, shape);
                } else {
                    // Complex visual info, allows setting texture, shape, etc?
                }
            },
            _ => {
                info!("Received invalid visualinfo");
            }
        }
    }
    
    parsed_visualinfo
}
