pub mod resettable {
    use rapier3d::prelude::{Real, Isometry, RigidBodyHandle};
    use crate::simulation::Simulation;

    pub trait Resettable {
        fn reset(&mut self, sim: &mut Simulation);
    }

    /// Resets a rigid body to its initial conditions
    pub struct RigidBodyResetter {
        pub body_handle: RigidBodyHandle,
        pub(crate) initial_position: Isometry<Real>,
        pub(crate) initial_angvel: nalgebra::Matrix<f32, nalgebra::Const<3>, nalgebra::Const<1>, nalgebra::ArrayStorage<f32, 3, 1>>,
        pub(crate) initial_linvel: nalgebra::Matrix<f32, nalgebra::Const<3>, nalgebra::Const<1>, nalgebra::ArrayStorage<f32, 3, 1>>,
    }

    impl RigidBodyResetter {
        pub fn new(body_handle: RigidBodyHandle, sim: &Simulation) -> RigidBodyResetter {
            let binding = sim.rigid_body_set.lock().unwrap();
            let body = binding.get(body_handle).unwrap();
            RigidBodyResetter { body_handle, initial_position: body.position().to_owned(), initial_angvel: body.angvel().to_owned(), initial_linvel: body.linvel().to_owned()}
        }
    }

    impl Resettable for RigidBodyResetter {
        fn reset(&mut self, sim: &mut Simulation) {
            let rigid_body_set = &mut sim.rigid_body_set.lock().unwrap();
            if rigid_body_set.contains(self.body_handle){
                let body = rigid_body_set.get_mut(self.body_handle).unwrap();
                body.set_position(self.initial_position, true);
                body.set_angvel(self.initial_angvel, true);
                body.set_linvel(self.initial_linvel, true);
            }
        }
    }
}