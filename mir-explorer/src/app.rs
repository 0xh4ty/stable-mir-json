//! Application state and main explorer logic

use wasm_bindgen::prelude::*;

use crate::graph::{ExplorerData, ExplorerFunction};
use crate::input::{parse_key, InputAction};
use crate::layout::GraphLayout;
use crate::render::Renderer;

/// The main MIR explorer application
#[wasm_bindgen]
pub struct MirExplorer {
    data: Option<ExplorerData>,
    current_fn_index: usize,
    current_block: usize,
    selected_edge: usize,
    path: Vec<usize>,
    layout: Option<GraphLayout>,
    renderer: Renderer,
    context_id: String,
    scale: f64,
    offset: (f64, f64),
}

#[wasm_bindgen]
impl MirExplorer {
    /// Create a new explorer attached to a canvas and context panel
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: &str, context_id: &str) -> Result<MirExplorer, JsValue> {
        let renderer = Renderer::new(canvas_id)?;
        Ok(Self {
            data: None,
            current_fn_index: 0,
            current_block: 0,
            selected_edge: 0,
            path: Vec::new(),
            layout: None,
            renderer,
            context_id: context_id.to_string(),
            scale: 1.0,
            offset: (0.0, 0.0),
        })
    }

    /// Load explorer data from JSON string
    pub fn load_json(&mut self, json: &str) -> Result<(), JsValue> {
        let data: ExplorerData = serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("JSON parse error: {}", e)))?;

        if data.functions.is_empty() {
            return Err(JsValue::from_str("No functions in data"));
        }

        self.data = Some(data);
        self.select_function(0);
        Ok(())
    }

    /// Get the number of functions
    pub fn function_count(&self) -> usize {
        self.data.as_ref().map(|d| d.functions.len()).unwrap_or(0)
    }

    /// Get the name of a function by index
    pub fn function_name(&self, index: usize) -> Option<String> {
        self.data.as_ref()
            .and_then(|d| d.functions.get(index))
            .map(|f| f.short_name.clone())
    }

    /// Get the crate name
    pub fn crate_name(&self) -> Option<String> {
        self.data.as_ref().map(|d| d.name.clone())
    }

    /// Select a function by index
    pub fn select_function(&mut self, index: usize) {
        if let Some(data) = &self.data {
            if index >= data.functions.len() {
                return;
            }
            self.current_fn_index = index;
            self.path.clear();
            self.selected_edge = 0;

            let func = &data.functions[index];
            self.layout = Some(GraphLayout::from_function(func));
            self.current_block = func.entry_block;

            // Auto-fit the graph to the viewport
            self.fit_to_view_internal();
            self.render();
        }
    }

    /// Internal fit to view (doesn't render, used during initialization)
    fn fit_to_view_internal(&mut self) {
        if let Some(layout) = &self.layout {
            let canvas_width = self.renderer.width();
            let canvas_height = self.renderer.height();

            let (min_x, min_y, max_x, max_y) = layout.bounds;
            let graph_width = max_x - min_x;
            let graph_height = max_y - min_y;

            if graph_width > 0.0 && graph_height > 0.0 {
                let padding = 60.0;
                let scale_x = (canvas_width - padding * 2.0) / graph_width;
                let scale_y = (canvas_height - padding * 2.0) / graph_height;
                // Use a minimum scale of 0.5 to avoid tiny graphs
                self.scale = scale_x.min(scale_y).clamp(0.5, 2.0);

                let center_x = (min_x + max_x) / 2.0;
                let center_y = (min_y + max_y) / 2.0;
                self.offset = (
                    canvas_width / 2.0 - center_x * self.scale,
                    canvas_height / 2.0 - center_y * self.scale,
                );
            }
        }
    }

    /// Navigate to a specific block
    pub fn go_to_block(&mut self, block_id: usize) {
        self.go_to_block_internal(block_id, true);
    }

    fn go_to_block_internal(&mut self, block_id: usize, add_to_path: bool) {
        if let Some(data) = &self.data {
            let func = &data.functions[self.current_fn_index];
            if block_id >= func.blocks.len() {
                return;
            }

            if add_to_path && self.current_block != block_id {
                self.path.push(self.current_block);
            }
            self.current_block = block_id;
            self.selected_edge = 0;
            self.center_on_block(block_id);
            self.render();
        }
    }

    /// Go back to the previous block in the path
    pub fn go_back(&mut self) {
        if let Some(prev) = self.path.pop() {
            self.current_block = prev;
            self.selected_edge = 0;
            self.center_on_block(prev);
            self.render();
        }
    }

    /// Reset to the entry block
    pub fn reset(&mut self) {
        self.path.clear();
        self.selected_edge = 0;
        if let Some(data) = &self.data {
            let entry = data.functions[self.current_fn_index].entry_block;
            self.go_to_block_internal(entry, false);
        }
    }

    /// Follow the currently selected edge
    pub fn follow_edge(&mut self, edge_index: usize) {
        if let Some(data) = &self.data {
            let func = &data.functions[self.current_fn_index];
            let block = &func.blocks[self.current_block];
            if let Some(edge) = block.terminator.edges.get(edge_index) {
                self.go_to_block(edge.target);
            }
        }
    }

    /// Select next edge (for j/down)
    pub fn select_next_edge(&mut self) {
        if let Some(data) = &self.data {
            let func = &data.functions[self.current_fn_index];
            let block = &func.blocks[self.current_block];
            let edge_count = block.terminator.edges.len();
            if edge_count > 0 {
                self.selected_edge = (self.selected_edge + 1) % edge_count;
                self.render();
            }
        }
    }

    /// Select previous edge (for k/up)
    pub fn select_prev_edge(&mut self) {
        if let Some(data) = &self.data {
            let func = &data.functions[self.current_fn_index];
            let block = &func.blocks[self.current_block];
            let edge_count = block.terminator.edges.len();
            if edge_count > 0 {
                self.selected_edge = if self.selected_edge == 0 {
                    edge_count - 1
                } else {
                    self.selected_edge - 1
                };
                self.render();
            }
        }
    }

    /// Handle a keyboard event, returns true if handled
    pub fn handle_key(&mut self, key: &str) -> bool {
        match parse_key(key) {
            InputAction::GoBack => {
                self.go_back();
                true
            }
            InputAction::Reset => {
                self.reset();
                true
            }
            InputAction::SelectEdge(n) => {
                self.follow_edge(n);
                true
            }
            InputAction::MoveDown => {
                self.select_next_edge();
                true
            }
            InputAction::MoveUp => {
                self.select_prev_edge();
                true
            }
            InputAction::MoveRight => {
                self.follow_edge(self.selected_edge);
                true
            }
            InputAction::FocusSearch => {
                // Let the JS handle this
                false
            }
            InputAction::None => false,
        }
    }

    /// Render the current state
    pub fn render(&self) {
        if let (Some(layout), Some(data)) = (&self.layout, &self.data) {
            let func = &data.functions[self.current_fn_index];
            self.renderer.render(
                layout,
                func,
                &self.path,
                self.current_block,
                self.selected_edge,
                self.scale,
                self.offset,
            );
        }
    }

    /// Get current block info as JSON for the context panel
    pub fn get_block_info_json(&self) -> Option<String> {
        let data = self.data.as_ref()?;
        let func = &data.functions[self.current_fn_index];
        let block = func.blocks.get(self.current_block)?;

        // Return a simple JSON with block info
        serde_json::to_string(&serde_json::json!({
            "id": block.id,
            "role": format!("{:?}", block.role).to_lowercase(),
            "summary": block.summary,
            "statements": block.statements,
            "terminator": {
                "kind": block.terminator.kind,
                "mir": block.terminator.mir,
                "annotation": block.terminator.annotation,
                "edges": block.terminator.edges,
            },
            "predecessors": block.predecessors,
            "path": self.path,
            "selected_edge": self.selected_edge,
        })).ok()
    }

    /// Get locals info as JSON
    pub fn get_locals_json(&self) -> Option<String> {
        let data = self.data.as_ref()?;
        let func = &data.functions[self.current_fn_index];
        serde_json::to_string(&func.locals).ok()
    }

    fn center_on_block(&mut self, block_id: usize) {
        if let Some(layout) = &self.layout {
            if let Some(node) = layout.nodes.get(block_id) {
                let canvas_width = self.renderer.width();
                let canvas_height = self.renderer.height();

                // Center the block in the viewport
                self.offset = (
                    canvas_width / 2.0 - (node.x + node.width / 2.0) * self.scale,
                    canvas_height / 2.0 - (node.y + node.height / 2.0) * self.scale,
                );
            }
        }
    }

    /// Handle mouse wheel for zooming
    pub fn handle_wheel(&mut self, delta_y: f64, mouse_x: f64, mouse_y: f64) {
        let zoom_factor = if delta_y > 0.0 { 0.9 } else { 1.1 };
        let new_scale = (self.scale * zoom_factor).clamp(0.2, 3.0);

        // Zoom toward mouse position
        let scale_change = new_scale / self.scale;
        self.offset.0 = mouse_x - (mouse_x - self.offset.0) * scale_change;
        self.offset.1 = mouse_y - (mouse_y - self.offset.1) * scale_change;
        self.scale = new_scale;

        self.render();
    }

    /// Handle mouse drag for panning
    pub fn handle_drag(&mut self, delta_x: f64, delta_y: f64) {
        self.offset.0 += delta_x;
        self.offset.1 += delta_y;
        self.render();
    }

    /// Fit the graph to the viewport
    pub fn fit_to_view(&mut self) {
        self.fit_to_view_internal();
        self.render();
    }
}

impl MirExplorer {
    /// Get the current function
    pub fn current_function(&self) -> Option<&ExplorerFunction> {
        self.data.as_ref()?.functions.get(self.current_fn_index)
    }
}
