# Intent Layer

Sparse hierarchy of `AGENTS.md` files that captures non-obvious architectural knowledge — invariants, pitfalls, anti-patterns, and tribal knowledge — that cannot be inferred from reading the code alone.

## Design principles

- **Progressive disclosure.** Start with high-signal context; drill down via downlinks.
- **Token efficiency.** Every file stays under 600 words. If the code already says it, delete it.
- **Hierarchical loading.** Ancestors load before descendants, so the broad picture precedes the specific detail.
- **LCA optimization.** Shared facts live at the Lowest Common Ancestor. Cross-reference siblings instead of duplicating.

## Maintenance rules

- If you change behavior documented in an `AGENTS.md`, update that file in the same PR.
- If you add a major directory with distinct concerns, add an `AGENTS.md` and wire it into the parent navigation table.
- Never create separate summary documents. The intent layer is the summary.

## Authoring constraints

- One file per semantic boundary, not per folder.
- Each fact appears in exactly one section within a file.
- Key Rules are positive ("always do X"). Anti-patterns are negative ("never do Y — consequence"). Pitfalls explain non-obvious gotchas.
- No how-to guides, templates, or procedural steps in `AGENTS.md`. Teaching content goes in a co-located `GUIDE.md`.
- Intermediate nodes must have a `Deeper Context` section with downlinks. Leaves contain only patterns unique to that directory.

## Control-flow scoring

| Points | Criterion |
|--------|-----------|
| +0 | Pure procedural step an agent can derive from the code. |
| +1 | Ordering dependency the code enforces. |
| +2 | Non-obvious ordering the code does not enforce. |
| +1 | Failure blast radius exceeds the immediate operation. |
| +1 | Side-effect surprise an agent would not expect. |
| +1 | Environment-dependent behavior that is not obvious. |

| Score | Action |
|-------|--------|
| 0–1 | Delete — the code says it. |
| 2 | Keep only the gotcha sentence in Pitfalls. |
| 3–4 | Extract to `FLOW.md` and reference from `AGENTS.md`. |
| 5 | Keep inline in `AGENTS.md` Anti-patterns/Pitfalls. |

## Navigation

| Node | What non-obvious knowledge lives there |
|------|----------------------------------------|
| [`src/AGENTS.md`](src/AGENTS.md) | Loopback enforcement, pidfile/health semantics, `ureq` v3 patterns, error handling. |
| [`scripts/AGENTS.md`](scripts/AGENTS.md) | Runtime HuggingFace checksum, `.part` + atomic `mv`, `__MODELS_DIR__` substitution, idempotent stop. |
| [`k8s/AGENTS.md`](k8s/AGENTS.md) | Digest-pinned image, base `replicas: 0`, in-cluster `0.0.0.0` bind, NodePort vs loopback semantics. |
