---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-10
---

# Local inference and GPU allocation

A reference for advising on and reviewing local model serving on a developer's own machine: Apple silicon with unified memory, and Linux or Windows boxes with NVIDIA or AMD GPUs. Used by the `local-inference` subagent. The scope is the serving runtime, the quantization choice, the memory arithmetic, the GPU-sharing mechanism, and the cost model that decides whether local was worth it. It is not the model's reasoning quality, which is the model's problem, and not the agent framework calling it, which is `llm-app`'s.

The unifying model: **local inference is a memory problem wearing a compute costume.** Nearly every failure that reaches a user, a model that will not load, one that runs at 4 tokens per second, a second agent that stalls behind the first, a Mac that swaps until it dies, is the same failure: someone summed weights and forgot the KV cache, or summed both and forgot that the GPU shares the machine. The operational question for every configuration is therefore: **what is the resident set at the peak of the longest context this will actually see, and what else on this machine wants those same bytes at that moment?**

Empirical priority order, by how often each bites:

1. **KV-cache arithmetic ignored.** Weights fit, the first 4k-token exchange works, and the 60k-token agentic session falls off a cliff. Nobody multiplied layers by kv-heads by head-dim by context.
2. **Contention with the rest of the machine.** A Docker Desktop VM, a browser, and a Metal working set that together promise more than physical RAM. The symptom is not an error; it is a slowdown that gets blamed on the model.
3. **Concurrency defaults that serialize agents.** Ollama's `OLLAMA_NUM_PARALLEL` default of 1, mlx-lm's `--kv-bits` disabling batching, two vLLM instances each pre-allocating 92% of one GPU. Two agents, one queue.
4. **Quantization chosen by folklore.** "Q4 is free" was measured on a 1-trillion-token LLaMA-1 in 2023. The models of 2026 were trained on 15 to 36 trillion tokens and lose more.
5. **Local chosen for cost when it is not cheaper.** For single-stream interactive use, local inference costs 10x to 100x the hosted open-weight price per token once hardware amortization is counted. It wins on privacy, latency for short warm prompts, and batch jobs that saturate the device, and loses everywhere else.

## Volatile surface

`last-verified` (see frontmatter; do not restate the date here). This domain rots at the speed of a weekly release train. Treat every version, default, price, and benchmark below as a dated snapshot.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Runtime versions, flags, defaults | **Very fast** (weekly) | `gh api repos/<owner>/<repo>/releases/latest`; each project's docs index |
| Ollama defaults (parallelism, context by VRAM, KV type) | Very fast | github.com/ollama/ollama `docs/faq.mdx`, `docs/context-length.mdx` |
| vLLM memory fraction, metrics names, OpenAI-compat gaps | Medium | vllm `vllm/config/cache.py`, `docs/design/metrics.md`, `docs/serving/online_serving/openai_compatible_server.md` |
| ROCm consumer support matrix | Medium | rocm.docs.amd.com install-on-linux system-requirements |
| MIG-capable GPU list | Low to medium | docs.nvidia.com MIG user guide, supported-gpus |
| Hardware prices, API prices, cost break-evens | **Very fast** | Vendor price pages; recompute, do not quote |
| Coding benchmarks (SWE-bench, Aider polyglot) | Very fast | Model cards on Hugging Face; aider.chat leaderboards (stale since 2025-08) |
| Apple Metal working-set cap | Low; measure, do not quote | `MTLDevice.recommendedMaxWorkingSetSize` or llama.cpp's Metal init log on the machine |
| KV-cache arithmetic per model | Low | `https://huggingface.co/<org>/<model>/raw/main/config.json` |
| Quantization quality papers | Low | arXiv identifiers cited below |

## Memory arithmetic, the part that never rots

### Weights

Parameters times bytes per parameter. bf16 is 2 bytes. Q8 is roughly 1. The GGUF K-quants are named for their nominal bits and land a little above them: Q4_K is 4.5 bits per weight (bpw), Q5_K 5.5, Q6_K 6.5625, Q3_K 3.4375, Q2_K 2.5625 (verified from llama.cpp PR #1684). Q4_K_M on a 7B lands near 4.83 bpw because attention and output layers are kept larger. Mixture-of-experts models store every expert, so a "30B-A3B" model needs 30B parameters of memory to run 3B of compute per token.

### KV cache

This is the term people omit. Per token:

```
bytes/token = 2 * layers * kv_heads * head_dim * bytes_per_element
```

The 2 is keys plus values. `bytes_per_element` is 2 for f16, 1 for q8_0, 0.5 for q4_0. Multiply by context length and by the number of concurrent sequences.

Grouped-query attention (GQA) is why `kv_heads` and not `attention_heads` appears: Llama-3.1-8B has 32 attention heads but 8 kv heads. Multi-head latent attention (MLA, DeepSeek-V2 and V3) compresses further: per token it stores `(d_c + d_h^R) * layers` elements, which DeepSeek's paper describes as equal to GQA with 2.25 groups.

Read from each model's `config.json` and computed for f16 (verified 2026-09-10):

| Model | layers / kv_heads / head_dim | KiB per token | at 32k | at 128k |
|---|---|---|---|---|
| Llama-3.1-8B | 32 / 8 / 128 | 128 | 4.0 GiB | 16 GiB |
| Llama-3.1-70B | 80 / 8 / 128 | 320 | 10 GiB | 40 GiB |
| Qwen3-32B | 64 / 8 / 128 | 256 | 8 GiB | 32 GiB |
| Qwen3-Coder-30B-A3B | 48 / 4 / 128 | 96 | 3 GiB | 12 GiB |
| Qwen3-Coder-480B-A35B | 62 / 8 / 128 | 248 | 7.8 GiB | 31 GiB |
| Devstral-Small-2507 | 40 / 8 / 128 | 160 | 5 GiB | 20 GiB |
| GLM-4.5-Air | 46 / 8 / 128 | 184 | 5.8 GiB | 23 GiB |
| Gemma-3-27B | 62 / 16 / 128 | 496 naive; sliding-window layers cut it hard | up to 15.5 GiB | up to 62 GiB |
| gpt-oss-120b | 36 (18 sliding, w=128) / 8 / 64 | 72 upper bound | up to 2.2 GiB | up to 9 GiB |
| DeepSeek-V3 (MLA) | 61, d_c 512 + rope 64 | 68.6 | 2.1 GiB | 8.6 GiB |

The non-obvious consequence: **KV scales with layers times kv-heads, not with parameter count.** A 3B-active MoE (Qwen3-Coder-30B-A3B at 96 KiB) has less KV per token than dense Llama-8B (128 KiB), and 671B DeepSeek-V3 (68.6 KiB) has less than both. Gemma-3-27B, with 16 kv heads, is the hog. A reviewer who sees "we upgraded from 8B to a 30B MoE, so we lowered the context" has it backwards.

### The resident set

Weights plus KV at peak context times concurrent sequences, plus the runtime's own buffers (compute graph, prompt-processing scratch, and on Apple silicon the Metal command buffers), plus everything else on the machine. The last term is the one this file exists to make people write down.

## Apple silicon

### Unified memory and the wired limit

The GPU and CPU share physical RAM. The GPU's share is governed by a wired-memory limit: pages pinned so they cannot be swapped. The controls, verified against llama.cpp's own discussion (#2182) and its Metal backend source:

- `sudo sysctl iogpu.wired_limit_mb=<MB>` raises the limit. It does **not** persist across reboot unless placed in `/etc/sysctl.conf`. The discussion's author "would not recommend going to 100%".
- Exceeding it produces one of two symptoms: a SIGKILL of the process, or, more often and worse, "models load much slower with a long swap/unswap process." **The boundary is a slowdown before it is a kill**, because Metal will allocate beyond the recommended set and the non-wired buffers page.
- MLX exposes the same knob as `mx.set_wired_limit`, macOS 15 and later; a value at or above total memory is an error, and `mx.device_info()["max_recommended_working_set_size"]` reports the ceiling.

Measured on the development machine, a 64 GiB M1 Max, on 2026-09-10: `iogpu.wired_limit_mb` is 0 (system default), `iogpu.dynamic_lwm` is 1, and Metal's `recommendedMaxWorkingSetSize` is 53,084 MiB, which is **81% of RAM**. The community rule of thumb, two thirds below 32 to 36 GB and three quarters above, is **FOUND-UNVERIFIED** against any Apple source and is imprecise on this machine. Query the property; do not quote a fraction. llama.cpp's `ggml-metal-device.m` logs the value at init and reports it as the device "total", then warns when `currentAllocatedSize` exceeds it, with a source comment noting it is possible to allocate more.

Apple's own definition of the property (verified): "An approximation of how much memory, in bytes, this GPU device can allocate without affecting its runtime performance." The phrase "without affecting its runtime performance" is the whole warning.

### Who else wants those bytes

Docker Desktop for Mac's VM "defaults to 50% of your host's memory" with 1 GB of swap and no GPU passthrough (verified). A Metal working set of 81% plus a VM ceiling of 50% is 131% of physical RAM, allocatable on paper. The VM does not release memory as its containers exit. **INFERRED**, and consistent with a kill sequence observed on this machine: a long session of container runs grows the VM toward its cap, the host's free memory erodes, and whatever tool enforces a host-side low-memory threshold starts killing background work while the OS still reports nearly half its memory free. Docker Model Runner, for what it is worth, runs its llama.cpp engine **outside** the VM on macOS and Windows (verified), so it competes with Metal directly rather than through the VM.

On a Mac that runs both containers and local models, the two memory ceilings are the design decision, and they are set in two different places (Docker Desktop settings, `iogpu.wired_limit_mb`) by two different people who each assume they own the machine.

### Performance shape

Verified from ggerganov's discussion #4167 on LLaMA 7B: M1 Max (400 GB/s) prompt-processes 512 tokens at 600 tok/s in F16 and generates Q4_0 at 61 tok/s; M2 Ultra (800 GB/s) 1402 and 94; M3 Ultra 1538 and 92; M4 Max (546 GB/s) 923 and 83. Two lessons: generation speed tracks memory bandwidth almost linearly (the model is streamed once per token), and prompt processing is compute-bound and an order of magnitude slower than an NVIDIA card. A 128k-token coding prompt on an M3 Ultra is minutes of prefill; the same prompt on an RTX 5090 is seconds. Wall power (Apple, verified): M3 Ultra 9 W idle, 270 W max; M4 Max 6 W and 145 W.

## NVIDIA and AMD

### Sharing one GPU between processes

Three mechanisms, each with a different isolation story (verified against NVIDIA's own documentation):

- **MIG** partitions a supported GPU into hardware-isolated instances with their own memory and fault domains. Supported (as of `last-verified`): GB200, B200, H100/H200/H20, A100 (7 instances); A30 and RTX PRO 6000 Blackwell (4); RTX PRO 5000 and 4500 (2). **No GeForce card supports MIG.** A developer's 4090 or 5090 cannot be partitioned.
- **Time-slicing** (GPU Operator) gives processes turns. "No memory or fault-isolation between replicas", and extra replicas add no proportional compute; it is fairness, not capacity.
- **MPS** (Multi-Process Service) lets processes share a context to overlap kernels. Linux and QNX only; 60 client contexts per device by default. Its documented fault model: "a fatal fault from one client may bring down a different user's client that shares any GPU"; Volta and later recover the server, earlier architectures shut it down. Per-client memory caps exist (`CUDA_MPS_PINNED_DEVICE_MEM_LIMIT='0=1G,1=512MB'`, `CUDA_MPS_ACTIVE_THREAD_PERCENTAGE`), and MPS v3 adds cgroup-scoped hard (`dmem.max`, OOM) and soft (`dmem.min`, eviction) limits. Whether MPS is supported on GeForce consumer cards: **UNVERIFIED** either way.

The practical consequence for two agents on one consumer GPU: they share a fault domain and a memory pool with no partition between them. vLLM makes this explicit in its `gpu_memory_utilization` docstring: the default of 0.92 is **pre-allocated**, so two instances on one GPU must each be set to 0.5 or the second fails to start.

### Selecting devices

Use `CUDA_VISIBLE_DEVICES` with **UUIDs from `nvidia-smi -L`**, not numeric indices; the numeric order is not stable across drivers and reboots (Ollama's GPU documentation says so, verified). AMD uses `ROCR_VISIBLE_DEVICES`. An invalid ID silently forces CPU, which is a slowdown that looks like a bad model.

### AMD consumer support

ROCm 7.x's consumer matrix (verified against AMD's system-requirements page, and **VOLATILE** (2026-09-10)): RX 9070 XT/GRE/9070 (gfx1201), RX 9060 XT LP/XT/9060 (gfx1200), RX 7900 XTX/XT/GRE (gfx1100), RX 7800 XT/7700 XT/7700 (gfx1101), on Ubuntu 24.04.4 and 22.04.5, RHEL 10.1 and 9.7 only. The RX 7600 and the whole RX 6000 line are absent. Two documented fallbacks: `HSA_OVERRIDE_GFX_VERSION` (10.3.0 for RDNA2, 11.0.0 for RDNA3), which llama.cpp's build guide and Ollama's GPU doc both describe and which is "not supported on Windows"; and the Vulkan backend, which needs no ROCm at all.

### Detecting contention before it is a slowdown

`nvidia-smi`'s `utilization.gpu` is, per NVML's definition, "percent of time during which one or more kernels was executing." It is **not** SM occupancy: 100% can be one tiny resident kernel. It is a lagging and coarse signal.

The signals that lead (**INFERRED** from each runtime's metrics surface, which is verified):

- vLLM: `vllm:num_requests_waiting`, `vllm:kv_cache_usage_perc`, `vllm:request_queue_time_seconds`, preemption counts, and the prefix-cache hit ratio. A rising waiting queue with flat throughput is the pool saturating.
- llama.cpp server: `/slots` occupancy (on by default) and `llamacpp:requests_processing` (needs `--metrics`).
- Ollama: `ollama ps` shows a PROCESSOR column splitting GPU and CPU; **any CPU percentage on a model meant for the GPU is the leading sign** that memory ran out and layers spilled.
- Apple silicon: `sysctl iogpu`, the headroom between `currentAllocatedSize` and `recommendedMaxWorkingSetSize` in the Metal init log, mlx-lm's "requires ... wired" warning, and `powermetrics --samplers gpu_power`.

The signals that lag: time to first token, inter-token latency, and `nvidia-smi` memory near its cap. By the time these move, the user has already noticed.

## Serving runtimes

Versions and dates verified via `gh api` on 2026-09-10 and **VOLATILE** at weekly cadence. What each is for and what it costs to choose it:

**llama.cpp** (v0.4.0, 2026-09-04; note the move from `b####` build numbers to semver). Any GPU or CPU, GGUF, single node, and every flag exposed the day it exists: v0.4.0 added per-slot context limits, `--lazy-mode` on-demand tensor reading, and an Apple RDMA RPC transport. Continuous batching is on by default; `--parallel` defaults to auto; `--kv-unified` is on, meaning slots share one KV buffer rather than splitting `--ctx-size` between them; `--cache-prompt` and `--cache-reuse` (KV shifting) are on; slots can be saved to and restored from disk. Speculative decoding ships several drafters. Its README states verbatim "no strong claims of compatibility with OpenAI API spec", which is the honest version of what every runtime should say.

**vLLM** (v0.29.0, 2026-09-09). Throughput under many concurrent requests on NVIDIA and AMD: PagedAttention, continuous batching, hash-based prefix caching over full blocks (sha256 by default since v0.11). Applies a model's Hugging Face `generation_config.json` defaults **silently**, which surprises people comparing against another runtime. OpenAI compatibility gaps (verified in its docs): `suffix` unsupported on completions; `user` ignored and `image_url.detail` unsupported on chat; runtime-specific knobs go through `extra_body`. **Apple silicon is not a target**: the docs call it "experimental ... build from source ... CPU ... FP32 and FP16", and Metal exists only through a community plugin. GGUF is "highly experimental and under-optimized" and has moved to a plugin.

**SGLang** (v0.5.19, 2026-09-05). RadixAttention keeps a radix tree over KV so shared prefixes across requests are reused, which is the agentic-workload shape (one long system prompt, many short turns). `--mem-fraction-static` (fallback 0.88), `--schedule-policy` fcfs by default with longest-prefix-match optional. Hardware list names NVIDIA, AMD MI300/MI355, Intel Xeon, TPU, Ascend. No Apple.

**Ollama** (v0.34.0, 2026-09-05). Zero configuration, a model registry, automatic load and unload. Its defaults are the ones that bite agents (verified in its FAQ and context-length docs): `OLLAMA_NUM_PARALLEL` "default 1", so a second concurrent agent queues; `OLLAMA_MAX_LOADED_MODELS` is three times the GPU count; `OLLAMA_MAX_QUEUE` 512; `OLLAMA_KEEP_ALIVE` five minutes. **Default context is set by VRAM**: under 24 GiB, 4k tokens; 24 to 48 GiB, 32k; 48 GiB and above, 256k. The same documentation says agents and coding tools "should be set to at least 64000 tokens" via `OLLAMA_CONTEXT_LENGTH`. So on the most common developer GPU, Ollama's default context is sixteen times smaller than its own recommendation for the workload it is most often used for. `OLLAMA_KV_CACHE_TYPE` (f16, q8_0, q4_0) is global to the server, not per model. Its scheduler blog (2025-09-23) describes measuring exact memory instead of estimating it, after which `nvidia-smi` and `ollama ps` agree. Ollama's relationship to llama.cpp: a new Go engine calls ggml directly for several model families while a legacy `llama/` runner remains. On attribution, ollama/ollama issue #3185, "doesn't distribute notice licenses in its release artifacts", has been **open since 2024-03-16** with 57 comments (verified); the README now credits "llama.cpp project founded by Georgi Gerganov" (verified). A circulated claim that v0.7.1 added a ggml acknowledgement in its release notes is **FOUND-UNVERIFIED**; the release body had no such text.

**mlx-lm / MLX** (mlx-lm v0.31.3, 2026-04-22; mlx v0.32.2, 2026-08-25). Apple silicon native; wires model and cache into memory on macOS 15 and later. The server is documented as "not recommended for production". `--kv-bits` quantizes the KV cache **and disables batching**: the docs say it then "processes requests one at a time". `--max-kv-size` gives a rotating cache; `mlx_lm.cache_prompt` persists a prefilled prompt to safetensors.

**LM Studio** (0.4.0 major release 2026-01-28; closed source). A GUI over llama.cpp and MLX engines with just-in-time model loading, a default TTL of 60 minutes, and an auto-evict that keeps at most one JIT-loaded model resident. Serves the OpenAI surface on port 1234. Which OpenAI parameters it ignores: **UNVERIFIED**, not found.

**TGI** (v3.3.7, 2025-12-19) is in maintenance mode; its own README recommends "vllm, SGLang ... llama.cpp or MLX" going forward. Do not start new work on it.

**ExLlamaV3** (v1.4.9, 2026-09-10). EXL3 is "a streamlined variant of QTIP from Cornell RelaxML"; CUDA 12.4 and later only; continuous batching, tensor and expert parallelism; `-hq` raises attention-layer precision for 0.05 to 0.10 bpw. EXL3-versus-GGUF quality evidence exists only as chart images that were not read; treat comparisons as unverified.

## Quantization: what is measured and where the folklore is wrong

### The measurements

- **K-quants** (llama.cpp PR #1684, ikawrakow, 2023-06-05, LLaMA-1 7B, verified): perplexity F16 5.91, Q6_K 5.91, Q5_K_S 5.94, Q4_K_S 6.02, Q3_K_M 6.15, Q2_K 6.78.
- **i-quants and the importance matrix** (discussion #5263, verified): activation-weighted with `sqrtf(sigma2 + x^2)`; a wikitext-calibrated imatrix gave KLD at the 99th percentile of 0.073 against 0.161 for pseudo-random calibration; the author uses 100k tokens of wikitext, and calibration-set leakage into benchmarks is a live concern.
- **KL divergence per quant** (Artefact2's gist, Mistral-7B, verified): Q6_K median KLD 0.0032 and 99th percentile 0.0222; Q4_K_M (4.83 bpw) 0.0075 and 0.0885; IQ2_XXS 0.1751 and 2.4983; IQ1_S 0.5495 and 5.5174. The gist's rule, verbatim: "Use the largest that fully fits in your GPU. If you can comfortably fit Q4_K_S, try using a model with more parameters." Note the 99th-percentile column: IQ2_XXS's tail is 28x Q4_K_M's. Medians hide tails, and tails are where a coding agent emits the wrong token.
- **Red Hat / Neural Magic** (2024-10-17, Kurtic, Marques, Kurtz, Alistarh, verified): Llama 3.1 8B, 70B and 405B under W8A8-FP8, W8A8-INT8 and W4A16 recover "over 99%" on OpenLLM v1; HumanEval 99.9% at 8-bit, HumanEval+ 98.9% at 4-bit; the 8B shows more word-choice variability than the larger models.
- **GPTQ** (arXiv 2210.17323, verified) reaches 3 to 4 bits with "negligible" loss and quantizes a 175B model in about four GPU hours. **AWQ** (arXiv 2306.00978, MLSys 2024 best paper, verified): "protecting only 1% salient weights can greatly reduce quantization error", with no backpropagation.
- **KV-cache quantization** (KIVI, arXiv 2402.02750, verified): keys want per-channel quantization and values per-token; 2-bit KV gives 2.6x peak-memory reduction. The practical caveats are the runtime ones above: mlx-lm serializes requests, Ollama's setting is global.
- **Unsloth Dynamic** (its docs, verified): per-layer bit selection with no quantization-aware training; Gemma 3 27B KLD improves at each level (Q2_K_XL 0.2297 to 0.2209, Q3_K_XL 0.0878 to 0.0806, Q4_K_XL 0.0249 to 0.0237); the page says "KL Divergence should be one of the gold standards", and notes that fixing MMLU tokenization moved a score from 67.8% to 68.2%, so **sub-point MMLU deltas between quants are implementation noise.**

### Where the folklore is wrong

**"Q4 is free."** Two verified results say otherwise for modern models. Ouyang et al. (arXiv 2411.17691) and Kumar et al. (arXiv 2411.04330) both find that quantization damage grows with pretraining tokens; more pretraining data can become "actively harmful" to the quantized model's quality, and models trained past roughly 100 trillion tokens "may not" quantize well at low bits. **INFERRED consequence**: the 2023 tables were measured on a 1-trillion-token model. A 2025 or 2026 model trained on 15 to 36 trillion tokens loses more at Q4 and much more at Q2 and Q3. The rule is to re-measure per model with KLD, not to inherit a table.

**"Same accuracy, so same model."** Dutta et al. (arXiv 2407.09141, verified) show that equal aggregate accuracy hides per-item "flips", questions the full model got right and the quantized one gets wrong and vice versa, and that compressed models are "significantly worse" on MT-Bench free-form generation even when multiple-choice accuracy matches. Use KLD and flip counts. Accuracy on a benchmark is the wrong instrument for a generation workload.

**FP8 is a hardware question, not a flag.** vLLM's quantization table (verified): FP8 W8A8 needs Ada (SM 8.9), Hopper, or AMD; **not Ampere**, so not a 3090 or an A100. Marlin kernels for GPTQ, AWQ, FP8 and FP4 need Turing or later. gpt-oss ships MXFP4 MoE weights and its model card notes "all evals were performed with the same MXFP4 quantization", which is the correct way to publish a quantized model's numbers and is rare.

## The cost model: when local is cheaper

Inputs verified; arithmetic inferred and dated **VOLATILE** (2026-09-10).

A worked break-even (kunalganglani.com, 2026-07-08): an RTX 4090 at $1,600 amortized over 24 months, $0.13/kWh, 400 W for 8 hours a day is $12.48 a month in power; with $25 a month of maintenance, about $104 a month, or $1.04 per million tokens at 100 million tokens a month. Hosted, blended: GPT-4o $13, Claude 3 Haiku $1.05, GPT-4o-mini $0.51, Gemini Flash 1.5 $0.255, DeepSeek V3.2 via OpenRouter $0.1145 per million. Break-even against GPT-4o-mini is 204 million tokens a month; against DeepSeek V3.2, 908 million. The author's conclusion: local is "10x more per token" than hosted DeepSeek V3.2.

For a 70B model single-stream on an M3 Ultra at 13.1 tok/s and 160 to 180 W at the wall (both from Javat and Kazakov, arXiv 2605.00519v2, and an XDA measurement), electricity alone is about $0.50 per million tokens, and 24-month amortization at 24/7 use adds $5 to $12 per million. That is **50 to 100x the hosted open-weight price.** Batching is the only lever that closes the gap: a device that is saturated by many concurrent sequences amortizes its cost; a single interactive agent never does.

The same paper reports the other side: the M3 Ultra shows "up to 23x advantage in energy efficiency (tokens/joule)" over an RTX 5090, runs Llama-3.3-70B 4-bit at 13.1 tok/s where the 5090 hits the VRAM wall, and an M4 Pro (52.3 tok/s) beats an M3 Ultra (49.1) on Qwen3-Next-80B MoE. On the 5090, a model that fits (Q2_K_XL) generates at 76.1 tok/s and one that must offload to CPU (Q4_K_M) at 4.7: fitting in VRAM is worth 16x, which is why the Artefact2 rule exists.

**Where local wins** (inferred from the above): privacy and air-gapped code; embeddings, rerankers, small classifiers and routers; batch jobs that saturate the device; short warm prompts where time to first token matters more than throughput. **Where it loses**: frontier-quality agentic coding; long-prompt prefill on Apple silicon; and any single-stream 70B-plus workload priced against a hosted open-weight API.

**Quality gap for coding** (all **VOLATILE**): Qwen3-Coder-Next (80B total, 3B active, 256k context, 2026-02-03) self-reports SWE-bench Verified 70.6 and Terminal-Bench 2.0 36.2 (verified model card). Frontier hosted models sit near 96% on SWE-bench Verified per an aggregator dated 2026-09-08 (**FOUND-UNVERIFIED**; the official leaderboard is JavaScript-rendered and was not fetchable). The Aider polyglot leaderboard (verified, but stale since 2025-08-25): gpt-5 88.0%, DeepSeek-V3.2-Exp 74.2%, gpt-oss-120b 41.8%, Qwen3-32B 40.0%, Gemma-3-27B 4.9%. A 27B dense model that a laptop can run is not a coding agent's model; a hosted open-weight model is closer, and a frontier model is in a different class.

## Schools of thought

Each position in its own strongest form. Do not average them.

### Quantize hard, or run a smaller model at higher precision

**Quantize hard.** Artefact2's rule is explicit: if Q4_K_S fits comfortably, use more parameters. Red Hat's data shows over 99% recovery at W4A16 across the Llama 3.1 line. The Silicon Showdown 5090 result is the argument in one number: Q2_K_XL in VRAM at 76 tok/s against Q4_K_M offloaded at 4.7. A bigger model at lower precision, resident, beats a smaller one that spills.

**Higher precision, smaller model.** Ouyang and Kumar show the damage grows with pretraining tokens, so the 2023 evidence for "hard quant is fine" is evidence about models nobody runs any more. Dutta shows accuracy tables hide the flips that generation workloads feel. And Artefact2's own data shows IQ2_XXS's 99th-percentile KLD at 28x Q4_K_M's: the median looks fine and the tail is where the agent writes the wrong line.

When each is right: hard quantization when the alternative is offloading (the spill cost dominates everything) and when the workload tolerates tails (classification, embeddings, chat). Higher precision when the workload is code or structured output, where one wrong token is a failed build, and when the model is a 2025-or-later long-pretrained one.

### One shared server, or one process per agent

**Shared.** vLLM and SGLang's continuous batching and prefix caching amortize the system prompt across every agent's requests; RadixAttention is built for exactly the many-agents-one-prompt shape. Two vLLM instances on one GPU each get half the memory and neither gets the batching. MPS shares a fault domain anyway, so per-process isolation on one consumer GPU is partly illusory.

**Per agent.** Runtimes serialize in ways a shared server hides: Ollama's `NUM_PARALLEL` default of 1 queues the second agent behind the first; llama.cpp's `--kv-unified` slots contend for one buffer; mlx-lm with `--kv-bits` is single-request by construction. A stalled or misbehaving agent in a shared server degrades every other agent; in its own process it degrades itself. llama-swap swaps whole processes for a reason. Secondary articles claiming Ollama "collapses at 5 concurrent users" are **FOUND-UNVERIFIED** and should not be cited either way.

When each is right: shared when the agents genuinely share a prefix and the runtime batches; per-process when the runtime does not batch, when fault isolation matters more than throughput, or on Apple silicon where the batching runtimes do not run.

### Ollama's convenience, or llama.cpp and vLLM's control

**Ollama.** The scheduler measures exact memory rather than estimating; `nvidia-smi` and `ollama ps` now agree; models load and unload without anyone managing processes; the Vulkan path runs on AMD cards ROCm does not list. For a developer who wants a model, not a serving stack, it is the shortest path.

**Direct control.** Ollama's default context below 24 GiB of VRAM is 4k while its own documentation says agents need 64k, and the KV cache type is global. llama.cpp exposes every flag the day it lands (per-slot context limits shipped in v0.4.0), and the license-notice issue has been open since March 2024. A runtime that hides its flags hides the ones the agent workload needs.

When each is right: Ollama for a single interactive user and for prototyping; direct control when two or more agents share the machine, when context must be set per model, or when a flag that does not yet exist in Ollama is the fix.

### Apple silicon: serious platform, or convenience

**Serious.** 23x tokens per joule against a 5090; 70B and 80B MoE models run at all where the 5090 hits the VRAM wall; DeepSeek R1 671B at 4-bit loads on an M3 Ultra and generates at 15 to 20 tok/s; an M4 Pro out-routes an M3 Ultra on MoE. For a machine that is already on the desk, the marginal cost of a local model is the electricity.

**Convenience.** Prefill is compute-bound and an M3 Ultra processes a 7B prompt at 1538 tok/s, so a 128k coding prompt is minutes before the first output token. A 5090 decodes 1.7x faster when the model fits. vLLM and SGLang do not run on Metal; mlx-lm's server is "not recommended for production"; the wired limit is a manual sysctl that resets on reboot. It is a fine place to run a model and a poor place to serve one.

When each is right: serious for decode-heavy, privacy-bound, or memory-bound work (big MoE, long conversations with short prompts); convenience for anything prefill-heavy or that needs a batching server.

## Anti-pattern catalog

Each: the pattern, the trigger, the consequence, the fix.

- **Summing weights and stopping.** Trigger: "the 30B fits in 24 GB." Consequence: works at 4k, spills or dies at 64k. Fix: compute KV per token from `config.json`, multiply by peak context and concurrency, and write the number down.
- **Trusting Ollama's default context for an agent.** Trigger: a coding agent on a sub-24 GiB GPU. Consequence: silent truncation at 4k, blamed on the model. Fix: `OLLAMA_CONTEXT_LENGTH` at or above 64000, per Ollama's own doc, and confirm with `ollama ps`.
- **Two agents, `NUM_PARALLEL` 1.** Trigger: a second agent starts. Consequence: it waits in a 512-deep queue behind the first; the symptom is "the model is slow", not "the model is busy". Fix: raise `OLLAMA_NUM_PARALLEL`, or a batching runtime, and budget KV for both.
- **Two vLLM instances at default memory fraction.** Trigger: a second `vllm serve` on the same GPU. Consequence: the second fails to allocate. Fix: `gpu_memory_utilization` at 0.5 each, or one instance.
- **`--kv-bits` for memory, then wondering why requests serialize.** Trigger: mlx-lm with KV quantization. Consequence: batching is off by documented design. Fix: accept it, or drop `--kv-bits` and pay the memory.
- **Numeric `CUDA_VISIBLE_DEVICES`.** Trigger: a driver update or reboot. Consequence: the wrong GPU, or an invalid index that silently forces CPU. Fix: UUIDs from `nvidia-smi -L`.
- **Quantization by a 2023 table.** Trigger: a long-pretrained 2025 model at Q3. Consequence: flips and tail errors accuracy tables do not show. Fix: measure KLD on the target model.
- **MMLU deltas of half a point between quants.** Trigger: any such comparison. Consequence: a decision on noise; tokenization alone moves it 0.4. Fix: KLD, or a task-specific generation eval.
- **A Metal working set and a Docker VM that sum past RAM.** Trigger: containers and a local model on one Mac. Consequence: paging, then kills of whatever a threshold hits first. Fix: set the two ceilings together, or do not run both at once.
- **Reading `utilization.gpu` as load.** Trigger: 100% in `nvidia-smi`. Consequence: a false "saturated" or a missed real saturation. Fix: the runtime's queue-depth and KV-usage metrics.
- **Local for cost, single stream.** Trigger: "cheaper than the API". Consequence: 10x to 100x the hosted open-weight price once amortized. Fix: price it honestly; choose local for privacy, latency, or batch, and say so.
- **New work on TGI.** Trigger: a tutorial from 2024. Consequence: maintenance-mode software. Fix: its own README's list.

## Authorities

- **Georgi Gerganov and the llama.cpp contributors**, and specifically **ikawrakow** for the K-quant and i-quant work (PR #1684, discussion #5263). The primary source for GGUF quantization and for Apple performance numbers (discussion #4167).
- **Artefact2's KLD gist** for Mistral-7B: the cleanest per-quant tail data in circulation and the origin of the "fits comfortably, use more parameters" rule.
- **Ouyang et al., arXiv 2411.17691**, and **Kumar et al., arXiv 2411.04330**, on quantization damage growing with pretraining tokens.
- **Dutta et al., arXiv 2407.09141**, on flips and why accuracy hides generation damage.
- **Kurtic, Marques, Kurtz, Alistarh (Red Hat / Neural Magic, 2024-10-17)** for the Llama 3.1 recovery numbers.
- **Frantar et al., GPTQ (arXiv 2210.17323)**; **Lin et al., AWQ (arXiv 2306.00978)**; **Liu et al., KIVI (arXiv 2402.02750)**; **DeepSeek-V2 paper** for MLA's KV accounting.
- **Javat and Kazakov, Silicon Showdown (arXiv 2605.00519v2, 2026-05-04)** for M-series against RTX 5090 on energy and decode.
- **Apple's MTLDevice documentation** for `recommendedMaxWorkingSetSize`; **NVIDIA's MIG, MPS and NVML documentation**; **AMD's ROCm system requirements page**.
- **Each runtime's own docs and release notes**: llama.cpp README and `build.md`; vLLM `cache.py`, metrics and OpenAI-compat pages; SGLang README; Ollama FAQ, context-length and GPU docs; mlx-lm server docs; LM Studio blog.
- **Unsloth's Dynamic quantization docs** for the KLD-as-gold-standard argument and the tokenization-noise finding.
- **kunalganglani.com break-even post (2026-07-08)** for one honest cost model with its inputs shown.

## Severity rubric

What the levels mean in this domain specifically.

- **blocker**: a configuration that cannot work at its stated context or concurrency (KV omitted from the budget and the peak exceeds memory; two vLLM instances at 0.92; a model targeted at a GPU whose family the quant kernel does not support); a Mac whose Docker VM cap plus Metal working set exceeds RAM while both are in use; a hard-quantized code model chosen on a 2023 table for a 2025 long-pretrained model with no KLD measured.
- **major**: Ollama defaults left for an agent workload (4k context, parallel 1, global KV type); numeric device indices; `utilization.gpu` used as the saturation signal; a cost claim for local inference without amortization and with single-stream utilization; new work on a maintenance-mode runtime.
- **minor**: a quant chosen without stating the KLD tail; `iogpu.wired_limit_mb` set by hand and not persisted; an OpenAI-compat parameter relied on that the runtime documents as ignored; a version pinned without a date.
- **nit**: a fraction quoted for the Metal working set instead of the queried value; a benchmark cited without its date.
- **insight**: structural observations. "This agent pool's prompts share a 6k-token prefix; RadixAttention or vLLM prefix caching would change the economics." "This machine runs containers and models; the two memory ceilings are set by two different tools and nobody owns their sum."

## Changelog

**Source research**: `~/.claude/local/research-notes/inference-gpu.md` (claims tagged verified / found-unverified / inferred, with an explicit gaps list). Read it before a refresh: it records what was verified against a primary source, what was not, that WebSearch was unavailable for the whole pass so every web fact came from WebFetch, `gh api`, Brave results pages or local measurement, and which claims were left unverified and why.

- **2026-09-10** -- Initial version. Runtime versions verified via `gh api .../releases/latest` on the day. KV arithmetic computed from each model's `config.json`. Apple working-set figure measured on a 64 GiB M1 Max (81%), with the community two-thirds/three-quarters rule left unverified. Quantization claims verified against llama.cpp PR #1684, discussion #5263, Artefact2's gist, the Red Hat post, and arXiv 2210.17323, 2306.00978, 2402.02750, 2407.09141, 2411.17691, 2411.04330. MIG, MPS, time-slicing and NVML definitions verified against NVIDIA docs; ROCm matrix against AMD's page. Cost inputs from kunalganglani.com and Javat and Kazakov; arithmetic inferred. Known gaps: no Apple primary source for the default wired fraction; MPS on GeForce unverified; LM Studio's ignored parameters not found; frontier SWE-bench figure from an aggregator; EXL3-vs-GGUF quality unread; Ollama concurrency-collapse claims secondary and unverified.
