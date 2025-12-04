//! Graph layout algorithms for positioning nodes and routing edges

use std::collections::{HashMap, HashSet, VecDeque};

use crate::graph::{BlockRole, EdgeKind, ExplorerFunction};

/// A positioned node in the layout
#[derive(Debug, Clone)]
pub struct LayoutNode {
    pub id: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub role: BlockRole,
}

/// A routed edge in the layout
#[derive(Debug, Clone)]
pub struct LayoutEdge {
    pub from: usize,
    pub to: usize,
    pub label: String,
    pub kind: EdgeKind,
    /// Control points for drawing the edge (start, optional control points, end)
    pub points: Vec<(f64, f64)>,
}

/// Complete layout information for a function's CFG
#[derive(Debug, Clone)]
pub struct GraphLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    /// Bounding box: (min_x, min_y, max_x, max_y)
    pub bounds: (f64, f64, f64, f64),
}

// Layout constants
const NODE_WIDTH: f64 = 60.0;
const NODE_HEIGHT: f64 = 35.0;
const HORIZONTAL_SPACING: f64 = 80.0;
const VERTICAL_SPACING: f64 = 100.0;

impl GraphLayout {
    /// Create a layout from a function
    pub fn from_function(func: &ExplorerFunction) -> Self {
        let block_count = func.blocks.len();
        if block_count == 0 {
            return Self {
                nodes: Vec::new(),
                edges: Vec::new(),
                bounds: (0.0, 0.0, 0.0, 0.0),
            };
        }

        // Build adjacency list for BFS
        let mut successors: HashMap<usize, Vec<usize>> = HashMap::new();
        for block in &func.blocks {
            let targets: Vec<usize> = block.terminator.edges.iter()
                .map(|e| e.target)
                .collect();
            successors.insert(block.id, targets);
        }

        // BFS to assign layers (distance from entry)
        let layers = Self::compute_layers(func.entry_block, block_count, &successors);

        // Position nodes within layers
        let nodes = Self::position_nodes(func, &layers);

        // Route edges
        let edges = Self::route_edges(func, &nodes);

        // Compute bounds
        let bounds = Self::compute_bounds(&nodes);

        Self { nodes, edges, bounds }
    }

    /// Compute layers using BFS from entry
    fn compute_layers(
        entry: usize,
        block_count: usize,
        successors: &HashMap<usize, Vec<usize>>,
    ) -> Vec<Vec<usize>> {
        let mut layer_map: HashMap<usize, usize> = HashMap::new();
        let mut visited: HashSet<usize> = HashSet::new();
        let mut queue: VecDeque<(usize, usize)> = VecDeque::new();

        queue.push_back((entry, 0));
        visited.insert(entry);

        while let Some((node, layer)) = queue.pop_front() {
            layer_map.insert(node, layer);

            if let Some(succs) = successors.get(&node) {
                for &succ in succs {
                    if !visited.contains(&succ) {
                        visited.insert(succ);
                        queue.push_back((succ, layer + 1));
                    }
                }
            }
        }

        // Handle unreachable nodes (put them in the last layer)
        let max_layer = layer_map.values().copied().max().unwrap_or(0);
        for id in 0..block_count {
            layer_map.entry(id).or_insert(max_layer + 1);
        }

        // Group by layer
        let num_layers = layer_map.values().copied().max().unwrap_or(0) + 1;
        let mut layers: Vec<Vec<usize>> = vec![Vec::new(); num_layers];
        for (node, layer) in layer_map {
            layers[layer].push(node);
        }

        // Sort nodes within each layer for consistent ordering
        for layer in &mut layers {
            layer.sort();
        }

        layers
    }

    /// Position nodes based on layer assignment
    fn position_nodes(func: &ExplorerFunction, layers: &[Vec<usize>]) -> Vec<LayoutNode> {
        let mut nodes: Vec<LayoutNode> = vec![
            LayoutNode {
                id: 0,
                x: 0.0,
                y: 0.0,
                width: NODE_WIDTH,
                height: NODE_HEIGHT,
                role: BlockRole::Linear,
            };
            func.blocks.len()
        ];

        for (layer_idx, layer) in layers.iter().enumerate() {
            let layer_width = layer.len() as f64 * (NODE_WIDTH + HORIZONTAL_SPACING) - HORIZONTAL_SPACING;
            let start_x = -layer_width / 2.0;

            for (pos_in_layer, &node_id) in layer.iter().enumerate() {
                if node_id < nodes.len() {
                    nodes[node_id] = LayoutNode {
                        id: node_id,
                        x: start_x + pos_in_layer as f64 * (NODE_WIDTH + HORIZONTAL_SPACING),
                        y: layer_idx as f64 * VERTICAL_SPACING,
                        width: NODE_WIDTH,
                        height: NODE_HEIGHT,
                        role: func.blocks[node_id].role,
                    };
                }
            }
        }

        nodes
    }

    /// Route edges between nodes
    fn route_edges(func: &ExplorerFunction, nodes: &[LayoutNode]) -> Vec<LayoutEdge> {
        let mut edges = Vec::new();

        for block in &func.blocks {
            let from = block.id;
            let from_node = &nodes[from];
            let from_center_x = from_node.x + from_node.width / 2.0;
            let from_bottom_y = from_node.y + from_node.height;

            for edge in &block.terminator.edges {
                let to = edge.target;
                let to_node = &nodes[to];
                let to_center_x = to_node.x + to_node.width / 2.0;
                let to_top_y = to_node.y;

                // Simple edge routing with optional curve for back edges
                let points = if to_node.y <= from_node.y {
                    // Back edge - route around
                    Self::route_back_edge(from_node, to_node)
                } else {
                    // Forward edge - simple bezier
                    Self::route_forward_edge(from_center_x, from_bottom_y, to_center_x, to_top_y)
                };

                edges.push(LayoutEdge {
                    from,
                    to,
                    label: edge.label.clone(),
                    kind: edge.kind,
                    points,
                });
            }
        }

        edges
    }

    /// Route a forward edge (going down)
    fn route_forward_edge(
        from_x: f64,
        from_y: f64,
        to_x: f64,
        to_y: f64,
    ) -> Vec<(f64, f64)> {
        let mid_y = (from_y + to_y) / 2.0;
        vec![
            (from_x, from_y),
            (from_x, mid_y),
            (to_x, mid_y),
            (to_x, to_y),
        ]
    }

    /// Route a back edge (going up or same level)
    fn route_back_edge(from: &LayoutNode, to: &LayoutNode) -> Vec<(f64, f64)> {
        let from_center_x = from.x + from.width / 2.0;
        let from_bottom_y = from.y + from.height;
        let to_center_x = to.x + to.width / 2.0;
        let to_bottom_y = to.y + to.height;

        // Route to the right, then up, then back
        let offset = 30.0;
        let right_x = from.x.max(to.x) + from.width + offset;

        vec![
            (from_center_x, from_bottom_y),
            (from_center_x, from_bottom_y + offset),
            (right_x, from_bottom_y + offset),
            (right_x, to_bottom_y + offset),
            (to_center_x, to_bottom_y + offset),
            (to_center_x, to_bottom_y),
        ]
    }

    /// Compute the bounding box of all nodes
    fn compute_bounds(nodes: &[LayoutNode]) -> (f64, f64, f64, f64) {
        if nodes.is_empty() {
            return (0.0, 0.0, 0.0, 0.0);
        }

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for node in nodes {
            min_x = min_x.min(node.x);
            min_y = min_y.min(node.y);
            max_x = max_x.max(node.x + node.width);
            max_y = max_y.max(node.y + node.height);
        }

        (min_x, min_y, max_x, max_y)
    }
}
