use std::{collections::BTreeMap, cell::{RefCell, Cell}, rc::Rc};

use neo_babylon::prelude::Color3;
use roboscapesim_common::ClientMessage;
use web_sys::window;

use crate::{util::*, console_log};

use super::send_message;

use js_sys::eval;
use wasm_bindgen::{prelude::Closure, JsValue, JsCast};

/// Set up UI elements for the 3D view window
pub(crate) fn init_ui() {
    create_button("Reset", Closure::new(|| { 
        console_log!("Reset");

        // Send reset message
        match get_selected_robot() {
            None => {
                send_message(&ClientMessage::ResetAll);
            }
            Some(robot) => {
                send_message(&ClientMessage::ResetRobot(robot));
            }
        }
    }));
    create_button("Chase Cam", Closure::new(|| { 
        console_log!("Chase Cam");

    }));
    create_button("First Person Cam", Closure::new(|| { 
        console_log!("First Person Cam");

    }));
    create_button("Free Cam", Closure::new(|| { 
        console_log!("Free Cam");
    }));
    create_button("Encrypt", Closure::new(|| { 
        console_log!("Encrypt");

        if let Some(robot) = get_selected_robot() {
            send_message(&ClientMessage::EncryptRobot(robot));
        }
    }));
    create_button("Claim", Closure::new(|| { 
        console_log!("Claim");

        if let Some(robot) = get_selected_robot() {
            send_message(&ClientMessage::ClaimRobot(robot));
        }
    }));


    eval("
        var setupJS = () => {

            if(BABYLON.GUI == undefined) {
                setTimeout(setupJS,200);
                return;
            }

            var advancedTexture = BABYLON.GUI.AdvancedDynamicTexture.CreateFullscreenUI('UI');

            var textStackPanel = new BABYLON.GUI.StackPanel();
            textStackPanel.setPadding(20, 20, 20, 20);
            textStackPanel.spacing = 20;
            textStackPanel.verticalAlignment = 'top';
            advancedTexture.addControl(textStackPanel);

            window.externalVariables['roboscapesim-textStackPanel'] = textStackPanel;
        };

        setTimeout(setupJS, 200);


        const observer = new ResizeObserver(function () {
            BABYLON.Engine.LastCreatedEngine.resize();
        });
        observer.observe(window.externalVariables.roboscapedialog);
        ").unwrap();
}


pub(crate) fn create_button(text: &str, callback: Closure<dyn Fn()>) -> web_sys::Element {
    let document = document();
    let button = document.create_element("button").unwrap();
    button.set_text_content(Some(text));
    button.add_event_listener_with_callback("click", &callback.into_js_value().into()).unwrap();
    document.get_element_by_id("roboscapebuttonbar").unwrap().append_child(&button).unwrap();
    button
}

pub(crate) fn set_title(title: &str) {
    let dialog = get_nb_externalvar("roboscapedialog").unwrap();
    let f = get_window_fn("setDialogTitle").unwrap();
    f.call2(&JsValue::NULL, &dialog, &JsValue::from_str(title)).unwrap();
}

struct TextBlock {
    pub id: Rc<RefCell<String>>,
    pub js_value: RefCell<JsValue>,
    pub timeout: Cell<Option<i32>>,
}
impl TextBlock {
    fn create_timeout(&mut self, timeout: f64) {
        TEXT_BLOCKS.with(|text_blocks| {
            let text_blocks_clone = text_blocks.clone();
            let id = self.id.clone();
            self.timeout.set(Some(window().unwrap().set_timeout_with_callback_and_timeout_and_arguments_0(
                Closure::<dyn Fn()>::new(move || {
                    text_blocks_clone.borrow_mut().remove(&id.borrow().clone()).unwrap();
                }).into_js_value().unchecked_ref(),
                (timeout * 1000.0) as i32
            ).unwrap()));
        });
    }

    fn clear_timeout(&mut self){
        if let Some(timeout) = self.timeout.get() {
            window().unwrap().clear_timeout_with_handle(timeout);
            self.timeout.set(None);
        }
    }
}

impl Drop for TextBlock {
    fn drop(&mut self) {
        console_log!("Dropping {}", self.id.borrow());
        js_call_member(&get_nb_externalvar("roboscapesim-textStackPanel").unwrap(), "removeControl", &[&self.js_value.borrow()]).unwrap();
        self.clear_timeout();
    }
}

thread_local! {
    static TEXT_BLOCKS: Rc<RefCell<BTreeMap<String, Rc::<RefCell<TextBlock>>>>> = Rc::new(RefCell::new(BTreeMap::new()));
}

/// Create a TextBlock in the 3D view's overlay.
/// If a TextBlock already has the id, that TextBlock's text and timeout will be updated.
pub(crate) fn add_or_update_text(text: &str, id: &str, timeout: Option<f64>) {
    let id = "textblock_".to_owned() + id;
    TEXT_BLOCKS.with(|text_blocks| {
        if !text_blocks.borrow().contains_key(&id) {
            let text_block = RefCell::new(eval(&("let textBlock = new BABYLON.GUI.TextBlock('textblock_' + ('".to_owned() + &id + "' ?? Math.round(Math.random() * 10000000)));
            textBlock.heightInPixels = 24;
            textBlock.outlineColor = '#2226';
            textBlock.outlineWidth = 3;
            textBlock.color = '#FFF';
            textBlock.fontSizeInPixels = 20;
            textBlock;")).unwrap());
            js_set(&text_block.borrow(), "text", text).unwrap();
            js_call_member(&get_nb_externalvar("roboscapesim-textStackPanel").unwrap(), "addControl", &[&text_block.borrow()]).unwrap();
            
            let id = js_get(&text_block.borrow(), "name").unwrap().as_string().unwrap();

            let block = Rc::new(RefCell::new(TextBlock { id: Rc::new(RefCell::new(id.clone())), js_value: text_block.clone(), timeout: Cell::new(None) }));

            if let Some(timeout) = timeout {
                block.borrow_mut().create_timeout(timeout);                
            }

            text_blocks.borrow_mut().insert(id, block);
        } else {
            text_blocks.borrow_mut().get_mut(&id).unwrap().borrow_mut().clear_timeout();   
            
            if let Some(timeout) = timeout {
                text_blocks.borrow_mut().get_mut(&id).unwrap().borrow_mut().create_timeout(timeout);           
            }         

            js_set(&text_blocks.borrow()[&id].borrow().js_value.borrow(), "text", text).unwrap();
        }
    });
}

/**
 * Removes all TextBlocks from the 3D view's overlay
 */
pub(crate) fn clear_all_text_blocks() {
    TEXT_BLOCKS.with(|text_blocks| {
        text_blocks.borrow_mut().clear();
    });
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