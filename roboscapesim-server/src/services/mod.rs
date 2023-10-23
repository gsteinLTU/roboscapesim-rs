use iotscape::Response;

use crate::vm::Intermediate;

pub(crate) mod service_struct;
pub(crate) mod world;
pub(crate) mod entity;
pub(crate) mod position;
pub(crate) mod lidar;
pub(crate) mod proximity;
pub(crate) mod trigger;

/// The result of a message handler, combines the intermediate result and a possible IoTScape event message
type HandleMessageResult = (Result<Intermediate, String>, Option<Response>);