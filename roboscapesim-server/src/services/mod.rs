#![allow(unused_imports)]

use std::collections::BTreeMap;

use netsblox_vm::runtime::SimpleValue;

pub(crate) mod service_struct;
pub(crate) mod world;
pub(crate) mod entity;
pub(crate) mod position;
pub(crate) mod lidar;
pub(crate) mod proximity;
pub(crate) mod trigger;
pub(crate) mod waypoint;

// Re-export services
pub use self::entity::EntityService;
pub use self::position::PositionService;
pub use self::lidar::LIDARService;
pub use self::proximity::ProximityService;
pub use self::trigger::TriggerService;
pub use self::waypoint::WaypointService;
pub use self::world::WorldService;

// Re-export service types
pub use self::service_struct::Service;
pub use self::service_struct::ServiceFactory;
pub use self::service_struct::ServiceInfo;
pub use self::service_struct::ServiceType;

/// The result of a message handler, combines the intermediate result and a possible IoTScape event message
type HandleMessageResult = (Result<SimpleValue, String>, Option<((String, ServiceType), String, BTreeMap<String, String>)>);
