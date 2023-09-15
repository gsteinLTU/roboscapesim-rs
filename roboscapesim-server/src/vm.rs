use std::{fmt, fs::File, io::{BufReader, BufWriter}, rc::Rc, cell::RefCell, thread, sync::{Arc, Mutex}};
use std::time::Duration;
use netsblox_vm::{ast, real_time::UtcOffset, runtime::{Config, CustomTypes, Value, GetType, Key, EntityKind, IntermediateType, ErrorCause, FromAstError, Settings, RequestStatus, Request, ToJsonError}, gc::{Mutation, Collect, RefLock, Gc, Arena, Rootable}, json::{Json, json}, project::{ProjectStep, Input, IdleAction, Project}, bytecode::{Locations, ByteCode}, std_system::StdSystem};

use crate::room::RoomData;

pub const SAMPLE_PROJECT: &'static str = include_str!("Default Scenario.xml");

const DEFAULT_BASE_URL: &'static str = "https://editor.netsblox.org";
const STEPS_PER_IO_ITER: usize = 64;
const YIELDS_BEFORE_IDLE_SLEEP: usize = 256;
const IDLE_SLEEP_TIME: Duration = Duration::from_micros(500);

#[derive(Collect)]
#[collect(no_drop, bound = "")]
pub struct Env<'gc, C: CustomTypes<StdSystem<C>>> {
                               proj: Gc<'gc, RefLock<Project<'gc, C, StdSystem<C>>>>,
    #[collect(require_static)] locs: Locations,
}
pub type EnvArena<S> = Arena<Rootable![Env<'_, S>]>;

fn get_env<C: CustomTypes<StdSystem<C>>>(role: &ast::Role, system: Rc<StdSystem<C>>) -> Result<EnvArena<C>, FromAstError> {
    let (bytecode, init_info, locs, _) = ByteCode::compile(role).unwrap();
    Ok(EnvArena::new(Default::default(), |mc| {
        let proj = Project::from_init(mc, &init_info, Rc::new(bytecode), Settings::default(), system);
        Env { proj: Gc::new(mc, RefLock::new(proj)), locs }
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeType {}

#[derive(Debug)]
pub enum NativeValue {}
impl GetType for NativeValue {
    type Output = NativeType;
    fn get_type(&self) -> Self::Output {
        unreachable!()
    }
}


pub struct EntityState;
impl From<EntityKind<'_, '_, C, StdSystem<C>>> for EntityState {
    fn from(_: EntityKind<'_, '_, C, StdSystem<C>>) -> Self {
        EntityState
    }
}

pub enum Intermediate {
    Json(Json),
    Image(Vec<u8>),
    Audio(Vec<u8>),
}
impl IntermediateType for Intermediate {
    fn from_json(json: Json) -> Self {
        Self::Json(json)
    }
    fn from_image(img: Vec<u8>) -> Self {
        Self::Image(img)
    }
    fn from_audio(audio: Vec<u8>) -> Self {
        Self::Audio(audio)
    }
}

pub struct C;
impl CustomTypes<StdSystem<C>> for C {
    type NativeValue = NativeValue;
    type Intermediate = Intermediate;

    type EntityState = EntityState;

    fn from_intermediate<'gc>(mc: &Mutation<'gc>, value: Self::Intermediate) -> Result<Value<'gc, C, StdSystem<C>>, ErrorCause<C, StdSystem<C>>> {
        Ok(match value {
            Intermediate::Json(x) => Value::from_json(mc, x)?,
            Intermediate::Image(x) => Value::Image(Rc::new(x)),
            Intermediate::Audio(x) => Value::Audio(Rc::new(x)),
        })
    }
}

#[derive(Debug)]
pub enum OpenProjectError<'a> {
    ParseError { error: Box<ast::Error> },
    RoleNotFound { role: &'a str },
    NoRoles,
    MultipleRoles { count: usize },
}
impl fmt::Display for OpenProjectError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenProjectError::ParseError { error } => write!(f, "failed to parse project: {error:?}"),
            OpenProjectError::RoleNotFound { role } => write!(f, "no role named '{role}'"),
            OpenProjectError::NoRoles => write!(f, "project had no roles"),
            OpenProjectError::MultipleRoles { count } => write!(f, "project had multiple ({count}) roles, but a specific role was not specified"),
        }
    }
}

pub fn open_project<'a>(content: &str) -> Result<(String, ast::Role), OpenProjectError<'a>> {
    let parsed = match ast::Parser::default().parse(content) {
        Ok(x) => x,
        Err(error) => return Err(OpenProjectError::ParseError { error }),
    };
    let role = match parsed.roles.len() {
        0 => return Err(OpenProjectError::NoRoles),
        // Always use first role
        _ => parsed.roles.into_iter().next().unwrap(),
    };
    Ok((parsed.name, role))
}

pub fn load_project(project_name: &str, role: &ast::Role, room: Arc<Mutex<RoomData>>) -> Result<EnvArena<C>, String> {
    let config = Config::default().fallback(&Config {
        request: Some(Rc::new(|system: &StdSystem<C>, _, key, request, _| {
            match &request {
                Request::Rpc { service, rpc, args } => {
                    match args.into_iter().map(|(k, v)| Ok(v.to_json()?)).collect::<Result<Vec<_>,ToJsonError<_,_>>>() {
                        Ok(args) => {
                            match service.as_str() {
                                "RoboScapeWorld" => {
                                    println!("{:?}", (service, rpc, args));
                                    key.complete(Ok(Intermediate::Json(json!(""))));
                                },
                                "RoboScapeEntity" => {
                                    println!("{:?}", (service, rpc, args));
                                },
                                "RoboScape" => {
                                    println!("{:?}", (service, rpc, args));
                                },
                                "PositionSensor" => {
                                    println!("{:?}", (service, rpc, args));
                                },
                                "LIDAR" => {
                                    println!("{:?}", (service, rpc, args));
                                },
                                _ => return RequestStatus::UseDefault { key, request },
                            }
                        },
                        Err(err) => key.complete(Err(format!("failed to convert RPC args to json: {err:?}"))),
                    }
                    RequestStatus::Handled
                }
                _ => RequestStatus::UseDefault { key, request },
            }
        })),
        command: None,
    });

    let system = Rc::new(StdSystem::new(DEFAULT_BASE_URL.to_owned(), Some(project_name), config, UtcOffset::UTC));
    println!(">>> public id: {}\n", system.get_public_id());

    match get_env(role, system) {
        Ok(x) => Ok(x),
        Err(e) => {
            Err(format!(">>> error loading project: {e:?}").to_owned())         
        }
    }
}

