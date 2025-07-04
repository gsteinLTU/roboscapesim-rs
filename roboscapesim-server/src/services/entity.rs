use std::{collections::BTreeMap, f32::consts::PI, sync::Arc};

use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, MethodParam, Request};
use log::{info, trace};
use nalgebra::{vector, UnitQuaternion};
use netsblox_vm::runtime::SimpleValue;
use rapier3d::prelude::RigidBodyHandle;

use crate::{robot::physics::RobotPhysics, room::RoomData, util::util::num_val};

use super::{service_struct::{Service, ServiceType, ServiceInfo, ServiceFactory}, HandleMessageResult};

pub struct EntityService {
    pub service_info: Arc<ServiceInfo>,
    pub rigid_body: RigidBodyHandle,
    pub is_robot: bool,
}

impl Service for EntityService {
    fn handle_message(&self, room: &RoomData, msg: &Request) -> HandleMessageResult {
        let mut response = vec![];

        trace!("{:?}", msg);
        
        // TODO: Determine why quotes are being added to device name in VM
        match msg.function.as_str() {
            "reset" => {
                if let Some(mut r) = room.reseters.get_mut(msg.device.as_str()) {
                    r.value_mut().reset(room.sim.clone());
                } else {
                    info!("Unrecognized device {}", msg.device);
                }
            },
            "setPosition" => {
                let x = num_val(&msg.params[0]);
                let y = num_val(&msg.params[1]);
                let z = num_val(&msg.params[2]);

                if self.is_robot {
                    RobotPhysics::update_transform(&mut room.robots.get_mut(msg.device.as_str()).unwrap(), room.sim.clone(), Some(vector![x, y, z]), None, true);
                } else {
                    if let Some(o) = room.sim.rigid_body_set.write().unwrap().get_mut(self.rigid_body) {
                        o.set_translation(vector![x, y, z], true);
                    }
                }
            },
            "setRotation" => {
                let pitch = num_val(&msg.params[1]) * PI / 180.0;
                let yaw = num_val(&msg.params[2]) * PI / 180.0;
                let roll = num_val(&msg.params[0]) * PI / 180.0;

                if self.is_robot {
                    RobotPhysics::update_transform(&mut room.robots.get_mut(msg.device.as_str()).unwrap(), room.sim.clone(), None, Some(roboscapesim_common::Orientation::Euler(vector![roll, pitch, yaw])), true);
                } else {
                    if let Some(o) = room.sim.rigid_body_set.write().unwrap().get_mut(self.rigid_body) {
                        o.set_rotation(UnitQuaternion::from_euler_angles(roll, pitch, yaw), true);
                    }
                }
            },
            "getPosition" => {
                if let Some(o) = room.sim.rigid_body_set.write().unwrap().get_mut(self.rigid_body) {
                    response = vec![o.translation().x.into(), o.translation().y.into(), o.translation().z.into()];              
                }
            },
            "getRotation" => {
                if let Some(o) = room.sim.rigid_body_set.write().unwrap().get_mut(self.rigid_body) {
                    let r = o.rotation().euler_angles();
                    response = vec![r.2.into(), r.0.into(), r.1.into()];              
                }
            },
            f => {
                info!("Unrecognized function {}", f);
            }
        };

        self.get_service_info().enqueue_response_to(msg, Ok(response.clone()));
        (Ok(SimpleValue::from_json(serde_json::to_value(response).unwrap()).unwrap()), None)
    }

    fn update(&self) {
        
    }

    fn get_service_info(&self) -> Arc<ServiceInfo> {
        self.service_info.clone()
    }
}

impl ServiceFactory for EntityService {
    type Config = (RigidBodyHandle, bool);

    async fn create(id: &str, config: Self::Config) -> Box<dyn Service> {
        // Create definition struct
        let mut definition = ServiceDefinition {
            id: id.to_owned(),
            methods: BTreeMap::new(),
            events: BTreeMap::new(),
            description: IoTScapeServiceDescription {
                description: Some("Service for managing objects in a RoboScape Online simulation".to_owned()),
                externalDocumentation: None,
                termsOfService: None,
                contact: Some("gstein@ltu.edu".to_owned()),
                license: None,
                version: "1".to_owned(),
            },
        };
    
        // Define methods
        definition.methods.insert(
            "setPosition".to_owned(),
            MethodDescription {
                documentation: Some("Set position".to_owned()),
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
                ],
                returns: MethodReturns {
                    documentation: None,
                    r#type: vec![],
                },
            },
        );
    
        definition.methods.insert(
            "setRotation".to_owned(),
            MethodDescription {
                documentation: Some("Set rotation".to_owned()),
                params: vec![
                    MethodParam {
                        name: "pitch".to_owned(),
                        documentation: Some("X rotation".to_owned()),
                        r#type: "number".to_owned(),
                        optional: false,
                    },
                    MethodParam {
                        name: "yaw".to_owned(),
                        documentation: Some("Y rotation".to_owned()),
                        r#type: "number".to_owned(),
                        optional: false,
                    },
                    MethodParam {
                        name: "roll".to_owned(),
                        documentation: Some("Z rotation".to_owned()),
                        r#type: "number".to_owned(),
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
            "reset".to_owned(),
            MethodDescription {
                documentation: Some("Reset conditions of Entity".to_owned()),
                params: vec![],
                returns: MethodReturns {
                    documentation: None,
                    r#type: vec![],
                },
            },
        );
    
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
            "getRotation".to_owned(),
            MethodDescription {
                documentation: Some("Get Euler angle rotation of object".to_owned()),
                params: vec![],
                returns: MethodReturns {
                    documentation: None,
                    r#type: vec!["number".to_owned(), "number".to_owned(), "number".to_owned()],
                },
            },
        );
    
        Box::new(EntityService {
            service_info: Arc::new(ServiceInfo::new(id, definition, ServiceType::Entity).await),
            rigid_body: config.0,
            is_robot: config.1,
        }) as Box<dyn Service>
    }
}