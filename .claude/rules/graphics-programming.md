---
paths:
  - "**/*.{wgsl,glsl,hlsl,msl,vert,frag,comp}"
  - "**/*.{metal,shader}"
---

# Graphics Programming

A reference for evaluating graphics code from a correctness, performance, portability, and modern-best-practice lens. Used by the `graphics-programming` subagent.

The scope: **WebGL / WebGL2 / WebGPU especially**, but extending to low-level graphics rendering, 2D vector graphics (Loop-Blinn, signed distance fields, compute-shader rasterization), text shaping and rendering (HarfBuzz, FreeType, SDF / MSDF / Slug / Vello), modern shader practices (WGSL, GLSL ES 3.0, HLSL, MSL, compute shaders, subgroup intrinsics, mesh shaders), and GPU best practices (frame graphs, GPU-driven rendering, tile-based architectures, bind-group design, texture compression).

Distinct from:
- **`rust-wasm`**: Rust-to-WASM specifically. We touch WGSL / wgpu but not Rust-WASM compilation.
- **`performance`**: general performance. We flag GPU-specific perf (bandwidth, fillrate, overdraw, sync-stalls) that the general agent doesn't have the depth for.
- **`accessibility`**: UI a11y. We touch font rendering quality but not screen-reader concerns.

The core thesis: **graphics code lives at the intersection of three failure modes** -- correctness (the GPU produces what you specified, but you specified the wrong thing), performance (the GPU spends its budget on the wrong work), and portability (the code works on your GPU and fails on the reviewer's). The reviewer's lens is "what about this code will break on a different GPU, fail to scale to the target frame budget, or render incorrectly under realistic content load?"

The empirical observation: **most graphics bugs are not algorithmic; they are state-management or boundary-cost bugs.** Per-draw bind-group creation, sync `mapAsync` on the hot path, missing context-loss handling, hand-rolled text shaping where HarfBuzz exists -- these compound. The strongest graphics codebases share architectural traits (render graphs, GPU-driven rendering, async pipeline creation, explicit destroy) more than they share specific algorithms.

---

## Universal principles

### State changes have ordered cost

GPU drivers do extensive work on every pipeline / shader / bind-group / vertex-buffer change. Cost ordering (high to low):

1. **Render pass change** -- highest; flushes tile state on mobile.
2. **Pipeline (shader + state) change** -- high; shader switch, possible recompile.
3. **Bind group / descriptor set change** -- medium.
4. **Vertex/index buffer change** -- low.
5. **Push constants / uniform updates within bind group** -- lowest.

**Idiomatic batching**: sort draws by (render pass → pipeline → bind group → vertex buffer) to minimize state changes. Modern engines do this via render graph machinery.

**Flag**: per-draw bind-group creation; pipeline creation in the render loop; render-pass churn (multiple short passes when one long pass would do the same work).

### GPU resources need explicit lifetime management

WebGPU resources (`GPUBuffer`, `GPUTexture`, `GPUBindGroup`) require `destroy()` to free GPU memory. JS GC will eventually finalize them, but on a non-deterministic schedule that doesn't match the GPU's lifecycle. Pipelines have a similar story.

**Flag**: missing `destroy()` calls; reliance on JS GC for GPU resource lifecycle; pooled resources that aren't actually returned to the pool.

### The JS↔GPU boundary is expensive; minimize crossings

Each WebGL/WebGPU call from JS has setup cost (argument marshalling, validation, possible cross-process IPC on browsers with the GPU process). Batch where possible: instancing, indirect draws, UBO updates instead of per-uniform writes, bulk buffer writes.

**Flag**: dozens of `gl.uniform*` calls per draw when a UBO would batch; per-draw `device.queue.writeBuffer` for state that's stable across many draws.

### Synchronous GPU operations kill frame rate

`glReadPixels`, blocking `mapAsync`, blocking `getQueryObject` force CPU-GPU synchronization. The CPU waits for the GPU to finish all in-flight work; the GPU pipeline stalls; frame budget is destroyed.

**Flag**: any sync GPU read in the render loop; `gpuBuffer.mapAsync` without proper async flow; `gl.getError()` after every call in production.

### Context loss / device loss is real and routine

WebGL: `webglcontextlost` event fires when the GPU is lost (driver crash, tab background, system sleep). All GPU state must be re-created. WebGPU: `device.lost` promise; similar contract.

**Flag**: graphics code without `webglcontextlost` / `webglcontextrestored` handlers; no `device.lost` promise handling; assumption that the GPU device is permanent.

---

## WebGL / WebGL2 / WebGPU specifics

### WebGL 1.0 (2011, deprecated for new code)

- Subset of OpenGL ES 2.0.
- Programmable shaders only (no fixed function).
- No 3D textures, MRT, compute, instancing core; extensions only.
- Extension-based for anything modern.

**Flag for new code**: targeting WebGL 1.0 without a documented reason; using WebGL 1.0 with extensions when WebGL 2.0 has the feature core.

### WebGL 2.0 (2017, the effective baseline)

- Based on OpenGL ES 3.0.
- 3D textures, MRT, instancing, UBOs, transform feedback, vertex array objects all core.
- **No compute shaders** (the major absence).
- **No geometry shaders, no tessellation** (deliberately).
- Safari shipped 2021; 100% effective adoption since.

**Flag**: WebGL 1.0 idioms (texture-unit churn, per-draw uniform updates) in WebGL 2.0 code; using extensions for features that became core (instancing, VAOs).

### WebGPU (stable 2023, rolling out)

- Modern explicit-GPU API. Mental model closer to Vulkan / Metal / D3D12 than to GL.
- **Bind groups** + **pipeline state objects** (PSOs).
- **Compute shaders first-class**.
- **WGSL shader language** (replacing GLSL on the web).
- **Adapter-and-device model**, separate from canvas context.
- **Async by default**: pipeline creation, buffer mapping, queue submission.
- Chrome 113+ (May 2023), Safari 18+ (2024), Firefox 2025.
- Worker thread support; SharedArrayBuffer not required for WebGPU itself.

**Flag**: WebGL idioms (global state, draw-time state changes) ported wholesale to WebGPU; sync `mapAsync` waits; pipeline creation in the render loop; missing `destroy()` calls.

### ANGLE: the underappreciated layer

ANGLE (the "Almost Native Graphics Layer Engine") translates WebGL / WebGPU calls to the platform's native API (D3D11 / D3D12 / Metal / Vulkan). Chrome ships it; same code, different backends. Behavior can differ subtly between backends; bug reports often need OS + backend specifics.

**Flag**: assumptions about driver-level behavior that only hold for one ANGLE backend.

---

## 2D vector graphics on the GPU

The hard problem: GPUs are designed for triangles; vector graphics are curves and stencil-fills.

**Approach 1: CPU tessellation + GPU triangles** (lyon, Skia legacy). Tessellate Bézier curves to triangle fans on the CPU; render. Robust; works on any GPU; CPU-bound for complex paths.

**Approach 2: Loop-Blinn (2005)**. Implicit Bézier representation; triangle around the curve has special interpolated coordinates; fragment shader evaluates implicit equation and discards. Resolution-independent. GPU-side; CPU prep is minimal.

**Approach 3: Stencil-then-cover (Kilgard, NV_path_rendering, Skia GPU)**. Render path silhouette into stencil with fill rule; cover the bounding region. Two passes. Resolution-independent.

**Approach 4: Tile-based with analytic coverage (Pathfinder, Walton)**. Tessellate to triangles with coverage analysis at edges; analytic anti-aliasing.

**Approach 5: Compute-shader sort-middle (Vello, piet-gpu, Levien)**. Encode scene to GPU memory; compute-shader passes rasterize. The modern approach where WebGPU is available; high quality at any scale.

**Flag**: 2025 graphics code with CPU tessellation for content that should use compute (when WebGPU is available); hand-rolled Bézier rasterization when established libraries exist; missing strategy selection based on GPU capabilities.

---

## Text shaping and rendering

Two stages: **shaping** (text → positioned glyphs) and **rendering** (glyphs → pixels).

### Shaping

**HarfBuzz** is the standard. Complex scripts (Arabic, Devanagari, Tamil), OpenType features (ligatures, contextual alternates), font fallback. Apple's CoreText and Microsoft's DirectWrite are platform alternatives.

**Flag**: hand-rolled text shaping; "we just split on spaces and look up codepoints" approaches; missing fallback chain for unsupported codepoints.

### Rendering

**Atlas-based**: pre-rasterize glyphs to a texture atlas; sample. Quality degrades at non-1:1 scales.

**SDF (Valve, 2007)**: single resolution-independent atlas; sample SDF; threshold for sharp edges. The practical baseline for 3D / UI text at scale. Degrades at very small sizes.

**MSDF (Chlumský 2015)**: multi-channel SDF; better corner preservation than plain SDF.

**Slug (Lengyel 2017)**: direct curve evaluation per pixel. High quality, more shader work.

**Vello / piet-gpu**: compute-shader-driven; high quality at any scale on WebGPU.

**Flag**: atlas-based plain texture text for resolution-independent UI (use SDF or MSDF); per-glyph draw calls instead of batched instanced quads; subpixel AA on modern 2x+ DPI displays (usually counterproductive); atlas overflow with no eviction policy.

---

## Modern shader practices

### WGSL displacing GLSL on the web

WGSL is the only shader language WebGPU accepts directly (browsers won't accept SPIR-V from web code for security reasons). Tooling exists (tint, naga) for GLSL→WGSL and HLSL→WGSL.

**Flag**: GLSL in WebGPU contexts without translation pipeline; assumptions that GLSL semantics carry to WGSL (some don't, especially around `vec3` alignment in storage buffers -- always 16-byte aligned).

### Compute shaders as the new normal

Critical for modern 2D vector rendering (Vello), particle systems, image processing, neural inference (WebGPU + ONNX Runtime). Not available in WebGL2; the major reason for moving to WebGPU.

**Flag**: rolling-rendering vector paths or particle effects without compute when WebGPU is available; missing compute shader feature detection with fallback.

### Subgroup / wave intrinsics

Reductions, scans, prefix sums, ballot, shuffle. Performance-critical for some algorithms. WebGPU has limited subgroup support (proposal); native APIs (Vulkan, Metal, D3D12) have full support.

### Mesh shaders / task shaders

Replace vertex + tessellation + geometry stages with a compute-flavored model. D3D12 Ultimate, Vulkan 1.3 (2022). **Not in WebGPU yet.** Required for Nanite-style virtualized geometry.

### Bind-group / descriptor set design

The single most consequential decision in WebGPU code. The pattern: **separate bind groups by update frequency**:
- Group 0: per-frame (camera, lights).
- Group 1: per-material (textures, material properties).
- Group 2: per-draw (object transform).

Mismatched layout = perf cliff (recreate bind group every draw).

**Flag**: bind groups mixing per-frame and per-draw data; per-draw bind-group creation in hot paths; bind-group layout mismatch with shader expectations.

### Shader pitfalls

- **Warp-divergent branching**: GPUs execute in warps (32) / wavefronts (64) threads in lockstep. Divergent branches serialize both paths. Prefer `mix()`, `step()`, branchless math.
- **Texture fetches with computed coordinates** that break cache locality (neighboring threads should access neighboring texels).
- **Per-fragment derivatives in divergent control flow**: `dFdx` / `dFdy` undefined under non-uniform execution.
- **Unnecessary `discard`**: disables early-Z; forces late-Z; throughput cost.
- **High register pressure**: limits occupancy; profiler shows it; 100+ temporaries per shader vs 32 affects warp count in flight.
- **Atomic on hot memory**: contention serializes warps. Privatize (per-workgroup atomic, then merge).
- **WGSL `vec3` alignment trap**: 12 bytes but 16-byte aligned in storage buffers. Pad explicitly or use `vec4`.

---

## GPU best practices

### Frame graphs / render graphs

The modern engine pattern. Each pass declares inputs / outputs; framework topologically sorts, allocates transient resources (alias GPU memory between non-overlapping passes), inserts barriers. Yuriy O'Donnell's Frostbite talk (GDC 2017) popularized; standard in Unreal RDG, Frostbite, Unity HDRP/SRP.

**Flag for non-trivial renderers**: manual pass ordering and barrier management; no render-graph abstraction; barrier bugs in Vulkan / D3D12 code.

### GPU-driven rendering

Build draw lists on the GPU via compute shaders; dispatch via indirect-draw commands. Avoids CPU-GPU round-trips for culling, LOD, draw enumeration. Frostbite, Unreal Nanite, Vello all use this.

**Flag for 2025 graphics code**: per-object CPU iteration for culling / LOD / draw-list construction when GPU-driven would scale better.

### Tile-based GPUs (mobile, Apple Silicon)

Mali / Adreno / Apple GPU are tile-based deferred renderers: framebuffer divided into tiles, rendered in on-chip memory, only final result written to main memory. **Per-tile bandwidth dominates.**

- **`loadOp` and `storeOp` matter**: `'clear'` (no load) and `'discard'` (no store) save bandwidth.
- **Multi-pass within one render pass is cheap on tile-based** (data stays on-chip); splitting forces tile flush.
- **MSAA is essentially free on tile-based** if resolved in the same render pass.
- **G-buffer deferred is expensive on mobile** (read-back across passes). **Forward+** or clustered shading preferred.

**Flag**: render pass design ignoring tile-based concerns; G-buffer + read patterns where forward+ would be cheaper; missing `loadOp: 'clear'` / `storeOp: 'discard'` where they'd save bandwidth.

### Texture compression

- **BC (DXT/S3TC)**: desktop GPUs.
- **ETC2 / EAC**: Android (mandatory in GL ES 3.0).
- **ASTC**: modern mobile (Apple, Mali, Adreno). Best quality/bitrate trade.
- **Basis Universal**: supercompressed; runtime-transcodes to native target. Solves the cross-platform format-fragmentation problem.

**Flag**: textures stored uncompressed for content that should be compressed; using BC on mobile (unsupported); using PVRTC on new code; missing Basis Universal for cross-platform pipelines.

### Validation layers

- **Vulkan validation layers**: mandatory in development.
- **D3D12 debug layer**: `D3D12GetDebugInterface`.
- **WebGPU**: validation always on; errors on invalid use.
- **WebGL**: `WEBGL_debug_renderer_info`, `WEBGL_debug_shaders`.

**Flag**: production-ready graphics code with no debug-build path enabling validation; suppressed validation warnings.

---

## Anti-pattern catalog

### Any GPU code
- **Sync GPU reads on the hot path** (`glReadPixels`, blocking `mapAsync`, blocking `getQueryObject`).
- **GPU resource creation in the render loop** (pipeline, texture, buffer, bind-group).
- **No frame-time monitoring** in development.
- **Hardcoded shader paths** (should be feature-detected).
- **Naive transparency** (no back-to-front sort, no OIT strategy).
- **Premature shader micro-optimization** (most wins at pipeline / batching level).
- **Driver-dependent behavior** (testing on one GPU only).

### WebGL-specific
- **No `webglcontextlost` / `webglcontextrestored` handlers**.
- **Texture-unit churn** (per-draw rebind).
- **Per-draw `gl.uniform*` calls** when UBO would batch.
- **Shader compilation in the render loop**.
- **`gl.getError()` after every call** in production.

### WebGPU-specific
- **Missing `destroy()` on resources**.
- **Pipeline creation in the render loop** (use `createRenderPipelineAsync`, pre-warm at load).
- **Per-draw bind-group creation**.
- **Sync `mapAsync` waits**.
- **Missing `device.lost` promise handling**.
- **Misconfigured `loadOp` / `storeOp`** on mobile.
- **No worker thread for non-trivial renderers** (main thread blocks input).

### 2D vector / text
- **CPU tessellation for content that should use GPU compute** (when WebGPU is available).
- **Hand-rolled text shaper** instead of HarfBuzz.
- **Plain atlas-based text** for resolution-independent UI.
- **Per-glyph draw calls**.
- **Subpixel AA on 2x+ DPI displays**.
- **Atlas overflow with no eviction policy**.

### Shader-level
- **Warp-divergent branching on per-pixel data**.
- **Texture fetches with cache-unfriendly coordinates**.
- **Derivatives in divergent control flow**.
- **Unnecessary `discard`**.
- **High register pressure** (occupancy collapse).
- **Atomic operations on hot memory** without privatization.
- **`vec3` storage alignment** bugs in WGSL / SPIR-V.

---

## Modern shifts

- **WebGPU shipping** (2023-2025): the single largest API transition since WebGL.
- **WGSL displacing GLSL on the web**.
- **Compute-shader 2D rendering** (Vello, piet-gpu) maturing.
- **Mesh shaders** in D3D12 / Vulkan; not WebGPU yet.
- **Ray tracing** in D3D12 / Vulkan / Metal; not WebGPU yet.
- **Temporal upscaling** (DLSS / FSR / XeSS / MetalFX) as standard.
- **GPU-driven rendering at scale** (Nanite, Frostbite).
- **AI / ML on the GPU**: WebGPU + ONNX Runtime, MediaPipe.
- **Bindless** in Vulkan / D3D12; deliberately absent from WebGPU.

---

## Schools of thought (preserve disagreement)

- **Forward vs deferred vs forward+ vs visibility buffer**: depends on hardware target (mobile / desktop / next-gen high-end) and content profile.
- **MSAA vs FXAA vs TAA vs SMAA**: TAA dominant but ghosting complaint is real.
- **Static vs dynamic shader compilation**: WebGPU forces dynamic; native can ship precompiled.
- **WebGPU's bind-group model vs bindless**: deliberately conservative; some argue performance cost is real.
- **ECS vs scene-graph for renderers**: Bevy / Flecs camp vs Three.js camp.
- **Frame graph vs ad-hoc pass management**: boundary depends on renderer scale.

---

## What is NOT a graphics finding

- **Game design / level design / art direction**: out of scope.
- **General performance** that isn't GPU-specific (route to `performance`).
- **Code-style / readability** unless graphics-specific (route to `readability`).
- **WASM compilation** for graphics work (route to `webassembly` or `rust-wasm`).
- **Audio + graphics combined**: graphics agent owns graphics side; audio agent owns audio side.
- **CSS / DOM rendering**: that's browser internals, not GPU programming.

---

## Severity calibration

Per `panel-contract.md`'s rubric:

- **blocker**: sync GPU read in the render loop (frame-rate killer); missing context-loss handling (production crashes); per-draw pipeline creation; data corruption from bind-group layout mismatch; non-portable code that fails on a major GPU vendor.
- **major**: bind-group churn under load; texture-unit churn in WebGL2; missing `destroy()` causing memory growth; G-buffer deferred on mobile; per-glyph draw calls; warp-divergent branching in heavy shaders; atlas overflow with no eviction.
- **minor**: `vec3` alignment without padding; missing `loadOp: 'clear'`; suboptimal texture compression choice; missing feature detection.
- **nit**: style choices in shader code (variable naming, ordering); minor uniform-update inefficiency.
- **insight**: structural -- "this renderer would benefit from a render graph at this scale"; "consider Vello / compute-shader 2D rendering"; "MSDF would solve the text-scaling concern."

Confidence: high when the bug shape is concrete (specific call site, specific GPU behavior); medium when reasoned from architecture (the agent infers from one shader that the convention isn't enforced).

---

## Process for the graphics-programming agent

1. **Identify the surface(s).** WebGL? WebGL2? WebGPU? Native (Vulkan / Metal / D3D12 via wgpu / bgfx)? Mobile? Multiple?
2. **Read project graphics conventions.** `docs/rendering.md`, `CLAUDE.md` graphics sections, the shader pipeline, the texture pipeline.
3. **Walk the bug-shape catalog**: state management, resource lifetime, JS↔GPU boundary, sync operations, context loss, bind-group design.
4. **Walk the shader code**: warp divergence, cache patterns, derivative correctness, alignment.
5. **Walk the 2D / text rendering** if present: shaping (HarfBuzz?), rendering approach (atlas / SDF / MSDF / Slug / compute), batching.
6. **Walk the architecture**: render graph? GPU-driven? Frame-time monitoring? Worker thread?
7. **Walk per-platform**: mobile tile-based concerns; ANGLE backend variance; WebGPU adapter capabilities.
8. **Route to other lenses**: general perf → `performance`; UI a11y → `accessibility`; WASM compilation → `webassembly`.
9. **Stay read-only.**
