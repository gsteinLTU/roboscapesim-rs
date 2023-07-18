use crate::room::Simulation;

pub(crate) trait Resettable {
    fn reset(&mut self, sim: &mut Simulation);
}