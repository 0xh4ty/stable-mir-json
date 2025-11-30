# stable-mir-json justfile

# Use nightly toolchain specified in rust-toolchain.toml
export RUSTUP_TOOLCHAIN := ""

# Default recipe
default: build

# Build the project
build:
    cargo build

# Build release
release:
    cargo build --release

# Run tests
test:
    make integration-test

# Format code
fmt:
    cargo fmt

# Lint
lint:
    cargo clippy

# Clean build artifacts
clean:
    cargo clean
    rm -rf output-html output-dot output-d2

# Test programs directory
test_dir := "tests/integration/programs"

# Generate HTML output for all test programs
html:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p output-html
    for rust in {{test_dir}}/*.rs; do
        name=$(basename "${rust%.rs}")
        echo "Generating HTML for $name..."
        cargo run -- --html -Zno-codegen --out-dir output-html "$rust" 2>/dev/null || true
        # Move the generated file to have a cleaner name
        if [ -f "output-html/${name}.smir.html" ]; then
            echo "  -> output-html/${name}.smir.html"
        fi
    done
    echo "Done. HTML files in output-html/"

# Generate HTML for a single file
html-file file:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p output-html
    name=$(basename "{{file}}" .rs)
    cargo run -- --html -Zno-codegen --out-dir output-html "{{file}}"
    echo "Generated: output-html/${name}.smir.html"

# Generate DOT output for all test programs
dot:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p output-dot
    for rust in {{test_dir}}/*.rs; do
        name=$(basename "${rust%.rs}")
        echo "Generating DOT for $name..."
        cargo run -- --dot -Zno-codegen --out-dir output-dot "$rust" 2>/dev/null || true
    done
    echo "Done. DOT files in output-dot/"

# Generate D2 output for all test programs
d2:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p output-d2
    for rust in {{test_dir}}/*.rs; do
        name=$(basename "${rust%.rs}")
        echo "Generating D2 for $name..."
        cargo run -- --d2 -Zno-codegen --out-dir output-d2 "$rust" 2>/dev/null || true
    done
    echo "Done. D2 files in output-d2/"

# Generate all output formats
all: html dot d2

# List available test programs
list-tests:
    @ls -1 {{test_dir}}/*.rs | xargs -n1 basename | sed 's/\.rs$//'
