use std::{collections::BTreeMap, f32::consts::PI};

use atomic_instant::AtomicInstant;
use dashmap::DashMap;
use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, MethodParam, EventDescription, Request};
use log::info;
use nalgebra::{vector, UnitQuaternion, Vector3};
use rapier3d::prelude::AngVector;
use roboscapesim_common::{UpdateMessage, VisualInfo, Shape};
use serde_json::{Number, Value};

use crate::{room::RoomData, vm::Intermediate, util::util::{num_val, bool_val, str_val}, services::{proximity::ProximityConfig, lidar::DEFAULT_LIDAR_CONFIGS}};

use super::{service_struct::{Service, ServiceType, setup_service, DEFAULT_ANNOUNCE_PERIOD}, HandleMessageResult};

const ENTITY_LIMIT: usize = 50;
const ROBOT_LIMIT: usize = 10;

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
        "addSensor".to_owned(),
        MethodDescription {
            documentation: Some("Add a sensor to some object in the World".to_owned()),
            params: vec![
                MethodParam {
                    name: "type".to_owned(),
                    documentation: Some("Type of sensor (position, LIDAR, proximity, etc)".to_owned()),
                    r#type: "string".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "object".to_owned(),
                    documentation: Some("Object to attach service to".to_owned()),
                    r#type: "string".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "options".to_owned(),
                    // TODO: Better documentation
                    documentation: Some("Two-dimensional list of options, e.g. lidar settings".to_owned()),
                    r#type: "string".to_owned(),
                    optional: true,
                },
            ],
            returns: MethodReturns {
                documentation: Some("ID of created sensor".to_owned()),
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
        EventDescription { params: vec!["username".into()] },
    );

    definition.events.insert(
        "userLeft".to_owned(),
        EventDescription { params: vec!["username".into()] },
    );

    let service = setup_service(definition, ServiceType::World, None);

    service
        .lock()
        .unwrap()
        .announce()
        .expect("Could not announce to server");

    let last_announce = AtomicInstant::now();
    let announce_period = DEFAULT_ANNOUNCE_PERIOD;

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

pub fn handle_world_msg(room: &mut RoomData, msg: Request) -> HandleMessageResult {
    let mut response: Vec<Value> = vec![];

    info!("{:?}", msg);

    match msg.function.as_str() {
        "reset" => {
            room.reset();
        },
        "showText" => {
            if msg.params.len() < 2 {
                return (Ok(Intermediate::Json(Value::Bool(false))), None);
            }

            let id = str_val(&msg.params[0]);
            let text = str_val(&msg.params[1]);
            let timeout = msg.params[2].as_f64();
            RoomData::send_to_clients(&UpdateMessage::DisplayText(id, text, timeout), room.sockets.iter().map(|p| *p.value()));
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
            RoomData::send_to_clients(&UpdateMessage::ClearText, room.sockets.iter().map(|p| *p.value()));
        },
        "addEntity" => {
            response = vec![add_entity(None, &msg.params, room).into()];
        },
        "instantiateEntities" => {
            if msg.params[0].is_array() {
                let objs = msg.params[0].as_array().unwrap();
                response = objs.iter().filter_map(|obj| obj.as_array().and_then(|obj| add_entity(obj[0].as_str().map(|s| s.to_owned()), &obj.iter().skip(1).map(|o| o.to_owned()).collect(), room))).collect();
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
                        options.push(vec!["uscale".into(), (*u).into()]);
                        options.push(vec!["vscale".into(), (*v).into()]);
                    },
                    Some(VisualInfo::Mesh(m)) => {
                        options.push(vec!["mesh".into(), m.clone().into()]);
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
            if msg.params.len() < 7 {
                return (Ok(Intermediate::Json(Value::Bool(false))), None);
            }
            let x = num_val(&msg.params[0]).clamp(-MAX_COORD, MAX_COORD);
            let y = num_val(&msg.params[1]).clamp(-MAX_COORD, MAX_COORD);
            let z = num_val(&msg.params[2]).clamp(-MAX_COORD, MAX_COORD);
            let heading = num_val(&msg.params[3]);
            let name = "block".to_string() + &room.objects.len().to_string();
            let width = num_val(&msg.params[4]);
            let height = num_val(&msg.params[5]);
            let depth = num_val(&msg.params[6]);
            let kinematic = bool_val(msg.params.get(7).unwrap_or(&serde_json::Value::Bool(false)));
            let visualinfo = msg.params.get(8).unwrap_or(&serde_json::Value::Null);

            let parsed_visualinfo = parse_visual_info(visualinfo, Shape::Box);
            info!("{:?}", visualinfo);

            let id = RoomData::add_shape(room, &name, vector![x, y, z], AngVector::new(0.0, heading, 0.0), Some(parsed_visualinfo), Some(vector![width, height, depth]), kinematic);
            response = vec![id.into()];            
        },
        "addRobot" => {
            if msg.params.len() < 3 {
                return (Ok(Intermediate::Json(Value::Bool(false))), None);
            }
            let x = num_val(&msg.params[0]);
            let y = num_val(&msg.params[1]);
            let z = num_val(&msg.params[2]);
            let heading = num_val(msg.params.get(3).unwrap_or(&serde_json::Value::Number(Number::from(0)))) * PI / 180.0;
            
            let id = RoomData::add_robot(room, vector![x, y, z], UnitQuaternion::from_axis_angle(&Vector3::y_axis(), heading), false);
            response = vec![id.into()];
        },
        "addSensor" => {
            if msg.params.len() < 2 {
                return (Ok(Intermediate::Json(Value::Bool(false))), None);
            }

            let service_type = str_val(&msg.params[0]).to_owned().to_lowercase();
            let object = str_val(&msg.params[1]);

            // Check if object exists
            if !room.robots.contains_key(&object) && !room.objects.contains_key(&object) {
                return (Ok(Intermediate::Json(Value::Bool(false))), None);
            }


            let is_robot = room.robots.contains_key(&object);
            let options = msg.params.get(2).unwrap_or(&serde_json::Value::Null);
            let mut override_name = None;
      
            // Options for proximity sensor
            let mut targetpos = None;
            let mut multiplier = 1.0;
            let mut offset = 0.0;
            
            // Options for lidar
            let mut config = "default".to_owned();

            if options.is_array() {
                for option in options.as_array().unwrap() {
                    if option.is_array() {
                        let option = option.as_array().unwrap();
                        if option.len() >= 2 && option[0].is_string() {
                            let key = str_val(&option[0]).to_lowercase();
                            let value = &option[1];
                            match key.as_str() {
                                // "name" => {
                                //     if value.is_string() {
                                //         override_name = Some(str_val(value));
                                //     }
                                // },
                                "targetpos" => {
                                    if value.is_array() {
                                        let value = value.as_array().unwrap();
                                        if value.len() >= 3 {
                                            targetpos = Some(vector![num_val(&value[0]), num_val(&value[1]), num_val(&value[2])]);
                                        }
                                    }
                                },
                                "multiplier" => {
                                    multiplier = num_val(&value);
                                },
                                "offset" => {
                                    offset = num_val(&value);
                                },
                                "config" => {
                                    if value.is_string() {
                                        config = str_val(&value);
                                    }
                                },
                                _ => {}
                            }
                        }
                    }
                }
            }

            let body = if is_robot { room.robots.get(&object).unwrap().body_handle.clone() } else {  room.sim.lock().unwrap().rigid_body_labels.get(&object).unwrap().clone() };

            response = vec![match service_type.as_str() {
                "position" => {
                    RoomData::add_sensor(room, ServiceType::PositionSensor, &object, override_name, body).unwrap().into()
                },
                "proximity" => {
                    let result = RoomData::add_sensor(room, ServiceType::ProximitySensor, &object, override_name, body).unwrap();
                    room.proximity_configs.insert(result.clone(), ProximityConfig { target: targetpos.unwrap_or(vector![0.0, 0.0, 0.0]), multiplier, offset, ..Default::default() });
                    result.into()
                },
                "lidar" => {
                    let result = RoomData::add_sensor(room, ServiceType::LIDAR, &object, override_name, body).unwrap();
                    let default = DEFAULT_LIDAR_CONFIGS.get("default").unwrap().clone();
                    let config = DEFAULT_LIDAR_CONFIGS.get(&config).unwrap_or_else(|| {
                        info!("Unrecognized LIDAR config {}, using default", config);
                        &default
                    }).clone();
                    room.lidar_configs.insert(result.clone(), config);
                    result.into()
                },
                "entity" => {
                    RoomData::add_sensor(room, ServiceType::Entity, &object, override_name, body).unwrap().into()
                },
                _ => {
                    info!("Unrecognized service type {}", service_type);
                    false.into()
                }
            }];
        },
        f => {
            info!("Unrecognized function {}", f);
        }
    };
    
    let s = room.services.get(&(msg.device.clone(), ServiceType::World));
    if let Some(s) = s {
        s.value().service.lock().unwrap().enqueue_response_to(msg, Ok(response.clone()));      
    } else {
        info!("No service found for {}", msg.device);
    }

    if response.len() == 1 {
        return (Ok(Intermediate::Json(response[0].clone())), None);
    }

    (Ok(Intermediate::Json(serde_json::to_value(response).unwrap())), None)
}

fn add_entity(_desired_name: Option<String>, params: &Vec<Value>, room: &mut RoomData) -> Option<Value> {

    if params.len() < 6 {
        return None;
    }

    // TODO: use ids to replace existing entities or recreate with same id (should it keep room part consistent?)

    let entity_type = str_val(&params[0]).to_lowercase();

    // Check limits
    if entity_type == "robot" && room.robots.len() >= ROBOT_LIMIT {
        return Some(Value::Bool(false));
    } else if entity_type != "robot" && room.count_non_robots() >= ENTITY_LIMIT {
        return Some(Value::Bool(false));
    }

    let x = num_val(&params[1]).clamp(-MAX_COORD, MAX_COORD);
    let y = num_val(&params[2]).clamp(-MAX_COORD, MAX_COORD);
    let z = num_val(&params[3]).clamp(-MAX_COORD, MAX_COORD);
    let rotation = &params[4];
    let options = &params[5];

    // Parse rotation
    let rotation = match rotation {
        serde_json::Value::Number(n) => AngVector::new(0.0, n.as_f64().unwrap() as f32  * PI / 180.0, 0.0),
        serde_json::Value::String(s) => AngVector::new(0.0, s.parse::<f32>().unwrap_or_default()  * PI / 180.0, 0.0),
        serde_json::Value::Array(a) => {
            if a.len() >= 3 {
                AngVector::new(num_val(&a[0]) * PI / 180.0, num_val(&a[1]) * PI / 180.0, num_val(&a[2]) * PI / 180.0)
            } else if !a.is_empty() {
                AngVector::new(0.0, num_val(&a[0]) * PI / 180.0, 0.0)
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
            "box" | "block" | "cube" | "cuboid" | "trigger" => Shape::Box,
            "ball" | "sphere" | "orb" | "spheroid" => Shape::Sphere,
            _ => Shape::Box
        };

        // Transform into dict
        let options = BTreeMap::from_iter(options.iter().filter_map(|option| { 
            if option.is_array() {
                let option = option.as_array().unwrap();

                if option.len() >= 2 && option[0].is_string() {
                    return Some((str_val(&option[0]).to_lowercase(), option[1].clone()));
                }
            }

            None
        }));

        // Check for each option
        let kinematic = options.get("kinematic").map(bool_val).unwrap_or(false);
        let mut size = vec![];
    
        if options.contains_key("size") {
            match &options.get("size").unwrap() {
                serde_json::Value::Number(n) => {
                    size = vec![n.as_f64().unwrap_or(1.0).clamp(0.05, 100000.0) as f32];
                },
                serde_json::Value::Array(a) =>  {
                    size = a.iter().map(|n| num_val(n).clamp(0.05, 100000.0)).collect();
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

            parsed_visualinfo = Some(VisualInfo::Texture(str_val(options.get("texture").unwrap()), uscale, vscale, shape));
        } else if options.contains_key("color") {
            // Parse color data
            parsed_visualinfo = Some(parse_visual_info(options.get("color").unwrap(), shape));
        } else if options.contains_key("mesh") {
            // Use mesh
            parsed_visualinfo = Some(VisualInfo::Mesh(str_val(options.get("mesh").unwrap())));
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
                } else if size.is_empty() {
                    size = vec![1.0, 1.0, 1.0];
                }

                Some(RoomData::add_shape(room, &name, vector![x, y, z], rotation, Some(parsed_visualinfo), Some(vector![size[0], size[1], size[2]]), kinematic))
            },
            "ball" | "sphere" | "orb" | "spheroid" => {
                let name = "ball".to_string() + &room.objects.len().to_string();

                if size.is_empty() {
                    size = vec![1.0];
                }

                Some(RoomData::add_shape(room, &name, vector![x, y, z], rotation, Some(parsed_visualinfo), Some(vector![size[0], size[0], size[0]]), kinematic))
            },
            "trigger" => {
                let name = "trigger".to_string() + &room.objects.len().to_string();
                Some(RoomData::add_trigger(room, &name, vector![x, y, z], rotation, Some(vector![size[0], size[1], size[2]])))
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
        info!("Invalid options provided");
    }
    None
}

fn parse_visual_info(visualinfo: &serde_json::Value, shape: roboscapesim_common::Shape) -> VisualInfo {
    let mut parsed_visualinfo = VisualInfo::default();

    if !visualinfo.is_null() {
        match visualinfo {
            serde_json::Value::String(s) => { 
                if !s.is_empty() {
                    if s.starts_with('#') || s.starts_with("rgb") {
                        // attempt to parse as hex/CSS color
                        let r: Result<colorsys::Rgb, _> = s.parse();

                        if let Ok(color) = r {
                            parsed_visualinfo = VisualInfo::Color(color.red() as f32, color.green() as f32, color.blue() as f32, shape);
                        } else if r.is_err() {
                            let r = colorsys::Rgb::from_hex_str(s);
                            if let Ok(color) = r {
                                parsed_visualinfo = VisualInfo::Color(color.red() as f32, color.green() as f32, color.blue() as f32, shape);
                            } else if r.is_err() {
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
                    parsed_visualinfo = VisualInfo::Color(num_val(&a[0]) / 255.0, num_val(&a[1]) / 255.0, num_val(&a[2]) / 255.0, shape);
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
