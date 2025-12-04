//! WASM-based MIR Graph Explorer
//!
//! An interactive visualization tool for exploring MIR control flow graphs.
//! Uses Canvas 2D for rendering and supports vim-style keyboard navigation.

use wasm_bindgen::prelude::*;

pub mod app;
pub mod graph;
pub mod input;
pub mod layout;
pub mod render;

pub use app::MirExplorer;

/// Initialize the WASM module (called automatically on load)
#[wasm_bindgen(start)]
pub fn init() {
    // Set up better panic messages in the browser console
    console_error_panic_hook::set_once();
}

/// Create a new MIR explorer instance
///
/// # Arguments
/// * `canvas_id` - The ID of the canvas element for graph rendering
/// * `context_id` - The ID of the container element for the context panel
///
/// # Returns
/// A new MirExplorer instance, or throws an error if initialization fails
#[wasm_bindgen]
pub fn create_explorer(canvas_id: &str, context_id: &str) -> Result<MirExplorer, JsValue> {
    MirExplorer::new(canvas_id, context_id)
}

/// Log a message to the browser console (for debugging)
#[wasm_bindgen]
pub fn log(msg: &str) {
    web_sys::console::log_1(&JsValue::from_str(msg));
}
