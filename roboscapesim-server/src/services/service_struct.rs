use std::{hash::Hash, sync::{Arc, LazyLock}, time::Duration};

use atomic_instant::AtomicInstant;
use derivative::Derivative;
use futures::FutureExt;
use iotscape::{IoTScapeServiceAsync, ServiceDefinition, Request};
use log::{error, info, trace};
use serde_json::Value;

use crate::room::RoomData;
use super::HandleMessageResult;

static SERVER: LazyLock<String> = LazyLock::new(|| std::env::var("IOTSCAPE_SERVER").unwrap_or("52.73.65.98".to_string()));
static PORT: LazyLock<String> = LazyLock::new(|| std::env::var("IOTSCAPE_PORT").unwrap_or("1978".to_string()));
static ANNOUNCE_ENDPOINT: LazyLock<String> = LazyLock::new(|| std::env::var("IOTSCAPE_ANNOUNCE_ENDPOINT").unwrap_or("https://services.netsblox.org/routes/iotscape/announce".to_string()));
static RESPONSE_ENDPOINT: LazyLock<String> = LazyLock::new(|| std::env::var("IOTSCAPE_RESPONSE_ENDPOINT").unwrap_or("https://services.netsblox.org/routes/iotscape/response".to_string()));

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
    pub service: Arc<IoTScapeServiceAsync>,
    pub last_announce: AtomicInstant,
    pub announce_period: Duration,
}

impl ServiceInfo {
    pub async fn new(id: &str, definition: ServiceDefinition, service_type: ServiceType) -> Self {
        let service = Self::setup_service(definition, service_type, None);

        if let Err(e) = service
            .announce()
            .await
        {
            error!("Could not announce service: {:?}", e);
        }

        let service2 = service.clone();
        tokio::spawn(async move {
            match service2.announce_http(&ANNOUNCE_ENDPOINT).await {
                Ok(_) => {},
                Err(e) => error!("Could not announce (HTTP) service: {:?}", e),
            }
        });

        Self {
            id: id.to_owned(),
            service_type,
            service,
            last_announce: AtomicInstant::now(),
            announce_period: DEFAULT_ANNOUNCE_PERIOD,
        }
    }

    fn setup_service(definition: ServiceDefinition, service_type: ServiceType, override_name: Option<&str>) -> Arc<IoTScapeServiceAsync> {
        trace!("Connecting to IoTScape server {} on port {}", SERVER.to_owned(), PORT.to_owned());

        let service = Arc::new(IoTScapeServiceAsync::new(
            override_name.unwrap_or(service_type.into()),
            definition,
            (SERVER.to_owned() + ":" + &PORT).parse().unwrap(),
        ).now_or_never().unwrap());
        service.into()
    }
}

/// Trait for defining a service
pub trait Service: Sync + Send {
    /// Update the service, return number of messages in queue
    fn update(&self);

    /// Get the service info
    fn get_service_info(&self) -> Arc<ServiceInfo>;

    /// Handle a message
    fn handle_message(&self, room: &RoomData, msg: &Request) -> HandleMessageResult;
}

/// Trait for defining services directly creatable by user (i.e. not world or trigger)
pub trait ServiceFactory: Sync + Send {
    /// Type used for configuration of service
    type Config;

    /// Create a new instance of the service
    async fn create(id: &str, config: Self::Config) -> Box<dyn Service>;
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
        // Check size of response
        if let Ok(mut p) = params.clone() {
            let size = p.iter().map(|v| v.to_string().len()).sum::<usize>();
            if size > 500 {
                if p.len() > 1 || (p.len() >= 1 && p[0].is_array()) {
                    p = vec![p.into()];
                }
                // Send response via HTTP
                let service = self.service.clone();
                let request = request.clone();
                tokio::spawn(async move {
                    let enqueued = service.enqueue_response_to_http(&RESPONSE_ENDPOINT, request, Ok(p)).await;
                    if let Err(e) = enqueued {
                        error!("Could not enqueue (HTTP) response: {}", e);
                    }
                });
                return; 
            } 
        }
        if let Err(e) = self.service.enqueue_response_to(request.clone(), params).now_or_never().unwrap_or_else(|| Err(std::io::Error::new(std::io::ErrorKind::Other, "Could not enqueue response".to_string()))) {
            error!("Could not enqueue response: {}", e);
        }
    }
    
    /// Update the service, return number of messages in queue
    pub async fn update(&self) -> usize {
        self.service.poll().await;

        // Re-announce to server regularly
        if self.last_announce.elapsed() > self.announce_period {
            if let Err(e) = self.service
                .announce_lite()
                .await {
                error!("Could not announce service: {:?}", e);
            }
            self.last_announce.set_now();
        }
        
        self.service.rx_queue.lock().unwrap().len()
    }
}

pub const DEFAULT_ANNOUNCE_PERIOD: Duration = Duration::from_secs(225);
