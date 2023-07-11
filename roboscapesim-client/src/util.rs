use std::rc::Rc;

use js_sys::{Function, Reflect};
use neo_babylon::prelude::{BabylonMesh, Vector3, Quaternion};
use wasm_bindgen::{JsValue, JsCast};
use web_sys::window;


// Try to get a value from window.externalVariables
pub(crate) fn get_nb_externalvar(name: &str) -> Result<JsValue, JsValue>
{
    let window = window().unwrap();
    let external_vars = Reflect::get(&window, &"externalVariables".into()).unwrap();
    Reflect::get(&external_vars, &name.into())
}

/// Try to get a function from the window
pub(crate) fn get_window_fn(name: &str) -> Result<Function, JsValue>
{
    let result = Reflect::get(&window().unwrap(), &name.into());

    match result {
        Ok(f) => Ok(f.unchecked_into::<Function>()),
        Err(e) => Err(e),
    }
}

/// Gets performance.now()
pub(crate) fn performance_now() -> f64 {
    window().unwrap().performance().unwrap().now()
}

/// Apply a transform to a BabylonMesh 
pub(crate) fn apply_transform(m: Rc<BabylonMesh>, transform: roboscapesim_common::Transform) {
    m.set_position(&Vector3::new(transform.position.x.into(), transform.position.y.into(), transform.position.z.into()));

    match transform.rotation {
        roboscapesim_common::Orientation::Euler(angles) => m.set_rotation(&Vector3::new(angles.x.into(), angles.y.into(), angles.z.into())),
        roboscapesim_common::Orientation::Quaternion(q) => m.set_rotation_quaternion(&Quaternion::new(q.i.into(), q.j.into(), q.k.into(), q.w.into())),
    }

    m.set_scaling(&Vector3::new(transform.scaling.x.into(), transform.scaling.y.into(), transform.scaling.z.into()));
}

pub(crate) fn js_set<T>(target: &JsValue, prop: &str, val: T) -> Result<bool, JsValue>
where JsValue: From<T> {
    Reflect::set(target, &prop.into(), &JsValue::from(val))
}

#[macro_export]
macro_rules! console_log {
    ($($tokens: tt)*) => {
        console::log_1(&format!($($tokens)*).into())
    }
}