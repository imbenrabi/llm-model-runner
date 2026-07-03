# llm-runner

## What this is

Fully local, OpenAI-compatible LLM serving: llama.cpp's `llama-server`,
supervised by the `llm-runner` Rust CLI.

The default model is Qwen2.5-Coder-7B-Instruct Q4_K_M (Apache-2.0, 4.68 GB).

GLM-5.2 was the original target model for this project. It cannot run on this
hardware — see [docs/GLM-5.2-FEASIBILITY.md](docs/GLM-5.2-FEASIBILITY.md) for
why.

## Prerequisites

- macOS with Homebrew.
- `llama-server` and `cargo`:

```bash
brew install llama.cpp rust
```

- For the Kubernetes path only: a local Kubernetes cluster (tested with
  Rancher Desktop).

## Quickstart (native, Metal-accelerated)

This is the fast/performance path. Run all commands from the repository root.

Download the model:

```bash
make model  # scripts/download-model.sh
```

Build the CLI:

```bash
make release  # cargo build --release
```

Start the server:

```bash
make start  # ./target/release/llm-runner start
```

List available models:

```bash
curl http://127.0.0.1:8080/v1/models
```

Send a chat completion (use the `id` from the `/v1/models` response above as
`model`):

```bash
curl http://127.0.0.1:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
        "model": "<model-id-from-above>",
        "messages": [{"role": "user", "content": "Say hello."}]
      }'
```

Check status:

```bash
make status  # ./target/release/llm-runner status
```

Verify the setup, including zero-egress:

```bash
make verify  # ./target/release/llm-runner verify --egress
```

Stop the server:

```bash
make stop  # ./target/release/llm-runner stop
```

## Make targets

A `Makefile` at the repository root wraps the commands above:

| Target | What it does |
| --- | --- |
| `help` | Lists available targets (default when running `make` with no arguments) |
| `build` | `cargo build` (debug binary) |
| `release` | `cargo build --release` (optimized binary) |
| `test` | `cargo test` |
| `lint` | `cargo clippy`, `cargo fmt --check`, and `shellcheck` on `scripts/*.sh` |
| `model` | Downloads the model weights via `scripts/download-model.sh` (no-op if the file already exists) |
| `start` | Builds the release binary and model, then runs `./target/release/llm-runner start` |
| `stop` | Runs `./target/release/llm-runner stop` |
| `status` | Runs `./target/release/llm-runner status` |
| `verify` | Runs `./target/release/llm-runner verify --egress` |
| `k8s-start` | Runs `scripts/k8s-start.sh` |
| `k8s-stop` | Runs `scripts/k8s-stop.sh` |
| `clean` | `cargo clean` |

## Configuration

Settings live in `config/runner.toml`:

| Key | Meaning | Default |
| --- | --- | --- |
| model_path | path to the GGUF model file, passed to `llama-server --model` | `models/qwen2.5-coder-7b-instruct-q4_k_m.gguf` |
| host | bind address, passed to `llama-server --host` | `127.0.0.1` |
| port | bind port, passed to `llama-server --port` | `8080` |
| ctx_size | context window size in tokens, passed to `llama-server --ctx-size` | `8192` |
| n_gpu_layers | number of layers offloaded to GPU/Metal, passed to `llama-server --n-gpu-layers` | `99` (effectively all layers) |

The server binds `127.0.0.1` only (loopback) — nothing leaves the machine.

Model swapping is done by editing `model_path`. On a machine with at least
256 GB of unified memory, this same runner serves a GLM-5.2 GGUF unmodified —
download it and change that one line.

## Kubernetes on-demand path

Start:

```bash
scripts/k8s-start.sh
```

Stop:

```bash
scripts/k8s-stop.sh
```

The endpoint is `http://localhost:30080/v1`. Set `MODELS_DIR` to override the
default `$(pwd)/models`:

```bash
MODELS_DIR=/path/to/models scripts/k8s-start.sh
```

Requirements: the model must already be downloaded, the cluster must already
be running, and the cluster needs roughly 8 GB of memory available.

On macOS, containers run inside a Linux VM, so this path is CPU-only
inference (no Metal) and is much slower than the native path. The native
Quickstart above is the fast path; Kubernetes is for on-demand and
portability semantics, not performance.

## Verification

Four checks establish that this setup is 100% local:

1. `/v1/models` lists the model. Curl it and confirm the response's `data`
   array is non-empty.
2. A chat completion returns HTTP 200 with sensible JSON. Curl the POST from
   Quickstart. `llm-runner verify` (no flag) automates checks 1 and 2 in a
   single command.
3. Offline test: turn Wi-Fi off, repeat the chat completion request, confirm
   it still succeeds, then restore Wi-Fi.

```bash
networksetup -listallhardwareports
networksetup -setairportpower <device> off
# repeat the chat completion request here
networksetup -setairportpower <device> on
```

4. Zero-egress: run `llm-runner verify --egress`, which uses an `lsof`-based
   check that the supervised process has no non-loopback network
   connections.

```bash
./target/release/llm-runner verify --egress
```

## Repository layout

```
.
├── Cargo.toml
├── Cargo.lock
├── Makefile
├── config/
│   └── runner.toml
├── docs/
│   └── GLM-5.2-FEASIBILITY.md
├── k8s/
│   ├── configmap.yaml
│   ├── deployment.yaml
│   └── service.yaml
├── scripts/
│   ├── download-model.sh
│   ├── k8s-start.sh
│   └── k8s-stop.sh
├── src/
│   ├── main.rs
│   ├── config.rs
│   ├── error.rs
│   ├── process.rs
│   └── verify.rs
└── README.md
```
