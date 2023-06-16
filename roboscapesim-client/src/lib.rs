#![allow(dead_code)]
mod util;

use js_sys::{Reflect, Function};
use netsblox_extension_macro::*;
use netsblox_extension_util::*;
use roboscapesim_common::ObjectData;
use wasm_bindgen::{prelude::{wasm_bindgen, Closure}, JsCast, JsValue, convert::IntoWasmAbi};
use web_sys::{console, window, RtcPeerConnection, RtcDataChannel};
use neo_babylon::prelude::*;
use self::util::*;
extern crate console_error_panic_hook;
use std::{panic, cell::RefCell, rc::Rc, collections::HashMap};

struct Game {
    scene: Rc<RefCell<Scene>>,
    models: RefCell<Vec<Rc<BabylonMesh>>>,
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

        Game {
            scene,
            models: RefCell::new(vec![]),
        }
    }

    async fn load_model(&self, name: &str, url: &str) -> Result<Rc<BabylonMesh>, JsValue> {
        let model = BabylonMesh::create_gltf(&self.scene.borrow(), name, url).await;

        let model = Rc::new(model);
        self.models.borrow_mut().push(model.clone());

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

    // wasm_bindgen_futures::spawn_local(GAME.with(|game| {
    //     let game_rc = Rc::clone(&game);

    //     let game_rc_2 = Rc::clone(&game);
    //     game_rc.borrow().scene.borrow().add_before_render_observable(Closure::new(move || {
    //         game_rc_2.borrow().models.borrow().iter().for_each(|m| {
    //             /*let r = m.rotation();
    //             r.set_y(r.y() + 0.1);
    //             m.set_rotation(&r);*/
    //         });
    //     }));

    //     let game_rc = Rc::clone(&game);
    //     async move {
    //         let gltf = game_rc
    //         .borrow()
    //         .load_model("robot", "http://localhost:4000/assets/parallax_robot.glb")
    //             .await
    //             .unwrap();

    //         gltf.set_scaling(&(-70.0, 70.0, 70.0).into());
    //         gltf.set_position_x(2.0);
    //     }
    // }));
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

        match serde_json::from_str::<Vec<ObjectData>>(payload.as_str()) {
            Ok(roomdata) => {
                console::log_1(&format!("Deserialized: {:?}", roomdata).into());

                GAME.with(|game| {

                        for obj in roomdata {
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
                                    material.set_diffuse_color(Color3::new(r.into(), g.into(), b.into()));
                                    m.set_material(&material);
                                    apply_transform(m.clone(), obj.transform);
                                    game.borrow().models.borrow_mut().push(m.clone());
                                    console::log_1(&format!("Created box").into());
                                },
                                roboscapesim_common::VisualInfo::Texture(tex) => {

                                },
                                roboscapesim_common::VisualInfo::Mesh(mesh) => {
                                    let game_rc = game.clone();
                                    wasm_bindgen_futures::spawn_local(async move {
                                        let m = Rc::new(BabylonMesh::create_gltf(&game_rc.borrow().scene.borrow(), &obj.name, ("http://localhost:4000/assets/".to_owned() + &mesh).as_str()).await);
                                        apply_transform(m.clone(), obj.transform);
                                        game_rc.borrow().models.borrow_mut().push(m.clone());
                                    });
                                },
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