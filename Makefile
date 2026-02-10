.PHONY: help build test lint fmt clean check

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

build: ## Build the WASM package
	bash scripts/build.sh

test: ## Run tests
	cargo test

lint: ## Run clippy
	cargo clippy --all-targets -- -D warnings

fmt: ## Format code
	cargo fmt

fmt-check: ## Check formatting
	cargo fmt -- --check

clean: ## Clean build artifacts
	cargo clean
	rm -rf pkg pkg-bundler

check: fmt-check lint test ## Run all checks
