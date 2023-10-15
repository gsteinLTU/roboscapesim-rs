use std::collections::{HashMap, BTreeMap};
use std::f32::consts::FRAC_PI_2;
use std::rc::Rc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Utc;
use dashmap::{DashMap, DashSet};
use derivative::Derivative;
use log::{error, info, trace, warn};
use nalgebra::{vector, Vector3, UnitQuaternion};
use netsblox_vm::{project::{ProjectStep, IdleAction}, real_time::UtcOffset, runtime::{RequestStatus, Config, ToJsonError, Key, System}, std_system::StdSystem};
use rand::Rng;
use rapier3d::prelude::{ColliderBuilder, RigidBodyBuilder, AngVector, Real};
use roboscapesim_common::*;
use serde_json::{json, Value};
use tokio::{spawn, time::sleep};
use std::sync::{mpsc, Arc, Mutex};

use crate::scenarios::load_environment;
use crate::{CLIENTS, ROOMS};
use crate::services::{entity::{create_entity_service, handle_entity_message}, lidar::{handle_lidar_message, LIDARConfig, create_lidar_service}, position::{handle_position_sensor_message, create_position_service}, proximity::handle_proximity_sensor_message, service_struct::{Service, ServiceType}, world::{self, handle_world_msg}};
use crate::simulation::Simulation;
use crate::util::extra_rand::UpperHexadecimal;
use crate::robot::RobotData;
use crate::util::traits::resettable::{Resettable, RigidBodyResetter};
use crate::vm::{STEPS_PER_IO_ITER, open_project, YIELDS_BEFORE_IDLE_SLEEP, IDLE_SLEEP_TIME, DEFAULT_BASE_URL, Intermediate, C, get_env};
pub(crate) mod netsblox_api;

#[derive(Derivative)]
#[derivative(Debug)]
/// Holds the data for a single room
pub struct RoomData {
    pub objects: DashMap<String, ObjectData>,
    pub name: String,
    pub environment: String,
    pub password: Option<String>,
    pub timeout: i64,
    pub last_interaction_time: i64,
    pub hibernating: Arc<AtomicBool>,
    pub sockets: DashMap<String, u128>,
    /// List of usernames of users who have visited the room
    pub visitors: Arc<Mutex<Vec<String>>>,
    pub last_update: Instant,
    pub last_full_update: i64,
    pub last_sim_update: Instant,
    pub roomtime: f64,
    pub robots: HashMap<String, RobotData>,
    #[derivative(Debug = "ignore")]
    pub sim: Arc<Mutex<Simulation>>,
    #[derivative(Debug = "ignore")]
    pub reseters: HashMap<String, Box<dyn Resettable + Send + Sync>>,
    #[derivative(Debug = "ignore")]
    pub services: Arc<DashMap<(String, ServiceType), Arc<Mutex<Service>>>>,
    #[derivative(Debug = "ignore")]
    pub lidar_configs: HashMap<String, LIDARConfig>,
    #[derivative(Debug = "ignore")]
    pub iotscape_rx: mpsc::Receiver<(iotscape::Request, Option<<StdSystem<C> as System<C>>::RequestKey>)>,
    #[derivative(Debug = "ignore")]
    pub netsblox_msg_tx: mpsc::Sender<(String, String, BTreeMap<String, String>)>,
    #[derivative(Debug = "ignore")]
    pub netsblox_msg_rx: Arc<Mutex<mpsc::Receiver<(String, String, BTreeMap<String, String>)>>>,
    /// Whether the room is in edit mode, if so, IoTScape messages are sent to NetsBlox server instead of being handled locally by VM
    pub edit_mode: bool,
    /// Thread with VM if not in edit mode
    pub vm_thread: Option<JoinHandle<()>>,
}

impl RoomData {
    pub fn new(name: Option<String>, environment: Option<String>, password: Option<String>, edit_mode: bool) -> RoomData {
        let (netsblox_msg_tx, netsblox_msg_rx) = mpsc::channel();
        let (iotscape_tx, iotscape_rx) = mpsc::channel();
        let netsblox_msg_rx = Arc::new(Mutex::new(netsblox_msg_rx));
        let vm_netsblox_msg_rx = netsblox_msg_rx.clone();
        let iotscape_netsblox_msg_rx = netsblox_msg_rx.clone();

        let mut obj = RoomData {
            objects: DashMap::new(),
            name: name.unwrap_or(Self::generate_room_id(None)),
            environment: environment.clone().unwrap_or("Default".to_owned()),
            password,
            timeout: 60 * 10,
            last_interaction_time: Utc::now().timestamp(),
            hibernating: Arc::new(AtomicBool::new(false)),
            sockets: DashMap::new(),
            visitors: Arc::new(Mutex::new(Vec::new())),
            last_full_update: 0,
            roomtime: 0.0,
            sim: Arc::new(Mutex::new(Simulation::new())),
            last_update: Instant::now(),
            robots: HashMap::new(),
            reseters: HashMap::new(),
            services: Arc::new(DashMap::new()),
            lidar_configs: HashMap::new(),
            last_sim_update: Instant::now(),
            iotscape_rx,
            netsblox_msg_tx,
            netsblox_msg_rx,
            edit_mode,
            vm_thread: None,
        };

        info!("Room {} created", obj.name);

        // Setup test room
        // Create IoTScape service
        let service = Arc::new(Mutex::new(world::create_world_service(obj.name.as_str())));
        let service_id = service.lock().unwrap().id.clone();
        obj.services.insert((service_id, ServiceType::World), service);
        
        // Create IoTScape network I/O Task
        let net_iotscape_tx = iotscape_tx.clone();
        let services = obj.services.clone();
        let hibernating = obj.hibernating.clone();
        spawn(async move {
            loop {
                if hibernating.load(Ordering::Relaxed) {
                    sleep(Duration::from_millis(50)).await;
                } else {
                    for service in services.iter() {
                        // Handle messages
                        let service = &mut service.value().lock().unwrap();
                        if service.update() > 0 {
                            let rx = &mut service.service.lock().unwrap().rx_queue;
                            while !rx.is_empty() {
                                let msg = rx.pop_front().unwrap();
                                net_iotscape_tx.send((msg, None)).unwrap();
                            }
                        }
                    }

                    sleep(Duration::from_millis(2)).await;
                }
            }
        });
        
        // Create VM Task
        if !edit_mode {
            let vm_iotscape_tx = iotscape_tx.clone();
            let hibernating = obj.hibernating.clone();
            let id_clone = obj.name.clone();
            obj.vm_thread = Some(thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let project = load_environment(environment).await;

                    // Setup VM
                    let (project_name, role) = open_project(&project).unwrap_or_else(|_| panic!("failed to read file"));
                    let mut idle_sleeper = IdleAction::new(YIELDS_BEFORE_IDLE_SLEEP, Box::new(|| thread::sleep(IDLE_SLEEP_TIME)));
                    info!("Loading project {}", project_name);
                    let system = Rc::new(StdSystem::new_async(DEFAULT_BASE_URL.to_owned(), Some(&project_name), Config {
                        request: Some(Rc::new(move |_system: &StdSystem<C>, _, key, request, _| {
                            match &request {
                                netsblox_vm::runtime::Request::Rpc { service, rpc, args } => {
                                    match args.iter().map(|(_k, v)| v.to_json()).collect::<Result<Vec<_>,ToJsonError<_,_>>>() {
                                        Ok(args) => {
                                            match service.as_str() {
                                                "RoboScapeWorld" | "RoboScapeEntity" | "PositionSensor" | "LIDAR" => {
                                                    // Keep IoTScape services local
                                                    //println!("{:?}", (service, rpc, &args));
                                                    vm_iotscape_tx.send((iotscape::Request { id: "".into(), service: service.to_owned(), device: args[0].to_string(), function: rpc.to_owned(), params: args.iter().skip(1).map(|v| v.to_owned()).collect() }, Some(key))).unwrap();
                                                },
                                                /*"RoboScape" => {
                                                    // TODO: RoboScape service but in Rust?
                                                    key.complete(Ok(Intermediate::Json(json!(""))));
                                                },*/
                                                _ => return RequestStatus::UseDefault { key, request },
                                            }
                                        },
                                        Err(err) => key.complete(Err(format!("failed to convert RPC args to string: {err:?}"))),
                                    }
                                    RequestStatus::Handled
                                },
                                netsblox_vm::runtime::Request::UnknownBlock { name, args: _ } => {
                                    match name.as_str() {
                                        "roomID" => {
                                            key.complete(Ok(Intermediate::Json(json!(format!("\"{id_clone}\"")))));
                                            RequestStatus::Handled
                                        },
                                        "robotsInRoom" => {

                                            RequestStatus::Handled
                                        },
                                        _ => {
                                            RequestStatus::UseDefault { key, request }
                                        }
                                    }
                                },
                                _ => RequestStatus::UseDefault { key, request },
                            }
                        })),
                        command: None,
                    }, UtcOffset::UTC).await);

                    println!(">>> public id: {}\n", system.get_public_id());
                
                    let env = match get_env(&role, system.clone()) {
                        Ok(x) => Ok(x),
                        Err(e) => {
                            Err(format!(">>> error loading project: {e:?}").to_owned())         
                        }
                    };

                    let env = env.unwrap();

                    info!("Loaded");
                    // Start program
                    env.mutate(|mc, env| {
                        let mut proj = env.proj.borrow_mut(mc);
                        proj.input(mc, netsblox_vm::project::Input::Start);
                    });

                    // Run program
                    loop {
                        if hibernating.load(Ordering::Relaxed) {
                            sleep(Duration::from_millis(50)).await;
                        } else {

                            if let Ok((_service_id, msg_type, values)) = vm_netsblox_msg_rx.lock().unwrap().recv_timeout(Duration::ZERO) {
                                // TODO: check for listen
                                system.inject_message(msg_type, values.iter().map(|(k, v)| (k.clone(), Value::from(v.clone()))).collect());
                            }

                            env.mutate(|mc, env| {
                                let mut proj = env.proj.borrow_mut(mc);

                                for _ in 0..STEPS_PER_IO_ITER {
                                    let res = proj.step(mc);
                                    if let ProjectStep::Error { error, proc } = &res {
                                        println!("\n>>> runtime error in entity {:?}: {:?}\n", proc.get_call_stack().last().unwrap().entity.borrow().name, error.cause);
                                    }
                                    idle_sleeper.consume(&res);
                                }
                            });
                        }
                    }
                });
            })); 
        } else {
            // In edit mode, send IoTScape messages to NetsBlox server
            let services = obj.services.clone();
            let mut event_id: usize = rand::random();
            spawn(async move {
                loop {
                    while let Ok((service_id, msg_type, values)) = iotscape_netsblox_msg_rx.lock().unwrap().recv_timeout(Duration::ZERO) {
                        let service = services.iter().find(|s| s.key().0 == service_id);
                        if let Some(service) = service {
                            service.value().lock().unwrap().service.lock().unwrap().send_event(event_id.to_string().as_str(), &msg_type, values);
                            event_id += 1;
                        }
                    }
                    sleep(Duration::from_millis(2)).await;
                }
            });
        }

        obj
    }

    /// Send UpdateMessage to a client
    pub fn send_to_client(msg: &UpdateMessage, client_id: u128) {
        let client = CLIENTS.get(&client_id);

        if let Some(client) = client {
            client.value().tx.send(msg.clone()).unwrap();
        } else {
            error!("Client {} not found!", client_id);
        }
    }

    /// Send UpdateMessage to all clients in list
    pub fn send_to_clients(msg: &UpdateMessage, clients: impl Iterator<Item = u128>) {
        for client_id in clients {
            let client = CLIENTS.get(&client_id);
            
            if let Some(client) = client {
                client.value().tx.send(msg.clone()).unwrap();
            } else {
                error!("Client {} not found!", client_id);
            }
        }
    }

    /// Send the room's current state data to a specific client
    pub fn send_info_to_client(&self, client: u128) {
        Self::send_to_client(
            &UpdateMessage::RoomInfo(
                RoomState { name: self.name.clone(), roomtime: self.roomtime, users: self.visitors.lock().unwrap().clone() }
            ),
            client,
        );
    }

    /// Send the room's current state data to a specific client
    pub fn send_state_to_client(&self, full_update: bool, client: u128) {
        if full_update {
            Self::send_to_client(
                &UpdateMessage::Update(self.roomtime, true, self.objects.iter().map(|kvp| (kvp.key().to_owned(), kvp.value().to_owned())).collect()),
                client,
            );
        } else {
            Self::send_to_client(
                &UpdateMessage::Update(
                    self.roomtime,
                    false,
                    self.objects
                        .iter()
                        .filter(|mvp| mvp.value().updated)
                        .map(|mvp| {
                            let mut val = mvp.value().clone();
                            val.visual_info = None;
                            (mvp.key().clone(), val)
                        })
                        .collect::<HashMap<String, ObjectData>>(),
                ),
                client,
            );
        }
    }

    /// Send an UpdateMessage to all clients in the room
    pub fn send_to_all_clients(&self, msg: &UpdateMessage) {
        for client in &self.sockets {
            Self::send_to_client(
                msg,
                client.value().to_owned(),
            );
        }
    }

    /// Send the room's current state data to all clients
    pub fn send_state_to_all_clients(&self, full_update: bool) {
        let update_msg: UpdateMessage;
        if full_update {
            update_msg = UpdateMessage::Update(self.roomtime, true, self.objects.iter().map(|kvp| (kvp.key().to_owned(), kvp.value().to_owned())).collect());
        } else {
            update_msg = UpdateMessage::Update(
                self.roomtime,
                false,
                self.objects
                    .iter()
                    .filter(|mvp| mvp.value().updated)
                    .map(|mvp| {
                        let mut val = mvp.value().clone();
                        val.visual_info = None;
                        (mvp.key().clone(), val)
                    })
                    .collect::<HashMap<String, ObjectData>>(),
            );
        }

        self.send_to_all_clients(
            &update_msg
        );

        for mut obj in self.objects.iter_mut() {
            obj.value_mut().updated = false;
        }
    }

    /// Generate a random hexstring room ID of the given length (default 5)
    fn generate_room_id(length: Option<usize>) -> String {
        let s: String = rand::thread_rng()
            .sample_iter(&UpperHexadecimal)
            .take(length.unwrap_or(5))
            .map(char::from)
            .collect();
        ("Room".to_owned() + &s).to_owned()
    }

    pub fn update(&mut self) {
        // Check for disconnected clients
        let mut disconnected = vec![];
        for client_id in &self.sockets {
            if !CLIENTS.contains_key(client_id.value()) {
                disconnected.push(client_id.key().to_owned());
            }
        }
        for client_id in disconnected {
            self.sockets.remove(&client_id);
        }

        // Check if room empty/not empty
        if !self.hibernating.load(Ordering::Relaxed) && self.sockets.is_empty() {
            self.hibernating.store(true, Ordering::Relaxed);
            info!("{} is now hibernating", self.name);
            return;
        } else if self.hibernating.load(Ordering::Relaxed) && !self.sockets.is_empty() {
            self.hibernating.store(false, Ordering::Relaxed);
            info!("{} is no longer hibernating", self.name);
        }

        if self.hibernating.load(Ordering::Relaxed) {
            return;
        }

        trace!("Updating {}", self.name);

        //let max_delta_time = 1.0 / 30.0;
        let now = Instant::now();
        /*let delta_time = (now - self.last_sim_update).as_secs_f64();
        let delta_time = f64::min(max_delta_time, delta_time);*/
        let delta_time = 1.0 / 60.0;
        self.last_sim_update = now;

        // Handle client messages
        let mut needs_reset = false;
        let mut robot_resets = vec![];
        for client in self.sockets.iter() {
            let client_username = client.key().to_owned();
            let client = CLIENTS.get(client.value());

            if let Some(client) = client {
                while let Ok(msg) = client.rx.recv_timeout(Duration::default()) {
                    match msg {
                        ClientMessage::ResetAll => { needs_reset = true; },
                        ClientMessage::ResetRobot(robot_id) => {
                            if self.is_authorized(*client.key(), &robot_id) {
                                robot_resets.push(robot_id);
                            }
                        },
                        ClientMessage::ClaimRobot(robot_id) => {
                            // Check if robot is free
                            if self.is_authorized(*client.key(), &robot_id) {
                                // Claim robot
                                if let Some(robot) = self.robots.get_mut(&robot_id) {
                                    if robot.claimed_by.is_none() {
                                        robot.claimed_by = Some(client_username.clone());

                                        // Send claim message to clients
                                        self.send_to_all_clients(&UpdateMessage::RobotClaimed(robot_id.clone(), client_username.clone()));
                                    } else {
                                        info!("Robot {} already claimed by {}, but {} tried to claim it", robot_id, robot.claimed_by.clone().unwrap(), client_username.clone());
                                    }
                                }
                            } else {
                                info!("Client {} not authorized to claim robot {}", client_username, robot_id);
                            }
                        },
                        ClientMessage::UnclaimRobot(robot_id) => {
                            // Check if robot is free
                            if self.is_authorized(*client.key(), &robot_id) {
                                // Claim robot
                                if let Some(robot) = self.robots.get_mut(&robot_id) {
                                    if robot.claimed_by.clone().is_some_and(|claimed_by| &claimed_by == &client_username) {
                                        robot.claimed_by = None;

                                        // Send Unclaim message to clients
                                        self.send_to_all_clients(&UpdateMessage::RobotClaimed(robot_id.clone(), "".to_owned()));
                                    } else {
                                        info!("Robot {} not claimed by {} who tried to unclaim it", robot_id, client_username);
                                    }
                                }
                            } else {
                                info!("Client {} not authorized to unclaim robot {}", client_username, robot_id);
                            }
                        },
                        ClientMessage::EncryptRobot(robot_id) => {
                            if let Some(robot) = self.robots.get_mut(&robot_id) {
                                robot.send_roboscape_message(&[b'P', 0]).unwrap();
                                robot.send_roboscape_message(&[b'P', 1]).unwrap();
                            }
                        },
                        _ => {
                            warn!("Unhandled client message: {:?}", msg);
                        }
                    }
                }
            }
        }

        if needs_reset {
            self.reset();
        } else {
            for robot in robot_resets {
                self.reset_robot(&robot);
            }
        }

        let time = Utc::now().timestamp();

        for robot in self.robots.iter_mut() {
            let (updated, msg) = RobotData::robot_update(robot.1, &mut self.sim.lock().unwrap(), &self.sockets, delta_time);
            if updated {
                self.last_interaction_time = Utc::now().timestamp();
            }

            // Check if claimed by user not in room
            if let Some(claimant) = &robot.1.claimed_by {
                if !self.sockets.contains_key(claimant) {
                    info!("Robot {} claimed by {} but not in room, unclaiming", robot.0, claimant);
                    robot.1.claimed_by = None;
                    RoomData::send_to_clients(&UpdateMessage::RobotClaimed(robot.0.clone(), "".to_owned()), self.sockets.iter().map(|c| c.value().to_owned()));
                }
            }

            // Check if message to send
            if let Some(msg) = msg {
                if let Some(claimant) = &robot.1.claimed_by {
                    if let Some(client) = self.sockets.get(claimant) {
                        // Only send to owner
                        RoomData::send_to_client(&msg, client.value().to_owned());
                    }
                } else {
                    RoomData::send_to_clients(&msg, self.sockets.iter().map(|c| c.value().to_owned()));
                }
            }
        }
        
        let mut msgs: Vec<(iotscape::Request, Option<<StdSystem<C> as System<C>>::RequestKey>)> = vec![];

        while let Ok(msg) = self.iotscape_rx.recv_timeout(Duration::ZERO) {
            if msg.0.function != "heartbeat" {
                self.last_interaction_time = Utc::now().timestamp();
                msgs.push(msg);
            }
        }
        
        for (msg, key) in msgs {
            info!("{:?}", msg);

            let response = match msg.service.as_str() {
                "RoboScapeWorld" => handle_world_msg(self, msg),
                "RoboScapeEntity" => handle_entity_message(self, msg),
                "PositionSensor" => handle_position_sensor_message(self, msg),
                "LIDAR" => handle_lidar_message(self, msg),
                "ProximitySensor" => handle_proximity_sensor_message(self, msg),
                t => {
                    info!("Service type {:?} not yet implemented.", t);
                    Err(format!("Service type {:?} not yet implemented.", t))
                }
            };

            if let Some(key) = key {
                key.complete(response);
            }
        }
        
        let simulation = &mut self.sim.lock().unwrap();
        simulation.update(delta_time);

        // Check for trigger events, this may need to be optimized in the future, possible switching to event-based
        for mut entry in simulation.sensors.iter_mut() {
            let (sensor, in_sensor) = entry.pair_mut();
            for (c1, c2, intersecting) in simulation.narrow_phase.intersections_with(*sensor) {
                if intersecting {
                    if in_sensor.contains(&c2) {
                        // Already in sensor
                        continue;
                    } else {
                        trace!("Sensor {:?} intersecting {:?} = {}", c1, c2, intersecting);
                        in_sensor.insert(c2);
                    }
                } else {
                    if in_sensor.contains(&c2) {
                        in_sensor.remove(&c2);
                        trace!("Sensor {:?} intersecting {:?} = {}", c1, c2, intersecting);
                    }
                }
            }
        }

        // Update data before send
        for mut o in self.objects.iter_mut()  {
            if simulation.rigid_body_labels.contains_key(o.key()) {
                let get = &simulation.rigid_body_labels.get(o.key()).unwrap();
                let handle = get.value();
                let rigid_body_set = &simulation.rigid_body_set.lock().unwrap();
                let body = rigid_body_set.get(*handle).unwrap();
                let old_transform = o.value().transform;
                o.value_mut().transform = Transform { position: (*body.translation()).into(), rotation: Orientation::Quaternion(*body.rotation().quaternion()), scaling: old_transform.scaling };

                if old_transform != o.value().transform {
                    o.value_mut().updated = true;
                }
            }
        }

        self.roomtime += delta_time;

        if time - self.last_full_update < 60 {
            if (now - self.last_update) > Duration::from_millis(120) {
                // Send incremental state to clients
                self.send_state_to_all_clients(false);
                self.last_update = now;
            }
        } else {
            // Send full state to clients
            self.send_state_to_all_clients(true);
            self.last_full_update = time;
            self.last_update = now;
        }
    }

    /// Reset entire room
    pub(crate) fn reset(&mut self){
        let simulation = &mut self.sim.lock().unwrap();

        // Reset robots
        for r in self.robots.iter_mut() {
            r.1.reset(simulation);
        }

        for resetter in self.reseters.iter_mut() {
            resetter.1.reset(simulation);
        }

        // Send
        let world_service = self.services.iter().find(|s| s.key().1 == ServiceType::World);
        if let Some(world_service) = world_service {
            self.netsblox_msg_tx.send((world_service.lock().unwrap().id.clone(), "reset".to_string(), BTreeMap::new())).unwrap();
        }
        
        self.last_interaction_time = Utc::now().timestamp();
    }
    
    /// Reset single robot
    pub(crate) fn reset_robot(&mut self, id: &str){
        if self.robots.contains_key(&id.to_string()) {
            self.robots.get_mut(&id.to_string()).unwrap().reset(&mut self.sim.lock().unwrap());
        } else {
            info!("Request to reset non-existing robot {}", id);
        }

        self.last_interaction_time = Utc::now().timestamp();
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
                let client = self.sockets.iter().find(|c| c.value() == &client);

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

    /// Add a robot to a room
    pub(crate) fn add_robot(room: &mut RoomData, position: Vector3<Real>, orientation: UnitQuaternion<f32>, wheel_debug: bool) -> String {
        let simulation = &mut room.sim.lock().unwrap();
        let mut robot = RobotData::create_robot_body(simulation, None, Some(position), Some(orientation));
        let robot_id: String = "robot_".to_string() + robot.id.as_str();
        simulation.rigid_body_labels.insert(robot_id.clone(), robot.body_handle);
        room.objects.insert(robot_id.clone(), ObjectData {
            name: robot_id.clone(),
            transform: Transform {scaling: vector![3.0,3.0,3.0], ..Default::default() },
            visual_info: Some(VisualInfo::Mesh("parallax_robot.glb".into())),
            is_kinematic: false,
            updated: true,
        });
        RobotData::setup_robot_socket(&mut robot);
            
        let service = Arc::new(Mutex::new(create_position_service(&robot.id, &robot.body_handle)));
        let service_id = service.lock().unwrap().id.clone();
        room.services.insert((service_id, ServiceType::PositionSensor), service);
            
        let service = Arc::new(Mutex::new(create_lidar_service(&robot.id, &robot.body_handle)));
        let service_id = service.lock().unwrap().id.clone();
        room.services.insert((service_id, ServiceType::LIDAR), service);
        room.lidar_configs.insert(robot.id.clone(), LIDARConfig { num_beams: 16, start_angle: -FRAC_PI_2, end_angle: FRAC_PI_2, offset_pos: vector![0.17,0.1,0.0], max_distance: 3.0 });
            
        // Wheel debug
        if wheel_debug {
            let mut i = 0;
            for wheel in &robot.wheel_bodies {
                simulation.rigid_body_labels.insert(format!("wheel_{}", i), *wheel);
                room.objects.insert(format!("wheel_{}", i), ObjectData {
                    name: format!("wheel_{}", i),
                    transform: Transform { scaling: vector![0.18,0.03,0.18], ..Default::default() },
                    visual_info: Some(VisualInfo::default()),
                    is_kinematic: false,
                    updated: true,
                });
                i += 1;
            }
        }

        let id = robot.id.to_string();
        room.robots.insert(robot.id.to_string(), robot);
        room.last_full_update = 0;
        id
    }

    /// Add a physics object to the room
    pub(crate) fn add_shape(room: &mut RoomData, name: &str, position: Vector3<Real>, rotation: AngVector<Real>, visual_info: Option<VisualInfo>, size: Option<Vector3<Real>>, is_kinematic: bool) -> String {
        let body_name = room.name.to_owned() + "_" + name;
        let rigid_body = if is_kinematic { RigidBodyBuilder::kinematic_position_based() } else { RigidBodyBuilder::dynamic() }
            .ccd_enabled(true)
            .translation(position)
            .rotation(rotation)
            .build();
        
        let mut size = size.unwrap_or_else(|| vector![1.0, 1.0, 1.0]);

        let visual_info = visual_info.unwrap_or_default();

        let shape = match visual_info {
            VisualInfo::Color(_, _, _, s) => {
                s
            },
            VisualInfo::Texture(_, _, _, s) => {
                s
            },
            _ => Shape::Box
        };

        let collider = match shape {
            Shape::Box => ColliderBuilder::cuboid(size.x / 2.0, size.y / 2.0, size.z / 2.0),
            Shape::Sphere => {
                size.y = size.x;
                size.z = size.x;
                ColliderBuilder::ball(size.x / 2.0)
            },
            Shape::Cylinder => {
                size.z = size.x;
                ColliderBuilder::cylinder(size.y / 2.0, size.x / 2.0)
            },
            Shape::Capsule => {
                size.z = size.x;
                ColliderBuilder::capsule_y(size.y / 2.0, size.x / 2.0)
            },
        };

        let simulation = &mut room.sim.lock().unwrap();
        let collider = collider.restitution(0.3).density(0.1).build();
        let cube_body_handle = simulation.rigid_body_set.lock().unwrap().insert(rigid_body);
        let rigid_body_set = simulation.rigid_body_set.clone();
        simulation.collider_set.insert_with_parent(collider, cube_body_handle, &mut rigid_body_set.lock().unwrap());
        simulation.rigid_body_labels.insert(body_name.clone(), cube_body_handle);

        room.objects.insert(body_name.clone(), ObjectData {
            name: body_name.clone(),
            transform: Transform { position: position.into(), scaling: size, rotation: Orientation::Euler(rotation), ..Default::default() },
            visual_info: Some(visual_info),
            is_kinematic,
            updated: true,
        });

        room.reseters.insert(body_name.clone(), Box::new(RigidBodyResetter::new(cube_body_handle, simulation)));

        let service = Arc::new(Mutex::new(create_entity_service(&body_name, &cube_body_handle)));
        let service_id = service.lock().unwrap().id.clone();
        room.services.insert((service_id, ServiceType::Entity), service);
        room.last_full_update = 0;
        body_name
    }

    /// Specialized add_shape for triggers
    pub(crate) fn add_trigger(room: &mut RoomData, name: &str, position: Vector3<Real>, rotation: AngVector<Real>, size: Option<Vector3<Real>>) -> String {
        let body_name = room.name.to_owned() + "_" + name;
        let rigid_body =  RigidBodyBuilder::kinematic_position_based()
            .ccd_enabled(true)
            .translation(position)
            .rotation(rotation)
            .build();

        let size = size.unwrap_or_else(|| vector![1.0, 1.0, 1.0]);

        let collider = ColliderBuilder::cuboid(size.x / 2.0, size.y / 2.0, size.z / 2.0).sensor(true).build();

        let simulation = &mut room.sim.lock().unwrap();
        let cube_body_handle = simulation.rigid_body_set.lock().unwrap().insert(rigid_body);
        let rigid_body_set = simulation.rigid_body_set.clone();
        let collider_handle = simulation.collider_set.insert_with_parent(collider, cube_body_handle, &mut rigid_body_set.lock().unwrap());
        simulation.rigid_body_labels.insert(body_name.clone(), cube_body_handle);
        simulation.sensors.insert(collider_handle, DashSet::new());

        room.objects.insert(body_name.clone(), ObjectData {
            name: body_name.clone(),
            transform: Transform { position: position.into(), scaling: size, rotation: Orientation::Euler(rotation), ..Default::default() },
            visual_info: Some(VisualInfo::None),
            is_kinematic: true,
            updated: true,
        });

        room.reseters.insert(body_name.clone(), Box::new(RigidBodyResetter::new(cube_body_handle, simulation)));

        let service = Arc::new(Mutex::new(create_entity_service(&body_name, &cube_body_handle)));
        let service_id = service.lock().unwrap().id.clone();
        room.services.insert((service_id, ServiceType::Entity), service);
        room.last_full_update = 0;
        body_name
    }

    pub(crate) fn remove(&mut self, id: &String) {
        let simulation = &mut self.sim.lock().unwrap();
        self.objects.remove(id);

        if simulation.rigid_body_labels.contains_key(id) {
            let handle = *simulation.rigid_body_labels.get(id).unwrap();
            simulation.rigid_body_labels.remove(id);
            simulation.remove_body(handle);
        }

        if self.robots.contains_key(id) {
            simulation.cleanup_robot(self.robots.get(id).unwrap());
            self.robots.remove(id);
        }

        self.send_to_all_clients(&UpdateMessage::RemoveObject(id.to_string()));
    }

    pub(crate) fn remove_all(&mut self) {
        for obj in self.objects.iter() {
            self.send_to_all_clients(&UpdateMessage::RemoveObject(obj.key().to_string()));
        }
        self.objects.clear();

        let simulation = &mut self.sim.lock().unwrap();
        let labels = simulation.rigid_body_labels.clone();
        for l in labels.iter() {
            if !l.key().starts_with("robot_") {
                simulation.remove_body(*l.value());
            }
        }
        simulation.rigid_body_labels.clear();

        for r in self.robots.iter() {
            simulation.cleanup_robot(r.1);

            self.send_to_all_clients(&UpdateMessage::RemoveObject(r.0.to_string()));
        }
        self.robots.clear();
        info!("All entities removed from {}", self.name);
    }

    pub(crate) fn count_non_robots(&self) -> usize {
        (self.objects.len() - self.robots.len()).clamp(0, self.objects.len())
    }
}

pub fn join_room(username: &str, password: &str, peer_id: u128, room_id: &str) -> Result<(), String> {
    info!("User {} (peer id {}), attempting to join room {}", username, peer_id, room_id);

    if !ROOMS.contains_key(room_id) {
        return Err(format!("Room {} does not exist!", room_id));
    }

    let room = ROOMS.get(room_id).unwrap();
    let room = &mut room.lock().unwrap();
    
    // Check password
    if room.password.clone().is_some_and(|pass| pass != password) {
        return Err("Wrong password!".to_owned());
    }
    
    // Setup connection to room
    {
        let visitors = &mut room.visitors.lock().unwrap();
        if !visitors.contains(&username.to_owned()) {
            visitors.push(username.to_owned());
        }
    }    
    room.sockets.insert(username.to_string(), peer_id);
    room.last_interaction_time = Utc::now().timestamp();

    // Give client initial update
    room.send_info_to_client(peer_id);
    room.send_state_to_client(true, peer_id);

    // Initial robot claim data
    for robot in room.robots.iter() {
        if robot.1.claimed_by.is_some() {   
            RoomData::send_to_client(&UpdateMessage::RobotClaimed(robot.0.clone(), robot.1.claimed_by.clone().unwrap_or("".to_owned())), peer_id);
        }
    }

    Ok(())
}

pub async fn create_room(environment: Option<String>, password: Option<String>, edit_mode: bool) -> String {
    let room = Arc::new(Mutex::new(RoomData::new(None, environment, password, edit_mode)));
    
    // Set last interaction to creation time
    room.lock().unwrap().last_interaction_time = Utc::now().timestamp();

    let room_id = room.lock().unwrap().name.clone();
    ROOMS.insert(room_id.to_string(), room.clone());
    room_id
}