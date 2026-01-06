//! Canvas 2D rendering for the MIR graph

use std::collections::HashSet;
use std::f64::consts::PI;

use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

use crate::graph::{BlockRole, EdgeKind, ExplorerFunction};
use crate::layout::{GraphLayout, LayoutEdge, LayoutNode};

// Colors from the theme
const BG_COLOR: &str = "#1a1a2e";
const NODE_BG: &str = "#3a3a5e";
const NODE_VISITED: &str = "#2a4a6e";
const NODE_CURRENT: &str = "#50fa7b";
const TEXT_COLOR: &str = "#eee";
const TEXT_DARK: &str = "#1a1a2e";
const EDGE_COLOR: &str = "#555";
const EDGE_TAKEN: &str = "#50fa7b";
const EDGE_CLEANUP: &str = "#ff5555";
const EDGE_SELECTED: &str = "#8be9fd";

/// Canvas renderer for the graph
pub struct Renderer {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    width: f64,
    height: f64,
}

impl Renderer {
    /// Create a new renderer for the given canvas element
    pub fn new(canvas_id: &str) -> Result<Self, JsValue> {
        let document = web_sys::window()
            .ok_or("no window")?
            .document()
            .ok_or("no document")?;

        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| JsValue::from_str(&format!("canvas '{}' not found", canvas_id)))?
            .dyn_into::<HtmlCanvasElement>()?;

        let ctx = canvas
            .get_context("2d")?
            .ok_or("no 2d context")?
            .dyn_into::<CanvasRenderingContext2d>()?;

        let width = canvas.width() as f64;
        let height = canvas.height() as f64;

        Ok(Self {
            canvas,
            ctx,
            width,
            height,
        })
    }

    pub fn width(&self) -> f64 {
        self.canvas.get_bounding_client_rect().width()
    }

    pub fn height(&self) -> f64 {
        self.canvas.get_bounding_client_rect().height()
    }

    /// Render the graph
    pub fn render(
        &self,
        layout: &GraphLayout,
        func: &ExplorerFunction,
        path: &[usize],
        current: usize,
        selected_edge: usize,
        scale: f64,
        offset: (f64, f64),
    ) {
        // Update canvas size if needed
        let dpr = web_sys::window()
            .map(|w| w.device_pixel_ratio())
            .unwrap_or(1.0);

        let rect = self.canvas.get_bounding_client_rect();
        let display_width = rect.width();
        let display_height = rect.height();

        if (self.canvas.width() as f64) != display_width * dpr {
            self.canvas.set_width((display_width * dpr) as u32);
            self.canvas.set_height((display_height * dpr) as u32);
            self.ctx.scale(dpr, dpr).unwrap_or(());
        }

        // Clear
        self.ctx.set_fill_style(&JsValue::from_str(BG_COLOR));
        self.ctx.fill_rect(0.0, 0.0, display_width, display_height);

        // Apply transform
        self.ctx.save();
        self.ctx.translate(offset.0, offset.1).unwrap_or(());
        self.ctx.scale(scale, scale).unwrap_or(());

        let path_set: HashSet<usize> = path.iter().copied().collect();

        // Get edges from current block for highlighting
        let current_edges: Vec<usize> = func
            .blocks
            .get(current)
            .map(|b| b.terminator.edges.iter().map(|e| e.target).collect())
            .unwrap_or_default();

        // Render edges first (behind nodes)
        for edge in &layout.edges {
            let is_taken = self.is_edge_in_path(edge.from, edge.to, path, current);
            let is_selected =
                edge.from == current && current_edges.get(selected_edge) == Some(&edge.to);
            self.render_edge(edge, is_taken, is_selected);
        }

        // Render nodes
        for node in &layout.nodes {
            let is_current = node.id == current;
            let is_visited = path_set.contains(&node.id);
            self.render_node(node, is_current, is_visited, &path_set);
        }

        self.ctx.restore();
    }

    fn render_node(
        &self,
        node: &LayoutNode,
        is_current: bool,
        is_visited: bool,
        visited_set: &HashSet<usize>,
    ) {
        let ctx = &self.ctx;

        // Determine opacity for unvisited nodes
        let is_reachable = is_current || is_visited || visited_set.is_empty();
        if !is_reachable {
            ctx.set_global_alpha(0.35);
        }

        // Background fill
        let fill = if is_current {
            NODE_CURRENT
        } else if is_visited {
            NODE_VISITED
        } else {
            NODE_BG
        };

        // Draw rounded rectangle
        ctx.begin_path();
        self.rounded_rect(node.x, node.y, node.width, node.height, 6.0);
        ctx.set_fill_style(&JsValue::from_str(fill));
        ctx.fill();

        // Border
        let border_color = node.role.border_color();
        let border_width = if is_current || node.role != BlockRole::Linear {
            3.0
        } else {
            2.0
        };
        ctx.set_stroke_style(&JsValue::from_str(border_color));
        ctx.set_line_width(border_width);
        ctx.stroke();

        // Label
        let text_color = if is_current { TEXT_DARK } else { TEXT_COLOR };
        ctx.set_fill_style(&JsValue::from_str(text_color));
        ctx.set_font("bold 12px monospace");
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");
        let label = format!("bb{}", node.id);
        ctx.fill_text(
            &label,
            node.x + node.width / 2.0,
            node.y + node.height / 2.0,
        )
        .unwrap_or(());

        // Reset opacity
        ctx.set_global_alpha(1.0);
    }

    fn render_edge(&self, edge: &LayoutEdge, is_taken: bool, is_selected: bool) {
        let ctx = &self.ctx;

        // Determine color and width
        let (color, width) = if is_selected {
            (EDGE_SELECTED, 3.0)
        } else if is_taken {
            (EDGE_TAKEN, 3.0)
        } else {
            match edge.kind {
                EdgeKind::Cleanup => (EDGE_CLEANUP, 2.0),
                _ => (EDGE_COLOR, 2.0),
            }
        };

        ctx.begin_path();
        ctx.set_stroke_style(&JsValue::from_str(color));
        ctx.set_line_width(width);

        // Set dash pattern for cleanup edges
        if edge.kind == EdgeKind::Cleanup {
            let dash = js_sys::Array::new();
            dash.push(&JsValue::from(5.0));
            dash.push(&JsValue::from(5.0));
            ctx.set_line_dash(&dash).unwrap_or(());
        } else {
            ctx.set_line_dash(&js_sys::Array::new()).unwrap_or(());
        }

        // Draw path through control points
        if let Some((first, rest)) = edge.points.split_first() {
            ctx.move_to(first.0, first.1);
            for point in rest {
                ctx.line_to(point.0, point.1);
            }
        }
        ctx.stroke();

        // Draw arrowhead
        if edge.points.len() >= 2 {
            let last = edge.points[edge.points.len() - 1];
            let prev = edge.points[edge.points.len() - 2];
            self.draw_arrowhead(prev.0, prev.1, last.0, last.1, color);
        }

        // Draw label if present
        if !edge.label.is_empty() && edge.points.len() >= 2 {
            self.draw_edge_label(edge, color);
        }

        // Reset line dash
        ctx.set_line_dash(&js_sys::Array::new()).unwrap_or(());
    }

    fn draw_arrowhead(&self, from_x: f64, from_y: f64, to_x: f64, to_y: f64, color: &str) {
        let ctx = &self.ctx;
        let angle = (to_y - from_y).atan2(to_x - from_x);
        let arrow_size = 8.0;

        ctx.begin_path();
        ctx.move_to(to_x, to_y);
        ctx.line_to(
            to_x - arrow_size * (angle - PI / 6.0).cos(),
            to_y - arrow_size * (angle - PI / 6.0).sin(),
        );
        ctx.line_to(
            to_x - arrow_size * (angle + PI / 6.0).cos(),
            to_y - arrow_size * (angle + PI / 6.0).sin(),
        );
        ctx.close_path();
        ctx.set_fill_style(&JsValue::from_str(color));
        ctx.fill();
    }

    fn draw_edge_label(&self, edge: &LayoutEdge, color: &str) {
        let ctx = &self.ctx;

        // Find midpoint of edge
        let mid_idx = edge.points.len() / 2;
        let (mid_x, mid_y) = if mid_idx > 0 && mid_idx < edge.points.len() {
            let p1 = edge.points[mid_idx - 1];
            let p2 = edge.points[mid_idx];
            ((p1.0 + p2.0) / 2.0, (p1.1 + p2.1) / 2.0)
        } else if !edge.points.is_empty() {
            edge.points[0]
        } else {
            return;
        };

        // Draw label background
        ctx.set_font("9px monospace");
        let metrics = ctx
            .measure_text(&edge.label)
            .unwrap_or_else(|_| ctx.measure_text("").unwrap());
        let text_width = metrics.width();
        let padding = 3.0;

        ctx.set_fill_style(&JsValue::from_str(BG_COLOR));
        ctx.fill_rect(
            mid_x - text_width / 2.0 - padding,
            mid_y - 6.0 - padding,
            text_width + padding * 2.0,
            12.0 + padding * 2.0,
        );

        // Draw label text
        ctx.set_fill_style(&JsValue::from_str(color));
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");
        ctx.fill_text(&edge.label, mid_x, mid_y).unwrap_or(());
    }

    fn is_edge_in_path(&self, from: usize, to: usize, path: &[usize], current: usize) -> bool {
        // Check if this edge was taken in the path
        for i in 0..path.len() {
            let path_from = path[i];
            let path_to = if i + 1 < path.len() {
                path[i + 1]
            } else {
                current
            };
            if path_from == from && path_to == to {
                return true;
            }
        }
        false
    }

    fn rounded_rect(&self, x: f64, y: f64, w: f64, h: f64, r: f64) {
        let ctx = &self.ctx;
        ctx.move_to(x + r, y);
        ctx.line_to(x + w - r, y);
        ctx.arc_to(x + w, y, x + w, y + r, r).unwrap_or(());
        ctx.line_to(x + w, y + h - r);
        ctx.arc_to(x + w, y + h, x + w - r, y + h, r).unwrap_or(());
        ctx.line_to(x + r, y + h);
        ctx.arc_to(x, y + h, x, y + h - r, r).unwrap_or(());
        ctx.line_to(x, y + r);
        ctx.arc_to(x, y, x + r, y, r).unwrap_or(());
    }
}
