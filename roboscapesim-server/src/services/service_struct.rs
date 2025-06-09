use std::{hash::Hash, sync::{Arc, LazyLock}, time::Duration};

use atomic_instant::AtomicInstant;
use derivative::Derivative;
use futures::FutureExt;
use iotscape::{IoTScapeServiceAsync, ServiceDefinition, Request};
use log::{error, info, trace};
use serde_json::Value;

use crate::room::RoomData;
use super::HandleMessageResult;

static SERVER: LazyLock<String> = LazyLock::new(|| 
    std::env::var("IOTSCAPE_SERVER").unwrap_or_else(|_| "52.73.65.98".to_string()));
static PORT: LazyLock<String> = LazyLock::new(|| 
    std::env::var("IOTSCAPE_PORT").unwrap_or_else(|_| "1978".to_string()));
static ANNOUNCE_ENDPOINT: LazyLock<String> = LazyLock::new(|| 
    std::env::var("IOTSCAPE_ANNOUNCE_ENDPOINT").unwrap_or_else(|_| "https://services.netsblox.org/routes/iotscape/announce".to_string()));
static RESPONSE_ENDPOINT: LazyLock<String> = LazyLock::new(|| 
    std::env::var("IOTSCAPE_RESPONSE_ENDPOINT").unwrap_or_else(|_| "https://services.netsblox.org/routes/iotscape/response".to_string()));

pub const DEFAULT_ANNOUNCE_PERIOD: Duration = Duration::from_secs(225);
const MAX_UDP_RESPONSE_SIZE: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceType {
    World, Entity, PositionSensor, LIDAR, ProximitySensor, Trigger, WaypointList, Unknown
}

impl From<String> for ServiceType {
    fn from(value: String) -> ServiceType {
        match value.as_str() {
            "RoboScapeWorld" => ServiceType::World,
            "RoboScapeEntity" => ServiceType::Entity,
            "PositionSensor" => ServiceType::PositionSensor,
            "LIDARSensor" => ServiceType::LIDAR,
            "ProximitySensor" => ServiceType::ProximitySensor,
            "RoboScapeTrigger" => ServiceType::Trigger,
            "WaypointList" => ServiceType::WaypointList,
            _ => {
                error!("Unrecognized service type {}", value);
                ServiceType::Unknown
            },
        }
    }
}

impl From<ServiceType> for &'static str {
    fn from(value: ServiceType) -> &'static str {
        match value {
            ServiceType::World => "RoboScapeWorld",
            ServiceType::Entity => "RoboScapeEntity",
            ServiceType::PositionSensor => "PositionSensor",
            ServiceType::LIDAR => "LIDARSensor",
            ServiceType::ProximitySensor => "ProximitySensor",
            ServiceType::Trigger => "RoboScapeTrigger",
            ServiceType::WaypointList => "WaypointList",
            ServiceType::Unknown => "Unknown",
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
        let params = match params {
            Ok(p) => p,
            Err(e) => return self.enqueue_udp_response(request, Err(e)),
        };

        // Check size of response
        let size: usize = params.iter().map(|v| v.to_string().len()).sum();

        // If response is too large, send via HTTP
        if size > MAX_UDP_RESPONSE_SIZE {
            self.enqueue_http_response(request, params);
        } else {
            // Otherwise, send via UDP
            self.enqueue_udp_response(request, Ok(params));
        } 
    }

    fn enqueue_udp_response(&self, request: &Request, params: Result<Vec<Value>, String>) {
        if let Err(e) = self.service
            .enqueue_response_to(request.clone(), params)
            .now_or_never()
            .unwrap_or_else(|| Err(std::io::Error::new(
                std::io::ErrorKind::Other, 
                "Could not enqueue response"
            ))) {
            error!("Could not enqueue UDP response: {}", e);
        }
    }

    fn enqueue_http_response(&self, request: &Request, mut params: Vec<Value>) {
        // Wrap in array if needed to reduce size
        if params.len() > 1 || (params.len() >= 1 && params[0].is_array()) {
            params = vec![params.into()];
        }

        let service = self.service.clone();
        let request = request.clone();
        tokio::spawn(async move {
            if let Err(e) = service
                .enqueue_response_to_http(&RESPONSE_ENDPOINT, request, Ok(params))
                .await 
            {
                error!("Could not enqueue HTTP response: {}", e);
            }
        });
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
