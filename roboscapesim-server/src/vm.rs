use std::{fmt, rc::Rc};
use std::time::Duration;
use netsblox_vm::runtime::{GetType, ProcessKind, SimpleValue, Unwindable};
use netsblox_vm::{ast, runtime::{CustomTypes, Value, EntityKind, FromAstError, Settings}, gc::{Mutation, Collect, RefLock, Gc, Arena, Rootable}, project::Project, bytecode::{Locations, ByteCode}, std_system::StdSystem};

pub const DEFAULT_BASE_URL: &str = "https://cloud.netsblox.org";
pub const STEPS_PER_IO_ITER: usize = 64;
pub const YIELDS_BEFORE_IDLE_SLEEP: usize = 100;
pub const IDLE_SLEEP_TIME: Duration = Duration::from_millis(5);

#[derive(Collect)]
#[collect(no_drop, bound = "")]
pub struct Env<'gc, C: CustomTypes<StdSystem<C>>> {
                               pub proj: Gc<'gc, RefLock<Project<'gc, C, StdSystem<C>>>>,
    #[collect(require_static)] pub locs: Locations,
}
pub type EnvArena<S> = Arena<Rootable!['gc => Env<'gc, S>]>;

pub struct ProcessState;
impl From<ProcessKind<'_, '_, C, StdSystem<C>>> for ProcessState {
    fn from(_: ProcessKind<'_, '_, C, StdSystem<C>>) -> Self {
        ProcessState
    }
}

impl Unwindable for ProcessState {
    type UnwindPoint = (); // a type to represent process (script) state unwind points - we don't have any process state, so just use a unit struct
    fn get_unwind_point(&self) -> Self::UnwindPoint { }
    fn unwind_to(&mut self, _: &Self::UnwindPoint) { }
}

pub fn get_env<C: CustomTypes<StdSystem<C>>>(role: &ast::Role, system: Rc<StdSystem<C>>) -> Result<EnvArena<C>, FromAstError> {
    let (bytecode, init_info, locs, _) = ByteCode::compile(role)?;
    Ok(EnvArena::new(|mc| {
        let proj = Project::from_init(mc, &init_info, Rc::new(bytecode), Settings::default(), system);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct C;
impl CustomTypes<StdSystem<C>> for C {
    type NativeValue = NativeValue;
    type Intermediate = SimpleValue;

    type EntityState = EntityState;
    type ProcessState = ProcessState;

    fn from_intermediate<'gc>(mc: &Mutation<'gc>, value: Self::Intermediate) -> Value<'gc, C, StdSystem<C>> {
        Value::from_simple(mc, value)
    }
}

#[allow(dead_code)]
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
    Ok((parsed.name.to_string(), role))
} 

