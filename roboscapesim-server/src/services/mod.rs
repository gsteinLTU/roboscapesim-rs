use std::collections::BTreeMap;

use netsblox_vm::runtime::SimpleValue;

use self::service_struct::ServiceType;

pub(crate) mod service_struct;
pub(crate) mod world;
pub(crate) mod entity;
pub(crate) mod position;
pub(crate) mod lidar;
pub(crate) mod proximity;
pub(crate) mod trigger;

/// The result of a message handler, combines the intermediate result and a possible IoTScape event message
type HandleMessageResult = (Result<SimpleValue, String>, Option<((String, ServiceType), String, BTreeMap<String, String>)>);