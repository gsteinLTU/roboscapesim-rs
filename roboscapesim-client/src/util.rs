use js_sys::{Function, Reflect};
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