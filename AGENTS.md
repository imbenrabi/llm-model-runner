# AGENTS.md

Project spec for coding agents working in this repository — read this before making any change.

## Project

`llm-runner` provides fully local, OpenAI-compatible LLM serving: llama.cpp's `llama-server` binary, supervised by the `llm-runner` Rust CLI, which owns process lifecycle (start/stop/status), configuration loading, and verification. The default model is Qwen2.5-Coder-7B-Instruct Q4_K_M (Apache-2.0, 4.68 GB) — see `docs/GLM-5.2-FEASIBILITY.md` for why the originally-targeted GLM-5.2 is infeasible on this hardware. A Kubernetes on-demand path (`k8s/`) serves the same OpenAI-compatible API for portability testing, at the cost of GPU acceleration (CPU-only inside the cluster).

## Hard constraints (never violate)

- **100% local inference.** No hosted or cloud inference, no API keys, and never a silent fallback to a cloud provider. Every response must come from the locally supervised `llama-server` process.
- **The native server binds loopback only.** `src/config.rs::validate_config` rejects any `host` in `config/runner.toml` that is not exactly `127.0.0.1` or `::1`. The `--egress` check in `src/verify.rs` exists specifically to prove this holds at runtime, via `lsof`.
- **Model weights are config, not code.** The model file path is never hardcoded in source: `config/runner.toml:model_path` for the native path, `k8s/configmap.yaml:LLAMA_ARG_MODEL` for the Kubernetes path.

## Success criteria

Four checks establish that a change has not broken local-only serving (see README.md "Verification" for exact commands):

1. `GET /v1/models` returns a non-empty `data` array.
2. `POST /v1/chat/completions` returns HTTP 200 with sensible, non-empty completion JSON.
3. Both of the above still succeed with every network interface disabled (Wi-Fi, and any wired Ethernet/Thunderbolt service).
4. `llm-runner verify --egress` reports zero non-loopback sockets held by the supervised `llama-server` process.

## Architecture

- `src/main.rs` — CLI argument parsing and subcommand dispatch.
- `src/config.rs` — TOML config loading (`Config`) and loopback-only host validation.
- `src/process.rs` — `llama-server` spawn, pidfile management, health polling, and the `start`/`stop`/`status` handlers.
- `src/verify.rs` — `/v1/models` and `/v1/chat/completions` checks, plus the `lsof`-based egress check.
- `src/error.rs` — `RunnerError`, the single error type shared across the crate.
- `scripts/` — `download-model.sh` (downloads the GGUF and verifies its sha256 against the HuggingFace tree API), `k8s-start.sh` / `k8s-stop.sh` (apply and scale the Kubernetes deployment).
- `k8s/` — `configmap.yaml`, `deployment.yaml`, `service.yaml`: a digest-pinned llama.cpp server image, a Service exposing NodePort 30080, and a Deployment whose base state is `replicas: 0` (scaled up on demand by `scripts/k8s-start.sh`).

## Canonical commands

Source of truth: the `Makefile`.

| Target | What it does |
| --- | --- |
| `help` | Lists available targets (default when running `make` with no arguments) |
| `build` | `cargo build` (debug binary) |
| `release` | `cargo build --release` (optimized binary) |
| `test` | `cargo test` |
| `lint` | `cargo clippy`, `cargo fmt --check`, and `shellcheck` on `scripts/*.sh` |
| `model` | Downloads the model weights via `scripts/download-model.sh` (no-op if already present) |
| `start` | Builds the release binary and model, then runs `./target/release/llm-runner start` |
| `stop` | Runs `./target/release/llm-runner stop` |
| `status` | Runs `./target/release/llm-runner status` |
| `verify` | Runs `./target/release/llm-runner verify --egress` |
| `k8s-start` | Runs `scripts/k8s-start.sh` |
| `k8s-stop` | Runs `scripts/k8s-stop.sh` |
| `clean` | `cargo clean` |

Keep this suite green before considering any change complete:

```bash
make release && make test && make lint
```

If any `k8s/*.yaml` manifest changes, additionally run:

```bash
kubectl apply --dry-run=client -f k8s/
```

## Coding standards

**Rust** — Rustdoc (`///`) on every public item: summary line, `# Arguments`, `# Returns`; no inline examples. Errors are returned as `Result<T, RunnerError>`; `.unwrap()`/`.expect()` are forbidden outside `#[cfg(test)]`. Functions are pure and single-responsibility, with complex logic broken into small private helpers. Dependencies are frozen to `serde`, `toml`, `serde_json`, `ureq` — adding a dependency requires explicit human approval.

**Shell** — Strict mode (`set -euo pipefail`) in every script. Functions are small and documented (`Arguments:` / `Outputs:`), with every function-scoped variable declared `local`. Scripts dispatch through `main "$@"`. `shellcheck` must pass with zero suppressions.

**Kubernetes** — Container images are pinned by digest, never a mutable tag. Keep ports and resource names consistent with the duplicated literals below.

## Cross-file invariants (drift risks — update ALL sites together)

- **Model filename** (`qwen2.5-coder-7b-instruct-q4_k_m.gguf`): `scripts/download-model.sh` (`MODEL_FILENAME`), `scripts/k8s-start.sh` (`MODEL_FILENAME`), `Makefile` (`MODEL`), `config/runner.toml` (`model_path`), `k8s/configmap.yaml` (`LLAMA_ARG_MODEL`), `README.md`, `src/config.rs` (`parses_committed_config` test fixture).
- **Resource name** `llm-runner`: `k8s/configmap.yaml`, `k8s/deployment.yaml`, `k8s/service.yaml` (metadata names and `app: llm-runner` labels/selectors), `scripts/k8s-start.sh` and `scripts/k8s-stop.sh` (`DEPLOYMENT_NAME`).
- **Ports** `8080` (native bind and in-cluster container port) / `30080` (NodePort): `config/runner.toml`, `k8s/configmap.yaml`, `k8s/deployment.yaml`, `k8s/service.yaml`, `README.md`.
