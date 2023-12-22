use std::rc::Rc;
use js_sys::{Function, Reflect, Array};
use neo_babylon::prelude::{BabylonMesh, Vector3, Quaternion};
use wasm_bindgen::{JsValue, JsCast};
use web_sys::{window, Document};

#[macro_export]
/// Macro to make console.logging easier
macro_rules! console_log {
    ($($tokens: tt)*) => {
        web_sys::console::log_1(&format!($($tokens)*).into())
    }
}

/// Set a property on a JsValue
pub fn js_set<T>(target: &JsValue, prop: &str, val: T) -> Result<bool, JsValue>
where JsValue: From<T> {
    Reflect::set(target, &prop.into(), &JsValue::from(val))
}

/// Get a property from a JsValue
pub fn js_get(target: &JsValue, prop: &str) -> Result<JsValue, JsValue> {
    Reflect::get(target, &prop.into())
}

/// Construct a new object
pub fn js_construct(type_name: &str, arguments_list: &[&JsValue]) -> Result<JsValue, JsValue> {
    Reflect::construct(&Reflect::get(&window().unwrap(), &type_name.into()).unwrap().unchecked_into(), &Array::from_iter(arguments_list.into_iter()))
}

/// Call a method on a JsValue
pub fn js_call_member(target: &JsValue, fn_name: &str, arguments_list: &[&JsValue]) -> Result<JsValue, JsValue> {
    Reflect::apply(Reflect::get(&target, &fn_name.into()).unwrap().unchecked_ref(), &target, &Array::from_iter(arguments_list.into_iter()))
}

/// Helper to get document
pub fn document() -> Document {
    window().unwrap().document().unwrap()
}


/// Try to get a function from the window
pub fn get_window_fn(name: &str) -> Result<Function, JsValue>
{
    let result = Reflect::get(&window().unwrap(), &name.into());

    match result {
        Ok(f) => Ok(f.unchecked_into::<Function>()),
        Err(e) => Err(e),
    }
}

/// Gets performance.now()
pub fn performance_now() -> f64 {
    window().unwrap().performance().unwrap().now()
}

/// Apply a transform to a BabylonMesh 
pub fn apply_transform(m: Rc<BabylonMesh>, transform: roboscapesim_common::Transform) {
    m.set_position(&Vector3::new(transform.position.x.into(), transform.position.y.into(), transform.position.z.into()));

    match transform.rotation {
        roboscapesim_common::Orientation::Euler(angles) => m.set_rotation(&Vector3::new(angles.x.into(), angles.y.into(), angles.z.into())),
        roboscapesim_common::Orientation::Quaternion(q) => m.set_rotation_quaternion(&Quaternion::new(q.i.into(), q.j.into(), q.k.into(), q.w.into())),
    }

    m.set_scaling(&Vector3::new(transform.scaling.x.into(), transform.scaling.y.into(), transform.scaling.z.into()));
}