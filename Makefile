.PHONY: all build test install clean format lint help

# Default target
all: build

# Build Slate-DE
build:
	@echo "Building Slate-DE..."
	cargo build --release --package slate-shell --package slate-command --package cli
	@echo "Build complete: target/release/slate (shell), target/release/cli (launcher)"

# Run tests
test:
	@echo "Running tests..."
	cargo test --workspace

# Install to system (requires sudo for some operations)
install: build
	@echo "Installing Slate-DE..."
	mkdir -p $(HOME)/.local/bin
	cp target/release/slate $(HOME)/.local/bin/slate 2>/dev/null || cp target/release/cli $(HOME)/.local/bin/slate-de
	chmod +x $(HOME)/.local/bin/slate $(HOME)/.local/bin/slate-de 2>/dev/null || true
	@echo "Installed to $(HOME)/.local/bin/ (slate / slate-de)"

# Quick install using the install script
quick-install:
	@echo "Running quick install script..."
	bash scripts/install.sh

# Clean build artifacts
clean:
	@echo "Cleaning..."
	cargo clean
	rm -rf target/

# Format code
format:
	@echo "Formatting code..."
	cargo fmt --all

# Lint code
lint:
	@echo "Linting..."
	cargo clippy --all-targets --all-features -- -D warnings

# Run Slate-DE (for development)
run:
	cargo run --release --package slate-shell --bin slate || cargo run --release --package cli

# Check (format + lint + test)
check: format lint test
	@echo "All checks passed!"

# Build documentation
docs:
	@echo "Building documentation..."
	cargo doc --no-deps --open

# Help
help:
	@echo "Slate-DE Makefile"
	@echo ""
	@echo "Targets:"
	@echo "  build         - Build the project (release mode)"
	@echo "  test          - Run all tests"
	@echo "  install       - Install binary to ~/.local/bin"
	@echo "  quick-install - Run the full install script"
	@echo "  clean         - Remove build artifacts"
	@echo "  format        - Format code with rustfmt"
	@echo "  lint          - Lint code with clippy"
	@echo "  run           - Run Slate-DE"
	@echo "  check         - Run format, lint, and test"
	@echo "  docs          - Build and open documentation"
	@echo "  help          - Show this help message"
