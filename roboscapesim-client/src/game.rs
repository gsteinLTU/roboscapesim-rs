use std::{cell::{RefCell, Cell}, rc::Rc, collections::HashMap};
use js_sys::{Reflect, Function};
use neo_babylon::prelude::*;
use roboscapesim_common::{ObjectData, RoomState};
use wasm_bindgen::{JsValue, JsCast};
use web_sys::{HtmlElement, window, Node};

use crate::{ui::{clear_robots_menu, update_robot_buttons_visibility, TEXT_BLOCKS}, console_log, util::get_nb_externalvar};

/// Stores information relevant to the current state
pub(crate) struct Game {
    pub(crate) in_room: Rc<Cell<bool>>,
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
    pub(crate) room_state: Rc<RefCell<Option<RoomState>>>,
    pub(crate) name_tags: Rc<RefCell<HashMap<String, JsValue>>>,
    pub(crate) ui_elements: Rc<RefCell<HashMap<String, HtmlElement>>>,
    pub(crate) main_camera: Rc<UniversalCamera>,
    pub(crate) follow_camera: Rc<FollowCamera>,
    pub(crate) first_person_camera: Rc<UniversalCamera>,
    pub(crate) robot_claims: Rc<RefCell<HashMap<String, String>>>,
}

impl Game {
    pub(crate) fn new() -> Self {
        let scene = neo_babylon::api::create_scene("#roboscape-canvas");
        
        Reflect::set(&window().unwrap(), &JsValue::from_str("BABYLON.Engine.LastCreatedEngine.useReverseDepthBuffer"), &JsValue::from_bool(true)).unwrap();
        // Add a camera to the scene and attach it to the canvas
        let main_camera = Rc::new(UniversalCamera::new(
            "Camera",
            Vector3::new(0.0, 1.0, -5.0),
            Some(&scene.borrow())
        ));
        main_camera.attachControl(neo_babylon::api::get_element("#roboscape-canvas"), true);
        main_camera.set_min_z(0.05);
        main_camera.set_max_z(200.0);
        main_camera.set_speed(0.5);
        
        // Other cameras
        let follow_camera = Rc::new(FollowCamera::new("followcam", Vector3::new(5.0, 5.0, 5.0), Some(&scene.borrow())));
        follow_camera.set_height_offset(1.25);
        follow_camera.set_radius(2.0);
        follow_camera.set_rotation_offset(-90.0);
        follow_camera.set_camera_acceleration(0.2);
        follow_camera.set_max_camera_speed(50.0);
        follow_camera.set_min_z(0.05);
        follow_camera.set_max_z(200.0);

        let first_person_camera = Rc::new(UniversalCamera::new("firstPersonCam", Vector3::new(5.0, 5.0, 5.0), Some(&scene.borrow())));
        first_person_camera.set_min_z(0.01);
        first_person_camera.set_max_z(150.0);

        // For the current version, lights are added here, later they will be requested as part of scenario to allow for other lighting conditions
        // Add lights to the scene
        let sun = DirectionalLight::new("light", Vector3::new(0.25, -1.0, 0.1), &scene.borrow());
        let ambient_light =  HemisphericLight::new("ambient", Vector3::new(0.0, 1.0, 0.0), &scene.borrow());
        ambient_light.set_intensity(0.5);
        // PointLight::new("light2", Vector3::new(0.0, 3.0, 0.0), &scene.borrow());

        let shadow_generator = Rc::new(CascadedShadowGenerator::new(1024.0, &sun));
        shadow_generator.set_bias(0.007);
        shadow_generator.set_cascade_blend_percentage(0.15);
        shadow_generator.set_lambda(0.9);
        shadow_generator.set_stabilize_cascades(true);
        shadow_generator.set_filtering_quality(1.0);
        shadow_generator.set_filter(6.0);
        shadow_generator.set_frustum_edge_falloff(1.0);
        shadow_generator.set_shadow_max_z(50.0);

        //neo_babylon::api::setup_vr_experience(&scene.borrow());
        scene.borrow().set_active_camera(&main_camera);

        Game {
            in_room: Rc::new(Cell::new(false)),
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
            room_state: Rc::new(RefCell::new(None)),
            name_tags: Rc::new(RefCell::new(HashMap::new())),
            ui_elements: Rc::new(RefCell::new(HashMap::new())),
            main_camera,
            follow_camera,
            first_person_camera,
            robot_claims: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    pub(crate) async fn load_model(&self, name: &str, url: &str) -> Result<Rc<BabylonMesh>, JsValue> {
        let model = BabylonMesh::create_gltf(&self.scene.borrow(), name, url).await;
        if let Err(e) =  model {
            console_log!("Failed to load mesh: {:?}", e);
            return Err(e);
        }

        let model = Rc::new(model.unwrap());
        self.models.borrow_mut().insert(name.to_owned(), model.clone());

        Ok(model)
    }

    /// Remove a model from the scene
    pub fn remove_object(&mut self, obj: String) {
        let removed = self.models.borrow_mut().remove(&obj);
    
        if let None = removed {
            console_log!("Object {} not found", &obj);
        }
    
        // Robot-specific behavior
        if obj.starts_with("robot_") {
            let robotmenu: Node = get_nb_externalvar("roboscapedialog-robotmenu").unwrap().unchecked_into();
        
            // Don't create duplicates in the menu
            let mut search_node = robotmenu.first_child();
    
            while search_node.is_some() {
                let node = search_node.unwrap();
    
                if let Some(txt) = node.text_content() {
                    if txt == &obj[6..]{
                        search_node = Some(node);
                        break;
                    }
                }
    
                search_node = node.next_sibling();
            }
        
            if let Some(search_node) = search_node {
                robotmenu.remove_child(&search_node).unwrap();
            }
        }
    }

    /// Remove all models from the scene
    pub fn remove_all_objects(&mut self) {
        let names = self.models.borrow().keys().cloned().collect::<Vec<_>>();
        for name in names {
            self.remove_object(name.to_owned());
        }
    }

    // After disconnect, cleanup will remove all models from the scene and perform other cleanup tasks
    pub(crate) fn cleanup(&self) {
        // Remove all models from the scene (BabylonMesh's drop will handle the rest)
        self.models.borrow_mut().clear();

        // Remove all beeps
        for beep in self.beeps.borrow().values() {
            Reflect::get(&beep, &"stop".into()).unwrap().unchecked_ref::<Function>().call0(&beep).unwrap_or_default();
        }
        self.beeps.borrow_mut().clear();

        // Remove all name tags
        for name_tag in self.name_tags.borrow().values() {
            Reflect::get(&name_tag, &"dispose".into()).unwrap().unchecked_ref::<Function>().call0(&name_tag).unwrap_or_default();
        }
        self.name_tags.borrow_mut().clear();

        // Cleanup state
        self.state.borrow_mut().clear();
        self.last_state.borrow_mut().clear();
        self.state_time.set(0.0);
        self.last_state_time.set(0.0);
        self.state_server_time.set(0.0);
        self.last_state_server_time.set(0.0);
        self.room_state.borrow_mut().take();
        self.robot_claims.borrow_mut().clear();

        // UI cleanup
        TEXT_BLOCKS.with(|text_blocks| {
            text_blocks.borrow_mut().clear();
        });
        clear_robots_menu();
        update_robot_buttons_visibility();
        self.reset_camera();
    }

    pub fn reset_camera(&self) {
        self.main_camera.set_position(&Vector3::new(0.0, 1.0, -5.0));
        self.main_camera.set_rotation(&Vector3::new(0.0, 0.0, 0.0));
        self.scene.borrow().set_active_camera(&self.main_camera);
    }
}
