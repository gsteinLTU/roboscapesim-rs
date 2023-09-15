use std::{fmt, rc::Rc};
use std::time::Duration;
use netsblox_vm::{ast, real_time::UtcOffset, runtime::{Config, CustomTypes, Value, GetType, Key, EntityKind, IntermediateType, ErrorCause, FromAstError, Settings, RequestStatus, Request, ToJsonError}, gc::{Mutation, Collect, RefLock, Gc, Arena, Rootable}, json::{Json, json}, project::Project, bytecode::{Locations, ByteCode}, std_system::StdSystem};
use std::sync::mpsc::Sender;

pub const SAMPLE_PROJECT: &'static str = include_str!("Default Scenario.xml");

pub const DEFAULT_BASE_URL: &'static str = "https://editor.netsblox.org";
pub const STEPS_PER_IO_ITER: usize = 64;
pub const YIELDS_BEFORE_IDLE_SLEEP: usize = 256;
pub const IDLE_SLEEP_TIME: Duration = Duration::from_micros(500);

#[derive(Collect)]
#[collect(no_drop, bound = "")]
pub struct Env<'gc, C: CustomTypes<StdSystem<C>>> {
                               pub proj: Gc<'gc, RefLock<Project<'gc, C, StdSystem<C>>>>,
    #[collect(require_static)] pub locs: Locations,
}
pub type EnvArena<S> = Arena<Rootable![Env<'_, S>]>;

pub fn get_env<C: CustomTypes<StdSystem<C>>>(role: &ast::Role, system: StdSystem<C>) -> Result<EnvArena<C>, FromAstError> {
    let (bytecode, init_info, locs, _) = ByteCode::compile(role).unwrap();
    Ok(EnvArena::new(Default::default(), |mc| {
        let proj = Project::from_init(mc, &init_info, Rc::new(bytecode), Settings::default(), Rc::new(system));
        Env { proj: Gc::new(mc, RefLock::new(proj)), locs }
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeType {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeValue {}
impl GetType for NativeValue {
    type Output = NativeType;
    fn get_type(&self) -> Self::Output {
        unreachable!()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityState;
impl From<EntityKind<'_, '_, C, StdSystem<C>>> for EntityState {
    fn from(_: EntityKind<'_, '_, C, StdSystem<C>>) -> Self {
        EntityState
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

pub async fn load_project(project_name: &str, role: &ast::Role, iotscape_tx: Sender<iotscape::Request>) -> Result<EnvArena<C>, String> {

    let config = Config {
        request: Some(Rc::new(move |system: &StdSystem<C>, _, key, request, _| {
            match &request {
                Request::Rpc { service, rpc, args } => {
                    match args.into_iter().map(|(k, v)| Ok(v.to_json()?)).collect::<Result<Vec<_>,ToJsonError<_,_>>>() {
                        Ok(args) => {
                            match service.as_str() {
                                "RoboScapeWorld" | "RoboScapeEntity" | "RoboScape" | "PositionSensor" | "LIDAR" => {
                                    println!("{:?}", (service, rpc, &args));
                                    iotscape_tx.send(iotscape::Request { id: "".into(), service: service.to_owned(), device: args[0].to_string(), function: rpc.to_owned(), params: args.iter().skip(1).map(|v| v.to_owned()).collect() }).unwrap();
                                    key.complete(Ok(Intermediate::Json(json!(""))));
                                },
                                _ => return RequestStatus::UseDefault { key, request },
                            }
                        },
                        Err(err) => key.complete(Err(format!("failed to convert RPC args to string: {err:?}"))),
                    }
                    RequestStatus::Handled
                }
                _ => RequestStatus::UseDefault { key, request },
            }
        })),
        command: None,
    };

    let system = StdSystem::new_async(DEFAULT_BASE_URL.to_owned(), Some(project_name), config, UtcOffset::UTC).await;
    println!(">>> public id: {}\n", system.get_public_id());

    match get_env(role, system) {
        Ok(x) => Ok(x),
        Err(e) => {
            Err(format!(">>> error loading project: {e:?}").to_owned())         
        }
    }
}

