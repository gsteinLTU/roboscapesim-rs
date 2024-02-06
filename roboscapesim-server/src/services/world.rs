use std::{collections::BTreeMap, f32::consts::PI, sync::atomic::Ordering};

use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, MethodParam, EventDescription, Request};
use log::{info, trace};
use nalgebra::{vector, UnitQuaternion, Vector3};
use netsblox_vm::runtime::SimpleValue;
use rapier3d::prelude::AngVector;
use roboscapesim_common::{UpdateMessage, VisualInfo, Shape};
use serde_json::{Number, Value};

use crate::{room::RoomData, util::util::{num_val, bool_val, str_val}, services::{*, proximity::ProximityConfig, lidar::DEFAULT_LIDAR_CONFIGS, waypoint::WaypointConfig}};

use super::{service_struct::{Service, ServiceType, ServiceInfo}, HandleMessageResult};

// TODO: Separate kinematic limit from dynamic entity limit
const DYNAMIC_ENTITY_LIMIT: usize = 25;
const KINEMATIC_ENTITY_LIMIT: usize = 100;
const ROBOT_LIMIT: usize = 4;

pub struct WorldService {
    pub service_info: ServiceInfo,
}

const MAX_COORD: f32 = 10000.0;

impl Service for WorldService {
    fn update(&self) -> usize {
        self.service_info.update()
    }

    fn get_service_info(&self) -> &ServiceInfo {
        &self.service_info
    }

    fn handle_message(&self, room: &RoomData, msg: &Request) -> HandleMessageResult {
        let mut response: Vec<Value> = vec![];

        trace!("{:?}", msg);

        match msg.function.as_str() {
            "reset" => {
                room.reset();
            },
            "showText" => {
                if msg.params.len() < 2 {
                    return (Ok(SimpleValue::Bool(false)), None);
                }

                let id = str_val(&msg.params[0]);
                let text = str_val(&msg.params[1]);
                let timeout = msg.params[2].as_f64();
                RoomData::send_to_clients(&UpdateMessage::DisplayText(id, text, timeout), room.sockets.iter().map(|p| p.clone().into_iter()).flatten());
            },
            "removeEntity" => {
                let id = str_val(&msg.params[0]).to_owned();
                if room.objects.contains_key(&id) {
                    room.remove(&id);
                } else if room.robots.contains_key(&id) {
                    room.remove(&id);
                    room.remove(&format!("robot_{id}"));
                }
            },
            "removeAllEntities" => {
                room.remove_all();
            },
            "clearText" => {
                RoomData::send_to_clients(&UpdateMessage::ClearText, room.sockets.iter().map(|p| p.clone().into_iter()).flatten());
            },
            "addEntity" => {
                response = vec![Self::add_entity(None, &msg.params, room).into()];
            },
            "instantiateEntities" => {
                if msg.params[0].is_array() {
                    let objs = msg.params[0].as_array().unwrap();
                    response = objs.iter().filter_map(|obj| obj.as_array().and_then(|obj| Self::add_entity(obj[0].as_str().map(|s| s.to_owned()), &obj.iter().skip(1).map(|o| o.to_owned()).collect(), room))).collect();
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
                    return (Ok(SimpleValue::Bool(false)), None);
                }

                let x = num_val(&msg.params[0]).clamp(-MAX_COORD, MAX_COORD);
                let y = num_val(&msg.params[1]).clamp(-MAX_COORD, MAX_COORD);
                let z = num_val(&msg.params[2]).clamp(-MAX_COORD, MAX_COORD);
                let heading = num_val(&msg.params[3]);

                let name = "block".to_string() + &room.next_object_id.load(Ordering::Relaxed).to_string();
                room.next_object_id.fetch_add(1, Ordering::Relaxed);
                
                let width = num_val(&msg.params[4]);
                let height = num_val(&msg.params[5]);
                let depth = num_val(&msg.params[6]);
                let kinematic = bool_val(msg.params.get(7).unwrap_or(&serde_json::Value::Bool(false)));
                let visualinfo = msg.params.get(8).unwrap_or(&serde_json::Value::Null);

                let parsed_visualinfo = if visualinfo.is_array() {
                    let mut options = visualinfo.as_array().unwrap().clone();

                    // Check for 1x2 array
                    if options.len() == 2 && options[0].is_string() {
                        options.push(serde_json::Value::Array(vec![options[0].clone(), options[1].clone()]));
                    }

                    let options = BTreeMap::from_iter(options.iter().filter_map(|option| { 
                        if option.is_array() {
                            let option = option.as_array().unwrap();
            
                            if option.len() >= 2 && option[0].is_string() {
                                return Some((str_val(&option[0]).to_lowercase(), option[1].clone()));
                            }
                        }
            
                        None
                    }));

                    Self::parse_visual_info(&options, Shape::Box).unwrap_or_default() 
                } else { 
                    Self::parse_visual_info_color(visualinfo, Shape::Box)
                };

                if (!kinematic && room.count_dynamic() >= DYNAMIC_ENTITY_LIMIT) || (kinematic && room.count_kinematic() >= KINEMATIC_ENTITY_LIMIT){
                    info!("Entity limit already reached");
                    response = vec![false.into()];
                } else {
                    let id = RoomData::add_shape(room, &name, vector![x, y, z], AngVector::new(0.0, heading, 0.0), Some(parsed_visualinfo), Some(vector![width, height, depth]), kinematic);
                    response = vec![id.into()];            
                }
            },
            "addRobot" => {
                if msg.params.len() < 3 {
                    return (Ok(SimpleValue::Bool(false)), None);
                }
                
                if room.robots.len() >= ROBOT_LIMIT {
                    info!("Robot limit already reached");
                    response = vec![false.into()];
                } else {
                    let x = num_val(&msg.params[0]);
                    let y = num_val(&msg.params[1]);
                    let z = num_val(&msg.params[2]);
                    let heading = num_val(msg.params.get(3).unwrap_or(&serde_json::Value::Number(Number::from(0)))) * PI / 180.0;
                    
                    let id = RoomData::add_robot(room, vector![x, y, z], UnitQuaternion::from_axis_angle(&Vector3::y_axis(), heading), false, None, None);
                    response = vec![id.into()];
                }
            },
            "addSensor" => {
                if msg.params.len() < 2 {
                    return (Ok(SimpleValue::Bool(false)), None);
                }

                let service_type = str_val(&msg.params[0]).to_owned().to_lowercase();
                let object = str_val(&msg.params[1]);

                // Check if object exists
                if !room.robots.contains_key(&object) && !room.objects.contains_key(&object) {
                    response = vec![false.into()];
                } else {
                    let is_robot = room.robots.contains_key(&object);
                    let options = msg.params.get(2).unwrap_or(&serde_json::Value::Null);
            
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

                    let body = if is_robot { room.robots.get(&object).unwrap().body_handle.clone() } else {  room.sim.rigid_body_labels.get(&object).unwrap().clone() };

                    response = vec![match service_type.as_str() {
                        "position" => {
                            RoomData::add_sensor::<PositionService>(room, &object, body.clone()).into()
                        },
                        "proximity" => {
                            RoomData::add_sensor::<ProximityService>(room, &object, ProximityConfig { target: targetpos.unwrap_or(vector![0.0, 0.0, 0.0]), multiplier, offset, ..Default::default() }).into()
                        },
                        "waypoint" => {
                            RoomData::add_sensor::<WaypointService>(room, &object, WaypointConfig { target: targetpos.unwrap_or(vector![0.0, 0.0, 0.0]), ..Default::default() }).into()
                        },
                        "lidar" => {
                            let default = DEFAULT_LIDAR_CONFIGS.get("default").unwrap().clone();
                            let mut config = DEFAULT_LIDAR_CONFIGS.get(&config).unwrap_or_else(|| {
                                info!("Unrecognized LIDAR config {}, using default", config);
                                &default
                            }).clone();

                            if is_robot {
                                config.offset_pos = vector![0.17,0.04,0.0];
                            }

                            config.body = body.clone();

                            RoomData::add_sensor::<LIDARService>(room, &object, config).into()
                        },
                        "entity" => {
                            RoomData::add_sensor::<EntityService>(room, &object, (body.clone(), is_robot)).into()
                        },
                        _ => {
                            info!("Unrecognized service type {}", service_type);
                            false.into()
                        }
                    }];
                }
            },
            "listTextures" => {
                response = vec![
                    "brick".into(),
                    "bricks".into(),
                    "cobble".into(),
                    "crate".into(),
                    "dirt".into(),
                    "grass".into(),
                    "gravel".into(),
                    "grid".into(),
                    "lava".into(),
                    "sand".into(),
                    "sandstone".into(),
                    "stone".into(),
                    "stone_brick".into(),
                    "tree".into(),
                    "wood".into(),
                ];
            },
            "listMeshes" => {
                response = vec![
                    "cactus_short".into(),
                    "cactus_tall".into(),
                    "chest".into(),
                    "chest_opt".into(),
                    "grass".into(),
                    "fence_simple".into(),
                    "house_type01".into(),
                    "house_type02".into(),
                    "house_type03".into(),
                    "house_type04".into(),
                    "house_type05".into(),
                    "house_type06".into(),
                    "log".into(),
                    "log_large".into(),
                    "log_stack".into(),
                    "log_stackLarge".into(),
                    "mushroom_red".into(),
                    "mushroom_redGroup".into(),
                    "mushroom_redTall".into(),
                    "mushroom_tan".into(),
                    "mushroom_tanGroup".into(),
                    "mushroom_tanTall".into(),
                    "parallax_robot".into(),
                    "sign".into(),
                    "small_buildingA".into(),
                    "small_buildingB".into(),
                    "small_buildingC".into(),
                    "small_buildingD".into(),
                    "small_buildingE".into(),
                    "small_buildingF".into(),
                    "sphere".into(),
                    "stump_round".into(),
                    "stump_square".into(),
                    "tree_blocks".into(),
                    "tree_cone".into(),
                    "tree_default".into(),
                    "tree_detailed".into(),
                    "tree_fat".into(),
                    "tree_oak".into(),
                    "tree_palmDetailedShort".into(),
                    "tree_palmDetailedTall".into(),
                    "tree_pineDefaultA".into(),
                    "tree_pineDefaultB".into(),
                    "tree_pineGroundA".into(),
                    "tree_pineGroundB".into(),
                    "tree_pineRoundA".into(),
                    "tree_pineRoundB".into(),
                    "tree_pineRoundC".into(),
                    "tree_pineRoundD".into(),
                    "tree_pineRoundE".into(),
                    "tree_pineRoundF".into(),
                    "tree_pineSmallA".into(),
                    "tree_pineSmallB".into(),
                    "tree_pineSmallC".into(),
                    "tree_pineSmallD".into(),
                    "tree_pineTallA".into(),
                    "tree_pineTallB".into(),
                    "tree_pineTallC".into(),
                    "tree_pineTallD".into(),
                    "tree_plateau".into(),
                    "tree_simple".into(),
                    "tree_small".into(),
                    "tree_tall".into(),
                    "tree_thin".into(),
                ];
            },
            f => {
                info!("Unrecognized function {}", f);
            }
        };
        
        self.service_info.enqueue_response_to(msg, Ok(response.clone()));      

        if response.len() == 1 {
            return (Ok(SimpleValue::from_json(response[0].clone()).unwrap()), None);
        }

        (Ok(SimpleValue::from_json(serde_json::to_value(response).unwrap()).unwrap()), None)
    }
}

impl WorldService {
    pub fn create(id: &str) -> Box<dyn Service> {
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
                        r#type: "string".to_owned(),
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

        definition.methods.insert(
            "listTextures".to_owned(),
            MethodDescription {
                documentation: Some("List available textures".to_owned()),
                params: vec![],
                returns: MethodReturns {
                    documentation: None,
                    r#type: vec!["string".to_owned(), "string".to_owned()],
                },
            },
        );

        definition.methods.insert(
            "listMeshes".to_owned(),
            MethodDescription {
                documentation: Some("List available meshes".to_owned()),
                params: vec![],
                returns: MethodReturns {
                    documentation: None,
                    r#type: vec!["string".to_owned(), "string".to_owned()],
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

        Box::new(WorldService {
            service_info: ServiceInfo::new(id, definition, ServiceType::World),
        }) as Box<dyn Service>
    }

    fn add_entity(_desired_name: Option<String>, params: &Vec<Value>, room: &RoomData) -> Option<Value> {

        if params.len() < 6 {
            return None;
        }

        // TODO: use ids to replace existing entities or recreate with same id (should it keep room part consistent?)

        let mut entity_type = str_val(&params[0]).to_lowercase();

        // Check limits
        if entity_type == "robot" && room.robots.len() >= ROBOT_LIMIT {
            info!("Robot limit already reached");
            return Some(Value::Bool(false));
        }

        let x = num_val(&params[1]).clamp(-MAX_COORD, MAX_COORD);
        let y = num_val(&params[2]).clamp(-MAX_COORD, MAX_COORD);
        let z = num_val(&params[3]).clamp(-MAX_COORD, MAX_COORD);
        let mut options = params[5].clone();

        // Parse rotation
        let rotation = Self::parse_rotation(&params[4]);

        if !options.is_array() {
            options = serde_json::Value::Array(vec![]);
        }

        // Parse options
        let mut options = options.as_array().unwrap().to_owned();

        // Check for 2x1 array
        if options.len() == 2 && options[0].is_string() {
            options = vec![serde_json::Value::Array(vec![options[0].clone(), options[1].clone()])];
        }

        let shape = match entity_type.as_str() {
            "box" | "block" | "cube" | "cuboid" | "trigger" => Shape::Box,
            "ball" | "sphere" | "orb" | "spheroid" => Shape::Sphere,
            "robot" => { Shape::Box },
            _ => {
                info!("Unknown entity type requested: {entity_type}");
                entity_type = "box".to_owned();
                Shape::Box
            }
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
                    size = vec![n.as_f64().unwrap_or(1.0).clamp(0.05, 1000.0) as f32].repeat(3);
                },
                serde_json::Value::String(s) => {
                    size = vec![s.parse::<f32>().unwrap_or(1.0).clamp(0.05, 1000.0)].repeat(3);
                },
                serde_json::Value::Array(a) =>  {
                    size = a.iter().map(|n| num_val(n).clamp(0.05, 1000.0)).collect();
                },
                other => {
                    info!("Invalid size option: {:?}", other);
                }
            }
        } else {
            size = vec![1.0, 1.0, 1.0];
        }

        while size.len() < 3 {
            size.push(1.0);
        }

        let parsed_visualinfo = Self::parse_visual_info(&options, shape).unwrap_or(VisualInfo::Color(1.0, 1.0, 1.0, shape));

        if entity_type != "robot" {
            if (!kinematic && room.count_dynamic() >= DYNAMIC_ENTITY_LIMIT) || ((kinematic || entity_type == "trigger") && room.count_kinematic() >= KINEMATIC_ENTITY_LIMIT) {
                info!("Entity limit already reached");
                return Some(Value::Bool(false));
            }
        }        

        // Number part of name
        let name_num =  room.next_object_id.load(Ordering::Relaxed).to_string();

        let id = match entity_type.as_str() {
            "robot" => {
                let speed_mult = options.get("speed").clone().map(num_val);
                Some(RoomData::add_robot(room, vector![x, y, z], UnitQuaternion::from_axis_angle(&Vector3::y_axis(), rotation.y), false, speed_mult, Some(size[0])))
            },
            "box" | "block" | "cube" | "cuboid" => {
                let name = "block".to_string() + &name_num;
            
                if size.len() == 1 {
                    size = vec![size[0], size[0], size[0]];
                } else if size.is_empty() {
                    size = vec![1.0, 1.0, 1.0];
                }

                Some(RoomData::add_shape(room, &name, vector![x, y, z], rotation, Some(parsed_visualinfo), Some(vector![size[0], size[1], size[2]]), kinematic))
            },
            "ball" | "sphere" | "orb" | "spheroid" => {
                let name = "ball".to_string() + &name_num;

                if size.is_empty() {
                    size = vec![1.0];
                }

                Some(RoomData::add_shape(room, &name, vector![x, y, z], rotation, Some(parsed_visualinfo), Some(vector![size[0], size[0], size[0]]), kinematic))
            },
            "trigger" => {
                let name = "trigger".to_string() + &name_num;
                Some(RoomData::add_trigger(room, &name, vector![x, y, z], rotation, Some(vector![size[0], size[1], size[2]])))
            },
            _ => {
                info!("Unknown entity type requested: {entity_type}");
                None
            }
        };

        if let Some(id) = id {
            // Increment only if successful
            room.next_object_id.fetch_add(1, Ordering::Relaxed);
            return Some(id.into());
        }
        
        None
    }

    fn parse_rotation(rotation: &Value) -> nalgebra::Matrix<f32, nalgebra::Const<3>, nalgebra::Const<1>, nalgebra::ArrayStorage<f32, 3, 1>> {
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
        rotation
    }

    fn parse_visual_info(options: &BTreeMap<String, Value>, shape: Shape) -> Option<VisualInfo> {
        if options.len() == 0 {
            return None;
        }
        
        if options.contains_key("texture") {
            let mut uscale = 1.0;
            let mut vscale = 1.0;

            if options.contains_key("uscale") {
                uscale = num_val(options.get("uscale").unwrap());
            }

            if options.contains_key("vscale") {
                vscale = num_val(options.get("vscale").unwrap());
            }

            return Some(VisualInfo::Texture(str_val(options.get("texture").unwrap()), uscale, vscale, shape));
        } else if options.contains_key("color") {
            // Parse color data
            return Some(Self::parse_visual_info_color(options.get("color").unwrap(), shape));
        } else if options.contains_key("mesh") {
            // Use mesh
            let mut mesh_name = str_val(options.get("mesh").unwrap());

            // Assume non-specified extension is glb
            if !mesh_name.contains('.') {
                mesh_name += ".glb";
            }

            return Some(VisualInfo::Mesh(mesh_name));
        }

        None
    }

    fn parse_visual_info_color(visualinfo: &serde_json::Value, shape: roboscapesim_common::Shape) -> VisualInfo {
        let mut parsed_visualinfo = VisualInfo::default_with_shape(shape);

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
                                    parsed_visualinfo = VisualInfo::Color(color.red() as f32 / 255.0, color.green() as f32 / 255.0, color.blue() as f32 / 255.0, shape);
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
                    if a.len() == 3 {
                        // Color as array
                        parsed_visualinfo = VisualInfo::Color(num_val(&a[0]) / 255.0, num_val(&a[1]) / 255.0, num_val(&a[2]) / 255.0, shape);
                    } else if a.len() == 4 {
                        // Color as array with alpha
                        parsed_visualinfo = VisualInfo::Color(num_val(&a[0]) / 255.0, num_val(&a[1]) / 255.0, num_val(&a[2]) / 255.0, shape);
                    } else if a.len() == 1 {
                        parsed_visualinfo = Self::parse_visual_info_color(&a[0], shape);
                    }
                },
                _ => {
                    info!("Received invalid visualinfo");
                }
            }
        }
        
        parsed_visualinfo
    }
}