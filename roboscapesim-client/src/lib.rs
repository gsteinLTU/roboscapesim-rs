#![allow(dead_code)]
mod util;

use js_sys::{Reflect, Function};
use netsblox_extension_macro::*;
use netsblox_extension_util::*;
use wasm_bindgen::{prelude::{wasm_bindgen, Closure}, JsCast, JsValue};
use web_sys::{console, window};
use neo_babylon::prelude::*;
use self::util::*;
extern crate console_error_panic_hook;
use std::{panic, cell::RefCell, rc::Rc};

struct Game {
    scene: Rc<RefCell<Scene>>,
    models: RefCell<Vec<Rc<BabylonMesh>>>,
}

impl Game {
    fn new() -> Self {
        Game {
            scene: neo_babylon::api::create_basic_scene("#roboscape-canvas"),
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
pub fn main() {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console::log_1(&"RoboScape Online loaded!".to_owned().into());

    wasm_bindgen_futures::spawn_local(GAME.with(|game| {
        let game_rc = Rc::clone(&game);

        let game_rc_2 = Rc::clone(&game);
        game_rc.borrow().scene.borrow().add_before_render_observable(Closure::new(move || {
            game_rc_2.borrow().models.borrow().iter().for_each(|m| {
                let mut r = m.rotation();
                r.set_y(r.y() + 0.1);
                m.set_rotation(&r);
            });
        }));

        let game_rc = Rc::clone(&game);
        async move {
            let gltf = game_rc
            .borrow()
            .load_model("robot", "http://localhost:4000/assets/parallax_robot.glb")
                .await
                .unwrap();

            gltf.set_scaling(&(-70.0, 70.0, 70.0).into());
            gltf.set_position_x(2.0);
        }
    }));
}

#[netsblox_extension_menu_item("Show 3D View")]
#[wasm_bindgen()]
pub fn show_3d_view() {
    let dialog = get_nb_externalvar("roboscapedialog").unwrap();
    let f = get_window_fn("showDialog").unwrap();
    f.call1(&JsValue::NULL, &dialog).unwrap();
}