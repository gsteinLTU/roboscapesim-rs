use std::{rc::Rc, cell::RefCell};

use reqwest::Client;
use roboscapesim_common::api::CreateRoomResponseData;
use wasm_bindgen::prelude::*;

use roboscapesim_client_common::{api::*, console_log, ASSETS_DIR};
use web_sys::WebSocket;

thread_local! {
    /// Allows reuse of client
    static REQWEST_CLIENT: Rc<Client> = Rc::new(Client::new());
}

#[wasm_bindgen(start)]
async fn main() {
    console_error_panic_hook::set_once();
    console_log!("Assets dir: {}", ASSETS_DIR);
    console_log!("API server: {}", API_SERVER);
}

#[wasm_bindgen]
pub async fn step1() -> Result<(), JsValue> {
    // Test API connection
    get_environments().await.map_err(|err| JsValue::from_str(&err.to_string()))?;
    Ok(())
}

thread_local! {
    static ROOM_CREATE_RESPONSE: RefCell<Option<CreateRoomResponseData>> = RefCell::new(None);
}

thread_local! {
    static WEBSOCKET: RefCell<Option<Rc<RefCell<WebSocket>>>> = RefCell::new(None);
}

#[wasm_bindgen]
pub async fn step2() -> Result<(), JsValue> {
    // Test join room
    let response = request_room("test".to_string(), None, false, None).await.map_err(|err| JsValue::from_str(&err.to_string()))?;

    ROOM_CREATE_RESPONSE.with(|r| {
        *r.borrow_mut() = Some(response);
    });

    Ok(())
}

#[wasm_bindgen]
pub async fn step3() -> Result<(), JsValue> {
    // Test room connection worked
    unimplemented!();
    Ok(())
}