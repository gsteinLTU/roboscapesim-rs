#![allow(dead_code)]
mod util;

use dashmap::DashMap;
use js_sys::Reflect;
use netsblox_extension_macro::*;
use netsblox_extension_util::*;
use roboscapesim_common::ObjectData;
use wasm_bindgen::{prelude::{wasm_bindgen, Closure}, JsValue};
use web_sys::{console, RtcPeerConnection, RtcDataChannel};
use neo_babylon::prelude::*;
use self::util::*;
extern crate console_error_panic_hook;
use std::{panic, cell::RefCell, rc::Rc, collections::HashMap};

struct Game {
    scene: Rc<RefCell<Scene>>,
    models: DashMap<String, Rc<BabylonMesh>>,
}

thread_local! {
    static PEER_CONNECTION: RefCell<Option<Rc<RefCell<RtcPeerConnection>>>> = RefCell::new(None);
}

thread_local! {
    static DATA_CHANNELS: RefCell<HashMap<String, Rc<RefCell<RtcDataChannel>>>> = RefCell::new(HashMap::new());
}

impl Game {
    fn new() -> Self {
        let scene = neo_babylon::api::create_scene("#roboscape-canvas");
        
        // Add a camera to the scene and attach it to the canvas
        let camera = UniversalCamera::new(
            "Camera",
            Vector3::new(0.0, 1.0, -5.0),
            Some(&scene.borrow())
        );
        camera.attachControl(neo_babylon::api::get_element("#roboscape-canvas"), true);
        camera.set_min_z(0.01);
        camera.set_max_z(300.0);
        camera.set_speed(0.35);

        // For the current version, lights are added here, later they will be requested as part of scenario to allow for other lighting conditions
        // Add lights to the scene
        HemisphericLight::new("light1", Vector3::new(1.0, 1.0, 0.0), &scene.borrow());
        PointLight::new("light2", Vector3::new(0.0, 1.0, -1.0), &scene.borrow());

        neo_babylon::api::setup_vr_experience(&scene.borrow());
        
        Game {
            scene,
            models: DashMap::new(),
        }
    }

    async fn load_model(&self, name: &str, url: &str) -> Result<Rc<BabylonMesh>, JsValue> {
        let model = BabylonMesh::create_gltf(&self.scene.borrow(), name, url).await;

        let model = Rc::new(model);
        self.models.insert(name.to_owned(), model.clone());

        Ok(model)
    }
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
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    GAME.with(|game| {
        // Init game
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

        match serde_json::from_str::<HashMap<String, ObjectData>>(payload.as_str()) {
            Ok(roomdata) => {
                console::log_1(&format!("Deserialized: {:?}", roomdata).into());
                
                let roomdata = roomdata.clone();

                GAME.with(move |game| {

                        for obj in roomdata.into_values() {
                            if game.borrow().models.contains_key(&obj.name) {
                                // Update existing mesh
                                let ref_cell = &game.clone();
                                let g = ref_cell.borrow();
                                // Guaranteed to exist
                                let existing = g.models.get(&obj.name).unwrap();
                                existing.set_position(&(obj.transform.position[0], obj.transform.position[1], obj.transform.position[2]).into());
                                existing.set_scaling(&(obj.transform.scaling[0], obj.transform.scaling[1], obj.transform.scaling[2]).into());
                                
                                match obj.transform.rotation {
                                    roboscapesim_common::Orientation::Euler(angles) => { existing.set_rotation(&(angles[0], angles[1], angles[2]).into()); },
                                    roboscapesim_common::Orientation::Quaternion(q) => { existing.set_rotation_quaternion(&Quaternion::new(q.i, q.j, q.k, q.w)); },
                                }

                            } else {
                                // Create new mesh
                                match obj.visual_info {
                                    roboscapesim_common::VisualInfo::None => {},
                                    roboscapesim_common::VisualInfo::Color(r, g, b) => {
                                        let m = Rc::new(BabylonMesh::create_box(&game.borrow().scene.borrow(), &obj.name, BoxOptions {
                                            depth: obj.transform.scaling.z.into(),
                                            height: obj.transform.scaling.y.into(),
                                            width: obj.transform.scaling.x.into(),
                                            ..Default::default()
                                        }));
                                        let material = StandardMaterial::new(&obj.name, &game.borrow().scene.borrow());
                                        material.set_diffuse_color((r.to_owned(), g.to_owned(), b.to_owned()).into());
                                        m.set_material(&material);
                                        apply_transform(m.clone(), obj.transform);
                                        game.borrow().models.insert(obj.name, m.clone());
                                        console::log_1(&format!("Created box").into());
                                    },
                                    roboscapesim_common::VisualInfo::Texture(tex) => {

                                    },
                                    roboscapesim_common::VisualInfo::Mesh(mesh) => {
                                        let game_rc = game.clone();
                                        wasm_bindgen_futures::spawn_local(async move {
                                            let m = Rc::new(BabylonMesh::create_gltf(&game_rc.borrow().scene.borrow(), &obj.name, ("http://localhost:4000/assets/".to_owned() + &mesh).as_str()).await);
                                            apply_transform(m.clone(), obj.transform);
                                            game_rc.borrow().models.insert(obj.name, m.clone());
                                        });
                                    },
                                }
                            }
                        }
                });
            },
            Err(e) => console::log_1(&format!("Failed to deserialize: {}", e).into()),
        }
        
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
    m.set_position(&Vector3::new(transform.position.x, transform.position.y, transform.position.z));

    match transform.rotation {
        roboscapesim_common::Orientation::Euler(angles) => m.set_rotation(&Vector3::new(angles.x, angles.y, angles.z)),
        roboscapesim_common::Orientation::Quaternion(q) => m.set_rotation_quaternion(&Quaternion::new(q.i, q.j, q.k, q.w)),
    }

    m.set_scaling(&Vector3::new(transform.scaling.x, transform.scaling.y, transform.scaling.z));
}