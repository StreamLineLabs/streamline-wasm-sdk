.PHONY: integration-test browser-test help build test lint fmt clean check

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

build: ## Build the WASM package
	bash scripts/build.sh

test: ## Run tests
	cargo test

browser-test: ## Run self-contained browser regression tests
	wasm-pack test --headless --chrome

lint: ## Run clippy (native + the actual wasm32 target)
	cargo clippy --all-targets -- -D warnings
	cargo clippy --all-targets --target wasm32-unknown-unknown -- -D warnings

fmt: ## Format code
	cargo fmt

fmt-check: ## Check formatting
	cargo fmt -- --check

clean: ## Clean build artifacts
	cargo clean
	rm -rf pkg pkg-bundler

check: fmt-check lint test ## Run all checks

integration-test: ## Run mandatory live-browser tests against an explicit fixture
	@set -eu; \
	started_local=0; \
	health_url="$${STREAMLINE_LIVE_HEALTH_URL:-}"; \
	websocket_url="$${STREAMLINE_LIVE_WEBSOCKET_URL:-}"; \
	if [ -n "$${STREAMLINE_FIXTURE_IMAGE:-}" ]; then \
		docker compose -f docker-compose.test.yml up -d; \
		started_local=1; \
		health_url="$${health_url:-http://localhost:9094/health}"; \
		websocket_url="$${websocket_url:-ws://localhost:9094/ws}"; \
	fi; \
	trap 'if [ "$$started_local" -eq 1 ]; then docker compose -f docker-compose.test.yml down -v; fi' EXIT; \
	if [ -z "$$health_url" ] || [ -z "$$websocket_url" ]; then \
		echo "Release blocker: set STREAMLINE_FIXTURE_IMAGE or both STREAMLINE_LIVE_HEALTH_URL and STREAMLINE_LIVE_WEBSOCKET_URL." >&2; \
		exit 1; \
	fi; \
	echo "Waiting for configured Streamline server..."; \
	ready=0; \
	for i in $$(seq 1 30); do \
		if curl -sf "$$health_url"; then \
			ready=1; \
			break; \
		fi; \
		echo "Waiting for Streamline... ($$i/30)"; \
		sleep 2; \
	done; \
	if [ "$$ready" -ne 1 ]; then \
		echo "Streamline fixture did not become healthy" >&2; \
		exit 1; \
	fi; \
	STREAMLINE_LIVE_WEBSOCKET_URL="$$websocket_url" \
		wasm-pack test --headless --chrome . \
		--features live-browser-tests --test live_browser
