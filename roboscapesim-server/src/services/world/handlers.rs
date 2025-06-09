use std::{collections::BTreeMap, f32::consts::PI, sync::{atomic::Ordering, Arc}};

use futures::executor::block_on;
use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, MethodParam, EventDescription, Request};
use log::{info, trace};
use nalgebra::{vector, UnitQuaternion, Vector3};
use netsblox_vm::runtime::SimpleValue;
use rapier3d::prelude::AngVector;
use roboscapesim_common::{UpdateMessage, VisualInfo, Shape};
use serde_json::{Number, Value};

use crate::{room::{clients::ClientsManager, RoomData}, services::{lidar::DEFAULT_LIDAR_CONFIGS, proximity::ProximityConfig, waypoint::WaypointConfig, world::{consts::{DYNAMIC_ENTITY_LIMIT, KINEMATIC_ENTITY_LIMIT, MAX_COORD, ROBOT_LIMIT}, util::{parse_visual_info, parse_visual_info_color}}, EntityService, LIDARService, PositionService, ProximityService, ServiceType, WaypointService}, util::util::{bool_val, num_val, str_val, try_num_val}};


pub fn handle_add_sensor(room: &RoomData, msg: &Request) -> Vec<Value> {
    let service_type = str_val(&msg.params[0]).to_owned().to_lowercase();
    let object = str_val(&msg.params[1]);

    // Check if object exists
    if !room.robots.contains_key(&object) && !room.objects.contains_key(&object) {
        info!("Object {} not found", object);
        return vec![false.into()];
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

        let body = if is_robot { room.robots.get(&object).unwrap().physics.body_handle.clone() } else {  room.sim.rigid_body_labels.get(&object).unwrap().clone() };

        trace!("Creating sensor of type {} for object {} with options {:?}", service_type, object, options);
        vec![
            block_on(async move { 
            match service_type.as_str() {
            "position" => {
                RoomData::add_sensor::<PositionService>(room, &object, body.clone()).await.into()
            },
            "proximity" => {
                RoomData::add_sensor::<ProximityService>(room, &object, ProximityConfig { target: targetpos.unwrap_or(vector![0.0, 0.0, 0.0]), multiplier, offset, body: body.clone(), ..Default::default() }).await.into()
            },
            "waypoint" => {
                RoomData::add_sensor::<WaypointService>(room, &object, WaypointConfig { target: targetpos.unwrap_or(vector![0.0, 0.0, 0.0]), ..Default::default() }).await.into()
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

                RoomData::add_sensor::<LIDARService>(room, &object, config).await.into()
            },
            "entity" => {
                RoomData::add_sensor::<EntityService>(room, &object, (body.clone(), is_robot)).await.into()
            },
            _ => {
                info!("Unrecognized service type {}", service_type);
                false.into()
            }
        }})]
    }
}

pub fn handle_add_robot(room: &RoomData, msg: &Request) -> Vec<Value> {
    if room.robots.len() >= ROBOT_LIMIT {
        info!("Robot limit already reached");
        vec![false.into()]
    } else {
        let x = num_val(&msg.params[0]);
        let y = num_val(&msg.params[1]);
        let z = num_val(&msg.params[2]);
        let heading = num_val(msg.params.get(3).unwrap_or(&serde_json::Value::Number(Number::from(0)))) * PI / 180.0;
    
        let id = RoomData::add_robot(room, vector![x, y, z], UnitQuaternion::from_axis_angle(&Vector3::y_axis(), heading), false, None, None);
        vec![id.into()]
    }
}

pub fn handle_add_block(room: &RoomData, msg: &Request) -> Vec<Value> {
    {
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

            parse_visual_info(&options, Shape::Box).unwrap_or_default() 
        } else { 
            parse_visual_info_color(visualinfo, Shape::Box)
        };

        if (!kinematic && room.count_dynamic() >= DYNAMIC_ENTITY_LIMIT) || (kinematic && room.count_kinematic() >= KINEMATIC_ENTITY_LIMIT) {
            info!("Entity limit already reached");
            vec![false.into()]
        } else {
            let id = RoomData::add_shape(room, &name, vector![x, y, z], AngVector::new(0.0, heading, 0.0), Some(parsed_visualinfo), Some(vector![width, height, depth]), kinematic, false);
            vec![id.into()]
        }
    }
}

pub fn list_entities(room: &RoomData) -> Vec<Value> {
    room.objects.iter().map(|e| { 
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
    }).collect::<Vec<Value>>()
}

pub fn remove_entity(room: &RoomData, msg: &Request) {
    let id = str_val(&msg.params[0]).to_owned();
    if room.objects.contains_key(&id) {
        room.remove(&id);
    } else if room.robots.contains_key(&id) {
        room.remove(&id);
        room.remove(&format!("robot_{id}"));
    }
}

pub fn show_text(room: &RoomData, msg: &Request) -> Option<(Result<SimpleValue, String>, Option<((String, ServiceType), String, BTreeMap<String, String>)>)> {
    if msg.params.len() < 2 {
        return Some((Ok(SimpleValue::Bool(false)), None));
    }
    let id = str_val(&msg.params[0]);
    let text = str_val(&msg.params[1]);
    let timeout = if msg.params.len() > 2 { try_num_val(&msg.params[2]).ok().map(|t| t as f64) } else { None };
    ClientsManager::send_to_clients(&UpdateMessage::DisplayText(id, text, timeout), room.clients_manager.sockets.iter().map(|p| p.clone().into_iter()).flatten());

    None
}