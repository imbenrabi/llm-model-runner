# Shell Automation

Provisioning and cluster lifecycle scripts. The invariant is idempotent, local-only model management and on-demand Kubernetes scaling.

## Key Rules

- Every script starts with `set -euo pipefail`.
- Download the model to a `.part` file, verify its sha256, then atomically `mv` it into place.
- Fetch the expected sha256 at runtime from the HuggingFace tree API, not from a hardcoded value.
- Declare every function variable `local`.

## Anti-patterns

- Never curl directly into the final model filename. A partial or failed download would be mistaken for a complete file.
- Never hardcode a model checksum. The tree API is the source of truth.
- Never assume the Kubernetes deployment exists in `k8s-stop.sh`. The script exits cleanly if it is absent.

## Pitfalls

- `download-model.sh` uses `curl -C -` to resume partial downloads, so a `.part` file may already exist from a previous run.
- `k8s-start.sh` substitutes `__MODELS_DIR__` in `deployment.yaml` with `sed` before piping to `kubectl apply`. The deployment manifest is therefore not applied verbatim.
- The base Kubernetes deployment has `replicas: 0`. Running `kubectl apply -f k8s/` alone does not start serving; `scripts/k8s-start.sh` scales it to 1.

## Testing

```bash
shellcheck scripts/*.sh
```
