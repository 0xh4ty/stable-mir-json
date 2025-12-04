.PHONY: build dev release serve clean check

# Development build
dev:
	wasm-pack build --dev --target web --out-dir www/pkg

# Release build
release:
	wasm-pack build --release --target web --out-dir www/pkg

# Alias for dev build
build: dev

# Check compilation without building WASM
check:
	cargo check --target wasm32-unknown-unknown

# Serve the www directory for testing
serve: dev
	@echo "Serving at http://localhost:8080"
	python3 -m http.server 8080 -d www

# Clean build artifacts
clean:
	cargo clean
	rm -rf www/pkg
