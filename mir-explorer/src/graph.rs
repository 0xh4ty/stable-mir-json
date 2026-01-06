//! Graph data model for the MIR explorer
//!
//! These structures mirror the ExplorerData types from the main crate's explore.rs,
//! but with Deserialize since we're loading JSON rather than generating it.

use serde::{Deserialize, Serialize};

/// Complete data for the explorer, loaded from JSON
#[derive(Debug, Clone, Deserialize)]
pub struct ExplorerData {
    pub name: String,
    pub functions: Vec<ExplorerFunction>,
}

/// A single function with its control flow graph
#[derive(Debug, Clone, Deserialize)]
pub struct ExplorerFunction {
    pub name: String,
    pub short_name: String,
    pub blocks: Vec<ExplorerBlock>,
    pub locals: Vec<ExplorerLocal>,
    pub entry_block: usize,
}

/// A basic block in the control flow graph
#[derive(Debug, Clone, Deserialize)]
pub struct ExplorerBlock {
    pub id: usize,
    pub statements: Vec<ExplorerStmt>,
    pub terminator: ExplorerTerminator,
    pub predecessors: Vec<usize>,
    pub role: BlockRole,
    pub summary: String,
}

/// A single MIR statement
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExplorerStmt {
    pub mir: String,
    pub annotation: String,
}

/// Assignment tracking for a local variable
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExplorerAssignment {
    pub block_id: usize,
    pub value: String,
}

/// A local variable with its type and assignments
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExplorerLocal {
    pub name: String,
    pub ty: String,
    pub source_name: Option<String>,
    pub assignments: Vec<ExplorerAssignment>,
}

/// The terminator instruction of a basic block
#[derive(Debug, Clone, Deserialize)]
pub struct ExplorerTerminator {
    pub kind: String,
    pub mir: String,
    pub annotation: String,
    pub edges: Vec<ExplorerEdge>,
}

/// An edge in the control flow graph
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExplorerEdge {
    pub target: usize,
    pub label: String,
    pub kind: EdgeKind,
    pub annotation: String,
}

/// Classification of control flow edges
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeKind {
    Normal,
    Cleanup,
    Otherwise,
    Branch,
}

/// Classification of basic blocks by their role in control flow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BlockRole {
    Entry,
    Exit,
    #[serde(rename = "branchpoint")]
    BranchPoint,
    #[serde(rename = "mergepoint")]
    MergePoint,
    Linear,
    Cleanup,
}

impl BlockRole {
    /// Get the border color for this role
    pub fn border_color(&self) -> &'static str {
        match self {
            BlockRole::Entry => "#50fa7b",       // Green
            BlockRole::Exit => "#bd93f9",        // Purple
            BlockRole::BranchPoint => "#ffb86c", // Orange
            BlockRole::MergePoint => "#8be9fd",  // Cyan
            BlockRole::Cleanup => "#ff5555",     // Red
            BlockRole::Linear => "#555",         // Gray
        }
    }
}
