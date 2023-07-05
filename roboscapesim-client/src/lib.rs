#![allow(dead_code)]
mod util;
mod game;

use js_sys::Reflect;
use netsblox_extension_macro::*;
use netsblox_extension_util::*;
use roboscapesim_common::UpdateMessage;
use wasm_bindgen::{prelude::{wasm_bindgen, Closure}, JsValue};
use web_sys::{console, RtcPeerConnection, RtcDataChannel};
use neo_babylon::prelude::*;
use self::util::*;
use self::game::*;
extern crate console_error_panic_hook;
use std::{cell::RefCell, rc::Rc, collections::HashMap, sync::Arc};

thread_local! {
    static PEER_CONNECTION: RefCell<Option<Rc<RefCell<RtcPeerConnection>>>> = RefCell::new(None);
}

thread_local! {
    static DATA_CHANNELS: RefCell<HashMap<String, Rc<RefCell<RtcDataChannel>>>> = RefCell::new(HashMap::new());
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

    GAME.with(|game| {
        // Init game
        let game_clone = game.clone();
        let before_render = Closure::new(move || {
            let next_state = &game_clone.borrow().state;
            let last_state = &game_clone.borrow().last_state;
            let now = instant::now();
            let t = (now - game_clone.borrow().state_time.get()) / (game_clone.borrow().state_time.get() - game_clone.borrow().last_state_time.get());
            //console::log_1(&format!("t = {}, now = {}, last_state_time = {}, state_time = {}", t, now, *game_clone.borrow().last_state_time.borrow(), *game_clone.borrow().state_time.borrow()).into());

            for update_obj in next_state.iter() {
                let name = update_obj.key();
                let update_obj = update_obj.value();
                
                if !game_clone.borrow().models.contains_key(name) {
                    continue;
                }

                // Don't update objects not loaded yet
                if last_state.contains_key(name) {
                    // Interpolate
                    let last_transform = last_state.get(name).unwrap().transform;
                    let interpolated_transform = last_transform.interpolate(&update_obj.transform, t as f32);
                    //console::log_1(&format!("{}: last_transform: {:?} \n next_transform: {:?} \ninterpolated_transform = {:?}", name, last_transform, update_obj.transform, interpolated_transform).into());

                    apply_transform(game_clone.borrow().models.get(name).unwrap().value().clone(), interpolated_transform);
                } else {
                    // Assign directly
                    apply_transform(game_clone.borrow().models.get(name).unwrap().value().clone(), update_obj.transform);
                }
            }
        });
        game.borrow().scene.borrow().add_before_render_observable(before_render);
    });
    
    console::log_1(&"RoboScape Online loaded!".to_owned().into());
    connect().await;
}

#[netsblox_extension_menu_item("Show 3D View")]
#[wasm_bindgen()]
pub fn show_3d_view() {
    let dialog = get_nb_externalvar("roboscapedialog").unwrap();
    let f = get_window_fn("showDialog").unwrap();
    f.call1(&JsValue::NULL, &dialog).unwrap();
}

pub async fn connect() {
    let pc: Rc<RefCell<RtcPeerConnection>> = cyberdeck_client_web_sys::create_peer_connection(None);
    let send_channel = cyberdeck_client_web_sys::create_data_channel(pc.clone(), "foo");
    
    let onclose = Closure::<dyn Fn()>::new(|| {
        console::log_1(&"sendChannel has closed".into());
    });
    let onopen = Closure::<dyn Fn()>::new(|| {
        console::log_1(&"sendChannel has opened".into());
    });
    
    let send_channel_clone = send_channel.clone();

    let onmessage = Closure::<dyn Fn(JsValue)>::new(move |e: JsValue| {
        let payload = Reflect::get(&e, &"data".into()).unwrap().as_string().unwrap();

        console::log_1(&format!("Message from DataChannel '{}' with payload '{}'", Reflect::get(&send_channel_clone.borrow(), &"label".into()).unwrap().as_string().unwrap(), payload).into());

        GAME.with(move |game| match serde_json::from_str::<UpdateMessage>(payload.as_str()) {
            Ok(UpdateMessage::Heartbeat) => {},
            Ok(UpdateMessage::RoomInfo(_)) => {

            },
            Ok(UpdateMessage::Update(t, full_update, roomdata)) => {
                let view = roomdata.to_owned();
                for obj in view.into_read_only().iter() {
                    let name = obj.0;
                    let obj = obj.1;

                    if !game.borrow().models.contains_key(name) {
                        // Create new mesh
                        match &obj.visual_info {
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
                                game.borrow().models.insert(obj.name.to_owned(), m.clone());
                                console::log_1(&format!("Created box").into());
                            },
                            roboscapesim_common::VisualInfo::Texture(tex) => {

                            },
                            roboscapesim_common::VisualInfo::Mesh(mesh) => {
                                let game_rc = game.clone();
                                let mesh = Arc::new(mesh.clone());
                                let obj = Arc::new(obj.clone());
                                wasm_bindgen_futures::spawn_local(async move {
                                    // TODO: detect assets dir
                                    let m = Rc::new(BabylonMesh::create_gltf(&game_rc.borrow().scene.borrow(), &obj.name, ("http://localhost:4000/assets/".to_owned() + &mesh).as_str()).await);
                                    game_rc.borrow().shadow_generator.add_shadow_caster(&m, true);
                                    apply_transform(m.clone(), obj.transform);
                                    game_rc.borrow().models.insert(obj.name.to_owned(), m.clone());
                                    console::log_1(&format!("Created mesh").into());
                                });
                            },
                        }
                    }
                }

                // Update state vars
                for entry in &game.borrow().state {
                    game.borrow().last_state.insert(entry.key().to_owned(), entry.value().clone());
                }
                for entry in &roomdata {
                    game.borrow().state.insert(entry.key().to_owned(), entry.value().clone());
                }

                // TODO: handle removed entities (server needs way to notify, full updates should also be able to remove)
                
                // Update times
                game.borrow().last_state_server_time.replace(game.borrow().state_server_time.get().clone());
                game.borrow().last_state_time.replace(game.borrow().state_time.get().clone());
                game.borrow().state_server_time.replace(t);
                game.borrow().state_time.replace(instant::now());
            },
            Ok(UpdateMessage::DisplayText(_)) => {},
            Err(e) => console::log_1(&format!("Failed to deserialize: {}", e).into()),
        });
    });

    cyberdeck_client_web_sys::init_data_channel(send_channel.clone(), onclose, onopen, onmessage);

    PEER_CONNECTION.with(|p| {
        p.replace(Some(pc.clone()));
    });

    DATA_CHANNELS.with(|d| {
        d.borrow_mut().insert("foo".to_owned(), send_channel.clone());
    });

    let pc_clone = pc.clone();
    let oniceconnectionstatechange = Closure::<dyn Fn(JsValue)>::new(move |_e: JsValue| {
        console::log_1(&Reflect::get(&pc_clone.borrow(), &"iceConnectionState".into()).unwrap().as_string().unwrap().into());
    });
    
    cyberdeck_client_web_sys::init_peer_connection(pc.clone(), "http://localhost:3000/connect".to_string().into(), oniceconnectionstatechange).await;
}

fn apply_transform(m: Rc<BabylonMesh>, transform: roboscapesim_common::Transform) {
    m.set_position(&Vector3::new(transform.position.x.into(), transform.position.y.into(), transform.position.z.into()));

    match transform.rotation {
        roboscapesim_common::Orientation::Euler(angles) => m.set_rotation(&Vector3::new(angles.x.into(), angles.y.into(), angles.z.into())),
        roboscapesim_common::Orientation::Quaternion(q) => m.set_rotation_quaternion(&Quaternion::new(q.i.into(), q.j.into(), q.k.into(), q.w.into())),
    }

    m.set_scaling(&Vector3::new(transform.scaling.x.into(), transform.scaling.y.into(), transform.scaling.z.into()));
}