use std::{sync::{Arc, Mutex}, time::Duration, hash::Hash};

use atomic_instant::AtomicInstant;
use derivative::Derivative;
use iotscape::{IoTScapeService, ServiceDefinition, Request};
use log::error;
use serde_json::Value;

use crate::room::RoomData;

use super::HandleMessageResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceType {
    World, Entity, PositionSensor, LIDAR, ProximitySensor, Trigger, WaypointList, Unknown
}

impl Into<ServiceType> for String {
    fn into(self) -> ServiceType {
        match self.as_str() {
            "RoboScapeWorld" => ServiceType::World,
            "RoboScapeEntity" => ServiceType::Entity,
            "PositionSensor" => ServiceType::PositionSensor,
            "LIDARSensor" => ServiceType::LIDAR,
            "ProximitySensor" => ServiceType::ProximitySensor,
            "RoboScapeTrigger" => ServiceType::Trigger,
            "WaypointList" => ServiceType::WaypointList,
            _ => {
                error!("Unrecognized service type {}", self);
                ServiceType::Unknown
            },
        }
    }
}

impl Into<&'static str> for ServiceType {
    fn into(self) -> &'static str {
        match self {
            Self::World => "RoboScapeWorld",
            Self::Entity => "RoboScapeEntity",
            Self::PositionSensor => "PositionSensor",
            Self::LIDAR => "LIDARSensor",
            Self::ProximitySensor => "ProximitySensor",
            Self::Trigger => "RoboScapeTrigger",
            Self::WaypointList => "WaypointList",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
/// Struct for storing service info common to all services
pub struct ServiceInfo {
    pub id: String,
    pub service_type: ServiceType,
    #[derivative(Debug = "ignore")]
    pub service: Arc<Mutex<IoTScapeService>>,
    pub last_announce: AtomicInstant,
    pub announce_period: Duration,
}

impl ServiceInfo {
    pub fn new(id: &str, definition: ServiceDefinition, service_type: ServiceType) -> Self {
        let service = setup_service(definition, service_type, None);

        service
            .lock()
            .unwrap()
            .announce()
            .expect("Could not announce to server");

        Self {
            id: id.to_owned(),
            service_type,
            service,
            last_announce: AtomicInstant::now(),
            announce_period: DEFAULT_ANNOUNCE_PERIOD,
        }
    }
}

/// Trait for defining a service
pub trait Service: Sync + Send {
    /// Update the service, return number of messages in queue
    fn update(&self) -> usize;

    /// Get the service info
    fn get_service_info(&self) -> &ServiceInfo;

    /// Handle a message
    fn handle_message(&self, room: &mut RoomData, msg: &Request) -> HandleMessageResult;
}

pub trait ServiceFactory: Sync + Send {
    type Config;

    fn create(id: &str, config: Self::Config) -> Box<dyn Service>;
}

impl Hash for ServiceInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.service_type.hash(state);
    }
}

impl PartialEq for ServiceInfo {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.service_type == other.service_type
    }
}

impl ServiceInfo {
    /// Enqueue a response to a request
    pub fn enqueue_response_to(&self, request: &Request, params: Result<Vec<Value>, String>) {
        if let Err(e) = self.service.lock().unwrap().enqueue_response_to(request.clone(), params) {
            error!("Could not enqueue response: {}", e);
        }
    }
    
    /// Update the service, return number of messages in queue
    pub fn update(&self) -> usize {
        let iotscape_service = &mut self.service.lock().unwrap();
        iotscape_service.poll(Some(Duration::from_millis(1)));

        // Re-announce to server regularly
        if self.last_announce.elapsed() > self.announce_period {
            iotscape_service
                .announce()
                .expect("Could not announce to server");
            self.last_announce.set_now();
        }
        
        iotscape_service.rx_queue.len()
    }
}

pub(crate) fn setup_service(definition: ServiceDefinition, service_type: ServiceType, override_name: Option<&str>) -> Arc<Mutex<IoTScapeService>> {
    let server = std::env::var("IOTSCAPE_SERVER").unwrap_or("52.73.65.98".to_string());
    let port = std::env::var("IOTSCAPE_PORT").unwrap_or("1978".to_string());
    let service: Arc<Mutex<IoTScapeService>> = Arc::from(Mutex::new(IoTScapeService::new(
        override_name.unwrap_or(service_type.into()),
        definition,
        (server + ":" + &port).parse().unwrap(),
    )));
    service
}

pub const DEFAULT_ANNOUNCE_PERIOD: Duration = Duration::from_secs(225);
