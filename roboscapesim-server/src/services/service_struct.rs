use std::{sync::{Arc, Mutex}, time::{Instant, Duration}};

use derivative::Derivative;
use iotscape::{IoTScapeService, ServiceDefinition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceType {
    World, Entity, PositionSensor, LIDAR, ProximitySensor
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Service {
    pub service_type: ServiceType,
    #[derivative(Debug = "ignore")]
    pub service: Arc<Mutex<IoTScapeService>>,
    pub last_announce: Instant,
    pub announce_period: Duration,
}

impl Service {
    pub fn update(&mut self) -> usize {
        self.service.lock().unwrap().poll(Some(Duration::from_millis(1)));

        // Re-announce to server regularly
        if self.last_announce.elapsed() > self.announce_period {
            self.service
                .lock()
                .unwrap()
                .announce()
                .expect("Could not announce to server");
            self.last_announce = Instant::now();
        }
        
        self.service.lock().unwrap().rx_queue.len()
    }
}

pub(crate) fn setup_service(definition: ServiceDefinition, service_type: ServiceType, override_name: Option<&str>) -> Arc<Mutex<IoTScapeService>> {
    let server = std::env::var("IOTSCAPE_SERVER").unwrap_or("52.73.65.98".to_string());
    let port = std::env::var("IOTSCAPE_PORT").unwrap_or("1975".to_string());
    let service: Arc<Mutex<IoTScapeService>> = Arc::from(Mutex::new(IoTScapeService::new(
        override_name.unwrap_or(
            match service_type {
                ServiceType::World => "RoboScapeWorld",
                ServiceType::Entity => "RoboScapeEntity",
                ServiceType::LIDAR => "LIDAR",
                ServiceType::PositionSensor => "PositionSensor",
                ServiceType::ProximitySensor => "ProximitySensor",
            }
        ),
        definition,
        (server + ":" + &port).parse().unwrap(),
    )));
    service
}