use std::{collections::BTreeMap, cell::RefCell, rc::Rc};

use roboscapesim_common::ClientMessage;

use crate::{util::*, console_log};

use super::send_message;

use js_sys::eval;
use wasm_bindgen::{prelude::Closure, JsValue};

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

thread_local! {
    static TEXT_BLOCKS: Rc<RefCell<BTreeMap<String, JsValue>>> = Rc::new(RefCell::new(BTreeMap::new()));
}

/**
 * Create a TextBlock in the 3D view's overlay.
 * If a TextBlock already has the id, that TextBlock's text and timeout will be updated.
 * 
 * @param {string} text Text to display in TextBlock
 * @param {string} id ID of TextBlock
 * @param {number | boolean} timeout TextBlock will be removed after timeout ms, or never if timeout is falsey.
 */
pub(crate) fn add_or_update_text(text: &str, id: &str, timeout: Option<f64>) {
    TEXT_BLOCKS.with(|text_blocks| {
        if !text_blocks.borrow().contains_key(id) {
            let text_block = eval(&("let textBlock = new BABYLON.GUI.TextBlock('textblock_' + ('".to_owned() + id + "' ?? Math.round(Math.random() * 10000000)));
            textBlock.heightInPixels = 24;
            textBlock.outlineColor = '#2226';
            textBlock.outlineWidth = 3;
            textBlock.color = '#FFF';
            textBlock.fontSizeInPixels = 20;
            textBlock;")).unwrap();
            js_set(&text_block, "text", text).unwrap();
            js_call_member(&get_nb_externalvar("roboscapesim-textStackPanel").unwrap(), "addControl", &[&text_block]).unwrap();
            text_blocks.borrow_mut().insert(js_get(&text_block, "name").unwrap().as_string().unwrap(), text_block);
        } else {
            js_set(&text_blocks.borrow()[id], "text", text).unwrap();
        }

        if let Some(timeout) = timeout {
            // if (textBlocks[id].timeout) {
            //     clearTimeout(textBlocks[id].timeout);
            // }

            // textBlocks[id].timeout = setTimeout(() => {
            //     textStackPanel.removeControl(textBlocks[id]);
            //     delete textBlocks[id];
            // }, timeout);
        }
    });
}

/**
 * Removes all TextBlocks from the 3D view's overlay
 */
pub(crate) fn clear_all_text_blocks() {
    TEXT_BLOCKS.with(|text_blocks| {
        for text_block in text_blocks.borrow().iter() {
            // if (Object.hasOwnProperty.call(textBlocks, id)) {
            //     const element = textBlocks[id];

            //     if (element.timeout) {
            //         clearTimeout(element.timeout);
            //     }

            //     textStackPanel.removeControl(element);
            //     delete textBlocks[id];
            // }
        }
        text_blocks.borrow_mut().clear();
    });
}