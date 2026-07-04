# Rust CLI

Supervises a native `llama-server` process: config loading, process lifecycle, and verification. The invariant is loopback-only local serving.

## Key Rules

- Always enforce loopback at config load time (`src/config.rs::validate_config`). Only `127.0.0.1` and `::1` are accepted.
- Always return `Result<T, RunnerError>`; never use `unwrap` or `expect` outside `#[cfg(test)]`.
- Always use `ureq` v3's builder pattern: `.config().timeout_global(...).build().call()`.
- Always keep the model path in config; never hardcode it in source.

## Anti-patterns

- Never widen the native host bind to `0.0.0.0` or a LAN address. The Kubernetes path intentionally uses `0.0.0.0` inside the cluster, but the native path must remain loopback.
- Never bypass the pidfile to determine process state. External `pgrep` of `llama-server` is unreliable because multiple instances may exist.
- Never ignore a failed health-poll startup. If the server does not become healthy, kill the child, wait on it, and remove the pidfile.

## Pitfalls

- `llama-server` can take tens of seconds to load a model into memory. The health poll waits up to 120 seconds; a shorter timeout will flake on cold starts.
- `handle_stop` does not accept `--config`. It always targets `.runner/llama-server.pid`, by design.
- `is_process_alive` uses `kill -0`, which succeeds for any process with that PID, even a zombie or a different process that reused the PID.
- `verify.rs` parses `lsof` output. IPv6 addresses are bracketed (`[::1]:8080`) and wildcard binds (`*:8080`, `0.0.0.0:8080`) must be classified as non-loopback failures.
- `verify --egress` requires a pidfile from a supervised process; it cannot run against an independently started `llama-server`.
- `start` truncates `.runner/llama-server.log` on every run, so previous server output is lost.
- `verify.rs` skips the first line of `lsof` output (the header); failing to skip it would falsely flag the header as a non-loopback connection.

## Flow Reference

See [`FLOW.md`](FLOW.md) for the ordered `start` and `stop` sequences and their failure consequences.

## Testing

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
