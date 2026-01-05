use std::sync::{Arc, Weak};

use iotscape::Request;
use netsblox_vm::{compact_str::CompactString, std_util::AsyncKey};

use super::*;

#[derive(Debug, Default)]
pub struct VMManager {
    vm_thread: OnceCell<JoinHandle<()>>,
    room: Weak<RoomData>,
}

impl VMManager {
    pub fn new(room: Weak<RoomData>) -> Self {
        Self {
            vm_thread: OnceCell::new(),
            room,
        }
    }

    fn with_room<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&RoomData) -> R,
    {
        self.room.upgrade().map(|room| f(&*room))
    }

    pub fn start(&self, iotscape_tx: &mpsc::Sender<(Request, Option<AsyncKey<Result<SimpleValue, CompactString>>>)>, vm_netsblox_msg_rx: Arc<Mutex<mpsc::Receiver<((String, ServiceType), String, BTreeMap<String, String>)>>>) {
        if self.vm_thread.get().is_some() {
            warn!("VM thread already started");
            return;
        }

        self.with_room(|room| {
            let vm_iotscape_tx = iotscape_tx.clone();
            let hibernating = room.metadata.hibernating.clone();
            let hibernating_since = room.metadata.hibernating_since.clone();
            let id_clone = room.metadata.name.clone();
            let id_clone2 = room.metadata.name.clone();
            let robots = room.robots.clone();
            let is_alive = room.is_alive.clone();
            let environment = room.metadata.environment.clone();

            self.vm_thread.set(thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let project = load_environment(Some(environment)).await;

                    // Setup VM
                    let (project_name, role) = open_project(&project).unwrap_or_else(|_| panic!("failed to read file"));
                    let mut idle_sleeper = IdleAction::new(YIELDS_BEFORE_IDLE_SLEEP, Box::new(|| thread::sleep(IDLE_SLEEP_TIME)));
                    info!("Loading project {}", project_name);
                    let system = Rc::new(StdSystem::new_async(DEFAULT_BASE_URL.to_owned().into(), Some(&project_name), Config {
                        request: Some(Rc::new(move |_mc, key, request: netsblox_vm::runtime::Request<'_, C, StdSystem<C>>,  _proc| {
                            match &request {
                                netsblox_vm::runtime::Request::Rpc { host: _, service, rpc, args } => {
                                    match args.iter().map(|(_k, v)| Ok(v.to_simple()?.into_json()?)).collect::<Result<Vec<_>,ErrorCause<_,_>>>() {
                                        Ok(args) => {
                                            match service.as_str() {
                                                "RoboScapeWorld" |
                                                "RoboScapeEntity" |
                                                "PositionSensor" |
                                                "LIDARSensor" |
                                                "ProximitySensor" |
                                                "RoboScapeTrigger" |
                                                "WaypointList" 
                                                    => {
                                                    // Keep IoTScape services local
                                                    //println!("{:?}", (service, rpc, &args));
                                                    let msg = (iotscape::Request { client_id: None, id: "".into(), service: service.to_owned().into(), device: args[0].to_string().replace("\"", "").replace("\\", ""), function: rpc.to_owned().into(), params: args.iter().skip(1).map(|v| v.to_owned()).collect() }, Some(key));
                                                    vm_iotscape_tx.send(msg).unwrap();
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
                                            key.complete(Ok(SimpleValue::Text(format!("{id_clone}").into())));
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
                        if !is_alive.load(Ordering::Relaxed) {
                            break;
                        }
                        
                        if hibernating.load(Ordering::Relaxed) && hibernating_since.load(Ordering::Relaxed) < get_timestamp() + 2 {
                            sleep(Duration::from_millis(50)).await;
                        } else {

                            if let Ok((_service_id, msg_type, values)) = vm_netsblox_msg_rx.lock().unwrap().recv_timeout(Duration::ZERO) {
                                // TODO: check for listen
                                system.inject_message(msg_type.into(), values.iter().map(|(k, v)| (k.clone().into(), SimpleValue::Text(v.clone().into()))).collect());
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
            })).unwrap();
            info!("VM thread started");
        });
    }
}

impl Drop for VMManager {
    fn drop(&mut self) {
        if let Some(handle) = self.vm_thread.take() {
            info!("Stopping VM thread");
            handle.join().unwrap();
            info!("VM thread stopped");
        } else {
            warn!("VM thread was not started or already stopped");
        }
    }
}