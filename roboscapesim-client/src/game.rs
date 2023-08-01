use std::{cell::{RefCell, Cell}, rc::Rc, collections::HashMap};

use neo_babylon::prelude::*;
use roboscapesim_common::{ObjectData, RoomState};
use wasm_bindgen::JsValue;


/// Stores information relevant to the current state
pub(crate) struct Game {
    pub(crate) scene: Rc<RefCell<Scene>>,
    pub(crate) models: Rc<RefCell<HashMap<String, Rc<BabylonMesh>>>>,
    pub(crate) state: Rc<RefCell<HashMap<String, ObjectData>>>,
    pub(crate) last_state: Rc<RefCell<HashMap<String, ObjectData>>>,
    pub(crate) state_server_time: Rc<Cell<f64>>,
    pub(crate) last_state_server_time: Rc<Cell<f64>>,
    pub(crate) state_time: Rc<Cell<f64>>,
    pub(crate) last_state_time: Rc<Cell<f64>>,
    pub(crate) shadow_generator: Rc<CascadedShadowGenerator>,
    pub(crate) beeps: Rc<RefCell<HashMap<String, Rc<JsValue>>>>,
    pub(crate) room_state: Rc<Cell<Option<RoomState>>>,
}

impl Game {
    pub(crate) fn new() -> Self {
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
        let sun = DirectionalLight::new("light", Vector3::new(0.25, -1.0, 0.1), &scene.borrow());
        PointLight::new("light2", Vector3::new(0.0, 1.0, -1.0), &scene.borrow());

        let shadow_generator = Rc::new(CascadedShadowGenerator::new(1024.0, &sun));
        shadow_generator.set_bias(0.007);
        shadow_generator.set_cascade_blend_percentage(0.15);
        shadow_generator.set_lambda(0.9);
        shadow_generator.set_stabilize_cascades(true);
        shadow_generator.set_filtering_quality(1.0);
        shadow_generator.set_filter(6.0);
        shadow_generator.set_frustum_edge_falloff(1.0);
        shadow_generator.set_shadow_max_z(50.0);

        neo_babylon::api::setup_vr_experience(&scene.borrow());

        Game {
            scene,
            models: Rc::new(RefCell::new(HashMap::new())),
            state: Rc::new(RefCell::new(HashMap::new())),
            last_state: Rc::new(RefCell::new(HashMap::new())),
            state_time: Rc::new(Cell::new(0.0)),
            last_state_time: Rc::new(Cell::new(0.0)),
            state_server_time: Rc::new(Cell::new(0.0)),
            last_state_server_time: Rc::new(Cell::new(0.0)),
            shadow_generator,
            beeps: Rc::new(RefCell::new(HashMap::new())),     
            room_state: Rc::new(Cell::new(None)),
        }
    }

    pub(crate) async fn load_model(&self, name: &str, url: &str) -> Result<Rc<BabylonMesh>, JsValue> {
        let model = BabylonMesh::create_gltf(&self.scene.borrow(), name, url).await;

        let model = Rc::new(model);
        self.models.borrow_mut().insert(name.to_owned(), model.clone());

        Ok(model)
    }
}