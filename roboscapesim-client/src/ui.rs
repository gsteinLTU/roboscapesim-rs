use std::{collections::BTreeMap, cell::{RefCell, Cell}, rc::Rc};

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
        // TODO: Allow robot reset requests too
        send_message(&ClientMessage::ResetAll);
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