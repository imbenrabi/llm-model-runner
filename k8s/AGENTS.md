# Kubernetes Path

On-demand, CPU-only cluster serving path. The invariant is that the native loopback rule does **not** apply inside the cluster: the container binds `0.0.0.0` and traffic enters via NodePort.

## Key Rules

- Pin the llama.cpp server image by digest in `deployment.yaml`.
- Keep the base deployment at `replicas: 0`; scale up only via `scripts/k8s-start.sh`.
- Bind the container to `0.0.0.0` through `configmap.yaml:LLAMA_ARG_HOST`.
- Mount the host model directory via `hostPath`, substituting `__MODELS_DIR__` at apply time.

## Anti-patterns

- Never apply the native loopback requirement to the Kubernetes ConfigMap. `0.0.0.0` is correct there.
- Never use a mutable image tag. Always use a digest-pinned reference.
- Never set a default replica count above 0. The path is meant to be off unless explicitly started.

## Pitfalls

- `LLAMA_ARG_N_GPU_LAYERS: "0"` makes this path CPU-only on macOS, because containers run inside a Linux VM without Metal.
- NodePort `30080` conflicts if another service already uses it.
- The model file must exist on the host before `scripts/k8s-start.sh` runs; the script checks for it and fails fast.
- The in-cluster bind is wider than loopback, but the cluster path is still local-only in the sense that no cloud inference occurs. The security boundary is the cluster, not the interface.

## Testing

After any manifest change:

```bash
kubectl apply --dry-run=client -f k8s/
```
