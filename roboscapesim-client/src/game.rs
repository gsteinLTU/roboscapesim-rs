use std::{cell::{RefCell, Cell}, rc::Rc, collections::HashMap};

use js_sys::eval;
use neo_babylon::prelude::*;
use roboscapesim_common::{ObjectData, RoomState};
use wasm_bindgen::JsValue;

use crate::util::{js_call_member, js_set, js_get};


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
        }
    }

    pub(crate) async fn load_model(&self, name: &str, url: &str) -> Result<Rc<BabylonMesh>, JsValue> {
        let model = BabylonMesh::create_gltf(&self.scene.borrow(), name, url).await;

        let model = Rc::new(model);
        self.models.borrow_mut().insert(name.to_owned(), model.clone());

        Ok(model)
    }

    pub(crate) fn create_label(text: &str, font: Option<&str>, color: Option<&str>, outline: Option<bool>) -> JsValue {
        // Defaults
        let font = font.unwrap_or("Arial");
        let color = color.unwrap_or("#ffffff");
        let outline = outline.unwrap_or(true);

        // Set font
        let font_size = 48;
        let font = "bold ".to_owned() + &i32::to_string(&font_size) + "px " + font;
    
        // Set height for plane
        let plane_height = 3.0;
    
        // Set height for dynamic texture
        let dtheight = 1.5 * font_size as f64; //or set as wished
    
        // Calcultae ratio
        let ratio = plane_height / dtheight;
    
        //Use a temporary dynamic texture to calculate the length of the text on the dynamic texture canvas
        let temp = eval("new BABYLON.DynamicTexture('DynamicTexture', 64)").unwrap();
        let tmpctx = js_call_member(&temp, "getContext", &[]).unwrap();
        js_set(&tmpctx, "font", &font).unwrap();

        let dtwidth = js_get(&js_call_member(&tmpctx, "measureText", &[&JsValue::from_str(&text)]).unwrap(), "width").unwrap();
        let dtwidth = dtwidth.as_f64().unwrap() + 8.0;
    
        // Calculate width the plane has to be 
        let plane_width = dtwidth * ratio;
    
        //Create dynamic texture and write the text
        let dynamic_texture = eval(&("new BABYLON.DynamicTexture('DynamicTexture', { width: ".to_owned() + &dtwidth.to_string() + " + 8, height: " +  &dtheight.to_string() + " + 8 }, null, false)")).unwrap();
        let mat = eval("new BABYLON.StandardMaterial('mat', null);").unwrap();
        js_set(&mat, "diffuseTexture", &dynamic_texture).unwrap();
        js_set(&mat, "ambientColor", &Color3::new(1.0, 1.0, 1.0)).unwrap();
        js_set(&mat, "specularColor", &Color3::new(0.0, 0.0, 0.0)).unwrap();
        js_set(&mat, "diffuseColor", &Color3::new(0.0, 0.0, 0.0)).unwrap();
        js_set(&mat, "emissiveColor", &Color3::new(1.0, 1.0, 1.0)).unwrap();
    
        // Create outline
        if outline {
            js_call_member(&dynamic_texture, "drawText", &[&JsValue::from_str(text), &JsValue::from_f64(2.0), &JsValue::from_f64(dtheight - 4.0), &JsValue::from_str(&font), &JsValue::from_str("#111111"), &JsValue::NULL, &JsValue::TRUE]).unwrap();
            js_call_member(&dynamic_texture, "drawText", &[&JsValue::from_str(text), &JsValue::from_f64(4.0), &JsValue::from_f64(dtheight - 2.0), &JsValue::from_str(&font), &JsValue::from_str("#111111"), &JsValue::NULL, &JsValue::TRUE]).unwrap();
            js_call_member(&dynamic_texture, "drawText", &[&JsValue::from_str(text), &JsValue::from_f64(6.0), &JsValue::from_f64(dtheight - 4.0), &JsValue::from_str(&font), &JsValue::from_str("#111111"), &JsValue::NULL, &JsValue::TRUE]).unwrap();
            js_call_member(&dynamic_texture, "drawText", &[&JsValue::from_str(text), &JsValue::from_f64(4.0), &JsValue::from_f64(dtheight - 6.0), &JsValue::from_str(&font), &JsValue::from_str("#111111"), &JsValue::NULL, &JsValue::TRUE]).unwrap();
        }
    
        // Draw text
        js_call_member(&dynamic_texture, "drawText", &[&JsValue::from_str(text),&JsValue::from_f64(4.0), &JsValue::from_f64(dtheight - 4.0), &JsValue::from_str(&font), &JsValue::from_str(&color), &JsValue::NULL, &JsValue::TRUE]).unwrap();
    
        js_set(&dynamic_texture, "hasAlpha", true).unwrap();
        js_set(&dynamic_texture, "getAlphaFromRGB", true).unwrap();
    
        //Create plane and set dynamic texture as material
        let plane = eval(&("BABYLON.MeshBuilder.CreatePlane('plane', { width: ".to_owned() + &plane_width.to_string() + ", height: " + &plane_width.to_string() + " }, null)")).unwrap();
        js_set(&plane, "material", mat).unwrap();
    
        plane
    }
}