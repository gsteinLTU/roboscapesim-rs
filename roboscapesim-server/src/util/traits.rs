use crate::room::Simulation;

pub trait Resettable {
    fn reset(&mut self, sim: &mut Simulation);
}