# llm-runner

Fully local, OpenAI-compatible LLM serving: llama.cpp's `llama-server`, supervised by a small Rust CLI.

Read [AGENTS.md](AGENTS.md) for the intent layer that maps non-obvious architectural knowledge.

## Hard constraints

- **100% local inference.** Responses must come only from the supervised local `llama-server` process. No cloud inference, no API keys, no silent fallback.
- **Native server binds loopback only.** `config/runner.toml:host` must be `127.0.0.1` or `::1`, enforced by `src/config.rs` and proved at runtime by `llm-runner verify --egress`.
- **Model weights are config, not code.** The GGUF path lives in `config/runner.toml:model_path` for the native path and `k8s/configmap.yaml:LLAMA_ARG_MODEL` for the Kubernetes path.

## Architecture

| File | Responsibility |
|------|----------------|
| `src/main.rs` | CLI argument parsing and subcommand dispatch. |
| `src/config.rs` | TOML config loading, loopback host validation, and `llama-server` flag translation. |
| `src/process.rs` | `llama-server` spawn, pidfile management, health polling, start/stop/status. |
| `src/verify.rs` | `/v1/models` and `/v1/chat/completions` checks, plus `lsof`-based egress verification. |
| `src/error.rs` | Single `RunnerError` enum used across the crate. |
| `scripts/download-model.sh` | Downloads the GGUF and verifies its sha256 against the HuggingFace tree API. |
| `scripts/k8s-start.sh` / `scripts/k8s-stop.sh` | Scale the Kubernetes deployment up and down on demand. |
| `k8s/*.yaml` | Digest-pinned llama.cpp server manifests; base state is `replicas: 0`. |

## Cross-file invariants

Changing any of these requires updating every site that references them:

- **Model filename** `qwen2.5-coder-7b-instruct-q4_k_m.gguf`: `Makefile`, `config/runner.toml`, `scripts/download-model.sh`, `scripts/k8s-start.sh`, `k8s/configmap.yaml`, `src/config.rs` test fixture, `README.md`.
- **Resource name** `llm-runner`: `k8s/configmap.yaml`, `k8s/deployment.yaml`, `k8s/service.yaml`, `scripts/k8s-start.sh`, `scripts/k8s-stop.sh`.
- **Ports** `8080` (native / container) and `30080` (NodePort): `config/runner.toml`, `k8s/configmap.yaml`, `k8s/deployment.yaml`, `k8s/service.yaml`, `README.md`.

## Canonical commands

Source of truth: `Makefile`.

| Target | Command |
|--------|---------|
| `make release` | `cargo build --release` |
| `make test` | `cargo test` |
| `make lint` | `cargo clippy`, `cargo fmt --check`, `shellcheck scripts/*.sh` |
| `make model` | Download model weights if absent |
| `make start` | Build release binary and start supervised server |
| `make stop` | Stop the supervised server |
| `make status` | Report server health |
| `make verify` | Run endpoint and zero-egress checks |
| `make k8s-start` / `make k8s-stop` | Scale the Kubernetes deployment |

Run the green suite before considering any change complete:

```bash
make release && make test && make lint
```

If any `k8s/*.yaml` changes, also run:

```bash
kubectl apply --dry-run=client -f k8s/
```

## Intent layer

- [`AGENTS.md`](AGENTS.md) — intent layer principles and navigation.
- [`src/AGENTS.md`](src/AGENTS.md) — Rust CLI invariants.
- [`src/FLOW.md`](src/FLOW.md) — server start/stop control flow.
- [`scripts/AGENTS.md`](scripts/AGENTS.md) — shell automation invariants.
- [`k8s/AGENTS.md`](k8s/AGENTS.md) — Kubernetes path invariants.
