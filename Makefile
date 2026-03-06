.PHONY: integration-test help build test lint fmt clean check

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

integration-test: ## Run integration tests (requires Docker)
	docker compose -f docker-compose.test.yml up -d
	@echo "Waiting for Streamline server..."
	@for i in $$(seq 1 30); do \
		if curl -sf http://localhost:9094/health/live > /dev/null 2>&1; then \
			echo "Server ready"; \
			break; \
		fi; \
		sleep 2; \
	done
	wasm-pack test --headless --chrome || true
	docker compose -f docker-compose.test.yml down -v
