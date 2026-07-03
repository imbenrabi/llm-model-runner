MODEL := models/qwen2.5-coder-7b-instruct-q4_k_m.gguf

.PHONY: help build release test lint model start stop status verify k8s-start k8s-stop clean

help: ## Show this help message
	@grep -E '^[a-zA-Z0-9_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

build: ## Build the debug binary
	cargo build

release: ## Build the optimized release binary
	cargo build --release

test: ## Run the test suite
	cargo test

lint: ## Run clippy, rustfmt check, and shellcheck
	cargo clippy --all-targets -- -D warnings
	cargo fmt --check
	shellcheck scripts/*.sh

$(MODEL):
	scripts/download-model.sh

model: $(MODEL) ## Download the model weights (no-op if already present)

start: release $(MODEL) ## Build the release binary and start the supervised server
	./target/release/llm-runner start

stop: ## Stop the supervised server
	./target/release/llm-runner stop

status: ## Report the health of the supervised server
	./target/release/llm-runner status

verify: ## Verify local serving and zero network egress
	./target/release/llm-runner verify --egress

k8s-start: ## Start the on-demand Kubernetes deployment
	scripts/k8s-start.sh

k8s-stop: ## Stop the Kubernetes deployment
	scripts/k8s-stop.sh

clean: ## Remove build artifacts
	cargo clean
