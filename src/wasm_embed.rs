//! Embedded WASM Explorer
//!
//! Generates a self-contained HTML file with the WASM-based MIR explorer
//! and pre-loaded data. All assets (WASM binary, JS glue, CSS) are embedded
//! inline for a single-file distribution.

use std::fs::File;
use std::io::{self, BufWriter, Write};

extern crate rustc_middle;
use rustc_middle::ty::TyCtxt;

extern crate rustc_session;
use rustc_session::config::{OutFileName, OutputType};

extern crate serde_json;

use crate::explore::build_explorer_data;
use crate::printer::collect_smir;

// Embedded WASM assets - these are included at compile time
// Build with `make wasm-release` first to generate these files
const WASM_JS: &str = include_str!("../mir-explorer/www/pkg/mir_explorer.js");
const WASM_BINARY: &[u8] = include_bytes!("../mir-explorer/www/pkg/mir_explorer_bg.wasm");

/// Entry point to generate the embedded WASM explorer HTML file
pub fn emit_wasm_explore(tcx: TyCtxt<'_>) {
    let smir = collect_smir(tcx);
    let data = build_explorer_data(&smir);
    let json_data = serde_json::to_string(&data).expect("Failed to serialize explorer data");

    let html = generate_embedded_html(&smir.name, &json_data);

    match tcx.output_filenames(()).path(OutputType::Mir) {
        OutFileName::Stdout => {
            write!(io::stdout(), "{}", html).expect("Failed to write HTML");
        }
        OutFileName::Real(path) => {
            let out_path = path.with_extension("wasm-explore.html");
            let mut b = BufWriter::new(
                File::create(&out_path)
                    .unwrap_or_else(|e| panic!("Failed to create {}: {}", out_path.display(), e)),
            );
            write!(b, "{}", html).expect("Failed to write wasm-explore.html");
            eprintln!("Wrote {}", out_path.display());
        }
    }
}

fn generate_embedded_html(crate_name: &str, json_data: &str) -> String {
    use base64::Engine;
    let wasm_base64 = base64::engine::general_purpose::STANDARD.encode(WASM_BINARY);

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{crate_name} - MIR Explorer</title>
    <style>
{css}
    </style>
</head>
<body>
    <header class="header">
        <h1 id="crate-name">{crate_name}</h1>
        <select id="function-select">
            <option>Loading...</option>
        </select>
    </header>

    <main class="main">
        <div class="graph-area">
            <canvas id="graph-canvas"></canvas>
            <div class="path-bar" id="path-bar">
                <span class="label">PATH:</span>
                <span class="breadcrumb" id="breadcrumb"></span>
                <div class="controls">
                    <button id="reset-btn">Reset</button>
                    <button id="back-btn">&larr; Back</button>
                    <button id="fit-btn">Fit</button>
                </div>
            </div>
        </div>

        <aside class="context-panel" id="context-panel">
            <h2 id="block-id">bb0</h2>
            <span class="badge" id="block-role">entry</span>
            <div class="summary" id="block-summary"></div>

            <div class="section-header">Locals</div>
            <ul class="locals-list" id="locals-list"></ul>

            <div class="section-header">Statements</div>
            <ul class="statements-list" id="statements-list"></ul>

            <div class="section-header">Terminator</div>
            <div class="terminator" id="terminator"></div>

            <div class="section-header">Next</div>
            <div class="edges-list" id="edges-list"></div>
        </aside>
    </main>

    <!-- Embedded WASM module (modified for inline loading) -->
    <script type="module">
{wasm_js_modified}

// Embedded explorer data
const EXPLORER_DATA = {json_data};

// Decode base64 WASM
const wasmBase64 = "{wasm_base64}";
const wasmBytes = Uint8Array.from(atob(wasmBase64), c => c.charCodeAt(0));

let explorer = null;
let isDragging = false;
let lastMouseX = 0;
let lastMouseY = 0;

async function main() {{
    // Initialize WASM with inline bytes
    await __wbg_init(wasmBytes.buffer);

    const canvas = document.getElementById('graph-canvas');
    explorer = create_explorer('graph-canvas', 'context-panel');

    // Load embedded data
    const jsonStr = JSON.stringify(EXPLORER_DATA);
    explorer.load_json(jsonStr);

    // Update crate name
    document.getElementById('crate-name').textContent = EXPLORER_DATA.name;
    document.title = `${{EXPLORER_DATA.name}} - MIR Explorer`;

    // Populate function selector
    const select = document.getElementById('function-select');
    select.innerHTML = '';
    const count = explorer.function_count();
    for (let i = 0; i < count; i++) {{
        const option = document.createElement('option');
        option.value = i;
        option.textContent = explorer.function_name(i);
        select.appendChild(option);
    }}

    select.addEventListener('change', (e) => {{
        explorer.select_function(parseInt(e.target.value, 10));
        updateContextPanel();
    }});

    // Control buttons
    document.getElementById('reset-btn').addEventListener('click', () => {{
        explorer.reset();
        updateContextPanel();
    }});

    document.getElementById('back-btn').addEventListener('click', () => {{
        explorer.go_back();
        updateContextPanel();
    }});

    document.getElementById('fit-btn').addEventListener('click', () => {{
        explorer.fit_to_view();
    }});

    // Keyboard handling
    document.addEventListener('keydown', (e) => {{
        if (e.target.tagName === 'SELECT') return;
        if (e.key === '/') {{
            e.preventDefault();
            select.focus();
            return;
        }}
        if (explorer.handle_key(e.key)) {{
            e.preventDefault();
            updateContextPanel();
        }}
    }});

    // Mouse handling
    canvas.addEventListener('wheel', (e) => {{
        e.preventDefault();
        const rect = canvas.getBoundingClientRect();
        explorer.handle_wheel(e.deltaY, e.clientX - rect.left, e.clientY - rect.top);
    }}, {{ passive: false }});

    canvas.addEventListener('mousedown', (e) => {{
        isDragging = true;
        lastMouseX = e.clientX;
        lastMouseY = e.clientY;
        canvas.style.cursor = 'grabbing';
    }});

    document.addEventListener('mousemove', (e) => {{
        if (isDragging) {{
            explorer.handle_drag(e.clientX - lastMouseX, e.clientY - lastMouseY);
            lastMouseX = e.clientX;
            lastMouseY = e.clientY;
        }}
    }});

    document.addEventListener('mouseup', () => {{
        isDragging = false;
        canvas.style.cursor = 'grab';
    }});

    canvas.style.cursor = 'grab';
    window.addEventListener('resize', () => explorer.render());

    explorer.fit_to_view();
    updateContextPanel();
}}

function updateContextPanel() {{
    const infoJson = explorer.get_block_info_json();
    if (!infoJson) return;
    const info = JSON.parse(infoJson);

    document.getElementById('block-id').textContent = `bb${{info.id}}`;
    const badge = document.getElementById('block-role');
    badge.textContent = info.role;
    badge.className = `badge ${{info.role}}`;
    document.getElementById('block-summary').textContent = info.summary;

    const stmtsList = document.getElementById('statements-list');
    stmtsList.innerHTML = info.statements.length === 0
        ? '<li style="color: var(--text-dim)">(none)</li>'
        : info.statements.map(s =>
            `<li><span class="mir">${{escapeHtml(s.mir)}}</span>${{s.annotation ? `<span class="annotation">${{escapeHtml(s.annotation)}}</span>` : ''}}</li>`
        ).join('');

    document.getElementById('terminator').innerHTML =
        `<span class="mir">${{escapeHtml(info.terminator.mir)}}</span>${{info.terminator.annotation ? `<span class="annotation">${{escapeHtml(info.terminator.annotation)}}</span>` : ''}}`;

    const edgesList = document.getElementById('edges-list');
    edgesList.innerHTML = info.terminator.edges.map((e, i) => {{
        const selectedClass = i === info.selected_edge ? ' selected' : '';
        const cleanupClass = e.kind === 'cleanup' ? ' cleanup' : '';
        const keyHint = i < 9 ? `<span class="key-hint">[${{i + 1}}]</span>` : '';
        return `<button class="edge-btn${{selectedClass}}${{cleanupClass}}" data-index="${{i}}">
            ${{keyHint}}<span class="target">&rarr; bb${{e.target}}</span>
            ${{e.label ? `<span class="label">${{escapeHtml(e.label)}}</span>` : ''}}
            ${{e.annotation ? `<span class="hint">${{escapeHtml(e.annotation)}}</span>` : ''}}
        </button>`;
    }}).join('');

    edgesList.querySelectorAll('.edge-btn').forEach(btn => {{
        btn.addEventListener('click', () => {{
            explorer.follow_edge(parseInt(btn.dataset.index, 10));
            updateContextPanel();
        }});
    }});

    const crumb = document.getElementById('breadcrumb');
    const fullPath = [...info.path, info.id];
    crumb.innerHTML = fullPath.map((b, i) =>
        `<span class="crumb${{i === fullPath.length - 1 ? ' current' : ''}}">bb${{b}}</span>`
    ).join(' &rarr; ');

    const localsJson = explorer.get_locals_json();
    if (localsJson) {{
        const locals = JSON.parse(localsJson);
        document.getElementById('locals-list').innerHTML = locals.map(l => {{
            const sourceName = l.source_name ? ` <span class="source-name">(${{escapeHtml(l.source_name)}})</span>` : '';
            const assigns = l.assignments && l.assignments.length > 0
                ? l.assignments.map(a => `bb${{a.block_id}}: ${{escapeHtml(a.value)}}`).join(', ')
                : '(arg/ret)';
            return `<li><span class="name">${{escapeHtml(l.name)}}</span>: <span class="type">${{escapeHtml(l.ty)}}</span>${{sourceName}}<br><span class="assignments">${{assigns}}</span></li>`;
        }}).join('');
    }}
}}

function escapeHtml(s) {{
    if (!s) return '';
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}}

main();
    </script>
</body>
</html>"##,
        crate_name = crate_name,
        css = EMBEDDED_CSS,
        wasm_js_modified = modify_wasm_js(WASM_JS),
        json_data = json_data,
        wasm_base64 = wasm_base64,
    )
}

/// Modify the wasm-bindgen generated JS to work with inline WASM loading
fn modify_wasm_js(js: &str) -> String {
    // The wasm-bindgen output has an init function that we need to expose
    // We rename it to __wbg_init and export create_explorer
    js.replace("export default __wbg_init;", "// init exposed as __wbg_init")
      .replace("export { initSync }", "// initSync removed for embedding")
}

const EMBEDDED_CSS: &str = r##":root {
    --bg: #1a1a2e;
    --bg-panel: #16213e;
    --bg-block: #0f0f1a;
    --text: #eee;
    --text-dim: #888;
    --accent: #8be9fd;
    --green: #50fa7b;
    --purple: #bd93f9;
    --pink: #ff79c6;
    --orange: #ffb86c;
    --border: #333;
}

* { box-sizing: border-box; margin: 0; padding: 0; }

body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: var(--bg);
    color: var(--text);
    height: 100vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
}

.header {
    background: var(--bg-panel);
    padding: 0.75rem 1rem;
    display: flex;
    align-items: center;
    gap: 1rem;
    border-bottom: 1px solid var(--border);
}

.header h1 {
    font-size: 1.1rem;
    color: var(--accent);
    font-weight: 600;
}

.header select {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    padding: 0.4rem 0.8rem;
    border-radius: 4px;
    font-size: 0.9rem;
    min-width: 200px;
}

.main {
    flex: 1;
    display: flex;
    overflow: hidden;
}

.graph-area {
    flex: 1;
    position: relative;
    background: var(--bg);
}

#graph-canvas {
    width: 100%;
    height: 100%;
    display: block;
}

.path-bar {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    background: rgba(22, 33, 62, 0.9);
    padding: 0.5rem 1rem;
    font-family: monospace;
    font-size: 0.85rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    border-bottom: 1px solid var(--border);
}

.path-bar .label { color: var(--text-dim); }
.path-bar .crumb { color: var(--text-dim); }
.path-bar .crumb.current { color: var(--pink); font-weight: bold; }

.path-bar .controls {
    margin-left: auto;
    display: flex;
    gap: 0.5rem;
}

.path-bar button {
    background: var(--bg);
    border: 1px solid var(--border);
    color: var(--text);
    padding: 0.25rem 0.6rem;
    border-radius: 3px;
    cursor: pointer;
    font-size: 0.8rem;
}

.path-bar button:hover { border-color: var(--accent); }

.context-panel {
    width: 320px;
    background: var(--bg-panel);
    border-left: 1px solid var(--border);
    overflow-y: auto;
    padding: 1rem;
}

.context-panel h2 {
    color: var(--accent);
    font-size: 1.1rem;
    font-family: monospace;
    margin-bottom: 0.5rem;
}

.badge {
    display: inline-block;
    background: var(--bg);
    color: var(--text-dim);
    padding: 0.15rem 0.5rem;
    border-radius: 3px;
    font-size: 0.7rem;
    text-transform: uppercase;
    margin-bottom: 0.75rem;
}

.badge.entry { background: var(--green); color: var(--bg); }
.badge.exit { background: var(--purple); color: var(--bg); }
.badge.branchpoint { background: var(--orange); color: var(--bg); }
.badge.mergepoint { background: var(--accent); color: var(--bg); }
.badge.cleanup { background: #ff5555; color: var(--bg); }

.summary { color: var(--text-dim); margin-bottom: 1rem; font-size: 0.9rem; }

.section-header {
    color: var(--text-dim);
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin: 1rem 0 0.5rem 0;
    border-bottom: 1px solid var(--border);
    padding-bottom: 0.25rem;
}

.statements-list, .locals-list {
    list-style: none;
    font-family: monospace;
    font-size: 0.8rem;
}

.statements-list li {
    padding: 0.3rem 0;
    border-bottom: 1px solid rgba(255,255,255,0.05);
}

.statements-list .mir { color: var(--green); }
.statements-list .annotation { color: var(--purple); font-size: 0.75rem; display: block; }

.terminator { font-family: monospace; font-size: 0.85rem; }
.terminator .mir { color: var(--pink); }
.terminator .annotation { color: var(--purple); font-size: 0.75rem; display: block; }

.edges-list {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    margin-top: 0.5rem;
}

.edge-btn {
    background: var(--bg);
    border: 1px solid var(--border);
    color: var(--text);
    padding: 0.5rem;
    border-radius: 4px;
    cursor: pointer;
    text-align: left;
    font-size: 0.8rem;
    font-family: monospace;
}

.edge-btn:hover { border-color: var(--accent); }
.edge-btn.selected { border-color: var(--accent); background: rgba(139, 233, 253, 0.1); }
.edge-btn.cleanup { border-color: #ff5555; border-style: dashed; }
.edge-btn .target { color: var(--green); }
.edge-btn .label { color: var(--orange); margin-left: 0.5rem; }
.edge-btn .hint { color: var(--text-dim); font-size: 0.7rem; display: block; margin-top: 0.25rem; }
.edge-btn .key-hint { float: right; color: var(--text-dim); font-size: 0.7rem; }

.locals-list {
    max-height: 200px;
    overflow-y: auto;
    font-size: 0.75rem;
    color: var(--text-dim);
}

.locals-list li {
    padding: 0.3rem 0;
    border-bottom: 1px solid rgba(255,255,255,0.05);
}

.locals-list .name { color: var(--accent); }
.locals-list .type { color: var(--green); }
.locals-list .source-name { color: var(--purple); font-size: 0.7rem; }
.locals-list .assignments { color: var(--text-dim); font-size: 0.7rem; margin-left: 1em; }
"##;
