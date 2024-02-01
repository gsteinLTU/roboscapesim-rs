use std::collections::{HashMap, BTreeMap};
use std::rc::Rc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};

use dashmap::{DashMap, DashSet};
use derivative::Derivative;
use log::{error, info, trace, warn};
use nalgebra::{vector, Vector3, UnitQuaternion};
use netsblox_vm::real_time::OffsetDateTime;
use netsblox_vm::{runtime::{SimpleValue, ErrorCause, CommandStatus, Command, RequestStatus, Config, Key, System}, std_util::Clock, project::{ProjectStep, IdleAction}, real_time::UtcOffset, std_system::StdSystem};
use once_cell::sync::Lazy;
use rand::Rng;
use rapier3d::prelude::{ColliderBuilder, RigidBodyBuilder, AngVector, Real};
use roboscapesim_common::{*, api::RoomInfo};
use tokio::{spawn, time::sleep};
use std::sync::{mpsc, Arc, Mutex};

use crate::{services::*, UPDATE_FPS};
use crate::util::util::get_timestamp;
use crate::{CLIENTS, ROOMS};
use crate::api::{get_server, REQWEST_CLIENT, get_main_api_server};
use crate::scenarios::load_environment;
use crate::simulation::{Simulation, SCALE};
use crate::util::extra_rand::UpperHexadecimal;
use crate::robot::RobotData;
use crate::util::traits::resettable::{Resettable, RigidBodyResetter};
use crate::vm::{STEPS_PER_IO_ITER, open_project, YIELDS_BEFORE_IDLE_SLEEP, IDLE_SLEEP_TIME, DEFAULT_BASE_URL, C, get_env};
pub(crate) mod netsblox_api;

const COLLECT_PERIOD: Duration = Duration::from_secs(60);

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
    pub sockets: DashMap<String, DashSet<u128>>,
    /// List of usernames of users who have visited the room
    pub visitors: DashSet<String>,
    pub last_update: OffsetDateTime,
    pub last_full_update: i64,
    pub hibernating_since: Arc<Mutex<Option<i64>>>,
    pub roomtime: f64,
    pub robots: Arc<DashMap<String, RobotData>>,
    #[derivative(Debug = "ignore")]
    pub sim: Arc<Mutex<Simulation>>,
    #[derivative(Debug = "ignore")]
    pub reseters: HashMap<String, Box<dyn Resettable + Send + Sync>>,
    #[derivative(Debug = "ignore")]
    pub services: Arc<DashMap<(String, ServiceType), Arc<Box<dyn Service>>>>,
    #[derivative(Debug = "ignore")]
    pub iotscape_rx: mpsc::Receiver<(iotscape::Request, Option<<StdSystem<C> as System<C>>::RequestKey>)>,
    #[derivative(Debug = "ignore")]
    pub netsblox_msg_tx: mpsc::Sender<((String, ServiceType), String, BTreeMap<String, String>)>,
    #[derivative(Debug = "ignore")]
    pub netsblox_msg_rx: Arc<Mutex<mpsc::Receiver<((String, ServiceType), String, BTreeMap<String, String>)>>>,
    /// Whether the room is in edit mode, if so, IoTScape messages are sent to NetsBlox server instead of being handled locally by VM
    pub edit_mode: bool,
    /// Thread with VM if not in edit mode
    #[derivative(Debug = "ignore")]
    pub vm_thread: Option<JoinHandle<()>>,
    /// Next object ID to use
    pub next_object_id: usize,
}

pub static SHARED_CLOCK: Lazy<Arc<Clock>> = Lazy::new(|| {
    Arc::new(Clock::new(UtcOffset::UTC, Some(netsblox_vm::runtime::Precision::Medium)))
});

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
            timeout: if edit_mode { 60 * 30 } else { 60 * 15 },
            last_interaction_time: get_timestamp(),
            hibernating: Arc::new(AtomicBool::new(false)),
            sockets: DashMap::new(),
            visitors: DashSet::new(),
            last_full_update: 0,
            roomtime: 0.0,
            sim: Arc::new(Mutex::new(Simulation::new())),
            last_update: SHARED_CLOCK.read(netsblox_vm::runtime::Precision::Medium),
            robots: Arc::new(DashMap::new()),
            reseters: HashMap::new(),
            services: Arc::new(DashMap::new()),
            iotscape_rx,
            netsblox_msg_tx,
            netsblox_msg_rx,
            edit_mode,
            vm_thread: None,
            hibernating_since: Arc::new(Mutex::new(None)),
            next_object_id: 0,
        };

        info!("Creating Room {}", obj.name);

        // Setup test room
        // Create IoTScape service
        let service = Arc::new(WorldService::create(obj.name.as_str()));
        let service_id = service.get_service_info().id.clone();
        obj.services.insert((service_id, ServiceType::World), service);
        
        // Create IoTScape network I/O Task
        let net_iotscape_tx = iotscape_tx.clone();
        let services = obj.services.clone();
        let hibernating = obj.hibernating.clone();
        let hibernating_since = obj.hibernating_since.clone();
        spawn(async move {
            loop {
                if hibernating.load(Ordering::Relaxed) && hibernating_since.lock().unwrap().clone().unwrap_or(0) < get_timestamp() + 2 {
                    sleep(Duration::from_millis(50)).await;
                } else {
                    for service in services.iter() {
                        // Handle messages
                        if service.value().update() > 0 {
                            let rx = &mut service.get_service_info().service.lock().unwrap().rx_queue;
                            while !rx.is_empty() {
                                let msg = rx.pop_front().unwrap();
                                net_iotscape_tx.send((msg, None)).unwrap();
                            }
                        }
                    }

                    sleep(Duration::from_millis(5)).await;
                }
            }
        });
        
        // Create VM Task
        if !edit_mode {
            let vm_iotscape_tx = iotscape_tx.clone();
            let hibernating = obj.hibernating.clone();
            let hibernating_since = obj.hibernating_since.clone();
            let id_clone = obj.name.clone();
            let id_clone2 = obj.name.clone();
            let robots = obj.robots.clone();
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
                    let system = Rc::new(StdSystem::new_async(DEFAULT_BASE_URL.to_owned().into(), Some(&project_name), Config {
                        request: Some(Rc::new(move |_mc, key, request: netsblox_vm::runtime::Request<'_, C, StdSystem<C>>,  _proc| {
                            match &request {
                                netsblox_vm::runtime::Request::Rpc { service, rpc, args } => {
                                    match args.iter().map(|(_k, v)| Ok(v.to_simple()?.into_json()?)).collect::<Result<Vec<_>,ErrorCause<_,_>>>() {
                                        Ok(args) => {
                                            match service.as_str() {
                                                "RoboScapeWorld" | "RoboScapeEntity" | "PositionSensor" | "LIDARSensor" => {
                                                    // Keep IoTScape services local
                                                    //println!("{:?}", (service, rpc, &args));
                                                    vm_iotscape_tx.send((iotscape::Request { client_id: None, id: "".into(), service: service.to_owned().into(), device: args[0].to_string().replace("\"", "").replace("\\", ""), function: rpc.to_owned().into(), params: args.iter().skip(1).map(|v| v.to_owned()).collect() }, Some(key))).unwrap();
                                                },
                                                /*"RoboScape" => {
                                                    // TODO: RoboScape service but in Rust?
                                                    key.complete(Ok(Intermediate::Json(json!(""))));
                                                },*/
                                                _ => return RequestStatus::UseDefault { key, request },
                                            }
                                        },
                                        Err(err) => key.complete(Err(format!("failed to convert RPC args to string: {err:?}").into())),
                                    }
                                    RequestStatus::Handled
                                },
                                netsblox_vm::runtime::Request::UnknownBlock { name, args: _ } => {
                                    match name.as_str() {
                                        "roomID" => {
                                            key.complete(Ok(SimpleValue::String(format!("{id_clone}").into())));
                                            RequestStatus::Handled
                                        },
                                        "robotsInRoom" => {
                                            key.complete(Ok(SimpleValue::List(robots.iter().map(|r| r.key().clone().into()).collect::<Vec<SimpleValue>>())));
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
                        command: Some(Rc::new(move |_mc, key, command, proc| match command {
                            Command::Print { style: _, value } => {
                                let entity = &*proc.get_call_stack().last().unwrap().entity.borrow();
                                if let Some(value) = value { info!("{entity:?} > {value:?}") }
                                key.complete(Ok(()));
                                CommandStatus::Handled
                            },
                            _ => CommandStatus::UseDefault { key, command },
                        })),
                    }, SHARED_CLOCK.clone()).await);

                    println!(">>> public id: {}\n", system.get_public_id());
                
                    let env = match get_env(&role, system.clone()) {
                        Ok(x) => Ok(x),
                        Err(e) => {
                            Err(format!(">>> error loading project: {e:?}").to_owned())         
                        }
                    };

                    let mut env = env.unwrap();

                    info!("Loaded");
                    // Start program
                    env.mutate(|mc, env| {
                        let mut proj = env.proj.borrow_mut(mc);
                        proj.input(mc, netsblox_vm::project::Input::Start);
                    });

                    let mut last_collect_time = SHARED_CLOCK.read(netsblox_vm::runtime::Precision::Medium);

                    // Run program
                    loop {
                        if hibernating.load(Ordering::Relaxed) && hibernating_since.lock().unwrap().clone().unwrap_or(0) < get_timestamp() + 2 {
                            sleep(Duration::from_millis(50)).await;
                        } else {

                            if let Ok((_service_id, msg_type, values)) = vm_netsblox_msg_rx.lock().unwrap().recv_timeout(Duration::ZERO) {
                                // TODO: check for listen
                                system.inject_message(msg_type.into(), values.iter().map(|(k, v)| (k.clone().into(), SimpleValue::String(v.clone().into()))).collect());
                            }

                            env.mutate(|mc, env| {
                                let mut proj = env.proj.borrow_mut(mc);

                                for _ in 0..STEPS_PER_IO_ITER {
                                    let res = proj.step(mc);
                                    if let ProjectStep::Error { error, proc } = &res {
                                        let entity = &*proc.get_call_stack().last().unwrap().entity.borrow();
                                        error!("\n>>> runtime error in entity {:?}: {:?}\n", entity.name, error);
                                        
                                        // TODO: Send error to clients
                                        let _msg = UpdateMessage::VMError(format!("{:?}", error.cause).to_string(), error.pos);
                                    }
                                    idle_sleeper.consume(&res);
                                }
                            });

                            if SHARED_CLOCK.read(netsblox_vm::runtime::Precision::Medium) > last_collect_time + COLLECT_PERIOD {
                                trace!("Collecting garbage for room {}", id_clone2);
                                env.collect_all();
                                last_collect_time = SHARED_CLOCK.read(netsblox_vm::runtime::Precision::Medium);
                            }                            
                        }
                    }
                });
            })); 
        } else {
            // In edit mode, send IoTScape messages to NetsBlox server
            let services = obj.services.clone();
            let mut event_id: u32 = rand::random();
            let hibernating = obj.hibernating.clone();
            let hibernating_since = obj.hibernating_since.clone();
            spawn(async move {
                loop {
                    while let Ok(((service_id, service_type), msg_type, values)) = iotscape_netsblox_msg_rx.lock().unwrap().recv_timeout(Duration::ZERO) {
                        if !hibernating.load(Ordering::Relaxed) && hibernating_since.lock().unwrap().clone().unwrap_or(0) < get_timestamp() + 2 {
                            let service = services.iter().find(|s| s.key().0 == service_id && s.key().1 == service_type);
                            if let Some(service) = service {
                                if let Err(e) = service.value().get_service_info().service.lock().unwrap().send_event(event_id.to_string().as_str(), &msg_type, values) {
                                    error!("Error sending event to NetsBlox server: {:?}", e);
                                }
                                event_id += 1;
                            } else {
                                info!("Service {} not found", service_id);
                            }
                        }
                    }
                    sleep(Duration::from_millis(5)).await;
                }
            });
        }

        info!("Room {} created", obj.name);
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
                RoomState { name: self.name.clone(), roomtime: self.roomtime, users: self.visitors.clone().into_iter().collect() }
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
            for client_id in client.iter() {
                Self::send_to_client(
                    msg,
                    client_id.to_owned(),
                );
            }
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
        let now = SHARED_CLOCK.read(netsblox_vm::runtime::Precision::Medium);
        
        let mut delta_time = (now - self.last_update).as_seconds_f64();
        
        delta_time = delta_time.clamp(0.5 / UPDATE_FPS, 2.0 / UPDATE_FPS);
        
//        info!("{}", delta_time);

        if !self.hibernating.load(Ordering::Relaxed) {

            // Check for disconnected clients
            let mut disconnected = vec![];
            for client_ids in self.sockets.iter() {
                for client_id in client_ids.value().iter() {
                    if !CLIENTS.contains_key(&client_id) {
                        disconnected.push((client_ids.key().clone(), client_id.to_owned()));
                    }
                }
            }
            for (username, client_id) in disconnected {
                info!("Removing client {} from room {}", client_id, &self.name);
                self.sockets.get(&username).and_then(|c| c.value().remove(&client_id));

                if self.sockets.get(&username).unwrap().value().is_empty() {
                    self.sockets.remove(&username);
                }

                // Send leave message to clients
                // TODO: handle multiple clients from one username better?
                let world_service_id = self.services.iter().find(|s| s.key().1 == ServiceType::World).unwrap().value().get_service_info().id.clone();
                self.netsblox_msg_tx.send(((world_service_id, ServiceType::World), "userLeft".to_string(), BTreeMap::from([("username".to_owned(), username.to_owned())]))).unwrap();
            }

            // Handle client messages
            let mut needs_reset = false;
            let mut robot_resets = vec![];
            let mut msgs = vec![];
            for client in self.sockets.iter() {
                let client_username = client.key().to_owned();

                for client in client.value().iter() {
                    let client = CLIENTS.get(&client);

                    if let Some(client) = client {
                        while let Ok(msg) = client.rx.recv_timeout(Duration::default()) {
                            msgs.push((msg, client_username.clone(), client.key().to_owned()));
                        }
                    }
                }
            }

            for (msg, client_username, client_id) in msgs {
                self.handle_client_message(msg, &mut needs_reset, &mut robot_resets, &client_username, client_id);
            }

            if needs_reset {
                self.reset();
            } else {
                for robot in robot_resets {
                    self.reset_robot(&robot);
                }
            }

            let time = get_timestamp();

            self.update_robots(delta_time);
            
            self.get_iotscape_messages();
            
            {
                let simulation = &mut self.sim.lock().unwrap();
                simulation.update(delta_time);

                // Check for trigger events, this may need to be optimized in the future, possible switching to event-based
                for mut entry in simulation.sensors.iter_mut() {
                    let ((name, sensor), in_sensor) = entry.pair_mut();
                    for (c1, c2, intersecting) in simulation.narrow_phase.intersection_pairs_with(*sensor) {
                        if intersecting {
                            if in_sensor.contains(&c2) {
                                // Already in sensor
                                continue;
                            } else {
                                trace!("Sensor {:?} intersecting {:?} = {}", c1, c2, intersecting);
                                in_sensor.insert(c2);
                                // TODO: find other object name
                                self.services.get(&(name.clone(), ServiceType::Trigger))
                                    .and_then(|s| Some(
                                        s.value().get_service_info().service.lock().unwrap().send_event("trigger", "triggerEnter", BTreeMap::from([("entity".to_owned(), "other".to_string())]))));
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
        } else {
            // Still do IoTScape handling
            self.get_iotscape_messages();
        }

        // Check if room empty/not empty
        if !self.hibernating.load(Ordering::Relaxed) && self.sockets.is_empty() {
            self.hibernating.store(true, Ordering::Relaxed);
            self.hibernating_since.lock().unwrap().replace(get_timestamp());
            info!("{} is now hibernating", self.name);
            self.announce();
            return;
        } else if self.hibernating.load(Ordering::Relaxed) && !self.sockets.is_empty() {
            self.hibernating.store(false, Ordering::Relaxed);
            info!("{} is no longer hibernating", self.name);
            self.announce();
        }

        if self.hibernating.load(Ordering::Relaxed) {
            return;
        }
    }

    fn update_robots(&mut self, delta_time: f64) {
        for mut robot in self.robots.iter_mut() {
            let (updated, msg) = RobotData::robot_update(robot.value_mut(), &mut self.sim.lock().unwrap(), &self.sockets, delta_time);
    
            if updated {
                self.last_interaction_time = get_timestamp();
            }

            // Check if claimed by user not in room
            if let Some(claimant) = &robot.value().claimed_by {
                if !self.sockets.contains_key(claimant) {
                    info!("Robot {} claimed by {} but not in room, unclaiming", robot.key(), claimant);
                    robot.value_mut().claimed_by = None;
                    RoomData::send_to_clients(&UpdateMessage::RobotClaimed(robot.key().clone(), "".to_owned()), self.sockets.iter().map(|c| c.value().clone().into_iter()).flatten());
                }
            }

            // Check if message to send
            if let Some(msg) = msg {
                if let Some(claimant) = &robot.value().claimed_by {
                    if let Some(client) = self.sockets.get(claimant) {
                        // Only send to owner
                        RoomData::send_to_clients(&msg, client.value().clone().into_iter());
                    }
                } else {
                    RoomData::send_to_clients(&msg, self.sockets.iter().map(|c| c.value().clone().into_iter()).flatten());
                }
            }
        }
    }

    fn get_iotscape_messages(&mut self) {
        let mut msgs: Vec<(iotscape::Request, Option<<StdSystem<C> as System<C>>::RequestKey>)> = vec![];

        while let Ok(msg) = self.iotscape_rx.recv_timeout(Duration::ZERO) {
            if msg.0.function != "heartbeat" {
                // TODO: figure out which interactions should keep room alive
                //self.last_interaction_time = get_timestamp();
                msgs.push(msg);
            }
        }
            
        for (msg, key) in msgs {
            trace!("{:?}", msg);

            let response = self.handle_iotscape_message(msg);

            if let Some(key) = key {
                key.complete(response.0.map_err(|e| e.into()));
            }

            // If an IoTScape event was included in the response, send it to the NetsBlox server
            if let Some(iotscape) = response.1 {
                self.netsblox_msg_tx.send(iotscape).unwrap();
            }
        }
    }

    fn handle_iotscape_message(&mut self, msg: iotscape::Request) -> (Result<SimpleValue, String>, Option<((String, ServiceType), String, BTreeMap<String, String>)>) {
        let mut response = None;

        let service = self.services.get(&(msg.device.clone(), msg.service.clone().into())).map(|s| s.value().clone());

        if let Some(service) = service {
            response = Some(service.handle_message(self, &msg));

            // Update entities if position or rotation changed
            if ServiceType::Entity == msg.service.clone().into() {
                if msg.function == "setPosition" || msg.function == "setRotation" {
                    if let Some(mut obj) = self.objects.get_mut(msg.device.as_str()) {
                        obj.value_mut().updated = true;
                    }
                }
            }
        }
        
        response.unwrap_or((Err(format!("Service type {:?} not yet implemented.", &msg.service)), None))
    }

    fn handle_client_message(&mut self, msg: ClientMessage, needs_reset: &mut bool, robot_resets: &mut Vec<String>, client_username: &String, client_id: u128) {
        let client = CLIENTS.get(&client_id);

        if let Some(client) = client {
            match msg {
                ClientMessage::ResetAll => { *needs_reset = true; },
                ClientMessage::ResetRobot(robot_id) => {
                    if self.is_authorized(*client.key(), &robot_id) {
                        robot_resets.push(robot_id);
                    } else {
                        info!("Client {} not authorized to reset robot {}", client_username, robot_id);
                    }
                },
                ClientMessage::ClaimRobot(robot_id) => {
                    // Check if robot is free
                    if self.is_authorized(*client.key(), &robot_id) {
                        // Claim robot
                        if let Some(mut robot) = self.robots.get_mut(&robot_id) {
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
                        if let Some(mut robot) = self.robots.get_mut(&robot_id) {
                            if robot.claimed_by.clone().is_some_and(|claimed_by| &claimed_by == client_username) {
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
                    if self.is_authorized(*client.key(), &robot_id) {
                        if let Some(mut robot) = self.robots.get_mut(&robot_id) {
                            robot.send_roboscape_message(&[b'P', 0]).unwrap();
                            robot.send_roboscape_message(&[b'P', 1]).unwrap();
                        }
                    } else {
                        info!("Client {} not authorized to encrypt robot {}", client_username, robot_id);
                    }
                },
                _ => {
                    warn!("Unhandled client message: {:?}", msg);
                }
            }
        }
    }

    /// Reset entire room
    pub(crate) fn reset(&mut self){
        let simulation = &mut self.sim.lock().unwrap();

        // Reset robots
        for mut r in self.robots.iter_mut() {
            r.value_mut().reset(simulation);
        }

        for resetter in self.reseters.iter_mut() {
            resetter.1.reset(simulation);
        }

        // Send
        let world_service = self.services.iter().find(|s| s.key().1 == ServiceType::World);
        if let Some(world_service) = world_service {
            self.netsblox_msg_tx.send(((world_service.get_service_info().id.clone(), ServiceType::World), "reset".to_string(), BTreeMap::new())).unwrap();
        }
        
        self.last_interaction_time = get_timestamp();
    }
    
    /// Reset single robot
    pub(crate) fn reset_robot(&mut self, id: &str){
        if self.robots.contains_key(&id.to_string()) {
            self.robots.get_mut(&id.to_string()).unwrap().reset(&mut self.sim.lock().unwrap());
        } else {
            info!("Request to reset non-existing robot {}", id);
        }

        self.last_interaction_time = get_timestamp();
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
                let client = self.sockets.iter().find(|c| c.value().contains(&client));

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
    pub(crate) fn add_robot(room: &mut RoomData, position: Vector3<Real>, orientation: UnitQuaternion<f32>, wheel_debug: bool, speed_mult: Option<f32>, scale: Option<f32>) -> String {
        let speed_mult = speed_mult.unwrap_or(1.0).clamp(-10.0, 10.0);
        let scale: f32 = scale.unwrap_or(1.0).clamp(1.0, 5.0);

        let simulation = &mut room.sim.lock().unwrap();
        let mut robot = RobotData::create_robot_body(simulation, None, Some(position), Some(orientation), Some(scale));
        robot.speed_scale = speed_mult;
        let robot_id: String = "robot_".to_string() + robot.id.as_str();
        simulation.rigid_body_labels.insert(robot_id.clone(), robot.body_handle);
        room.objects.insert(robot_id.clone(), ObjectData {
            name: robot_id.clone(),
            transform: Transform {scaling: vector![scale * SCALE, scale * SCALE, scale * SCALE], ..Default::default() },
            visual_info: Some(VisualInfo::Mesh("parallax_robot.glb".into())),
            is_kinematic: false,
            updated: true,
        });
        RobotData::setup_robot_socket(&mut robot);

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
        let mut position = position;

        // Apply jitter with extra objects to prevent lag from overlap
        let count_non_robots = room.count_non_robots();
        if count_non_robots > 10 {
            let mut rng = rand::thread_rng();
            let mult = if count_non_robots > 40 { 2.0 } else if count_non_robots > 20 { 1.5 } else { 1.0 };
            let jitter = vector![rng.gen_range(-0.0015..0.0015) * mult, rng.gen_range(-0.0025..0.0025) * mult, rng.gen_range(-0.0015..0.0015) * mult];
            position += jitter;
        }
        
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
        let collider = collider.restitution(0.3).density(0.045).friction(0.6).build();
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
        
        room.last_full_update = 0;
        body_name
    }

    /// Add a service to the room
    pub(crate) fn add_sensor<'a, T: ServiceFactory>(&mut self, id: &'a str, config: T::Config) -> &'a str {
        let service = Arc::new(T::create(id, config));
        self.services.insert((id.into(), service.get_service_info().service_type), service);
        id
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

        room.objects.insert(body_name.clone(), ObjectData {
            name: body_name.clone(),
            transform: Transform { position: position.into(), scaling: size, rotation: Orientation::Euler(rotation), ..Default::default() },
            visual_info: Some(VisualInfo::None),
            is_kinematic: true,
            updated: true,
        });

        room.reseters.insert(body_name.clone(), Box::new(RigidBodyResetter::new(cube_body_handle, simulation)));

        let service = Arc::new(TriggerService::create(&body_name, &cube_body_handle));
        let service_id = service.get_service_info().id.clone();
        room.services.insert((service_id.clone(), ServiceType::Trigger), service);
        simulation.sensors.insert((service_id, collider_handle), DashSet::new());
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
            simulation.cleanup_robot(self.robots.get(id).unwrap().value());
            self.robots.remove(id);
        }

        self.send_to_all_clients(&UpdateMessage::RemoveObject(id.to_string()));
    }

    pub(crate) fn remove_all(&mut self) {
        info!("Removing all entities from {}", self.name);
        self.objects.clear();

        // Remove non-world services
        self.services.retain(|k, _| k.1 == ServiceType::World);

        let simulation = &mut self.sim.lock().unwrap();
        let labels = simulation.rigid_body_labels.clone();
        for l in labels.iter() {
            if !l.key().starts_with("robot_") {
                simulation.remove_body(*l.value());
            }
        }

        simulation.rigid_body_labels.clear();

        for r in self.robots.iter() {
            simulation.cleanup_robot(r.value());
        }
        self.robots.clear();
        self.send_to_all_clients(&UpdateMessage::RemoveAll());
        info!("All entities removed from {}", self.name);
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

    pub(crate) fn get_room_info(&self) -> RoomInfo {
        RoomInfo{
            id: self.name.clone(),
            environment: self.environment.clone(),
            server: get_server().to_owned(),
            creator: "TODO".to_owned(),
            has_password: self.password.is_some(),
            is_hibernating: self.hibernating.load(std::sync::atomic::Ordering::Relaxed),
            visitors: self.visitors.clone().into_iter().collect(),
        }
    }

    pub fn announce(&self) {
        let room_info = self.get_room_info();
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
    if !room.visitors.contains(&username.to_owned()) {
        room.visitors.insert(username.to_owned());
    }

    if !room.sockets.contains_key(username) {
        room.sockets.insert(username.to_string(), DashSet::new());
    }

    room.sockets.get_mut(username).unwrap().insert(peer_id);
    room.last_interaction_time = get_timestamp();

    // Give client initial update
    room.send_info_to_client(peer_id);
    room.send_state_to_client(true, peer_id);

    // Send room info to API
    room.announce();

    // Initial robot claim data
    for robot in room.robots.iter() {
        if robot.value().claimed_by.is_some() {   
            RoomData::send_to_client(&UpdateMessage::RobotClaimed(robot.key().clone(), robot.value().claimed_by.clone().unwrap_or("".to_owned())), peer_id);
        }
    }

    // Send user join event
    let world_service_id = room.services.iter().find(|s| s.key().1 == ServiceType::World).unwrap().value().get_service_info().id.clone();
    room.netsblox_msg_tx.send(((world_service_id, ServiceType::World), "userJoined".to_string(), BTreeMap::from([("username".to_owned(), username.to_owned())]))).unwrap();

    Ok(())
}

pub async fn create_room(environment: Option<String>, password: Option<String>, edit_mode: bool) -> String {
    let room = Arc::new(Mutex::new(RoomData::new(None, environment, password, edit_mode)));
    
    // Set last interaction to creation time
    room.lock().unwrap().last_interaction_time = get_timestamp();

    let room_id = room.lock().unwrap().name.clone();
    ROOMS.insert(room_id.to_string(), room.clone());
    room_id
}