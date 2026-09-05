---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-05
---

# AI and 3D tool integration

A reference for reviewing systems where a model drives a visual creative tool: Model Context Protocol (MCP) servers for digital content creation (DCC) applications, model-written `bpy` and equivalent scripting, the perception loop, generated-asset provenance, and approval gating on destructive operations. Used by the `ai-3d-integration` subagent and the `/expert-review` / `/expert-plan` / `/expert-consult` skills.

The unifying thesis: **an agent driving a visual tool is blind, and almost every failure mode in this domain is silent.** A coding agent has a cheap, fast, deterministic oracle -- the compiler, the type checker, the test suite -- that runs in seconds and says precisely what is wrong. A 3D agent has a success boolean that only means "finished or cancelled," a scene graph that answers only the questions you thought to ask, and a render that is expensive, slow, and interpreted by a vision model with its own error rate. **There is no compiler for "looks right."**

The operational question: **"what does this system actually verify, versus what does it assume because the model said so?"**

Empirical priority, in rough order of how often it bites: **silent failure and missing verification > destructive operations without a recovery boundary > trust boundary and injection surface > perception-loop cost > provenance and licensing > model capability limits.**

## Volatile surface

`last-verified: 2026-09-05`. **This is the fastest-rotting file in the reference set.** The MCP specification has had five revisions in under two years, the most recent of which removed primitives that existing servers depend on. Treat every specific claim below as a dated snapshot.

| Claim class | Rots | Re-verify at |
|---|---|---|
| MCP spec revision, primitives, transports | **Very fast** (months) | modelcontextprotocol.io/specification/latest and its changelog |
| Specific MCP server capabilities and security posture | Very fast | The repo itself, its README and source |
| AI 3D generation model quality, licensing, availability | Very fast | Each model's card and LICENSE |
| Legal and regulatory status of generated assets | Fast, jurisdiction-dependent | Primary court and agency sources |
| Platform AI-disclosure policies | Fast | Each platform's own policy page |
| Blender Python API behavior | Medium | Blender's Python API reference for the target version |
| The perception-loop reasoning and failure taxonomy | Slow | Durable; it follows from the structure |

**As of 2026-09-05** the current MCP specification revision is **`2026-07-28`**, and it is a re-architecture rather than a point release. Revision lineage: `2024-11-05`, `2025-03-26` (Streamable HTTP introduced, HTTP+SSE deprecated), `2025-06-18` (elicitation, structured tool output), `2025-11-25` (experimental tasks), `2026-07-28` (current).

## MCP as it stands, and what changed

The `2026-07-28` revision matters disproportionately for creative-tool servers because it removed or deprecated several primitives those servers leaned on.

**MCP is now stateless.** The `initialize` / `notifications/initialized` handshake was removed, and protocol-level sessions are gone -- the `Mcp-Session-Id` header no longer exists in Streamable HTTP, and `tools/list`, `resources/list`, and `prompts/list` no longer vary per connection. Every request carries its protocol version and client capabilities in `_meta`. **Servers needing cross-call state must mint explicit handles and pass them as ordinary tool arguments.** For a DCC server tracking scene state, this is a spec-level endorsement of "return a handle, not a session."

**Servers must implement `server/discover`**, advertising supported protocol versions, capabilities, and identity.

**`subscriptions/listen` replaces the HTTP GET endpoint and `resources/subscribe`.** One long-lived POST-response stream, with clients opting into specific notification types. Note the exception: **request-scoped notifications such as `notifications/progress` still flow on the response stream of the request they relate to**, not on the subscription stream. That is the mechanism for reporting progress on a long render.

**Stream resumability was removed.** `Last-Event-ID` and SSE event IDs are gone. **A broken response stream loses the in-flight request entirely, and the retry is a fresh request with a new id.** For a four-minute render streamed as one response, a broken stream means the whole render is lost and redone. This pushes genuinely long operations toward the Tasks extension (`io.modelcontextprotocol/tasks`, now an official extension rather than core, polled via `tasks/get`).

**Roots, Sampling, and Logging are all deprecated.** The spec's own suggested migrations: pass directories via tool parameters or resource URIs instead of Roots; integrate directly with a provider API instead of Sampling; log to stderr or use OpenTelemetry instead of Logging. **Sampling's deprecation matters specifically here** -- it was the primitive that let a server ask the client's model to do inference, which is exactly what a perception loop wants. A server building a perception loop now brings its own model credentials, which changes its trust and cost profile.

**Multi Round-Trip Requests (MRTR)** replace server-initiated requests. A server returns an `InputRequiredResult` with `inputRequests`, and the client responds with `inputResponses` on a retry of the original request. All results now carry a required `resultType` of `"complete"` or `"input_required"`.

## The trust boundary

The specification states its own position plainly: MCP "enables powerful capabilities through arbitrary data access and code execution paths," tools "represent arbitrary code execution and must be treated with appropriate caution," and tool descriptions are **untrusted** unless the server is trusted. Critically, it also concedes that **MCP cannot enforce these principles at the protocol level** -- everything is a SHOULD on implementors, and hosts vary.

The attack classes that matter for creative tooling:

- **Prompt injection through tool results.** This is not hypothetical here. Asset marketplaces return attacker-influenceable free text: model titles, descriptions, author bios, tags. A server that fetches asset metadata and returns it verbatim is piping untrusted instructions into the model's context.
- **Tool poisoning.** Malicious instructions in the tool *description*, which the model reads and the user usually never sees.
- **Rug pull.** Benign tools at approval time, changed afterward. Worse now that list results are cacheable with a TTL, making cache invalidation a security-relevant event.
- **Cross-server shadowing.** Server A's tool descriptions influencing how the model uses server B's tools, including exfiltrating B's data through A.
- **Confused deputy.** The server holds credentials -- an asset service key, a cloud render account -- and acts for whoever asks.
- **The lethal trifecta.** Private data access, exposure to untrusted content, and outbound network capability. A creative agent setup hits all three routinely: local project files, marketplace text, and an asset downloader.

**Arbitrary code execution is the default posture of the DCC-server category.** A server that accepts Python and executes it inside the application has, by construction, full host access: `os`, `subprocess`, network, filesystem. It is not sandboxed by Blender. This is the source of the category's power and is not a bug to be fixed so much as a property to be gated.

## Why model-written `bpy` fails, specifically

Blender's own Python documentation supplies most of this, which makes it citable rather than speculative.

**Undo does not cross the boundary.** The API documentation says to "assume that undo and redo always invalidates all `bpy.types.ID` instances" and demonstrates it with pointer hashes changing across an undo. There is a modern nuance -- the current undo system does not systematically invalidate every pointer, and unchanged data may keep valid pointers -- but the docs attach an explicit warning that there is "no guarantee of any kind that it will be safe and consistent." **That is worse than no guarantee: it makes the bug intermittent by design.** Most of the time the stale reference happens to still be valid. Occasionally it is not, and Blender segfaults.

The agent-level consequence: **there is no transaction and no "undo the last agent turn."** A user's Ctrl+Z returns one Blender undo step, at whatever granularity the script happened to produce -- often the whole script, sometimes nothing if the script manipulated `bpy.data` directly. A five-step sequence whose fourth step was wrong cannot be rolled back to the third. **The only real recovery boundary is the saved `.blend` file.** Approval gating has to be built at the agent layer; the application's undo will not do it.

**Operators are context-dependent and diagnose badly.** `bpy.ops` cannot be passed the data to operate on -- it reads the context: active object, selection, mode, editor type. Its return value indicates only that it finished or cancelled. The canonical error is `RuntimeError: Operator bpy.ops.action.clean.poll() failed, context is incorrect`, and Blender's own documentation admits the only way to learn what context it wanted is to read the C source for the poll function, noting that "downloading and searching the C code isn't so simple." Some operators work only from specific editors.

**This is the dominant failure mode for model-written Blender code, and the reason is structural.** Training data is saturated with `bpy.ops` calls, because that is what Blender's "copy as Python" feature emits and therefore what tutorials and answers contain. But `bpy.ops` is the *UI* layer. In a headless or agent-driven run, the context is not what the model assumes, so `bpy.ops.object.modifier_add(type='SUBSURF')` applies to whatever is active -- possibly a different object than the one just created, possibly nothing. **The robust idiom is the data API (`bpy.data.objects["X"].modifiers.new(...)`), which takes an explicit target and cannot silently address the wrong thing. Steering a model from `bpy.ops` toward `bpy.data` is the highest-value single instruction to put in a Blender tool description or system prompt.**

**Names are not what you asked for.** The documentation states directly that "a common mistake is to assume newly created data is given the requested name," and that names may differ if they exceed the maximum length, are already in use, or are empty. So a script that creates `Chair` and later looks up `bpy.data.objects["Chair"]` may silently be addressing a *pre-existing* object while its own creation sits at `Chair.001`. **Hold the reference the creation call returned; never re-look-up by name.**

**The depsgraph is stale until told otherwise.** Reading evaluated geometry without updating the dependency graph returns a plausible, wrong number -- no error.

**Mode switching and container mutation invalidate references.** The API documentation warns that internal containers may reallocate, and that holding a reference across such an operation is likely to crash.

**Scripts block the application.** While a script runs, Blender does not redraw or respond. A render triggered from a script blocks.

The honest risk table for model-written `bpy`, by whether the failure announces itself:

| Risk | Reports an error? | Recoverable? |
|---|---|---|
| Arbitrary host code execution | No -- it just works | No |
| Overwrites the user's `.blend` via `save_mainfile` | No | Only from a backup |
| Name collision silently modifies a pre-existing object | No | Hard; damage is in the file |
| Wrong `bpy.ops` context | Yes -- poll failure | Yes, but diagnosis needs the C source |
| Stale depsgraph read | No -- returns a plausible wrong number | Yes, if you know to update |
| Stale reference after mutation, mode switch, or undo | **Segfault** | No; unsaved work lost |
| Argument-order error in a headless render | No -- exit code 0, wrong output path | Yes, if noticed |
| Model believes it made a chair, made a cube | No | Yes, with verification |

**Six of eight fail silently.** That distribution is the entire argument for treating verification as the system's central design problem rather than a nicety.

## The perception loop

**Render-and-look-back** converts the visual modality into tokens: render the viewport or camera, encode the image, feed it back, let a vision model judge. The costs are real and structural.

**Token cost.** For Claude models the documented approximation is roughly `(width x height) / 750` tokens. A 1024x1024 screenshot is about 1,400 tokens; a 1568x1568 image about 3,300. Multiply by a multi-view check and by every iteration of a refinement loop -- twenty iterations at four views is over 100k tokens spent on looking, mostly at nearly identical images. It also **breaks prompt caching**, since each turn appends new image bytes.

**Latency.** A viewport OpenGL or workbench render is sub-second to a few seconds; a real Cycles render is seconds to minutes. **The practical pattern is a two-tier oracle: cheap viewport renders inside the iteration loop, a real render only at a checkpoint. Using a path tracer inside a refinement loop is the most common cost mistake in agentic 3D.**

**A single view is systematically insufficient, and the reason is geometric rather than perceptual.** A front orthographic view cannot distinguish a cube from a prism of any depth. One view cannot detect interpenetration, floating objects, or objects hidden inside others. Perspective makes a small near object and a large far object identical. **Nothing about scale is recoverable from an image with no reference object** -- a chair rendered alone is the same image at 0.5 m and 5 m, which is why scale errors are among the most common and most consequential agentic 3D failures: they are invisible in exactly the check people perform.

The standard mitigation is an orthographic triptych plus a three-quarter perspective view -- the traditional model sheet, for the same reason. Cost: four times the tokens and render time. And even that does not reveal scale, normal direction, UV correctness, material assignment, or topology, none of which are visible in a render at all.

### The "it said chair, it made a cube" failure class

"Hallucination" is the wrong diagnosis and leads to the wrong fix. Four distinct mechanisms with different remedies:

1. **The narration/action gap.** The model writes prose describing its intent in the same turn as code that does less. The prose is generation, not a report of the tool's return value, and nothing in the loop checks that they agree. Remedy: never trust narration; require the model to state the value it *read back*.
2. **Silent partial failure.** Statement 5 of 12 cancelled; statements 6 through 12 operated on the wrong thing. `bpy.ops` returning `{'CANCELLED'}` rather than raising means execution continues. Remedy: check operator returns; prefer `bpy.data`.
3. **Name aliasing.** The script created `Chair.001` but every later reference resolved to a pre-existing `Chair`. Remedy: hold references or an explicit id map.
4. **Genuine capability gap.** The model writes valid `bpy` but lacks the spatial model to place four legs at the corners of a seat at the right height and orientation. **No amount of verification fixes this** -- verification only converts a silent failure into a visible one. Distinguishing this from the first three matters, because they are engineering problems with known fixes and this is a limit to design around, typically with parametric templates the model fills in rather than geometry it constructs from scratch.

**A fifth, subtler one: the vision model confirming the text model's claim.** If you render the result and ask "is this a chair?", the check is performed by a model that has just been told in its own context that a chair was created. That is not an independent oracle; it is anchored. **Asking "what object is shown in this image?" with the prior claim stripped from context is a meaningfully better check, and almost nobody does it.**

### Programmatic verification dominates

For anything checkable, deterministic validation strictly beats visual inspection: milliseconds rather than seconds, tens of tokens rather than thousands, a number rather than an impression, and -- critically -- **it does not require a model to be honest about what it sees.**

Cheap assertions worth making on a Blender scene: object existence and count by name; bounding-box dimensions and world-space position against expected values; polygon, vertex, and triangle counts against budget; material slot count and that every polygon's material index resolves; UV layer presence and count; the absence of negative scale (which flips normals on export); modifier stack contents; armature bone count and influence counts per vertex; the absence of loose or non-manifold geometry.

**Visual inspection is genuinely irreplaceable only for aesthetic judgment** -- proportion, composition, whether the thing reads as what it is meant to be. Everything structural should be an assertion. A system that renders to check whether an object exists has chosen the expensive, unreliable oracle for a question the cheap, reliable one answers.

## Generated assets: quality, provenance, licensing

**The AI 3D generation licensing landscape is genuinely split**, and it is not the OSI-versus-proprietary split people assume. Some models ship permissive licences (TRELLIS and TripoSR are MIT); others ship non-OSI usage-restricted licences (Hunyuan3D-2 and Stable Fast 3D). **Read the actual licence file rather than trusting a repository badge.**

**On quality: neither Hunyuan3D-2 nor TRELLIS claims topology or UV parity with hand-authored assets -- they benchmark against other AI tools.** That is the honest state. Generated meshes typically need full retopology for anything that deforms, and their UVs are rarely production-usable. They are useful for greybox, background dressing, and reference; treating them as final game-ready assets is where projects get hurt.

**The Gaussian-splatting licensing trap is the sharpest one in this space.** The original Inria/MPII 3D Gaussian Splatting reference implementation carries a **non-commercial research licence**, and it propagates through shared codebase lineage to mesh-extraction tools built on it (SuGaR, 2DGS). Attribution does not cure it. Clean alternatives exist: Niantic's `.spz` (MIT) and `gsplat` (Apache-2.0), with `KHR_gaussian_splatting` now ratified as vendor-neutral interchange. Splats currently substitute for photogrammetry on static capture; animated and rigged splats remain an open research problem with no production pipeline.

**Legal status of generated output is fast-moving, jurisdiction-dependent, and not legal advice.** The load-bearing points for engineering decisions: US copyright protection requires human authorship, which affects whether generated output is protectable at all; multiple training-data suits are in varying postures; the EU AI Act imposes transparency and training-data-summary obligations on a timeline; and platforms impose their own disclosure rules -- **Valve requires disclosure on the storefront page for both pre-generated AI assets and live in-game generation**. Vendor indemnification offers exist and cover far less than they appear to. A studio shipping AI-assisted assets needs a provenance record and a counsel review, not a reference file's opinion.

## Agent workflows for creative work

**Approval gates belong at the agent layer.** As established above, the application's undo is not a recovery boundary. The gate list that matters: writing or overwriting a file, deleting scene data, any operation on pre-existing objects the agent did not create, and anything with a network side effect.

**Reproducibility means capturing provenance**: the prompt, the seed, the model version, and the tool versions. Without them a generated asset cannot be regenerated, varied, or defended. This is also the record a licensing review needs.

**The diffable-artifact argument.** Creative agent work should produce a script, a node graph, or a parameter set -- something reviewable and re-runnable -- rather than an opaque binary. A committed generation script can be read, diffed, corrected, and re-run when the tool updates. A committed `.blend` produced by an unrecorded chat session cannot. This is the strongest single structural recommendation in the domain, and it also happens to solve the provenance problem.

**Evaluating creative output** has no unit test for "looks good." The available proxies: structural assertions (above), regression against a golden render at fixed seed and settings, human spot-check at a defined sampling rate, and downstream validation (does it import, does it pass the engine's validator). Treat "the model said it looks good" as unmeasured.

## Schools of thought

### Is agentic creative tooling production-ready?

**The ready position.** The tools exist, they work, and the productivity difference on greybox, boilerplate scene setup, batch operations, and asset variation is large and immediate. Waiting for perfect verification is waiting forever; the correct response to silent failure is to build the verification layer, which is ordinary engineering. Every objection below is an argument for better scaffolding, not for abstention.

**The not-ready position.** Six of eight known failure modes are silent, the recovery boundary is a manually saved file, the perception oracle is anchored and expensive, and the capability gap on genuine spatial reasoning is not closing on the same curve as code generation. A tool whose failures are invisible and whose damage is unrecoverable is not a tool an artist can trust in a production file. The demos succeed because they start from an empty scene; the failures happen in a scene with existing work in it.

### AI in the creative process: labor and consent

**The pro-adoption position.** These are tools, and artists have always adopted tools that automate the tedious parts. Ideation, variation, and drudgery are exactly where they help; a small team gains capability it could not otherwise afford, and refusing them on principle costs the artist, not the vendor.

**The critical position.** The models were trained on working artists' output without consent, licence, or compensation, and are now sold as substitutes for the labour they were trained on. The harm is not to a craft abstraction but to identifiable people whose portfolios are in the training set. Adoption normalizes the appropriation regardless of individual intent, and "it is just a tool" is a category error about where the value came from.

**Both are recorded because both identify something real.** Reconciling them into a moderate position loses the diagnosis on each side. A team's actual decision is a policy question with legal, ethical, and practical dimensions, and the reference file's job is to name the dimensions rather than settle them.

### AI concept art as ideation

**The pro position.** Divergence is where models are strongest -- generating a hundred silhouettes to react to costs minutes and surfaces directions a human would not reach.

**The critique.** AI concept art produces plausible-looking images that are unbuildable, unriggable, and unmodelable, because the model optimizes image plausibility rather than physical or topological coherence. Worse, early output **anchors** the design space: the team converges on the first striking image rather than exploring, so the tool that promised divergence delivers premature convergence.

## Anti-pattern catalog

| Pattern | Trigger | Consequence | Fix |
|---|---|---|---|
| Trusting model narration as a result | Prose and code in one turn | Claimed outcome never checked against actual state | Require read-back of observed values |
| Asking "did I make X?" over a render | Natural phrasing of a check | Anchored oracle; the vision model confirms the prior claim | Ask "what is shown?" with the claim stripped from context |
| Rendering to check existence | Reaching for the visual oracle | Slow, expensive, unreliable answer to a cheap question | Assert on the scene graph |
| Path-traced render inside the loop | Wanting a good look each iteration | Minutes and thousands of tokens per iteration | Viewport render in-loop, real render at checkpoints |
| Re-looking-up objects by name | Model writes idiomatic-looking code | Silently addresses a pre-existing object; damage in the file | Hold the returned reference or an explicit id map |
| `bpy.ops` in agent-written scripts | Training-data bias toward the UI layer | Acts on whatever is active; wrong object or nothing | Steer to `bpy.data` in the tool description and prompt |
| Relying on undo for rollback | Assuming application semantics | No transaction; a bad step cannot be reverted | Save before, gate destructive ops at the agent layer |
| Holding a reference across mode switch or mutation | Long procedural script | Segfault; unsaved work lost | Re-acquire after any operation that can reallocate |
| Marketplace text returned verbatim | Asset-search tool | Untrusted instructions enter the model's context | Treat as data; strip or delimit; never as instruction |
| Long render on a streamed response | Natural for a slow operation | Broken stream loses the whole request; retry re-renders | Use the Tasks extension for genuinely long work |
| Generated mesh shipped as final | Impressive preview quality | Topology and UVs unusable for deformation or lightmaps | Retopo and re-unwrap, or scope to greybox and background |
| Inria-lineage splat code in a product | Reaching for SuGaR or 2DGS | Non-commercial research licence violation | `gsplat`, `.spz`, or a separate licence |
| No provenance record | One-shot chat generation | Asset cannot be regenerated, varied, or defended | Capture prompt, seed, model version, tool versions |
| Opaque binary as the deliverable | Chat-driven workflow | Nothing reviewable, diffable, or re-runnable | Commit the generation script or node graph |

## Severity rubric for this lens

- **blocker** -- unrecoverable damage or a licensing violation that ships: destructive operation with no save or gate, arbitrary code execution exposed to untrusted input, non-commercial-licensed code in a commercial product, a generated asset shipped where its licence forbids it.
- **major** -- silent wrongness that reaches output: no verification of claimed effects, name-aliasing risk, anchored perception checks used as the only oracle, missing provenance where licensing review will need it.
- **minor** -- cost and reliability: path-traced renders in a loop, single-view verification, avoidable image tokens, cache-breaking loops.
- **nit** -- ergonomics: tool naming, description phrasing that does not affect model behavior.
- **insight** -- a structural reframe: this check should be an assertion rather than a render; this should emit a committed script rather than a binary; this capability gap wants a parametric template rather than freeform generation.

## Authorities

- **The MCP specification and its changelog** (modelcontextprotocol.io) -- the only authority on the current revision, and it moves fast enough that nothing else is reliable.
- **Blender's Python API reference**, particularly the "Gotchas" page -- the source for undo invalidation, operator context limitations, name assignment, and reference invalidation. It is unusually candid about its own sharp edges.
- **The model cards and LICENSE files** of any generation model in use -- the licensing split here is real and badges are unreliable.
- **Khronos glTF extension registry** -- for `KHR_gaussian_splatting` and interchange status.
- **Kate Compton** -- the "10,000 bowls of oatmeal" formulation of perceptual uniqueness, which applies directly to generated-asset variation: mathematically distinct output that reads as identical.
- **Simon Willison** -- the "lethal trifecta" framing for agent security (private data, untrusted content, outbound communication), which describes the creative-agent setup precisely.
- **Primary legal sources** -- court dockets and the relevant copyright office, never a secondary summary, for anything in the legal section.

## Changelog

- **2026-09-05** -- Initial version. MCP facts verified against the specification and its changelog at revision `2026-07-28`, including the removal of sessions and the initialize handshake, the `server/discover` requirement, `subscriptions/listen`, the loss of stream resumability, MRTR, and the deprecation of Roots, Sampling, and Logging. Blender failure modes verified against Blender's own Python API documentation, including the undo-invalidation warning and its "use at your own risk" nuance, the operator poll-failure diagnosis problem, and the name-assignment caveat. Perception-loop token arithmetic uses the documented image-token approximation. The legal section is deliberately thin and points at primary sources: it is the fastest-rotting and most jurisdiction-dependent material here, and it is not legal advice.
