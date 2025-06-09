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


pub fn get_service_definition(id: &str) -> ServiceDefinition {
    let mut definition = ServiceDefinition {
        id: id.to_owned(),
        methods: BTreeMap::new(),
        events: BTreeMap::new(),
        description: IoTScapeServiceDescription {
            description: Some("Service for managing a RoboScape Online simulation".to_owned()),
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
            documentation: Some("Add robot to the World".to_owned()),
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
                documentation: Some("ID of created Entity".to_owned()),
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
                    documentation: Some("Should block be unaffected by physics".to_owned()),
                    r#type: "boolean".to_owned(),
                    optional: true,
                },
                MethodParam {
                    name: "visualInfo".to_owned(),
                    documentation: Some("Block's looks. Color or texture".to_owned()),
                    r#type: "string".to_owned(),
                    optional: true,
                },
            ],
            returns: MethodReturns {
                documentation: Some("ID of created block".to_owned()),
                r#type: vec!["string".to_owned()],
            },
        },
    );

    definition.methods.insert(
        "addEntity".to_owned(),
        MethodDescription {
            documentation: Some("Add Entity to the World".to_owned()),
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
                    documentation: Some("Yaw, or list of pitch, yaw, roll".to_owned()),
                    r#type: "string".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "options".to_owned(),
                    documentation: Some("2-D list of e.g. visualInfo, size, isKinematic".to_owned()),
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
                    documentation: Some("List of Entity data to add".to_owned()),
                    r#type: "Array".to_owned(),
                    optional: false,
                },
            ],
            returns: MethodReturns {
                documentation: Some("Created Entities' IDs".to_owned()),
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
                documentation: Some("Info of Entities in World".to_owned()),
                r#type: vec!["string".to_owned(), "string".to_owned()],
            },
        },
    );

    definition.methods.insert(
        "removeEntity".to_owned(),
        MethodDescription {
            documentation: Some("Remove an Entity from the world".to_owned()),
            params: vec![
                MethodParam {
                    name: "entity".to_owned(),
                    documentation: Some("ID of Entity to remove".to_owned()),
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
            documentation: Some("Remove all Entities from the world".to_owned()),
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
            documentation: Some("Clear messages on 3d view".to_owned()),
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
            documentation: Some("Show a message on 3d view".to_owned()),
            params: vec![
                MethodParam {
                    name: "textbox_id".to_owned(),
                    documentation: Some("ID of text box to update/create".to_owned()),
                    r#type: "string".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "text".to_owned(),
                    documentation: Some("Message text".to_owned()),
                    r#type: "string".to_owned(),
                    optional: false,
                },
                MethodParam {
                    name: "timeout".to_owned(),
                    documentation: Some("Time (in s) to show message for".to_owned()),
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

    definition.methods.insert(
        "listUsers".to_owned(),
        MethodDescription {
            documentation: Some("List users in room".to_owned()),
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
    definition
}