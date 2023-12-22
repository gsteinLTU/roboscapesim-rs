#![allow(dead_code)]
pub mod game;
pub mod ui;
pub mod util;

use gloo_timers::future::sleep;
use instant::Duration;
use js_sys::{Reflect, Array, eval, Uint8Array};
use netsblox_extension_macro::*;
use netsblox_extension_util::*;
use roboscapesim_common::{UpdateMessage, ClientMessage, Interpolatable};
use roboscapesim_client_common::{api::*, console_log, ASSETS_DIR};
use wasm_bindgen::{prelude::{wasm_bindgen, Closure}, JsValue, JsCast};
use web_sys::{window, WebSocket, Node, HtmlDataListElement};
use neo_babylon::prelude::*;
use std::{cell::RefCell, rc::Rc, sync::Arc};
use wasm_bindgen_futures::spawn_local;

use crate::ui::*;
use crate::util::*;
use crate::game::*;


extern crate console_error_panic_hook;

thread_local! {
    static WEBSOCKET: RefCell<Option<Rc<RefCell<WebSocket>>>> = RefCell::new(None);
}

thread_local! {
    static GAME: Rc<RefCell<Game>> = Rc::new(RefCell::new(Game::new()));
}

#[netsblox_extension_info]
const INFO: ExtensionInfo = ExtensionInfo { 
    name: "RoboScape Online" 
};

#[wasm_bindgen(start)]
async fn main() {
    console_error_panic_hook::set_once();
    console_log!("Assets dir: {}", ASSETS_DIR);
    console_log!("API server: {}", API_SERVER);

    // Get environments list
    spawn_local(async move {
        let environments = get_environments().await.unwrap_or_default();
        let list = get_nb_externalvar("roboscapedialog-new-environments-list").unwrap().unchecked_into::<HtmlDataListElement>();
        list.set_inner_html("");
        for result in &environments {
            let option = document().create_element("option").unwrap();
            option.set_attribute("value", &result.id).unwrap();
            list.append_child(&option).unwrap();
        }

        console_log!("{:?}", &environments);
    });

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
                    let clamped_t = t.clamp(0.0, 2.0) as f32;
                    let interpolated_transform = last_transform.try_interpolate(&update_obj.transform, clamped_t).unwrap_or(update_obj.transform);
                    
                    //console::log_1(&format!("{}: last_transform: {:?} \n next_transform: {:?} \ninterpolated_transform = {:?}", name, last_transform, update_obj.transform, interpolated_transform).into());
                    
                    apply_transform(game_clone.borrow().models.borrow().get(name).unwrap().clone(), interpolated_transform);
                } else {
                    // Assign directly
                    apply_transform(game_clone.borrow().models.borrow().get(name).unwrap().clone(), update_obj.transform);
                }
            }
        });
        game.borrow().scene.borrow().add_before_render_observable(before_render);
        ui::init_ui();
    });
    
    console_log!("RoboScape Online loaded!");
}

/// Send a ClientMessage to the server
fn send_message(msg: &ClientMessage) {
    WEBSOCKET.with(|socket| {
        let socket = socket.borrow().clone();
        if socket.is_none() {
            console_log!("Attempt to send without socket!");
        } else if let Some(socket) = socket {
            let socket = socket.borrow();
            let buf = rmp_serde::to_vec(&msg).unwrap();
            socket.send_with_u8_array(&buf).unwrap();
        }
    });
}

/// Process an UpdateMessage from the server
fn handle_update_message(msg: Result<UpdateMessage, rmp_serde::decode::Error>, game: &Rc<RefCell<Game>>) {
    match msg {
        Ok(UpdateMessage::Heartbeat) => {
            send_message(&ClientMessage::Heartbeat);
        },
        Ok(UpdateMessage::RoomInfo(state)) => {
            set_title(&state.name);
            GAME.with(|game| {
                game.borrow().room_state.replace(Some(state));
            });
        },
        Ok(UpdateMessage::Update(t, full_update, roomdata)) => {
            for obj in roomdata.iter() {
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
                game.borrow().state.borrow_mut().insert(entry.0.to_owned(), entry.1.clone());
            }

            // TODO: handle removed entities (server needs way to notify, full updates should also be able to remove)
        
            // Update times
            game.borrow().last_state_server_time.replace(game.borrow().state_server_time.get().clone());
            game.borrow().last_state_time.replace(game.borrow().state_time.get().clone());
            game.borrow().state_server_time.replace(t);
            game.borrow().state_time.replace(instant::now());
        },
        Ok(UpdateMessage::DisplayText(id, text, timeout)) => {
            // TODO: show on canvas
            console_log!("Display Text \"{}\" in position {} for {:?} s", text, id, timeout);
            add_or_update_text(&text, &id, timeout)
        },
        Ok(UpdateMessage::ClearText) => {
            clear_all_text_blocks();
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
        Ok(UpdateMessage::Hibernating) => {
            console_log!("Hibernating");
            
            game.borrow().cleanup();

            set_title("Disconnected");
        },
        Ok(UpdateMessage::RemoveObject(obj)) => {
            console_log!("Removing object {}", &obj);
            game.borrow_mut().remove_object(obj);

            update_robot_buttons_visibility();
        },
        Ok(UpdateMessage::RemoveAll()) => {
            console_log!("Removing all objects");
            game.borrow_mut().remove_all_objects();
        
            clear_robots_menu();
            update_robot_buttons_visibility();
        },
        Ok(UpdateMessage::RobotClaimed(robot, user)) => {
            console_log!("Robot {} claimed by {}", &robot, &user);
            if user.is_empty() {
                game.borrow().robot_claims.borrow_mut().remove(&robot);
            } else {
                game.borrow().robot_claims.borrow_mut().insert(robot, user);
            }

            update_claim_text();
            update_robot_buttons_visibility();
        },
        Ok(update) => {
            console_log!("Unhandled update: {:?}", update);
        }
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
        roboscapesim_common::VisualInfo::Color(r, g, b, shape) => {
            let m = match shape {
                roboscapesim_common::Shape::Box => Rc::new(BabylonMesh::create_box(&game.borrow().scene.borrow(), &obj.name, BoxOptions {
                    ..Default::default()
                })),
                roboscapesim_common::Shape::Sphere => Rc::new(BabylonMesh::create_sphere(&game.borrow().scene.borrow(), &obj.name, SphereOptions { 
                    ..Default::default() 
                })),
                _ => { todo!() }
            };
            let material = StandardMaterial::new(&obj.name, &game.borrow().scene.borrow());
            material.set_diffuse_color((r.to_owned(), g.to_owned(), b.to_owned()).into());
            material.set_specular_color((0.5, 0.5, 0.5).into());
            m.set_material(&material);
            m.set_receive_shadows(true);
            game.borrow().shadow_generator.add_shadow_caster(&m, true);
            apply_transform(m.clone(), obj.transform);
            game.borrow().models.borrow_mut().insert(obj.name.to_owned(), m.clone());
            console_log!("Created box");
        },
        roboscapesim_common::VisualInfo::Texture(tex, uscale, vscale, shape) => {
            let m = match shape {
                roboscapesim_common::Shape::Box => Rc::new(BabylonMesh::create_box(&game.borrow().scene.borrow(), &obj.name, BoxOptions {
                    ..Default::default()
                })),
                roboscapesim_common::Shape::Sphere => Rc::new(BabylonMesh::create_sphere(&game.borrow().scene.borrow(), &obj.name, SphereOptions { 
                    ..Default::default() 
                })),
                _ => { todo!() }
            };
            let material = StandardMaterial::new(&obj.name, &game.borrow().scene.borrow());
            
            let tex = Texture::new(&(ASSETS_DIR.to_owned() + (&("textures/".to_owned() + tex + ".png")).as_str()));
            tex.set_u_scale(uscale.to_owned().into());
            tex.set_v_scale(vscale.to_owned().into());
            material.set_diffuse_texture(tex);
            
            material.set_diffuse_color((0.5, 0.5, 0.5).into());
            material.set_specular_color((0.5, 0.5, 0.5).into());
            m.set_material(&material);
            m.set_receive_shadows(true);
            game.borrow().shadow_generator.add_shadow_caster(&m, true);
            apply_transform(m.clone(), obj.transform);
            game.borrow().models.borrow_mut().insert(obj.name.to_owned(), m.clone());
        },
        roboscapesim_common::VisualInfo::Mesh(mesh) => {
            let game_rc = game.clone();
            let mesh = Arc::new(mesh.clone());
            let obj = Arc::new(obj.clone());
            spawn_local(async move {
                let value = BabylonMesh::create_gltf(&game_rc.borrow().scene.borrow(), &obj.name, (ASSETS_DIR.to_owned() + (&mesh).as_str()).as_str()).await;
                if let Err(e) = value {
                    console_log!("Failed to load mesh: {:?}", e);
                    return;
                }

                let m = Rc::new(value.unwrap());
                game_rc.borrow().shadow_generator.add_shadow_caster(&m, true);
                apply_transform(m.clone(), obj.transform);
                game_rc.borrow().models.borrow_mut().insert(obj.name.to_owned(), m.clone());
                console_log!("Created mesh");

                // Robot-specific behavior
                if obj.name.starts_with("robot_") {
                    let tag = create_label(&obj.name[(obj.name.len() - 4)..], None, None, None);
                    
                    js_set(&tag, "billboardMode", &eval("BABYLON.TransformNode.BILLBOARDMODE_ALL").unwrap()).unwrap();
                    js_call_member(&tag, "setParent", &[(*m).as_ref()]).unwrap();
                    
                    // Set tag transform
                    let tag_scaling = js_get(&tag, "scaling").unwrap();
                    js_set(&tag_scaling, "x", 0.04).unwrap(); 
                    js_set(&tag_scaling, "y", 0.035).unwrap(); 
                    let tag_position = js_get(&tag, "position").unwrap();
                    js_set(&tag_position, "z", 0.0).unwrap();
                    js_set(&tag_position, "y", 0.175).unwrap();
                    js_set(&tag_position, "x", 0.0).unwrap();
                    let tag_rotation = js_get(&tag, "rotation").unwrap();
                    js_set(&tag_rotation, "x", 0.0).unwrap();
                    js_set(&tag_rotation, "y", 0.0).unwrap();
                    js_set(&tag_rotation, "z", 0.0).unwrap();
                    
                    game_rc.borrow().name_tags.borrow_mut().insert(obj.name.to_owned(), tag);

                    let robotmenu: Node = get_nb_externalvar("roboscapedialog-robotmenu").unwrap().unchecked_into();
                    
                    // Don't create duplicates in the menu
                    let mut search_node = robotmenu.first_child();

                    while search_node.is_some() {
                        let node = search_node.unwrap();

                        if let Some(txt) = node.text_content() {
                            if txt == &obj.name[6..]{
                                search_node = Some(node);
                                break;
                            }
                        }

                        search_node = node.next_sibling();
                    }
                    
                    if search_node.is_none() {
                        let new_option = document().create_element("option").unwrap();
                        new_option.set_inner_html(&obj.name[6..]);
                        new_option.set_attribute("value", &obj.name[6..]).unwrap();
                        robotmenu.append_child(&new_option).unwrap();
                    }
                }
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
pub async fn new_sim_menu() {
    show_dialog("roboscapedialog-new");
}

#[netsblox_extension_menu_item("Join room...")]
#[wasm_bindgen]
pub async fn join_sim_menu() {
    spawn_local(async move {
        let results = get_rooms_list(Some(get_username()), None).await.unwrap_or_default();
        let list = get_nb_externalvar("roboscapedialog-join-rooms-list").unwrap().unchecked_into::<HtmlDataListElement>();
        list.set_inner_html("");
        for result in &results {
            let option = document().create_element("option").unwrap();
            option.set_attribute("value", &format!("{} ({})", result.id, result.environment)).unwrap();
            list.append_child(&option).unwrap();
        }

        console_log!("{:?}", &results);
        show_dialog("roboscapedialog-join");
    });
}

pub async fn new_room(environment: Option<String>, password: Option<String>, edit_mode: bool) {
    set_title("Connecting...");
    let response = request_room(get_username(), password, edit_mode, environment).await;

    if let Ok(response) = response {
        connect(&response.server).await;
        send_message(&ClientMessage::JoinRoom(response.room_id, get_username(), None));
        GAME.with(|game| {
            game.borrow().in_room.replace(true);
        });
        show_3d_view();
    } else if let Err(e) = response {
        show_message("Error", "Error creating room");
        console_log!("Error creating room: {:?}", e);
        // Reopen new dialog
        new_sim_menu().await;
    }
}

pub async fn join_room(id: String, password: Option<String>) {
    set_title("Connecting...");
    let response = request_room_info(&id).await;

    if let Ok(response) = response {
        connect(&response.server).await;
        send_message(&ClientMessage::JoinRoom(id, get_username(), password));
        GAME.with(|game| {
            game.borrow().in_room.replace(true);
        });
        show_3d_view();
    } else if let Err(_) = response {
        show_message("Error", "Error joining room");
        // Reopen join dialog
        join_sim_menu().await;
    }
}

pub async fn connect(server: &String) {
    GAME.with(|game| {
        let in_room = game.borrow().in_room.get();
        if in_room {
            // Disconnect and clean up
            game.borrow().cleanup();
            WEBSOCKET.with(|socket| {
                socket.borrow().clone().and_then(|s| Some(s.borrow().close()));
                socket.replace(None);
            });
        }
    });
    
    set_title("Connecting...");

    WEBSOCKET.with(|socket| {
        let s = WebSocket::new(server);
        let s = Rc::new(RefCell::new(s.unwrap()));
        s.borrow().set_binary_type(web_sys::BinaryType::Arraybuffer);
        GAME.with(|game| { 
            let gc = game.clone();
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
                    handle_update_message(Ok(msg), &gc);
                }
            });
            s.borrow().set_onmessage(Some(onmessage.into_js_value().unchecked_ref()));
            let gc = game.clone();
            s.borrow().set_onclose(Some(&Closure::<(dyn Fn() -> _ + 'static)>::new(move ||{
                set_title("Disconnected");
                gc.borrow().cleanup();
            }).into_js_value().unchecked_ref()));
            s.borrow().set_onerror(Some(&Closure::<(dyn Fn() -> _ + 'static)>::new(||{
                console_log!("error");
                show_message("Error", "Failed to connect to server");
            }).into_js_value().unchecked_ref()));
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
    show_dialog("roboscapedialog");
}

#[netsblox_extension_menu_item("Reset Camera")]
#[wasm_bindgen]
pub async fn reset_camera_menu() {
    reset_camera();
}

#[netsblox_extension_block(name = "robotsInRoom", category = "network", spec = "robots in room", target = netsblox_extension_util::TargetObject::Both)]
#[wasm_bindgen]
pub fn robots_in_room() -> JsValue {
    let list = GAME.with(|game| {
        game.borrow().state.borrow().keys().filter(|k| k.starts_with("robot_")).map(|k| k[6..].to_owned()).collect::<Vec<_>>()
    });
    js_construct("List", &[&Array::from_iter(list.iter().map(|s| JsValue::from_str(&s)))]).unwrap()
}

#[netsblox_extension_block(name = "roomID", category = "network", spec = "RoboScape room id", target = netsblox_extension_util::TargetObject::Both)]
#[wasm_bindgen]
pub fn room_id() -> JsValue {
    let state = GAME.with(|game| {
        game.borrow().room_state.borrow().clone()
    });

    if let Some(state) = state {
        return JsValue::from_str(&state.name.clone());
    }

    // If no room info
    JsValue::from_bool(false)
}
