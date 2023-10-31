use std::{sync::{Arc, Mutex}, time::Duration, hash::Hash};

use atomic_instant::AtomicInstant;
use dashmap::DashMap;
use derivative::Derivative;
use iotscape::{IoTScapeService, ServiceDefinition};
use netsblox_vm::runtime::System;
use rapier3d::prelude::RigidBodyHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceType {
    World, Entity, PositionSensor, LIDAR, ProximitySensor, Trigger
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Service {
    pub id: String,
    pub service_type: ServiceType,
    #[derivative(Debug = "ignore")]
    pub service: Arc<Mutex<IoTScapeService>>,
    pub last_announce: AtomicInstant,
    pub announce_period: Duration,
    pub attached_rigid_bodies: DashMap<String, RigidBodyHandle>,
}

impl Hash for Service {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.service_type.hash(state);
    }
}

impl PartialEq for Service {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.service_type == other.service_type
    }
}

impl Service {

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
        override_name.unwrap_or(
            match service_type {
                ServiceType::World => "RoboScapeWorld",
                ServiceType::Entity => "RoboScapeEntity",
                ServiceType::LIDAR => "LIDARSensor",
                ServiceType::PositionSensor => "PositionSensor",
                ServiceType::ProximitySensor => "ProximitySensor",
                ServiceType::Trigger => "RoboScapeTrigger",
            }
        ),
        definition,
        (server + ":" + &port).parse().unwrap(),
    )));
    service
}

pub const DEFAULT_ANNOUNCE_PERIOD: Duration = Duration::from_secs(360);
