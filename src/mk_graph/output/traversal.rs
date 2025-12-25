//! Shared MIR traversal and analysis logic.
//!
//! This module provides common types, analysis functions, and a traversal
//! framework that can be used by different output formats (markdown, typst, etc.)

use std::collections::{HashMap, HashSet};

extern crate stable_mir;
use stable_mir::mir::{
    BasicBlock, Body, Rvalue, Statement, StatementKind, Terminator, TerminatorKind, UnwindAction,
};
use stable_mir::ty::IndexedVal;

use crate::render::{
    annotate_rvalue, extract_call_name, render_operand, render_place, render_rvalue,
};

// =============================================================================
// Common Types
// =============================================================================

/// Span information: (filename, start_line, start_col, end_line, end_col)
pub type SpanInfo = (String, usize, usize, usize, usize);

/// Detected properties of a function
#[derive(Default, Clone)]
pub struct FunctionProperties {
    pub has_panic_path: bool,
    pub has_checked_ops: bool,
    pub has_borrows: bool,
    pub has_drops: bool,
    pub has_recursion: bool,
    pub has_assertions: bool,
    pub has_switches: bool,
}

/// Inferred role of a basic block
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BlockRole {
    Entry,
    Return,
    Panic,
    Cleanup,
    Branch,
    Loop,
    Normal,
}

impl BlockRole {
    /// Human-readable title for the block role
    pub fn title(&self) -> &'static str {
        match self {
            BlockRole::Entry => "entry",
            BlockRole::Return => "return / success",
            BlockRole::Panic => "panic path",
            BlockRole::Cleanup => "cleanup / unwind",
            BlockRole::Branch => "branch point",
            BlockRole::Loop => "loop",
            BlockRole::Normal => "",
        }
    }

    /// Suffix for ASCII CFG diagrams
    pub fn cfg_suffix(&self) -> &'static str {
        match self {
            BlockRole::Entry => " (entry)",
            BlockRole::Return => " (return)",
            BlockRole::Panic => " (panic)",
            BlockRole::Cleanup => " (cleanup)",
            BlockRole::Branch => " (branch)",
            BlockRole::Loop => " (loop)",
            BlockRole::Normal => "",
        }
    }
}

/// A rendered MIR row (statement or terminator)
#[derive(Clone)]
pub struct AnnotatedRow {
    pub mir: String,
    pub annotation: String,
    pub is_terminator: bool,
    pub is_recursive: bool,
}

// =============================================================================
// Analysis Functions
// =============================================================================

/// Analyze a function body to detect notable properties
pub fn analyze_function(body: &Body, current_fn: &str) -> FunctionProperties {
    let mut props = FunctionProperties::default();

    for block in &body.blocks {
        // Check statements
        for stmt in &block.statements {
            if let StatementKind::Assign(_, rvalue) = &stmt.kind {
                match rvalue {
                    Rvalue::CheckedBinaryOp(..) => props.has_checked_ops = true,
                    Rvalue::Ref(..) | Rvalue::AddressOf(..) => props.has_borrows = true,
                    _ => {}
                }
            }
        }

        // Check terminator
        match &block.terminator.kind {
            TerminatorKind::Call { func, target, .. } => {
                let func_name = extract_call_name(func);
                if func_name == current_fn {
                    props.has_recursion = true;
                }
                if func_name.contains("panic")
                    || func_name.contains("assert_failed")
                    || target.is_none()
                {
                    props.has_panic_path = true;
                }
            }
            TerminatorKind::Assert { .. } => {
                props.has_assertions = true;
                props.has_panic_path = true;
            }
            TerminatorKind::SwitchInt { .. } => props.has_switches = true,
            TerminatorKind::Drop { .. } => props.has_drops = true,
            TerminatorKind::Resume {} | TerminatorKind::Abort {} | TerminatorKind::Unreachable {} => {
                props.has_panic_path = true
            }
            _ => {}
        }
    }

    props
}

/// Format detected properties as strings
pub fn format_properties(props: &FunctionProperties) -> Vec<&'static str> {
    let mut result = Vec::new();
    if props.has_panic_path {
        result.push("Contains panic path");
    }
    if props.has_checked_ops {
        result.push("Uses checked arithmetic");
    }
    if props.has_borrows {
        result.push("Introduces borrows");
    }
    if props.has_drops {
        result.push("Has explicit drops");
    }
    if props.has_recursion {
        result.push("Recursive");
    }
    if props.has_assertions {
        result.push("Contains assertions");
    }
    if props.has_switches {
        result.push("Has conditional branches");
    }
    result
}

/// Infer the role of each basic block
pub fn infer_block_roles(body: &Body) -> HashMap<usize, BlockRole> {
    let mut roles = HashMap::new();

    // Entry block is always bb0
    roles.insert(0, BlockRole::Entry);

    // Find cleanup targets
    let mut cleanup_blocks = HashSet::new();
    for block in &body.blocks {
        let unwind = match &block.terminator.kind {
            TerminatorKind::Drop { unwind, .. } => Some(unwind),
            TerminatorKind::Call { unwind, .. } => Some(unwind),
            TerminatorKind::Assert { unwind, .. } => Some(unwind),
            _ => None,
        };
        if let Some(UnwindAction::Cleanup(target)) = unwind {
            cleanup_blocks.insert(*target);
        }
    }

    // Detect loops (blocks that can reach themselves)
    let loop_blocks = detect_loops(body);

    for (idx, block) in body.blocks.iter().enumerate() {
        if roles.contains_key(&idx) {
            continue;
        }

        if cleanup_blocks.contains(&idx) {
            roles.insert(idx, BlockRole::Cleanup);
            continue;
        }

        if loop_blocks.contains(&idx) {
            roles.insert(idx, BlockRole::Loop);
            continue;
        }

        match &block.terminator.kind {
            TerminatorKind::Return {} => {
                roles.insert(idx, BlockRole::Return);
            }
            TerminatorKind::Resume {} | TerminatorKind::Abort {} | TerminatorKind::Unreachable {} => {
                roles.insert(idx, BlockRole::Panic);
            }
            TerminatorKind::Call { target: None, .. } => {
                roles.insert(idx, BlockRole::Panic);
            }
            TerminatorKind::Call { func, .. } => {
                let name = extract_call_name(func);
                if name.contains("panic") || name.contains("assert_failed") {
                    roles.insert(idx, BlockRole::Panic);
                }
            }
            TerminatorKind::SwitchInt { .. } => {
                roles.insert(idx, BlockRole::Branch);
            }
            _ => {}
        }
    }

    roles
}

/// Detect blocks that are part of loops
fn detect_loops(body: &Body) -> HashSet<usize> {
    let mut loop_blocks = HashSet::new();

    // Build successor map
    let successors: Vec<Vec<usize>> = body
        .blocks
        .iter()
        .map(|b| get_terminator_targets(&b.terminator))
        .collect();

    // For each block, check if it can reach itself
    for start in 0..body.blocks.len() {
        let mut visited = HashSet::new();
        let mut stack = successors[start].clone();

        while let Some(curr) = stack.pop() {
            if curr == start {
                loop_blocks.insert(start);
                break;
            }
            if visited.insert(curr) && curr < successors.len() {
                stack.extend(successors[curr].iter().copied());
            }
        }
    }

    loop_blocks
}

/// Get target block indices from a terminator
pub fn get_terminator_targets(term: &Terminator) -> Vec<usize> {
    match &term.kind {
        TerminatorKind::Goto { target } => vec![*target],
        TerminatorKind::SwitchInt { targets, .. } => {
            let mut result: Vec<usize> = targets.branches().map(|(_, t)| t).collect();
            result.push(targets.otherwise());
            result
        }
        TerminatorKind::Return {}
        | TerminatorKind::Resume {}
        | TerminatorKind::Abort {}
        | TerminatorKind::Unreachable {} => vec![],
        TerminatorKind::Drop { target, unwind, .. } => {
            let mut result = vec![*target];
            if let UnwindAction::Cleanup(t) = unwind {
                result.push(*t);
            }
            result
        }
        TerminatorKind::Call { target, unwind, .. } => {
            let mut result = vec![];
            if let Some(t) = target {
                result.push(*t);
            }
            if let UnwindAction::Cleanup(t) = unwind {
                result.push(*t);
            }
            result
        }
        TerminatorKind::Assert { target, unwind, .. } => {
            let mut result = vec![*target];
            if let UnwindAction::Cleanup(t) = unwind {
                result.push(*t);
            }
            result
        }
        TerminatorKind::InlineAsm {
            destination,
            unwind,
            ..
        } => {
            let mut result = vec![];
            if let Some(t) = destination {
                result.push(*t);
            }
            if let UnwindAction::Cleanup(t) = unwind {
                result.push(*t);
            }
            result
        }
    }
}

// =============================================================================
// Statement and Terminator Rendering
// =============================================================================

/// Render a statement with annotation
pub fn render_statement_annotated(stmt: &Statement) -> (String, String) {
    match &stmt.kind {
        StatementKind::Assign(place, rvalue) => {
            let mir = format!("{} = {}", render_place(place), render_rvalue(rvalue));
            let annotation = annotate_rvalue(rvalue);
            (mir, annotation)
        }
        StatementKind::SetDiscriminant {
            place,
            variant_index,
        } => (
            format!(
                "discr({}) = {}",
                render_place(place),
                variant_index.to_index()
            ),
            "Set enum discriminant".to_string(),
        ),
        StatementKind::StorageLive(local) => (
            format!("StorageLive(_{local})"),
            format!("Allocate stack slot for _{local}"),
        ),
        StatementKind::StorageDead(local) => (
            format!("StorageDead(_{local})"),
            format!("Deallocate stack slot for _{local}"),
        ),
        StatementKind::Nop => ("nop".to_string(), "No operation".to_string()),
        StatementKind::Retag(_, place) => (
            format!("retag({})", render_place(place)),
            "Stacked borrows retag".to_string(),
        ),
        StatementKind::FakeRead(_, place) => (
            format!("FakeRead({})", render_place(place)),
            "Compiler hint for borrow checker".to_string(),
        ),
        StatementKind::PlaceMention(place) => (
            format!("PlaceMention({})", render_place(place)),
            "Compiler hint for borrow checker".to_string(),
        ),
        _ => (format!("{:?}", stmt.kind), String::new()),
    }
}

/// Render a terminator with annotation
/// Returns (mir_string, annotation, is_recursive)
pub fn render_terminator_annotated(term: &Terminator, current_fn: &str) -> (String, String, bool) {
    match &term.kind {
        TerminatorKind::Goto { target } => (
            format!("goto bb{target}"),
            format!("Jump to bb{target}"),
            false,
        ),
        TerminatorKind::Return {} => ("return".to_string(), "Return from function".to_string(), false),
        TerminatorKind::Unreachable {} => (
            "unreachable".to_string(),
            "Unreachable code".to_string(),
            false,
        ),
        TerminatorKind::SwitchInt { discr, targets } => {
            let discr_str = render_operand(discr);
            let branches: Vec<String> = targets
                .branches()
                .map(|(val, bb)| format!("{val}→bb{bb}"))
                .collect();
            let otherwise = targets.otherwise();
            let mir = format!(
                "switch({}) [{}; else→bb{}]",
                discr_str,
                branches.join(", "),
                otherwise
            );
            let annotation = format!("Branch on {}", discr_str);
            (mir, annotation, false)
        }
        TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            ..
        } => {
            let func_name = extract_call_name(func);
            let args_str: Vec<String> = args.iter().map(|a| render_operand(&a.clone())).collect();
            let dest = render_place(destination);
            let target_str = target.map(|t| format!(" → bb{t}")).unwrap_or_default();
            let mir = format!(
                "{} = {}({}){}",
                dest,
                func_name,
                args_str.join(", "),
                target_str
            );

            let is_recursive = func_name == current_fn;
            let annotation = if is_recursive {
                format!("Recursive call to {}", func_name)
            } else {
                format!("Call {}", func_name)
            };
            (mir, annotation, is_recursive)
        }
        TerminatorKind::Assert {
            cond,
            expected,
            target,
            ..
        } => {
            let cond_str = render_operand(cond);
            let mir = format!("assert({} == {}) → bb{}", cond_str, expected, target);
            let annotation = if *expected {
                format!("Panic if {} is false", cond_str)
            } else {
                format!("Panic if {} is true", cond_str)
            };
            (mir, annotation, false)
        }
        TerminatorKind::Drop { place, target, .. } => {
            let place_str = render_place(place);
            let mir = format!("drop({}) → bb{}", place_str, target);
            let annotation = format!("Drop {}", place_str);
            (mir, annotation, false)
        }
        TerminatorKind::Resume {} => ("resume".to_string(), "Resume unwinding".to_string(), false),
        TerminatorKind::Abort {} => ("abort".to_string(), "Abort program".to_string(), false),
        _ => (format!("{:?}", term.kind), String::new(), false),
    }
}

/// Render a basic block as annotated rows
pub fn render_block_rows(block: &BasicBlock, current_fn: &str) -> Vec<AnnotatedRow> {
    let mut rows = Vec::new();

    // Process each statement
    for stmt in &block.statements {
        let (mir, annotation) = render_statement_annotated(stmt);
        rows.push(AnnotatedRow {
            mir,
            annotation,
            is_terminator: false,
            is_recursive: false,
        });
    }

    // Process terminator
    let (mir, annotation, is_recursive) = render_terminator_annotated(&block.terminator, current_fn);
    rows.push(AnnotatedRow {
        mir,
        annotation,
        is_terminator: true,
        is_recursive,
    });

    rows
}

// =============================================================================
// ASCII CFG Generation
// =============================================================================

/// Generate ASCII control-flow graph
pub fn generate_ascii_cfg(body: &Body, roles: &HashMap<usize, BlockRole>) -> String {
    let mut lines = Vec::new();

    for (idx, block) in body.blocks.iter().enumerate() {
        let role = roles.get(&idx).copied().unwrap_or(BlockRole::Normal);
        let role_suffix = role.cfg_suffix();

        let targets = get_terminator_targets(&block.terminator);
        if targets.is_empty() {
            lines.push(format!("bb{}{}", idx, role_suffix));
        } else {
            let arrows: Vec<String> = targets.iter().map(|t| format!("bb{}", t)).collect();
            lines.push(format!("bb{}{} ──▶ {}", idx, role_suffix, arrows.join(", ")));
        }
    }

    lines.join("\n") + "\n"
}

// =============================================================================
// Source Extraction
// =============================================================================

/// Extract the source code for a function from spans
pub fn extract_function_source(
    span_index: &HashMap<usize, &SpanInfo>,
    body: &Body,
) -> Option<String> {
    // Try to find the span covering the function body
    let first_span = if !body.blocks.is_empty() {
        let block = &body.blocks[0];
        if !block.statements.is_empty() {
            Some(block.statements[0].span.to_index())
        } else {
            Some(block.terminator.span.to_index())
        }
    } else {
        None
    };

    let info = first_span.and_then(|id| span_index.get(&id))?;
    let (file, _, _, _, _) = info;

    if file.contains(".rustup") || file.contains("no-location") {
        return None;
    }

    // Read the source file and extract relevant lines
    let content = std::fs::read_to_string(file).ok()?;

    // Find function boundaries by looking at all spans
    let mut min_line = usize::MAX;
    let mut max_line = 0usize;

    for block in &body.blocks {
        for stmt in &block.statements {
            if let Some(span_info) = span_index.get(&stmt.span.to_index()) {
                if span_info.0 == *file {
                    min_line = min_line.min(span_info.1);
                    max_line = max_line.max(span_info.3);
                }
            }
        }
        if let Some(span_info) = span_index.get(&block.terminator.span.to_index()) {
            if span_info.0 == *file {
                min_line = min_line.min(span_info.1);
                max_line = max_line.max(span_info.3);
            }
        }
    }

    if min_line == usize::MAX {
        return None;
    }

    // Expand to include function signature (look for fn keyword above)
    let lines: Vec<&str> = content.lines().collect();
    let mut start = min_line.saturating_sub(1);
    while start > 0 {
        let line = lines.get(start - 1).unwrap_or(&"");
        if line.trim().starts_with("fn ") || line.trim().starts_with("pub fn ") {
            start -= 1;
            break;
        }
        if line.trim().is_empty() || line.trim().starts_with("//") || line.trim().starts_with("#[") {
            start -= 1;
        } else {
            break;
        }
    }

    // Extract lines
    let end = max_line.min(lines.len());
    let source_lines: Vec<&str> = lines[start..end].to_vec();
    Some(source_lines.join("\n"))
}

// =============================================================================
// Traversal Framework
// =============================================================================

/// Context for function traversal
pub struct FunctionContext<'a> {
    pub short_name: &'a str,
    pub full_name: &'a str,
    pub body: &'a Body,
    pub properties: FunctionProperties,
    pub block_roles: HashMap<usize, BlockRole>,
    pub source: Option<String>,
}

impl<'a> FunctionContext<'a> {
    /// Create a new function context with all analysis pre-computed
    pub fn new(
        short_name: &'a str,
        full_name: &'a str,
        body: &'a Body,
        span_index: &HashMap<usize, &SpanInfo>,
    ) -> Self {
        let properties = analyze_function(body, short_name);
        let block_roles = infer_block_roles(body);
        let source = extract_function_source(span_index, body);

        Self {
            short_name,
            full_name,
            body,
            properties,
            block_roles,
            source,
        }
    }

    /// Get the role of a block
    pub fn block_role(&self, idx: usize) -> BlockRole {
        self.block_roles.get(&idx).copied().unwrap_or(BlockRole::Normal)
    }

    /// Render a block to annotated rows
    pub fn render_block(&self, idx: usize) -> Vec<AnnotatedRow> {
        render_block_rows(&self.body.blocks[idx], self.short_name)
    }

    /// Generate ASCII CFG
    pub fn ascii_cfg(&self) -> String {
        generate_ascii_cfg(self.body, &self.block_roles)
    }

    /// Get formatted property strings
    pub fn property_strings(&self) -> Vec<&'static str> {
        format_properties(&self.properties)
    }
}
