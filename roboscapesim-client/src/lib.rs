#![allow(dead_code)]
mod util;
mod game;

use gloo_timers::future::sleep;
use instant::Duration;
use js_sys::{Reflect, Array};
use netsblox_extension_macro::*;
use netsblox_extension_util::*;
use reqwest::Client;
use roboscapesim_common::{UpdateMessage, ClientMessage, Interpolatable, api::{CreateRoomRequestData, CreateRoomResponseData}};
use wasm_bindgen::{prelude::{wasm_bindgen, Closure}, JsValue, JsCast};
use web_sys::{console, window, WebSocket};
use neo_babylon::prelude::*;
use std::{cell::RefCell, rc::Rc, sync::Arc};
use wasm_bindgen_futures::spawn_local;

use self::util::*;
use self::game::*;

extern crate console_error_panic_hook;

thread_local! {
    static WEBSOCKET: RefCell<Option<Rc<RefCell<WebSocket>>>> = RefCell::new(None);
}

thread_local! {
    static GAME: Rc<RefCell<Game>> = Rc::new(RefCell::new(Game::new()));
}

thread_local! {
    /// Allows reuse of client
    static REQWEST_CLIENT: Rc<Client> = Rc::new(Client::new());
}

#[netsblox_extension_info]
const INFO: ExtensionInfo = ExtensionInfo { 
    name: "RoboScape Online" 
};

#[wasm_bindgen(start)]
async fn main() {
    console_error_panic_hook::set_once();

    GAME.with(|game| {
        // Init game
        let game_clone = game.clone();
        let before_render = Closure::new(move || {
            let next_state = &game_clone.borrow().state;
            let last_state = &game_clone.borrow().last_state;
            let now = instant::now();
            let t = (now - game_clone.borrow().state_time.get()) / (game_clone.borrow().state_time.get() - game_clone.borrow().last_state_time.get());
            //console::log_1(&format!("t = {}, now = {}, last_state_time = {}, state_time = {}", t, now, *game_clone.borrow().last_state_time.borrow(), *game_clone.borrow().state_time.borrow()).into());

            for update_obj in next_state.borrow().iter() {
                let name = update_obj.0;
                let update_obj = update_obj.1;
                
                if !game_clone.borrow().models.borrow().contains_key(name) {
                    continue;
                }

                // Don't update objects not loaded yet
                if last_state.borrow().contains_key(name) {
                    // Interpolate
                    let last_transform = last_state.borrow().get(name).unwrap().transform;
                    let interpolated_transform = last_transform.try_interpolate(&update_obj.transform, t as f32).unwrap_or(update_obj.transform);

                    //console::log_1(&format!("{}: last_transform: {:?} \n next_transform: {:?} \ninterpolated_transform = {:?}", name, last_transform, update_obj.transform, interpolated_transform).into());

                    apply_transform(game_clone.borrow().models.borrow().get(name).unwrap().clone(), interpolated_transform);
                } else {
                    // Assign directly
                    apply_transform(game_clone.borrow().models.borrow().get(name).unwrap().clone(), update_obj.transform);
                }
            }
        });
        game.borrow().scene.borrow().add_before_render_observable(before_render);
        init_ui();
    });
    
    console_log!("RoboScape Online loaded!");
}

/// Set up UI elements for the 3D view window
fn init_ui() {
    create_button("Reset", Closure::new(|| { 
        console_log!("Reset");

        // Send reset message
        // TODO: Allow robot reset requests too
        send_message(&ClientMessage::ResetAll);
    }));
}

fn send_message(msg: &ClientMessage) {
    WEBSOCKET.with(|socket| {
        let socket = socket.borrow().clone();
        if socket.is_none() {
            console_log!("Attempt to send without socket!");
        } else if let Some(socket) = socket {
            let socket = socket.borrow();
            let message = serde_json::to_string(msg).unwrap();
            let message = message.as_str();
            socket.send_with_str(message).unwrap();
        }
    });
}

/// Process an UpdateMessage from the server
fn handle_update_message(msg: Result<UpdateMessage, serde_json::Error>, game: &Rc<RefCell<Game>>) {
    match msg {
        Ok(UpdateMessage::Heartbeat) => {
            send_message(&ClientMessage::Heartbeat);
        },
        Ok(UpdateMessage::RoomInfo(state)) => {
            set_title(&state.name);
        },
        Ok(UpdateMessage::Update(t, full_update, roomdata)) => {
            let view = roomdata.to_owned();
            for obj in view.into_read_only().iter() {
                let name = obj.0;
                let obj = obj.1;

                if !game.borrow().models.borrow().contains_key(name) {
                    if obj.visual_info.is_none() {
                        continue;
                    }

                    // Create new mesh
                    create_object(obj, game);
                }
            }

            // Update state vars
            for entry in game.borrow().state.borrow().iter() {
                game.borrow().last_state.borrow_mut().insert(entry.0.to_owned(), entry.1.clone());
            }
            for entry in &roomdata {
                game.borrow().state.borrow_mut().insert(entry.key().to_owned(), entry.value().clone());
            }

            // TODO: handle removed entities (server needs way to notify, full updates should also be able to remove)
        
            // Update times
            game.borrow().last_state_server_time.replace(game.borrow().state_server_time.get().clone());
            game.borrow().last_state_time.replace(game.borrow().state_time.get().clone());
            game.borrow().state_server_time.replace(t);
            game.borrow().state_time.replace(instant::now());
        },
        Ok(UpdateMessage::DisplayText(id, text, duration)) => {
            // TODO: show on canvas
            console_log!("Display Text \"{}\" in position {} for {} s", text, id, duration);
        },
        Ok(UpdateMessage::Beep(id, freq, duration)) => {
            if BEEPS_ENABLED.get() {
                // TODO: change volume based on distance to location?
                console_log!("Beep {} {}", freq, duration);
                create_beep(game, id, freq, duration);
            } else {
                console_log!("Beep received, but beeps are disabled");
            }
        },
        Err(e) => console_log!("Failed to deserialize: {}", e),
    }
}

fn create_beep(game: &Rc<RefCell<Game>>, id: String, freq: u16, duration: u16) {
    let beeps = &game.borrow().beeps;
    if beeps.borrow().contains_key(&id) {
        // Stop existing beep
        let mut beeps_mut = beeps.borrow_mut();
        let beep = beeps_mut.get(&id).unwrap();
        if let Ok(stop_fn) = Reflect::get(beep, &"stop".into()) {
            match Reflect::apply(&stop_fn.unchecked_into(), &beep, &Array::new()) {
                Ok(_) => {},
                Err(e) => console_log!("{:?}", e),
            }
        }
        beeps_mut.remove(&id);
    }

    let n = Rc::new(js_construct("Note", &[&JsValue::from_f64(69.0)]).unwrap());
    js_set(&n, "frequency", freq as f64).unwrap();
            
    let audio_context = js_get(&n, "audioContext").unwrap();
    let gain_node = js_call_member(&audio_context, "createGain", &[]).unwrap();
    let gain_node_gain = js_get(&gain_node, "gain").unwrap();
    js_set(&gain_node_gain, "value", 0.05).unwrap();
    js_call_member(&n, "play", &[&JsValue::from_f64(2.0), &gain_node]).unwrap();

    let n_clone = n.clone();
    window().unwrap().set_timeout_with_callback_and_timeout_and_arguments_0(&Closure::once_into_js(move || {
        js_call_member(&n_clone, "stop", &[]).unwrap();
    }).unchecked_into(), duration as i32).unwrap();

    beeps.borrow_mut().insert(id, n);
}

fn create_object(obj: &roboscapesim_common::ObjectData, game: &Rc<RefCell<Game>>) {
    match obj.visual_info.as_ref().unwrap() {
        roboscapesim_common::VisualInfo::None => {},
        roboscapesim_common::VisualInfo::Color(r, g, b) => {
            let m = Rc::new(BabylonMesh::create_box(&game.borrow().scene.borrow(), &obj.name, BoxOptions {
                depth: Some(obj.transform.scaling.z.into()),
                height: Some(obj.transform.scaling.y.into()),
                width: Some(obj.transform.scaling.x.into()),
                ..Default::default()
            }));
            let material = StandardMaterial::new(&obj.name, &game.borrow().scene.borrow());
            material.set_diffuse_color((r.to_owned(), g.to_owned(), b.to_owned()).into());
            m.set_material(&material);
            m.set_receive_shadows(true);
            game.borrow().shadow_generator.add_shadow_caster(&m, true);
            apply_transform(m.clone(), obj.transform);
            game.borrow().models.borrow_mut().insert(obj.name.to_owned(), m.clone());
            console_log!("Created box");
        },
        roboscapesim_common::VisualInfo::Texture(tex) => {

        },
        roboscapesim_common::VisualInfo::Mesh(mesh) => {
            let game_rc = game.clone();
            let mesh = Arc::new(mesh.clone());
            let obj = Arc::new(obj.clone());
            spawn_local(async move {
                // TODO: detect assets dir
                let m = Rc::new(BabylonMesh::create_gltf(&game_rc.borrow().scene.borrow(), &obj.name, ("http://localhost:4000/assets/".to_owned() + &mesh).as_str()).await);
                game_rc.borrow().shadow_generator.add_shadow_caster(&m, true);
                apply_transform(m.clone(), obj.transform);
                game_rc.borrow().models.borrow_mut().insert(obj.name.to_owned(), m.clone());
                console_log!("Created mesh");
            });
        },
    }
}

#[netsblox_extension_setting]
const BEEPS_ENABLED: ExtensionSetting = ExtensionSetting { 
    name: "Beeps Enabled", 
    id: "roboscape_beep", 
    default_value: true,
    on_hint: "Robots can beep", 
    off_hint: "Robots cannot beep", 
    hidden: false
};

#[netsblox_extension_setting]
const ID_BILLBOARDS_ENABLED: ExtensionSetting = ExtensionSetting { 
    name: "Robot ID Billboards Enabled", 
    id: "roboscape_id_billboards", 
    default_value: true,
    on_hint: "Robot IDs show over heads", 
    off_hint: "Robots IDs hidden", 
    hidden: false
};

#[netsblox_extension_menu_item("New simulation...")]
#[wasm_bindgen]
pub async fn new_room() {
    let in_room = GAME.with(|game| {
        game.borrow().in_room.get()
    });
    
    if in_room {
        // TODO: disconnect and clean up
    }

    if !in_room {
        let response = request_room(get_username(), None).await;

        if let Ok(response) = response {
            connect(&response).await;
            send_message(&ClientMessage::JoinRoom(response.room_id, get_username(), None));
            GAME.with(|game| {
                game.borrow().in_room.replace(true);
            });
            show_3d_view();
        }
    }
}

async fn request_room(username: String, password: Option<String>) -> Result<CreateRoomResponseData, reqwest::Error> {
    set_title("Connecting...");

    let mut client_clone = Default::default();
    REQWEST_CLIENT.with(|client| {
        client_clone = client.clone();
    });

    // TODO: get API URL through env var for deployed version
    let response = client_clone.post("http://127.0.0.1:3000/rooms/create").json(&CreateRoomRequestData {
        username,
        password
    }).send().await.unwrap();

    response.json().await
}

async fn connect(response: &CreateRoomResponseData) {
    WEBSOCKET.with(|socket| {
        let s = WebSocket::new(&response.server);
        let s = Rc::new(RefCell::new(s.unwrap()));
        GAME.with(|game| { 
            let gc = game.clone();
            let onmessage: Closure<(dyn Fn(JsValue) -> _ + 'static)> = Closure::new(move |evt: JsValue| {
                let msg = serde_json::from_str(&js_get(&evt, "data").unwrap().as_string().unwrap());
                handle_update_message(msg, &gc);
            });
            s.borrow().set_onmessage(Some(onmessage.into_js_value().unchecked_ref()));
        });

        s.borrow().set_onopen(Some(&Closure::<(dyn Fn() -> _ + 'static)>::new(||{
            console_log!("open");
        }).into_js_value().unchecked_ref()));
        socket.replace(Some(s));  
    });

    loop {
        sleep(Duration::from_millis(25)).await;

        let mut status = 0;
        WEBSOCKET.with(|socket| {
            status = socket.borrow().clone().unwrap().clone().borrow().ready_state();
        });

        if status != WebSocket::CONNECTING {
            break;
        }
    }
}

#[netsblox_extension_menu_item("Show 3D View")]
#[wasm_bindgen]
pub fn show_3d_view() {
    let dialog = get_nb_externalvar("roboscapedialog").unwrap();
    let f = get_window_fn("showDialog").unwrap();
    f.call1(&JsValue::NULL, &dialog).unwrap();
}

#[netsblox_extension_block(name = "robotsInRoom", category = "network", spec = "robots in room", target = netsblox_extension_util::TargetObject::Both)]
#[wasm_bindgen]
pub fn robots_in_room() -> JsValue {
    let list = GAME.with(|game| {
        game.borrow().state.borrow().keys().filter(|k| k.starts_with("robot_")).map(|k| k[6..].to_owned()).collect::<Vec<_>>()
    });
    js_construct("List", &[&Array::from_iter(list.iter().map(|s| JsValue::from_str(&s)))]).unwrap()
}
