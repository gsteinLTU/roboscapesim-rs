use std::{collections::BTreeMap, f32::consts::PI, sync::{atomic::Ordering, Arc}};

use futures::executor::block_on;
use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, MethodParam, EventDescription, Request};
use log::{info, trace};
use nalgebra::{vector, UnitQuaternion, Vector3};
use netsblox_vm::runtime::SimpleValue;
use rapier3d::prelude::AngVector;
use roboscapesim_common::{UpdateMessage, VisualInfo, Shape};
use serde_json::{Number, Value};

use crate::{room::{clients::ClientsManager, RoomData}, services::{lidar::DEFAULT_LIDAR_CONFIGS, proximity::ProximityConfig, waypoint::WaypointConfig, *}, util::util::{bool_val, num_val, str_val, try_num_val}};

use super::{service_struct::{Service, ServiceType, ServiceInfo}, HandleMessageResult};

mod consts;
use consts::{DYNAMIC_ENTITY_LIMIT, KINEMATIC_ENTITY_LIMIT, ROBOT_LIMIT, AVAILABLETEXTURES, AVAILABLEMESHES, MAX_COORD};

mod util;
use util::{parse_visual_info, parse_visual_info_color, parse_rotation};

mod handlers;
use handlers::{handle_add_block, handle_add_robot, handle_add_sensor, list_entities, remove_entity, show_text};

mod config;
use config::get_service_definition;

pub struct WorldService {
    pub service_info: Arc<ServiceInfo>,
}

impl Service for WorldService {
    fn update(&self) {
    }

    fn get_service_info(&self) -> Arc<ServiceInfo> {
        self.service_info.clone()
    }

    fn handle_message(&self, room: &RoomData, msg: &Request) -> HandleMessageResult {
        let mut response: Vec<Value> = vec![];

        trace!("{:?}", msg);

        match msg.function.as_str() {
            "reset" => {
                room.reset();
            },
            "showText" => {
                if let Some(value) = show_text(room, msg) {
                    return value;
                }
            },
            "removeEntity" => {
                remove_entity(room, msg);
            },
            "removeAllEntities" => {
                room.remove_all();
            },
            "clearText" => {
                ClientsManager::send_to_clients(&UpdateMessage::ClearText, room.clients_manager.sockets.iter().map(|p| p.clone().into_iter()).flatten());
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
                response = list_entities(room);
            },
            "addBlock" => {
                if msg.params.len() < 7 {
                    return (Ok(SimpleValue::Bool(false)), None);
                }

                response = handle_add_block(room, msg);
            },
            "addRobot" => {
                if msg.params.len() < 3 {
                    return (Ok(SimpleValue::Bool(false)), None);
                }

                response = handle_add_robot(room, msg);
            },
            "addSensor" => {
                if msg.params.len() < 2 {
                    return (Ok(SimpleValue::Bool(false)), None);
                }

                response = handle_add_sensor(room, msg);
            },
            "listTextures" => {
                response = AVAILABLETEXTURES.iter().map(|s| Value::from(*s)).collect::<Vec<_>>();
            },
            "listMeshes" => {
                response = AVAILABLEMESHES.iter().map(|s| Value::from(*s)).collect::<Vec<_>>();
            },
            "listUsers" => {
                response = room.clients_manager.sockets.iter().map(|kvp| Value::from(kvp.key().clone())).collect::<Vec<_>>();
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
    pub async fn create(id: &str) -> Box<dyn Service> {
        Box::new(WorldService {
            service_info: Arc::new(ServiceInfo::new(id, get_service_definition(id), ServiceType::World).await),
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
        let rotation = parse_rotation(&params[4]);

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
        let visual_only = options.get("visualonly").map(bool_val).unwrap_or(false);

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

        let parsed_visualinfo = parse_visual_info(&options, shape).unwrap_or(VisualInfo::Color(1.0, 1.0, 1.0, shape));

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

                Some(RoomData::add_shape(room, &name, vector![x, y, z], rotation, Some(parsed_visualinfo), Some(vector![size[0], size[1], size[2]]), kinematic, visual_only))
            },
            "ball" | "sphere" | "orb" | "spheroid" => {
                let name = "ball".to_string() + &name_num;

                if size.is_empty() {
                    size = vec![1.0];
                }

                Some(RoomData::add_shape(room, &name, vector![x, y, z], rotation, Some(parsed_visualinfo), Some(vector![size[0], size[0], size[0]]), kinematic, visual_only))
            },
            "trigger" => {
                let name = "trigger".to_string() + &name_num;
                Some(block_on(async { RoomData::add_trigger(room, &name, vector![x, y, z], rotation, Some(vector![size[0], size[1], size[2]])).await }))
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
}
