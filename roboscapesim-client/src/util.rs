use js_sys::Reflect;
use roboscapesim_client_common::util::*;
use wasm_bindgen::JsValue;
use web_sys::window;

use crate::GAME;

/// Try to get a value from window.externalVariables
pub(crate) fn get_nb_externalvar(name: &str) -> Result<JsValue, JsValue>
{
    let window = window().unwrap();
    let external_vars = Reflect::get(&window, &"externalVariables".into()).unwrap();
    Reflect::get(&external_vars, &name.into())
}

/// Try to get NetsBlox username
pub(crate) fn get_username() -> String
{
    //world.children[0].cloud.username
    let ide = get_ide();
    let cloud = Reflect::get(&ide, &"cloud".into()).unwrap();

    // If the username is not set, use the CLIENT_ID (although this is now less reliable)
    Reflect::get(&cloud, &"username".into()).unwrap().as_string().unwrap_or_else(|| {
        js_get(&window().unwrap(), "CLIENT_ID").unwrap().as_string().unwrap_or("Unknown".to_owned())
    })
}

/// Get the NetsBloxMorph
fn get_ide() -> JsValue {
    let window = window().unwrap();
    let world = Reflect::get(&window, &"world".into()).unwrap();
    Reflect::get(&Reflect::get(&world, &"children".into()).unwrap(), &0.into()).unwrap()
}

/// Get the robot selected in the dropdown (or None if none selected)
pub(crate) fn get_selected_robot() -> Option<String> {
    let robotmenu = get_nb_externalvar("roboscapedialog-robotmenu").unwrap();
    let value = js_get(&robotmenu, "value").unwrap().as_string().unwrap();
    let value = value.trim();
    match value {
        "" => None,
        v => Some(v.to_owned()),
    }
}

/// Reset the camera to the default position and type
pub(crate) fn reset_camera() {
    GAME.with(|game| {
        game.borrow().reset_camera();
    });
}

/// Show a message box
pub(crate) fn show_message(title: &str, body: &str) {
    get_window_fn("pmAlert").unwrap().call2(&JsValue::NULL, &title.into(), &format!("<strong>{}</strong>", body).into()).unwrap();
}
