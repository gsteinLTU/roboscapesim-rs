use std::{collections::BTreeMap, f32::consts::PI};

use iotscape::{ServiceDefinition, IoTScapeServiceDescription, MethodDescription, MethodReturns, Request};
use log::info;
use nalgebra::Vector3;
use netsblox_vm::runtime::SimpleValue;
use rapier3d::prelude::{RigidBodyHandle, Real};

use crate::room::RoomData;

use super::{service_struct::{ServiceType, Service, ServiceInfo, ServiceFactory}, HandleMessageResult};

pub struct PositionService {
    pub service_info: ServiceInfo,
    pub rigid_body: RigidBodyHandle,
}

impl ServiceFactory for PositionService {
    type Config = RigidBodyHandle;

    fn create(id: &str, config: Self::Config) -> Box<dyn Service> {
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

        Box::new(PositionService {
            service_info: ServiceInfo::new(id, definition, ServiceType::PositionSensor),
            rigid_body: config,
        }) as Box<dyn Service>
    }
}

impl Service for PositionService {
    fn update(&self) -> usize {
        self.service_info.update()
    }

    fn get_service_info(&self) -> &ServiceInfo {
        &self.service_info
    }

    fn handle_message(&self, room: &RoomData, msg: &Request) -> HandleMessageResult {
        let mut response = vec![];
        
        let msg = msg;
        // TODO: figure out why this is necessary for VM requests to PositionSensor

        if let Some(o) = room.sim.rigid_body_set.read().unwrap().get(self.rigid_body) {
            match msg.function.as_str() {
                "getX" => {
                        response.push(o.translation().x.into());
                },
                "getY" => {
                        response.push(o.translation().y.into());
                },
                "getZ" => {
                        response.push(o.translation().z.into());
                },
                "getPosition" => {
                        response = vec![o.translation().x.into(), o.translation().y.into(), o.translation().z.into()];
                },
                "getHeading" => {
                        let q = o.position().rotation;
                        let v1 = q.transform_vector(&Vector3::<Real>::x_axis());
                        let mut angle = v1.dot(&Vector3::<Real>::x_axis()).acos();
                        let cross = v1.cross(&Vector3::<Real>::x_axis());
                        if Vector3::<Real>::y_axis().dot(&cross) < 0.0 {
                            angle = -angle;
                        }
                        angle = angle * 180.0 / PI;

                        if angle < -180.0 {
                            angle = angle + 360.0;
                        }

                        response = vec![angle.into()];
                },
                f => {
                    info!("Unrecognized function {}", f);
                }
            };
        } else {
            info!("Unrecognized object {}", msg.device);
        };

        self.get_service_info().enqueue_response_to(&msg, Ok(response.clone()));

        if response.len() == 1 {
            return (Ok(SimpleValue::from_json(response[0].clone()).unwrap()), None);
        }
        (Ok(SimpleValue::from_json(serde_json::to_value(response).unwrap()).unwrap()), None)
    }
}
