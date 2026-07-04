# Server Lifecycle Control Flow

Ordered sequences for `start` and `stop` with non-obvious failure consequences.

## Start

1. Load and validate config (`config::load_config`).
2. Ensure no live supervised process exists; remove stale pidfile if present.
3. Create `.runner/` directory.
4. Open/truncate `.runner/llama-server.log`.
5. Spawn `llama-server` with stdout/stderr redirected to the log.
6. Write the child's PID to `.runner/llama-server.pid`.
7. Poll `/health` until HTTP 200 or 120 s timeout.

If step 7 fails, the child must be killed, waited on, and the pidfile removed before returning an error. Skipping the cleanup leaves a zombie supervised process and a stale pidfile that breaks the next `start`.

## Stop

1. Read `.runner/llama-server.pid`; error if absent.
2. Send SIGTERM to the PID.
3. Poll liveness (`kill -0`) until the process exits or 5 s timeout.
4. Remove the pidfile.

`stop` intentionally ignores `--config`: the pidfile is the single source of truth for which process to terminate.

## Why this is in the intent layer

Score: 5. Non-obvious ordering (pidfile written after spawn but before health confirmation), failure blast radius (a stuck process blocks all future starts), side-effect surprise (health-timeout cleanup), and environment-dependent timing (model load duration) all make these sequences easy to break accidentally.
