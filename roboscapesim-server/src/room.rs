use std::collections::{HashMap, BTreeMap};
use std::rc::Rc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering, AtomicI64};

use dashmap::{DashMap, DashSet};
use derivative::Derivative;
use futures::FutureExt;
use log::{error, info, trace, warn};
use nalgebra::{vector, Vector3, UnitQuaternion};
use netsblox_vm::real_time::OffsetDateTime;
use netsblox_vm::{runtime::{SimpleValue, ErrorCause, CommandStatus, Command, RequestStatus, Config, Key, System}, std_util::Clock, project::{ProjectStep, IdleAction}, real_time::UtcOffset, std_system::StdSystem};
use once_cell::sync::{Lazy, OnceCell};
use rand::Rng;
use rapier3d::geometry::ColliderHandle;
use rapier3d::prelude::{ColliderBuilder, RigidBodyBuilder, AngVector, Real};
use roboscapesim_common::*;
use tokio::time;
use tokio::{spawn, time::sleep};
use std::sync::{Arc, mpsc};

#[cfg(feature = "no_deadlocks")]
use no_deadlocks::{Mutex, RwLock};
#[cfg(not(feature = "no_deadlocks"))]
use std::sync::{Mutex, RwLock};

use crate::room::clients::ClientsManager;
use crate::room::messages::MessageHandler;
use crate::room::metadata::RoomMetadata;
use crate::room::vm::VMManager;
use crate::{services::*, UPDATE_FPS};
use crate::util::util::get_timestamp;
use crate::{CLIENTS};
use crate::api::{REQWEST_CLIENT, get_main_api_server};
use crate::scenarios::load_environment;
use crate::simulation::{Simulation, SCALE};
use crate::util::extra_rand::UpperHexadecimal;
use crate::robot::RobotData;
use crate::util::traits::resettable::{Resettable, RigidBodyResetter};
use crate::vm::{STEPS_PER_IO_ITER, open_project, YIELDS_BEFORE_IDLE_SLEEP, IDLE_SLEEP_TIME, DEFAULT_BASE_URL, C, get_env};
pub(crate) mod netsblox_api;
pub(crate) mod management;
mod messages;
mod vm;
pub(crate) mod objects;
pub(crate) mod clients;
pub(crate) mod metadata;

const COLLECT_PERIOD: Duration = Duration::from_secs(60);

#[derive(Derivative)]
#[derivative(Debug)]
/// Holds the data for a single room
pub struct RoomData {
    #[derivative(Debug = "ignore")]
    pub is_alive: Arc<AtomicBool>,
    pub objects: DashMap<String, ObjectData>,
    pub metadata: RoomMetadata,
    pub last_interaction_time: Arc<AtomicI64>,
    #[derivative(Debug = "ignore")]
    pub last_update_run: Arc<RwLock<OffsetDateTime>>,
    #[derivative(Debug = "ignore")]
    pub last_update_sent: Arc<RwLock<OffsetDateTime>>,
    pub last_full_update_sent: Arc<AtomicI64>,
    #[derivative(Debug = "ignore")]
    pub roomtime: Arc<RwLock<f64>>,
    pub robots: Arc<DashMap<String, RobotData>>,
    #[derivative(Debug = "ignore")]
    pub sim: Arc<Simulation>,
    #[derivative(Debug = "ignore")]
    pub reseters: DashMap<String, Box<dyn Resettable + Send + Sync>>,
    #[derivative(Debug = "ignore")]
    pub services: Arc<DashMap<(String, ServiceType), Arc<Box<dyn Service>>>>,
    #[derivative(Debug = "ignore")]
    pub iotscape_rx: Arc<Mutex<mpsc::Receiver<(iotscape::Request, Option<<StdSystem<C> as System<C>>::RequestKey>)>>>,
    #[derivative(Debug = "ignore")]
    pub netsblox_msg_tx: mpsc::Sender<((String, ServiceType), String, BTreeMap<String, String>)>,
    #[derivative(Debug = "ignore")]
    pub netsblox_msg_rx: Arc<Mutex<mpsc::Receiver<((String, ServiceType), String, BTreeMap<String, String>)>>>,
    /// Next object ID to use
    pub next_object_id: Arc<AtomicI64>,
    /// Message handler for this room
    #[derivative(Debug = "ignore")]
    message_handler: OnceCell<Arc<MessageHandler>>,
    /// VM Manager
    #[derivative(Debug = "ignore")]
    pub vm_manager: OnceCell<Arc<VMManager>>,
    pub clients_manager: ClientsManager,
}

pub static SHARED_CLOCK: Lazy<Arc<Clock>> = Lazy::new(|| {
    Arc::new(Clock::new(UtcOffset::UTC, Some(netsblox_vm::runtime::Precision::Medium)))
});

impl RoomData {
    pub async fn new(name: Option<String>, environment: Option<String>, password: Option<String>, edit_mode: bool) -> Arc<RoomData> {
        let (netsblox_msg_tx, netsblox_msg_rx) = mpsc::channel();
        let (iotscape_tx, iotscape_rx) = mpsc::channel();
        let netsblox_msg_rx = Arc::new(Mutex::new(netsblox_msg_rx));
        let iotscape_rx = Arc::new(Mutex::new(iotscape_rx));

        let obj = Arc::new(RoomData {
            is_alive: Arc::new(AtomicBool::new(true)),
            objects: DashMap::new(),
            metadata: RoomMetadata::new(name.clone().unwrap_or_else(|| Self::generate_room_id(None)), environment.clone().unwrap_or("Default".to_owned()), password, if edit_mode { 60 * 30 } else { 60 * 15 }, 9 * 60 * 60, edit_mode),
            last_interaction_time: Arc::new(AtomicI64::new(get_timestamp())),
            last_update_run: Arc::new(RwLock::new(SHARED_CLOCK.read(netsblox_vm::runtime::Precision::Medium))),
            last_update_sent: Arc::new(RwLock::new(SHARED_CLOCK.read(netsblox_vm::runtime::Precision::Medium))),
            last_full_update_sent: Arc::new(AtomicI64::new(0)),
            roomtime: Arc::new(RwLock::new(0.0)),
            sim: Arc::new(Simulation::new()),
            robots: Arc::new(DashMap::new()),
            reseters: DashMap::new(),
            services: Arc::new(DashMap::new()),
            iotscape_rx,
            netsblox_msg_tx,
            netsblox_msg_rx,
            next_object_id: Arc::new(AtomicI64::new(0)),
            message_handler: OnceCell::new(),
            vm_manager: OnceCell::new(),
            clients_manager: clients::ClientsManager::new(),
        });

        // Initialize message handler
        obj.message_handler.set(Arc::new(messages::MessageHandler::new(Arc::downgrade(&obj)))).unwrap();

        // Initialize VM manager
        obj.vm_manager.set(Arc::new(vm::VMManager::new(Arc::downgrade(&obj)))).unwrap();

        info!("Creating Room {}", obj.metadata.name);

        // Create IoTScape service
        setup_world_service(&obj).await;
        
        // Create IoTScape network I/O Task
        setup_networking(&iotscape_tx, &obj);
         
        // Create VM Task
        setup_vm(&iotscape_tx, &obj);

        info!("Room {} created", obj.metadata.name);
        obj
    }

    /// Generate a random hexstring room ID of the given length (default 5)
    fn generate_room_id(length: Option<usize>) -> String {
        let s: String = rand::rng()
            .sample_iter(&UpperHexadecimal)
            .take(length.unwrap_or(5))
            .map(char::from)
            .collect();
        ("Room".to_owned() + &s).to_owned()
    }

    pub fn update(&self) {
        //let now = SHARED_CLOCK.read(netsblox_vm::runtime::Precision::Medium);
        let now = OffsetDateTime::now_utc();
        
        if !self.metadata.hibernating.load(Ordering::Relaxed) {
            // Calculate delta time
            let delta_time = (now - *self.last_update_run.read().unwrap()).as_seconds_f64();
            let delta_time = delta_time.clamp(0.5 / UPDATE_FPS, 2.0 / UPDATE_FPS);
            
            // Check for disconnected clients
            self.clients_manager.remove_disconnected_clients(&self);

            // Handle client messages
            let mut needs_reset = false;
            let mut robot_resets = vec![];
            let msgs = self.clients_manager.get_messages();

            for (msg, client_username, client_id) in msgs {
                self.message_handler.get().unwrap().handle_client_message(msg, &mut needs_reset, &mut robot_resets, &client_username, client_id);
            }

            if needs_reset {
                self.reset();
            } else {
                for robot in robot_resets {
                    self.reset_robot(&robot);
                }
            }

            let time = get_timestamp();

            // Do updates
            self.update_robots(delta_time);
            self.message_handler.get().unwrap().get_iotscape_messages();
            self.sim.update(delta_time);

            // Check for trigger events, this may need to be optimized in the future, possible switching to event-based
            self.update_triggers();

            // Update data before send
            for mut o in self.objects.iter_mut()  {
                if self.sim.rigid_body_labels.contains_key(o.key()) {
                    let get = &self.sim.rigid_body_labels.get(o.key()).unwrap();
                    let handle = get.value();
                    let rigid_body_set = &self.sim.rigid_body_set.read().unwrap();
                    let body = rigid_body_set.get(*handle);

                    if let Some(body) = body {
                        let old_transform = o.value().transform;
                        o.value_mut().transform = Transform { position: (*body.translation()).into(), rotation: Orientation::Quaternion(*body.rotation().quaternion()), scaling: old_transform.scaling };

                        if old_transform != o.value().transform {
                            o.value_mut().updated = true;
                        }
                    }
                }
            }
            
            *self.roomtime.write().unwrap() += delta_time;

            if time - self.last_full_update_sent.load(Ordering::Relaxed) < 60 {
                if (now - *self.last_update_sent.read().unwrap()) > Duration::from_millis(120) {
                    //trace!("Sending incremental state to clients");
                    // Send incremental state to clients
                    self.clients_manager.send_state_to_all_clients(self, false);
                    *self.last_update_sent.write().unwrap() = now;
                }
            } else {
                // Send full state to clients
                trace!("Sending full state to clients");
                self.clients_manager.send_state_to_all_clients(self, true);
                self.last_full_update_sent.store(time, Ordering::Relaxed);
                *self.last_update_sent.write().unwrap() = now;
            }

            *self.last_update_run.write().unwrap() = now;
        } else {
            // Still do IoTScape handling
            self.message_handler.get().unwrap().get_iotscape_messages();
        }

        // Check if room empty/not empty
        self.metadata.check_hibernation_state(&self.clients_manager);
        self.announce(false);
    }
    
    fn update_triggers(&self) {
        for mut entry in self.sim.sensors.iter_mut() {
            let ((name, sensor), in_sensor) = entry.pair_mut();
            let new_in_sensor = DashSet::new();
    
            for (mut c1, mut c2, intersecting) in self.sim.narrow_phase.lock().unwrap().intersections_with(*sensor) {
    
                // Check which handle is the sensor
                if c2 == *sensor {
                    std::mem::swap(&mut c1, &mut c2);
                }
    
                // Find if other object has name
                let other_name = self.get_rigid_body_name_from_collider(c2);
    
    
                if let Some(other_name) = other_name {
                    trace!("Sensor {:?} ({name}) intersecting {:?} {other_name} = {}", c1, c2, intersecting);
                    if intersecting {
                        new_in_sensor.insert(other_name);
                    }
                }
    
            }
    
            for other in in_sensor.iter() {
                // Check if object left sensor
                if !new_in_sensor.contains(other.key()) {
                    self.netsblox_msg_tx.send(((name.clone(), ServiceType::Trigger),  "triggerExit".into(), BTreeMap::from([("entity".to_owned(), other.key().clone()),("trigger".to_owned(), name.clone())])))
                        .map_err(|e| error!("Error sending triggerExit message: {:?}", e)).unwrap();
                }
            }
    
            for new_other in new_in_sensor.iter() {
                // Check if new object
                if !in_sensor.contains(new_other.key()) {
                    self.netsblox_msg_tx.send(((name.clone(), ServiceType::Trigger),  "triggerEnter".into(), BTreeMap::from([("entity".to_owned(), new_other.key().clone()),("trigger".to_owned(), name.clone())])))
                        .map_err(|e| error!("Error sending triggerEnter message: {:?}", e)).unwrap();
                }
            }

            *in_sensor = new_in_sensor;
        }
    }
    
    /// If the given collider's parent is a named rigid body, return the name of the rigid body
    pub(crate) fn get_rigid_body_name_from_collider(&self, c: ColliderHandle) -> Option<String> {
        let other_body = self.sim.collider_set.read().unwrap().get(c).unwrap().parent().unwrap_or_default();
        let other_name = self.sim.rigid_body_labels.iter().find(|kvp| kvp.value() == &other_body).map(|kvp| kvp.key().clone());
        other_name
    }

    pub(crate) fn update_robots(&self, delta_time: f64) {
        let mut any_robot_updated = false;

        for mut robot in self.robots.iter_mut() {
            let (updated, msg) = RobotData::robot_update(robot.value_mut(), self.sim.clone(), &self.clients_manager.sockets, delta_time);
    
            any_robot_updated |= updated;

            // Check if claimed by user not in room
            if let Some(claimant) = &robot.value().claimed_by {
                if !self.clients_manager.sockets.contains_key(claimant) {
                    info!("Robot {} claimed by {} but not in room, unclaiming", robot.key(), claimant);
                    robot.value_mut().claimed_by = None;
                    ClientsManager::send_to_clients(&UpdateMessage::RobotClaimed(robot.key().clone(), "".to_owned()), self.clients_manager.sockets.iter().map(|c| c.value().clone().into_iter()).flatten());
                }
            }

            // Check if message to send
            if let Some(msg) = msg {
                if let Some(claimant) = &robot.value().claimed_by {
                    if let Some(client) = self.clients_manager.sockets.get(claimant) {
                        // Only send to owner
                        ClientsManager::send_to_clients(&msg, client.value().clone().into_iter());
                    }
                } else {
                    ClientsManager::send_to_clients(&msg, self.clients_manager.sockets.iter().map(|c| c.value().clone().into_iter()).flatten());
                }
            }
        }
        
        if any_robot_updated {
            self.last_interaction_time.store(get_timestamp(), Ordering::Relaxed);
        }
    }

    /// Reset entire room
    pub(crate) fn reset(&self){
        info!("Resetting room {}", self.metadata.name);

        // Reset robots
        for mut r in self.robots.iter_mut() {
            r.value_mut().reset(self.sim.clone());
        }

        for mut resetter in self.reseters.iter_mut() {
            resetter.value_mut().reset(self.sim.clone());
        }

        // Send
        let world_service = self.services.iter().find(|s| s.key().1 == ServiceType::World);
        if let Some(world_service) = world_service {
            self.netsblox_msg_tx.send(((world_service.get_service_info().id.clone(), ServiceType::World), "reset".to_string(), BTreeMap::new())).unwrap();
        }
        
        self.last_interaction_time.store(get_timestamp(),Ordering::Relaxed);
    }
    
    /// Reset single robot
    pub(crate) fn reset_robot(&self, id: &str){
        if self.robots.contains_key(&id.to_string()) {
            self.robots.get_mut(&id.to_string()).unwrap().reset(self.sim.clone());
        } else {
            info!("Request to reset non-existing robot {}", id);
        }

        self.last_interaction_time.store(get_timestamp(),Ordering::Relaxed);
    }

    /// Test if a client is allowed to interact with a robot (for encrypt, reset)
    pub(crate) fn is_authorized(&self, client: u128, robot_id: &str) -> bool {
        let robot = self.robots.get(robot_id);
        // Require robot to exist first
        if let Some(robot) = robot {
            // Test if robot is not claimable
            if !robot.claimable {
                info!("Robot {} is not claimable, no client actions allowed", robot_id);
                return false;
            }

            // If no claim, approve
            if let Some(claimant) = &robot.claimed_by {
                // Make sure not only claim matches but also that claimant is still in-room
                // Get client username
                let client = self.clients_manager.sockets.iter().find(|c| c.value().contains(&client));

                // Only test if client is still in room
                if let Some(client) = client {
                    let client_username = client.key().to_owned();
                    // If claimant is client, approve
                    if claimant == &client_username {
                        return true;
                    } else {
                        // Client not claimant, deny
                        info!("Client {} attempting to use robot {} but {} already claimed it", client_username, robot_id, claimant);
                        return false;
                    }
                } else {
                    // Client not in room, approve
                    info!("Client {} claiming robot {} not in room, approving", claimant, robot_id);
                    return true;
                }
            }
        }
        
        // If no robot (or fell through), approve
        true
    }

    pub(crate) fn remove(&self, id: &String) {
        self.objects.remove(id);

        if self.sim.rigid_body_labels.contains_key(id) {
            let handle = *self.sim.rigid_body_labels.get(id).unwrap();
            self.sim.rigid_body_labels.remove(id);
            self.sim.remove_body(handle);
        }

        if self.robots.contains_key(id) {
            self.sim.cleanup_robot(self.robots.get(id).unwrap().value());
            self.robots.remove(id);
        }

        self.clients_manager.send_to_all_clients(&UpdateMessage::RemoveObject(id.to_string()));
    }

    pub(crate) fn remove_all(&self) {
        info!("Removing all entities from {}", self.metadata.name);
        self.objects.clear();

        // Remove non-world services
        self.services.retain(|k, _| k.1 == ServiceType::World);

        let labels = self.sim.rigid_body_labels.clone();
        for l in labels.iter() {
            if !l.key().starts_with("robot_") {
                self.sim.remove_body(*l.value());
            }
        }

        self.sim.rigid_body_labels.clear();

        for r in self.robots.iter() {
            self.sim.cleanup_robot(r.value());
        }
        self.robots.clear();
        self.clients_manager.send_to_all_clients(&UpdateMessage::RemoveAll());
        info!("All entities removed from {}", self.metadata.name);
    }

    pub(crate) fn count_non_robots(&self) -> usize {
        (self.objects.len() - self.robots.len()).clamp(0, self.objects.len())
    }

    pub(crate) fn count_kinematic(&self) -> usize {
        self.objects.iter().filter(|o| o.value().is_kinematic).count()
    }

    pub(crate) fn count_dynamic(&self) -> usize {
        self.objects.iter().filter(|o| !o.value().is_kinematic).count() - self.robots.len()
    }

    pub fn announce(&self, force: bool) {
        // Throttle announcements to at most once every 30 seconds (unless forced)
        const ANNOUNCE_INTERVAL_SECS: i64 = 30;
        
        let current_time = get_timestamp();
        let last_announce_time = self.metadata.last_announce_time.load(Ordering::Relaxed);
        
        if force || current_time - last_announce_time >= ANNOUNCE_INTERVAL_SECS {
            // Update the announce time atomically
            if self.metadata.last_announce_time.compare_exchange(
                last_announce_time, 
                current_time, 
                Ordering::Relaxed, 
                Ordering::Relaxed
            ).is_ok() {
                let room_info = self.metadata.get_room_info();
                tokio::task::spawn(async move {
                    let response = REQWEST_CLIENT.put(format!("{}/server/rooms", get_main_api_server()))
                    .json(&vec![room_info])
                    .send().await;
                    
                    if let Err(e) = response {
                        error!("Error sending room info to API: {e:?}");
                    }
                });
            }
        }
    }

    pub fn launch(room: Arc<RoomData>) {
        let mut interval = time::interval(Duration::from_millis((1000.0 / UPDATE_FPS) as u64));
    
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

        let m = room.clone();
        tokio::task::spawn(async move {
            loop {
                interval.tick().await;

                if !m.is_alive.load(Ordering::Relaxed) {
                    break;
                }
        
                let update_time = get_timestamp();

                //trace!("Updating room {}", &m.name);
                if !m.metadata.hibernating.load(std::sync::atomic::Ordering::Relaxed) {
                    // Check timeout
                    if update_time - m.last_interaction_time.load(Ordering::Relaxed) > m.metadata.hibernate_timeout {
                        m.metadata.hibernating.store(true, Ordering::Relaxed);
                        m.metadata.hibernating_since.store(get_timestamp(), Ordering::Relaxed);

                        // Kick all users out
                        m.clients_manager.send_to_all_clients(&roboscapesim_common::UpdateMessage::Hibernating);
                        m.clients_manager.sockets.clear();
                        info!("{} is now hibernating", &m.metadata.name);
                    }
                }
                m.update();
            }
        });
    }
}

fn setup_vm(iotscape_tx: &mpsc::Sender<(iotscape::Request, Option<netsblox_vm::std_util::AsyncKey<Result<SimpleValue, netsblox_vm::compact_str::CompactString>>>)>, obj: &Arc<RoomData>) {
    let vm_netsblox_msg_rx = obj.netsblox_msg_rx.clone();
    let iotscape_netsblox_msg_rx = obj.netsblox_msg_rx.clone();

    if !obj.metadata.edit_mode {
        obj.vm_manager.get().unwrap().start(&iotscape_tx, vm_netsblox_msg_rx);
    } else {
        // In edit mode, send IoTScape messages to NetsBlox server
        let services = obj.services.clone();
        let mut event_id: u32 = rand::random();
        let hibernating = obj.metadata.hibernating.clone();
        let hibernating_since = obj.metadata.hibernating_since.clone();
        spawn(async move {
            loop {
                while let Ok(((service_id, service_type), msg_type, values)) = iotscape_netsblox_msg_rx.lock().unwrap().recv_timeout(Duration::ZERO) {
                    if !hibernating.load(Ordering::Relaxed) && hibernating_since.load(Ordering::Relaxed) < get_timestamp() + 2 {
                        let service = services.iter().find(|s| s.key().0 == service_id && s.key().1 == service_type);
                        if let Some(service) = service {
                            if let Err(e) = service.value().get_service_info().service.send_event(event_id.to_string().as_str(), &msg_type, values).now_or_never().unwrap_or(Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to send event to NetsBlox server"))) {
                                error!("Error sending event to NetsBlox server: {:?}", e);
                            }
                            event_id += 1;
                        } else {
                            info!("Service {} not found", service_id);
                        }
                    }
                }
                sleep(Duration::from_millis(3)).await;
            }
        });
    }
}

fn setup_networking(iotscape_tx: &mpsc::Sender<(iotscape::Request, Option<netsblox_vm::std_util::AsyncKey<Result<SimpleValue, netsblox_vm::compact_str::CompactString>>>)>, obj: &Arc<RoomData>) {
    let net_iotscape_tx = iotscape_tx.clone();
    let services = obj.services.clone();
    let hibernating = obj.metadata.hibernating.clone();
    let hibernating_since = obj.metadata.hibernating_since.clone();
    let is_alive = obj.is_alive.clone();
    spawn(async move {
        loop {
            if !is_alive.load(Ordering::Relaxed) {
                break;
            }

            if hibernating.load(Ordering::Relaxed) && hibernating_since.load(Ordering::Relaxed) < get_timestamp() + 2 {
                sleep(Duration::from_millis(50)).await;
            } else {
                for service in services.iter().map(|s| s.key().clone()).collect::<Vec<_>>() {
                    let service = services.get(&service).unwrap().clone();
                
                    // Handle messages
                    if service.get_service_info().update().await > 0 {
                        let service_info = service.get_service_info().clone();
                        let mut rx = service_info.service.rx_queue.lock().unwrap();
                        while !rx.is_empty() {
                            let msg = rx.pop_front().unwrap();
                            net_iotscape_tx.send((msg, None)).unwrap();
                        }
                    }
                }

                sleep(Duration::from_millis(3)).await;
            }
        }
    });
}

async fn setup_world_service(obj: &Arc<RoomData>) {
    if obj.services.contains_key(&(obj.metadata.name.clone(), ServiceType::World)) {
        warn!("World service already exists for room {}", obj.metadata.name);
        return;
    }

    let service = Arc::new(WorldService::create(obj.metadata.name.as_str()).await);
    let service_id = service.get_service_info().id.clone();
    service.get_service_info().service.announce().await.unwrap();
    obj.services.insert((service_id, ServiceType::World), service);
}

impl Drop for RoomData {
    fn drop(&mut self) {
        self.is_alive.store(false, Ordering::Relaxed);
    }
}
