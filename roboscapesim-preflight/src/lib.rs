use std::{rc::Rc, cell::RefCell};

use gloo_timers::future::sleep;
use instant::Duration;
use js_sys::Uint8Array;
use roboscapesim_common::{api::CreateRoomResponseData, UpdateMessage, ClientMessage};
use wasm_bindgen::prelude::*;

use roboscapesim_client_common::{api::*, console_log, ASSETS_DIR, util::js_get};
use web_sys::WebSocket;

#[wasm_bindgen(start)]
async fn main() {
    console_error_panic_hook::set_once();
    console_log!("Assets dir: {}", ASSETS_DIR);
    console_log!("API server: {}", API_SERVER);
}

/// Test ability to connect to API server
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

thread_local! {
    static LAST_MESSAGE: RefCell<Option<UpdateMessage>> = RefCell::new(None);
}

#[wasm_bindgen]
/// Test ability to create a room
pub async fn step2() -> Result<(), JsValue> {
    // Test create room
    let response = request_room("test".to_string(), None, false, None).await.map_err(|err| JsValue::from_str(&err.to_string()))?;

    ROOM_CREATE_RESPONSE.with(|r| {
        *r.borrow_mut() = Some(response);
    });

    Ok(())
}

#[wasm_bindgen]
/// Test ability to join a room
pub async fn step3() -> Result<(), JsValue> {
    // Verify room response
    let response = ROOM_CREATE_RESPONSE.with(|r| {
        r.borrow().clone()
    }).ok_or_else(|| JsValue::from_str("No room create response"))?;

    // Join room
    // Connect to websocket
    WEBSOCKET.with(|socket| {
        let s = WebSocket::new(&response.server).unwrap();
        let s = Rc::new(RefCell::new(s));
        s.borrow().set_binary_type(web_sys::BinaryType::Arraybuffer);

        // Set callbacks
        let onmessage: Closure<(dyn Fn(JsValue) -> _ + 'static)> = Closure::new(move |evt: JsValue| {
            let mut msg = None;
            let data = js_get(&evt, "data").unwrap();

            if data.is_string() {
                let parsed = serde_json::from_str(&data.as_string().unwrap());
                if let Ok(parsed) = parsed {
                    msg = Some(parsed);
                } else if let Err(e) = parsed {
                    console_log!("Failed to parse JSON: {}", e);
                }
            } else if let Ok(array_buffer) = data.clone().dyn_into::<js_sys::ArrayBuffer>() {
                // Convert the ArrayBuffer to a Uint8Array
                let data = Uint8Array::new(&array_buffer).to_vec();
                
                let parsed = rmp_serde::from_slice(data.as_slice());

                if let Ok(parsed) = parsed {
                    msg = Some(parsed);
                } else if let Err(e) = parsed {
                    console_log!("Failed to parse MessagePack: {}", e);
                }
            } else {
                console_log!("Unknown message type: {:?}", &data);
            }

            if let Some(msg) = msg {
                handle_update_message(msg);
            }
        });
        s.borrow().set_onmessage(Some(onmessage.into_js_value().unchecked_ref()));
        s.borrow().set_onclose(Some(&Closure::<(dyn Fn() -> _ + 'static)>::new(move ||{
            console_log!("close");
        }).into_js_value().unchecked_ref()));
        s.borrow().set_onerror(Some(&Closure::<(dyn Fn() -> _ + 'static)>::new(||{
            console_log!("error");
        }).into_js_value().unchecked_ref()));
        s.borrow().set_onopen(Some(&Closure::<(dyn Fn() -> _ + 'static)>::new(||{
            console_log!("open");
        }).into_js_value().unchecked_ref()));
        socket.replace(Some(s));  
    });

    // Wait for connection
    let mut attempts = 0;
    let mut status = 0;
    loop {
        // Timeout after 10 seconds
        if attempts > (10000 / 25) {
            return Err(JsValue::from_str("Failed to connect to websocket"));
        }

        sleep(Duration::from_millis(25)).await;

        WEBSOCKET.with(|socket| {
            status = socket.borrow().clone().unwrap().clone().borrow().ready_state();
        });

        if status != WebSocket::CONNECTING {
            break;
        }

        attempts += 1;
    }

    // Check status
    if status == WebSocket::CLOSED || status == WebSocket::CLOSING {
        return Err(JsValue::from_str("Failed to connect to websocket"));
    }

    // Send room join message
    let join_msg = ClientMessage::JoinRoom(response.room_id.clone(), "test".to_owned(), None);
    WEBSOCKET.with(|socket| {
        let socket = socket.borrow().clone().unwrap();
        let socket = socket.borrow();
        let buf = rmp_serde::to_vec(&join_msg).unwrap();
        socket.send_with_u8_array(&buf).unwrap();
    });

    Ok(())
}

#[wasm_bindgen]
/// Test ability to receive messages
pub async fn step4() -> Result<(), JsValue> {
    // Give the server a chance to send a message
    sleep(Duration::from_millis(250)).await;

    // Check for received messages
    let mut last_message = None;
    LAST_MESSAGE.with(|msg| {
        let msg = msg.borrow().clone();
        if let Some(msg) = msg {
            last_message.replace(msg);
        }
    });
    
    match last_message {
        Some(_) => Ok(()),
        None => Err(JsValue::from_str("No message received")),
    }
}

fn handle_update_message(msg: UpdateMessage) {
    LAST_MESSAGE.with(|m| {
        *m.borrow_mut() = Some(msg);
    });
}