use crate::room::Simulation;

pub trait Resettable : Send + Sync {
    fn reset(&mut self, sim: &mut Simulation);
}