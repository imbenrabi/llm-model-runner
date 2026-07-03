# GLM-5.2 Local Feasibility

Facts below verified 2026-07-03.

## Verdict

GLM-5.2 cannot execute locally on the evaluated machine (Apple M4, 16 GB
unified memory, approximately 22 GB free disk at evaluation time) at any
available quantization.

## Background

GLM-5.2 was released 2026-06-13 by Z.ai under the MIT license, with open
weights at [huggingface.co/zai-org/GLM-5.2](https://huggingface.co/zai-org/GLM-5.2).
It is a Mixture-of-Experts model with approximately 744 B total parameters
and approximately 40 B active parameters, and a 1M-token context window.
BF16 weights are approximately 1.5 TB.

## Smallest practical quantization

The smallest practical quantization is the Unsloth dynamic 2-bit (UD-IQ2_M)
GGUF, approximately 240 GB, which retains approximately 82% of BF16 quality.
It is sized for 256 GB unified-memory machines with tight headroom. 4-bit
quantization needs approximately 512 GB. The 1-bit variant (UD-IQ1_S) reaches
approximately 21.6 tok/s on a Mac Studio M3 Ultra with 256 GB.

## Gap on this machine

Memory is short by roughly 15x (16 GB available vs 256 GB needed). Disk is
short by roughly 10x (approximately 22 GB free vs approximately 240 GB
needed). This is impossible on this hardware, not merely slow.

## The Ollama cloud trap

[ollama.com/library/glm-5.2](https://ollama.com/library/glm-5.2) exposes only
a `:cloud` tag, meaning inference actually runs on Ollama's servers (cloud
passthrough) rather than locally. This violates the 100%-local requirement
for this project. The same is true of glm-5.1, glm-5, and glm-4.7-flash on
Ollama. The newest genuinely downloadable (local-weights) GLM on Ollama is
the 2024-era `glm4`. As of July 2026 there is no small GLM-5-family variant
(no Air/mini edition).

## Minimum viable hardware for true local GLM-5.2

- A 256 GB unified-memory Mac Studio (M3 Ultra class) for 1-2 bit
  quantizations, at single-digit to approximately 20 tok/s.
- 512 GB unified memory for 4-bit quantization.
- Multi-GPU servers (roughly 8x H200-class GPUs) for BF16/FP8 serving.

## Chosen alternative and rationale

Qwen2.5-Coder-7B-Instruct Q4_K_M was selected instead: Apache-2.0 license,
coding-focused, 4.68 GB, leaves approximately 10 GB of headroom on this 16 GB
machine, Metal-accelerated.

Alternatives considered and rejected:

- GLM-4-9B-0414 (MIT license, the closest GLM-family option that runs
  locally, but April-2025 vintage with weaker coding ability than
  Qwen2.5-Coder).
- Gemma 4 12B (June 2026, strong model, but ships under a custom non-OSI
  license that conflicts with this project's open-source-only constraint).

## Forward path

This repo is model-agnostic. On hardware meeting the memory/disk
requirements above, GLM-5.2 GGUF runs by changing exactly one line:
`model_path` in `config/runner.toml` for the native path, or
`LLAMA_ARG_MODEL` in `k8s/configmap.yaml` for the Kubernetes path.

## Sources

- [huggingface.co/zai-org/GLM-5.2](https://huggingface.co/zai-org/GLM-5.2)
- [huggingface.co/unsloth/GLM-5.2-GGUF](https://huggingface.co/unsloth/GLM-5.2-GGUF)
- [unsloth.ai/docs/models/glm-5.2](https://unsloth.ai/docs/models/glm-5.2)
- [ollama.com/library/glm-5.2](https://ollama.com/library/glm-5.2)
- [ofox.ai/blog/glm-5-2-run-locally-gguf-2026](https://ofox.ai/blog/glm-5-2-run-locally-gguf-2026)
- [latent.space/p/ainews-glm-gpt-glm-52-passes-vibe](https://latent.space/p/ainews-glm-gpt-glm-52-passes-vibe)
- [models.dev/models/zhipuai/glm-5.2](https://models.dev/models/zhipuai/glm-5.2)
