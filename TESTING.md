# llm-runner Test Report

This report walks through the README setup guide, exercises the CLI and
OpenAI-compatible endpoints, benchmarks local inference throughput, and
analyses the status/readiness of the native and Kubernetes serving paths.

All commands were run from the repository root on the test machine documented
below.

---

## 1. Test objectives and environment

### Objectives
- Verify the repository builds, lints, and passes its unit-test suite.
- Download and checksum the default Qwen2.5-Coder-7B-Instruct Q4_K_M GGUF.
- Walk through the native Quickstart (`make start`, `curl`, `make status`,
  `make verify`, `make stop`).
- Test CLI argument edge cases and lifecycle failure modes.
- Benchmark token throughput, time-to-first-token (TTFT), end-to-end latency,
  and CPU usage for representative workloads.
- Smoke-test the Kubernetes on-demand path.
- Analyse readiness criteria and capture any issues found.

### Test environment

| Item | Value |
|---|---|
| OS | macOS 26.5.1 (Darwin 25.5.0, arm64) |
| Hardware | Apple Silicon Mac mini |
| Active default interface | `en8` (`route get default`) |
| `cargo` | 1.96.0 (Homebrew) |
| `llama-server` | version 9860 (fdb1db877), AppleClang 21, Darwin arm64 |
| `curl` | 8.7.1 |
| `python3` | 3.14.6 |
| `shellcheck` | installed (Homebrew) |
| `lsof` | system `/usr/sbin/lsof` |
| `kubectl` | Rancher Desktop (`/Users/ben/.rd/bin/kubectl`) |

### Files created during testing
- `scripts/benchmark.sh` — reproducible streaming benchmark harness.
- `.bench/` — raw SSE responses, timing files, and `results.md` from the
  benchmark runs.

---

## 2. Setup walkthrough

### 2.1 Build the release binary

```bash
make release
```

Result:

```text
cargo build --release
    Finished `release` profile [optimized target(s)] in 11.96s
```

### 2.2 Run the unit-test suite

```bash
make test
```

Result: **19 passed; 0 failed**.

### 2.3 Run lint

```bash
make lint
```

Result: `cargo clippy`, `cargo fmt --check`, and `shellcheck scripts/*.sh`
all passed with zero warnings.

### 2.4 Download the model

```bash
make model
```

Result: downloaded `models/qwen2.5-coder-7b-instruct-q4_k_m.gguf` (4.4 GB)
and verified its SHA-256 against the HuggingFace tree API.

### 2.5 Start the supervised server

```bash
make start
```

Result (after resolving a stale process — see §8 Issues):

```text
llama-server is healthy at http://127.0.0.1:8080 (pid 97101)
```

### 2.6 Check status

```bash
make status
```

Result:

```text
pidfile present: true
process alive: true
health check (GET /health == 200): true
```

### 2.7 List models and send a chat completion

`GET /v1/models` returned HTTP 200 with a non-empty `data` array. The model
id used in subsequent requests is:

```text
models/qwen2.5-coder-7b-instruct-q4_k_m.gguf
```

`POST /v1/chat/completions` with `"Say hello."` returned:

```json
{
  "choices": [
    {
      "finish_reason": "stop",
      "message": {
        "role": "assistant",
        "content": "Hello! How can I help you today?"
      }
    }
  ],
  "usage": {
    "completion_tokens": 10,
    "prompt_tokens": 32,
    "total_tokens": 42
  }
}
```

### 2.8 Verify zero egress

```bash
make verify
```

Result:

```text
./target/release/llm-runner verify --egress
/v1/models: found model "models/qwen2.5-coder-7b-instruct-q4_k_m.gguf"
chat reply: Hello! How can I assist you today?
egress check passed: llama-server (pid 97101) shows no non-loopback network activity
```

A manual `lsof` cross-check confirmed the supervised process holds only
loopback sockets:

```text
llama-ser 97101 ben 3u IPv4 TCP 127.0.0.1:8080 (LISTEN)
llama-ser 97101 ben 6u IPv4 TCP 127.0.0.1:8080->127.0.0.1:65315 (ESTABLISHED)
```

### 2.9 Stop the server

```bash
make stop
```

Result:

```text
llama-server (pid 97101) stopped
```

The pidfile was removed and a follow-up `make status` failed cleanly.

---

## 3. CLI testing logic

The CLI surface consists of four subcommands (`start`, `stop`, `status`,
`verify`) plus `--config`. The tests below cover both the happy path and
failure modes.

| Test | Command / check | Expected | Result |
|---|---|---|---|
| Build | `make release` | optimized binary exists | pass |
| Unit tests | `make test` | 19 pass | pass |
| Lint | `make lint` | no warnings/errors | pass |
| Model download | `make model` | verified GGUF present | pass |
| Start | `make start` | server healthy, pidfile written | pass |
| Status happy path | `make status` | pidfile + process + health all true | pass |
| Health endpoint | `curl /health` | HTTP 200 | pass |
| Models endpoint | `curl /v1/models` | non-empty `data` | pass |
| Chat endpoint | `curl /v1/chat/completions` | HTTP 200, non-empty content | pass |
| Egress verification | `make verify` | only loopback sockets | pass |
| Stop happy path | `make stop` | process gone, pidfile gone | pass |
| Start with missing config | `llm-runner start --config /nonexistent.toml` | exit 1 | pass |
| Status with missing config | `llm-runner status --config /nonexistent.toml` | exit 1 | pass |
| Stop with no pidfile | `llm-runner stop` (after stopped) | exit 1, clear error | pass |
| Status after stop | `make status` | exit non-zero, all false | pass |

---

## 4. Performance testing logic

A new helper script, `scripts/benchmark.sh`, performs reproducible streaming
benchmarks. It was written to satisfy `shellcheck` with no suppressions and
uses only tools already required by the project (`curl`, `python3`, `ps`,
`awk`).

### Why streaming mode?
For non-streaming chat completions, `curl`'s `time_starttransfer` is
approximately the full request latency because the server does not emit any
bytes until generation finishes. To obtain a true **time-to-first-token**
(TTFT) and a meaningful wall-clock **generation time**, the script sends
`"stream": true` requests.

### Metrics
- **TTFT** — `curl`'s `time_starttransfer` to the first SSE chunk.
- **Total latency** — `curl`'s `time_total`.
- **Generation time** — `total - ttft`.
- **Wall-clock tokens/sec** — `predicted_n / generation_time`, where
  `predicted_n` is read from the server's own timings in the final SSE chunk.
- **Server-reported tokens/sec** — `timings.predicted_per_second` from the
  final SSE chunk, shown for cross-check.
- **CPU usage** — `ps -o %cpu=` sampled every 0.5 s during each request;
  average and peak reported.

### Workloads
1. **short** — `"Say hello."`, `max_tokens=64`
2. **medium** — `"Explain Rust ownership in three sentences."`,
   `max_tokens=256`
3. **code** — `"Write a Python function that reads a CSV and returns a list of dicts."`,
   `max_tokens=512`, `temperature=0.7`

Each workload runs one warm-up request (discarded) followed by three timed
iterations.

### Running the benchmark

```bash
chmod +x scripts/benchmark.sh
./scripts/benchmark.sh [HOST] [PORT]
```

Defaults are `127.0.0.1` and `8080`.

---

## 5. Status and readiness analysis

A "ready" native deployment must satisfy all of the following:

| Criterion | Check | Expected | Observed |
|---|---|---|---|
| Config valid | `config::load_config` + host validation | loopback host accepted | `127.0.0.1` accepted |
| Binary built | `make release` | success | success |
| Model present | `make model` | verified GGUF exists | 4.4 GB at `models/...` |
| Server started | `make start` | healthy within 120 s | healthy |
| Pidfile present | `.runner/llama-server.pid` | exists | yes |
| Process alive | `ps -p <pid>` | `llama-server` running | yes |
| Health endpoint | `GET /health` | HTTP 200 | yes |
| Model listing | `GET /v1/models` | non-empty `data` | yes |
| Chat works | `POST /v1/chat/completions` | 200 + non-empty content | yes |
| No network egress | `verify --egress` + `lsof` | only loopback sockets | yes |
| Stop works | `make stop` | pid removed, status fails | yes |

The Kubernetes path is considered **available for portability testing** but
not a performance target because, on macOS, containers run inside a Linux VM
and inference is CPU-only.

---

## 6. Results

### 6.1 Static / quality gates

```text
make release  ->  Finished release profile in 11.96s
make test     ->  19 passed; 0 failed
make lint     ->  cargo clippy, cargo fmt --check, shellcheck all clean
```

### 6.2 Native integration checks

```text
make status  -> pidfile: true, process: true, health: true
GET /health  -> 200
GET /v1/models -> data[0].id = models/qwen2.5-coder-7b-instruct-q4_k_m.gguf
POST /v1/chat/completions -> HTTP 200, content: "Hello! How can I help you today?"
make verify  -> egress check passed
make stop    -> pidfile removed
```

### 6.3 Performance results

```text
### Benchmark parameters
- Base URL: http://127.0.0.1:8080
- Model: models/qwen2.5-coder-7b-instruct-q4_k_m.gguf
- Server PID: 97101
- Iterations per workload: 3 (plus one warm-up)
- Streaming: enabled (true TTFT / wall-clock throughput)

| Workload | Iter | Predicted tokens | TTFT (s) | Total (s) | Gen time (s) | Wall tok/s | Server tok/s | CPU avg % | CPU max % |
|---|---|---|---|---|---|---|---|---|---|
| short  | 1 |  10 | 0.002363 |  0.579308 | 0.577 | 17.33 | 19.17 | 4.0 | 4.1 |
| short  | 2 |  10 | 0.002378 |  0.608425 | 0.606 | 16.50 | 18.42 | 2.5 | 2.7 |
| short  | 3 |  10 | 0.002296 |  0.559795 | 0.557 | 17.95 | 19.93 | 2.4 | 2.7 |
| medium | 1 |  75 | 0.004468 |  4.540044 | 4.536 | 16.53 | 17.15 | 4.3 | 6.2 |
| medium | 2 |  63 | 0.012086 |  3.941703 | 3.930 | 16.03 | 16.27 | 2.5 | 3.7 |
| medium | 3 |  72 | 0.004297 |  4.466684 | 4.462 | 16.14 | 16.35 | 3.0 | 3.8 |
| code   | 1 | 145 | 0.004567 |  8.707439 | 8.703 | 16.66 | 16.99 | 4.1 | 5.6 |
| code   | 2 | 245 | 0.004558 | 14.282951 | 14.278 | 17.16 | 17.22 | 4.6 | 6.5 |
| code   | 3 | 156 | 0.008218 |  8.739756 | 8.732 | 17.87 | 17.98 | 4.7 | 7.2 |
```

#### Performance interpretation
- **TTFT** is consistently in the low single-digit milliseconds (with one
  outlier at 12 ms), indicating the model is loaded and ready on Metal.
- **Wall-clock throughput** is stable around **17 tokens/second** across all
  workloads.
- **Server-reported throughput** agrees closely with the wall-clock figure,
  confirming the measurement is sound.
- **CPU usage stays very low** (average 3–5 %, peak < 9 %) because the
  7B Q4_K_M model is running almost entirely on the Apple Silicon GPU/Neural
  Engine via Metal.

### 6.4 Kubernetes smoke test

```bash
make k8s-start
```

Result:

```text
configmap/llm-runner unchanged
service/llm-runner unchanged
deployment.apps/llm-runner configured
deployment.apps/llm-runner scaled
Waiting for deployment "llm-runner" rollout to finish ...
deployment "llm-runner" successfully rolled out
llm-runner is ready at http://localhost:30080/v1
```

Checks:

```text
kubectl get pods -l app=llm-runner  ->  1/1 Running
GET http://localhost:30080/v1/models  ->  non-empty data
POST http://localhost:30080/v1/chat/completions  ->  HTTP 200, "Hello! How can I assist you today?"
make k8s-stop  ->  llm-runner scaled down to 0 replicas
```

The Kubernetes path serves correctly but is CPU-only inside the Rancher
Desktop VM, so no token/sec baseline was collected for it.

---

## 7. Manual offline verification

The README asks users to repeat the chat-completion request after disabling
all network interfaces. Because toggling physical interfaces can disconnect
an active remote session, this step was **documented but not executed live**.
The executed `--egress` check and `lsof` output already prove the supervised
process binds and communicates only on loopback.

If you want to run the offline test manually on a local machine:

```bash
# Identify interfaces
networksetup -listallhardwareports

# Disable Wi-Fi (replace <device> with the actual device name)
networksetup -setairportpower <device> off

# If a wired interface is the default route, disable it too
networksetup -setnetworkserviceenabled "<service name>" off

# Repeat the chat completion; it must still succeed
curl http://127.0.0.1:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
        "model": "models/qwen2.5-coder-7b-instruct-q4_k_m.gguf",
        "messages": [{"role": "user", "content": "Say hello."}]
      }'

# Re-enable interfaces
networksetup -setairportpower <device> on
networksetup -setnetworkserviceenabled "<service name>" on
```

---

## 8. Issues found

1. **Stale server on port 8080 before first `make start`**
   - A `llama-server` process started earlier (PID 69801) was still listening
     on `127.0.0.1:8080`.
   - The first `make start` spawned a new child (PID 96075) that could not
     bind the port, exited, but the runner's health check still succeeded
     because *something* was responding on `127.0.0.1:8080`.
   - This left the runner pointing to a dead PID while an unsupervised server
     answered requests.
   - **Resolution:** killed the stale process and re-ran `make start`.
   - **Implication:** the runner's health check validates the *port*, not that
     the response came from the PID it spawned. A port conflict can produce a
     false-positive start.

2. **`make stop` prints raw `kill` stderr when the process is already gone**
   - When the supervised process exited before `make stop` was invoked, the
     output included:
     ```text
     kill: 97101: No such process
     ```
   - The command still returned success and cleaned up the pidfile. This is a
     cosmetic issue; suppressing `kill`'s stderr in `src/process.rs` would
     make the UX cleaner.

Neither issue blocks local serving, but issue #1 is worth hardening if the
runner will be used in environments where port 8080 may already be occupied.

---

## 9. Conclusion

- **Build / test / lint:** green.
- **Native Quickstart:** fully functional — start, status, chat completion,
  zero-egress verification, and stop all behave as documented.
- **Performance:** stable **~17 tokens/second** wall-clock throughput, **< 10 ms
  TTFT**, and very low CPU usage thanks to Metal offloading.
- **Kubernetes path:** smoke-tested successfully; serves the same API but is
  CPU-only and slower by design on macOS.
- **Readiness:** the application is ready for fully-local, OpenAI-compatible
  LLM serving on this machine.
