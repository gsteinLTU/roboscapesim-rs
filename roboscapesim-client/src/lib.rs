#![allow(dead_code)]

use netsblox_extension_macro::*;
use netsblox_extension_util::*;
use wasm_bindgen::prelude::wasm_bindgen;
use web_sys::console;
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
}
