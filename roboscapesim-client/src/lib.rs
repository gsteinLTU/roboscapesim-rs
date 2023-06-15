#![allow(dead_code)]

use js_sys::{Reflect, Function};
use netsblox_extension_macro::*;
use netsblox_extension_util::*;
use wasm_bindgen::{prelude::wasm_bindgen, JsCast, JsValue};
use web_sys::{console, window};
use neo_babylon::{prelude::*};
extern crate console_error_panic_hook;
use std::panic;

#[netsblox_extension_info]
const INFO: ExtensionInfo = ExtensionInfo { 
    name: "RoboScape Online" 
};

#[wasm_bindgen(start)]
pub fn main() {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console::log_1(&"RoboScape Online loaded!".to_owned().into());
    neo_babylon::api::create_basic_scene("#roboscape-canvas");
}

#[netsblox_extension_menu_item("Show 3D View")]
#[wasm_bindgen()]
pub fn show_3d_view() {
    let window = window().unwrap();
    let external_vars = Reflect::get(&window, &"externalVariables".into()).unwrap();
    let dialog = Reflect::get(&external_vars, &"roboscapedialog".into()).unwrap();
    let f = Reflect::get(&window, &"showDialog".into()).unwrap().unchecked_into::<Function>();
    f.call1(&JsValue::NULL, &dialog).unwrap();
}