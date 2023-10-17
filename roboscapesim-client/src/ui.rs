use std::{collections::BTreeMap, cell::{RefCell, Cell}, rc::Rc};

use neo_babylon::prelude::{Color3, Vector3};
use roboscapesim_common::ClientMessage;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, HtmlElement, HtmlInputElement, Event, HtmlDialogElement};

use crate::{util::*, console_log, GAME, new_room, join_room};

use super::send_message;

use js_sys::eval;
use wasm_bindgen::{prelude::Closure, JsValue, JsCast};

/// Set up UI elements for the 3D view window
pub(crate) fn init_ui() {
    GAME.with(|game| {
        game.borrow().ui_elements.borrow_mut().insert("reset".into(), create_button("Reset", Closure::new(|| { 
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
        })));
        
        game.borrow().ui_elements.borrow_mut().insert("chase".into(), create_button("Chase Cam", Closure::new(|| { 
            console_log!("Chase Cam");
            
            GAME.with(|game| {
                if let Some(robot_id) = get_selected_robot() {
                    if let Some(robot) = game.borrow().models.borrow().get(&("robot_".to_owned() + &robot_id)) {
                        game.borrow().follow_camera.set_locked_target(Some(robot.get_mesh_as_js_value()));
                        game.borrow().scene.borrow().set_active_camera(game.borrow().follow_camera.as_ref());
                    }
                }
            });
        })));

        game.borrow().ui_elements.borrow_mut().insert("fps".into(),create_button("First Person Cam", Closure::new(|| { 
            console_log!("First Person Cam");
            GAME.with(|game| {
                if let Some(robot_id) = get_selected_robot() {
                    if let Some(robot) = game.borrow().models.borrow().get(&("robot_".to_owned() + &robot_id)) {
                        game.borrow().scene.borrow().set_active_camera(game.borrow().first_person_camera.as_ref());
                        js_set(game.borrow().first_person_camera.as_ref(), "parent", robot.get_mesh_as_js_value()).unwrap();
                        game.borrow().first_person_camera.set_position(&Vector3::new(0.035, 0.05, 0.0));
                        game.borrow().first_person_camera.set_rotation(&Vector3::new(0.0, std::f64::consts::FRAC_PI_2, 0.0));
                    }
                }
            });
        })));

        game.borrow().ui_elements.borrow_mut().insert("free".into(),create_button("Free Cam", Closure::new(|| { 
            console_log!("Free Cam");

            GAME.with(|game| {
                game.borrow().scene.borrow().set_active_camera(game.borrow().main_camera.as_ref());
            });
        })));

        game.borrow().ui_elements.borrow_mut().insert("encrypt".into(), create_button("Encrypt", Closure::new(|| { 
            console_log!("Encrypt");

            if let Some(robot) = get_selected_robot() {
                send_message(&ClientMessage::EncryptRobot(robot));
            }
        })));
        
        let game_clone = game.clone();
        game.borrow().ui_elements.borrow_mut().insert("claim".into(), create_button("Claim", Closure::new(move || { 
            console_log!("Claim");

            // Claim or unclaim robot based on current claim status
            if let Some(robot) = get_selected_robot() {
                if let Some(claim) = game_clone.borrow().robot_claims.borrow().get(&robot) {
                    if claim.to_owned() == get_username() {
                        send_message(&ClientMessage::UnclaimRobot(robot));
                    } else {
                        console_log!("Attempt to unclaim robot claimed by {}", claim);
                    }
                } else {
                    send_message(&ClientMessage::ClaimRobot(robot));
                }
            }
        })));
        
        game.borrow().ui_elements.borrow_mut().insert("claim_text".into(), create_text("Claimed by: None"));
    });

    
    let robotmenu: HtmlElement = get_nb_externalvar("roboscapedialog-robotmenu").unwrap().unchecked_into();
    robotmenu.set_onchange(Some(Closure::<dyn Fn() >::new(|| {
        update_robot_buttons_visibility();
        update_claim_text();
    }).into_js_value().unchecked_ref()));

    update_robot_buttons_visibility();

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

        let new_buttons = get_nb_externalvar("roboscapedialog-new").unwrap().unchecked_into::<HtmlElement>().query_selector_all("button").unwrap();


        let new_confirm_button = new_buttons.get(0).unwrap();
        new_confirm_button.add_event_listener_with_callback("click", Closure::<dyn Fn()>::new(|| {
            let new_inputs = get_nb_externalvar("roboscapedialog-new").unwrap().unchecked_into::<HtmlElement>().query_selector_all("input").unwrap();
            let new_env = new_inputs.get(0).unwrap().unchecked_ref::<HtmlInputElement>().clone();
            let new_password = new_inputs.get(1).unwrap().unchecked_ref::<HtmlInputElement>().clone();

            spawn_local(async move {
                let mut env = new_env.value();
                let password = new_password.value();
                let password = if password.trim().is_empty() { None } else { Some(password) };

                if env.trim().is_empty() {
                    env = "Default".to_owned();
                }

                // Clear inputs
                new_env.set_value("");
                new_password.set_value("");

                new_room(Some(env), password, false).await;
            });

            hide_dialog("roboscapedialog-new");
        }).into_js_value().unchecked_ref()).unwrap();

        let new_edit_button = new_buttons.get(1).unwrap();
        new_edit_button.add_event_listener_with_callback("click", Closure::<dyn Fn()>::new(|| {
            let new_inputs = get_nb_externalvar("roboscapedialog-new").unwrap().unchecked_into::<HtmlElement>().query_selector_all("input").unwrap();
            let new_password = new_inputs.get(1).unwrap();

            let password = new_password.unchecked_ref::<HtmlInputElement>().value();
            let password = if password.trim().is_empty() { None } else { Some(password) };

            spawn_local(async move {
                new_room(None, password, true).await;
            });

            hide_dialog("roboscapedialog-new");
        }).into_js_value().unchecked_ref()).unwrap();


        let join_buttons = get_nb_externalvar("roboscapedialog-join").unwrap().unchecked_into::<HtmlElement>().query_selector_all("button").unwrap();
        let join_button = join_buttons.get(0).unwrap();
        join_button.add_event_listener_with_callback("click", Closure::<dyn Fn()>::new(|| {
            let join_inputs = get_nb_externalvar("roboscapedialog-join").unwrap().unchecked_into::<HtmlElement>().query_selector_all("input").unwrap();
            let join_id = join_inputs.get(0).unwrap().unchecked_ref::<HtmlInputElement>().clone();
            let join_password = join_inputs.get(1).unwrap().unchecked_ref::<HtmlInputElement>().clone();

            spawn_local(async move {
                let id = join_id.value();
                let password = join_password.value();
                let password = if password.trim().is_empty() { None } else { Some(password) };

                // Clear inputs
                join_id.set_value("");
                join_password.set_value("");

                if id.trim().is_empty() {
                    // TODO: Show error
                    return;
                }

                let id = id.split(" ").collect::<Vec<&str>>()[0].to_owned();

                join_room(id, password).await;
            });

            hide_dialog("roboscapedialog-join");
        }).into_js_value().unchecked_ref()).unwrap();

}

/// Add a button to the 3D view button bar
pub(crate) fn create_button(text: &str, callback: Closure<dyn Fn()>) -> web_sys::HtmlElement {
    let document = document();
    let button = document.create_element("button").unwrap();
    button.set_text_content(Some(text));
    button.add_event_listener_with_callback("click", &callback.into_js_value().into()).unwrap();
    button.add_event_listener_with_callback("mousedown", &Closure::<dyn Fn(Event)>::new(|e: Event| { e.prevent_default(); }).into_js_value().into()).unwrap();
    document.get_element_by_id("roboscapebuttonbar").unwrap().append_child(&button).unwrap();
    button.unchecked_into()
}

/// Add text to the 3D view button bar
pub(crate) fn create_text(text: &str) -> web_sys::HtmlElement {
    let document = document();
    let span = document.create_element("span").unwrap();
    span.set_text_content(Some(text));
    document.get_element_by_id("roboscapebuttonbar").unwrap().append_child(&span).unwrap();
    span.unchecked_into()
}

/// Set title of 3D view
pub(crate) fn set_title(title: &str) {
    let dialog = get_nb_externalvar("roboscapedialog").unwrap();
    let f = get_window_fn("setDialogTitle").unwrap();
    f.call2(&JsValue::NULL, &dialog, &JsValue::from_str(title)).unwrap();
}

/// Holds information about a text message displayed overlaying the 3D view
pub(crate) struct TextBlock {
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
    pub(crate) static TEXT_BLOCKS: Rc<RefCell<BTreeMap<String, Rc::<RefCell<TextBlock>>>>> = Rc::new(RefCell::new(BTreeMap::new()));
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

/// Create a label in the 3D view
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

/// Update the visibility of the buttons in the 3D view button bar based on if there is a selected robot
pub(crate) fn update_robot_buttons_visibility() {
    GAME.with(|game| {
        match get_selected_robot() {
            None => {
                // Hide  
                game.borrow().ui_elements.borrow().get("chase").unwrap().style().set_property("display", "none").unwrap();
                game.borrow().ui_elements.borrow().get("fps").unwrap().style().set_property("display", "none").unwrap();
                game.borrow().ui_elements.borrow().get("encrypt").unwrap().style().set_property("display", "none").unwrap();
                game.borrow().ui_elements.borrow().get("claim").unwrap().style().set_property("display", "none").unwrap();
                game.borrow().ui_elements.borrow().get("claim_text").unwrap().style().set_property("display", "none").unwrap();
            }
            Some(_) => {
                // Show  
                game.borrow().ui_elements.borrow().get("chase").unwrap().style().remove_property("display").unwrap();
                game.borrow().ui_elements.borrow().get("fps").unwrap().style().remove_property("display").unwrap();
                game.borrow().ui_elements.borrow().get("encrypt").unwrap().style().remove_property("display").unwrap();
                game.borrow().ui_elements.borrow().get("claim").unwrap().style().remove_property("display").unwrap();

                let claimant = game.borrow().robot_claims.borrow().get(&get_selected_robot().unwrap_or_default()).unwrap_or(&"None".to_owned()).clone();

                if claimant == get_username() {
                    game.borrow().ui_elements.borrow().get("reset").unwrap().style().remove_property("display").unwrap();
                    game.borrow().ui_elements.borrow().get("encrypt").unwrap().style().remove_property("display").unwrap();

                    game.borrow().ui_elements.borrow().get("claim").unwrap().set_inner_text("Unclaim");
                } else {
                    game.borrow().ui_elements.borrow().get("claim").unwrap().set_inner_text("Claim");
                }
                game.borrow().ui_elements.borrow().get("claim_text").unwrap().style().set_property("display", "inline-block").unwrap();
            }
        }
    });
}

pub(crate) fn clear_robots_menu() {
    let robotmenu: HtmlElement = get_nb_externalvar("roboscapedialog-robotmenu").unwrap().unchecked_into();
    robotmenu.set_inner_html("<option></option>");
}

pub(crate) fn update_claim_text() {
    GAME.with(|game| {
        let claimant = game.borrow().robot_claims.borrow().get(&get_selected_robot().unwrap_or_default()).unwrap_or(&"None".to_owned()).clone();
        game.borrow().ui_elements.borrow().get("claim_text").unwrap().set_inner_text(format!("Claimed by: {}", claimant).as_str());
    });
}

pub(crate) fn show_dialog(dialog_name: &str) {
    let dialog = get_nb_externalvar(dialog_name).unwrap();
    let f = get_window_fn("showDialog").unwrap();
    f.call1(&JsValue::NULL, &dialog).unwrap();
}

pub(crate) fn hide_dialog(dialog_name: &str) {
    let dialog = get_nb_externalvar(dialog_name).unwrap();
    let f = get_window_fn("hideDialog").unwrap();
    f.call1(&JsValue::NULL, &dialog).unwrap();
}