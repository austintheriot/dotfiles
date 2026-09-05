---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-05
---

# Blender and 3D content creation

A reference for reviewing and advising on Blender work and 3D content authoring: modeling, UVs, texturing, rigging, animation, scene organization, and the Python automation surface. Used by the `blender-3d` subagent and the `/expert-review` / `/expert-plan` / `/expert-consult` skills.

The unifying thesis: **a 3D asset is a bundle of loosely-coupled conventions -- transform state, winding and normal direction, UV layout, color-space tagging, influence counts, naming -- and almost every "it looked right in Blender and broke downstream" bug is one of those conventions silently disagreeing across a boundary.** Blender is permissive. It will happily let you carry an unapplied non-uniform scale, six bone influences per vertex, and a normal map tagged sRGB all the way to export, because none of those are errors *in Blender*. They become errors somewhere else, later, usually in someone else's hands.

The operational question: **"which convention does this asset assume, and does every consumer of it assume the same one?"**

Empirical priority, in rough order of how often it bites: **transform and unit state > color-space tagging > normal/tangent conventions > UV and texel discipline > influence limits and rig export > topology > render settings.** The first three are near-universal and cheap to check. Topology, which gets the most forum argument, causes the fewest shipped bugs.

## Volatile surface

`last-verified: 2026-09-05`. These rot. The conventions and failure mechanisms in the body are comparatively durable; version-gated API and UI locations are not.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Blender version, LTS windows, operator and UI paths | Fast (per release) | blender.org/download/lts, the release notes |
| `bpy` PyPI wheel to Python version pinning | Fast | pypi.org/project/bpy |
| Add-on maintenance status and licenses | Fast | The repo's commit recency and its actual LICENSE file |
| Engine-side import behavior (Unity, Unreal, Godot) | Medium | Each engine's current docs; see `game-art-pipeline.md` |
| glTF extension ratification status | Medium | KhronosGroup/glTF extensions README |
| Studio adoption and professional-use claims | Slow | Primary reporting |

**As of 2026-09-05**: Blender stable is **5.2** (released 2026-07-14, itself an LTS). 5.0 shipped 2025-11-18; 4.5 LTS July 2025; 4.2 LTS ended around 4.2.23 in July 2026. LTS means two years of critical fixes.

## Transform state: the first thing to check

Blender separates an object's transform from its mesh data. The viewport shows the composition; exporters, physics, modifiers, and engines do not all agree on what to do with the object-level part.

**Unapplied scale** is the canonical failure. A cube scaled to `(2, 2, 0.5)` in object mode has unit-cube mesh data plus an object-level scale. The symptoms, none of which look like a scale problem:

- Bevel and Solidify modifiers produce uneven width, because they operate in local space before the object scale applies.
- Normals appear correct in Blender and arrive wrong downstream, because non-uniform scale requires the inverse-transpose for normal transformation and not every consumer does it.
- Physics and collision behave at the wrong size.
- Child objects inherit the parent scale and compound it.

`Ctrl+A > Apply > Scale` (`object.transform_apply`) bakes it into the vertex data and resets the object scale to 1. Rotation has the same problem for anything orientation-sensitive. **A negative scale on any axis inverts winding order**, producing inside-out geometry with backface culling on -- Blender's default two-sided viewport display hides it completely.

**Units and up-axis.** Blender is Z-up, right-handed, 1 unit = 1 metre by default. Unity is Y-up left-handed with 1 unit = 1 m; Unreal is Z-up left-handed with 1 unit = 1 cm. The exporters apply conversions, and whether they are correct depends on settings that differ per format. This is where the notorious 0.01 and 100 scale factors come from. Set the scene unit scale deliberately rather than compensating with object scale, which reintroduces the unapplied-scale problem.

**Origins.** The object origin is the pivot for rotation, scaling, parenting, and the transform an engine reads as the object's position. An origin left at the world centre after modeling somewhere else produces objects that rotate around a distant point and instance at the wrong location. For modular kit pieces the origin placement *is* the snapping contract.

**Delta transforms** (`delta_location` and friends) are a second, separate transform channel. They are useful, and they are also a place for an unexplained offset to hide, since the normal transform panel reads as zero.

## Color space: the second thing to check

Blender manages color through OpenColorIO. Every image texture node has a color-space setting, and the rule is not stylistic:

- **Basecolor / albedo / emissive**: sRGB. These encode perceptual color and need the transfer function.
- **Everything else -- normal, roughness, metallic, ambient occlusion, displacement, masks**: **Non-Color**. These encode measurements, not colors. Applying an sRGB transfer function to them corrupts the values.

Tagging a roughness map sRGB is the single most common texture bug in Blender. The symptom is subtle and easy to misattribute: materials read too glossy or too rough, midtones shift, and the artist compensates by editing the texture, which bakes the error in permanently. The same mistake on a normal map produces lighting that is wrong in a way that looks like a bad bake.

Blender's view transform defaults to **AgX** in 4.0+ (replacing Filmic). AgX desaturates highlights heavily by design. Judging a texture's saturation through AgX and then shipping it to an engine using a different view transform means the asset was authored against the wrong reference. Use **Standard** when authoring textures for a target with its own tonemapping.

## Normals and tangent space

**Normal map green-channel convention.** OpenGL-style (+Y, green up) and DirectX-style (-Y, green down) differ only in the green channel's sign. Blender, Godot, and Unity expect OpenGL; Unreal expects DirectX. The symptom of a mismatch is lighting that appears inverted along one axis: bumps read as dents when the light moves vertically but look fine horizontally. It is easy to miss on a static screenshot and obvious the moment the light moves. Invert the green channel at export or in the engine's import settings, and be consistent about which one owns the conversion.

**Tangent space must match between baker and renderer.** A normal map is only meaningful relative to the tangent basis used to sample it. `mikktspace` exists to fix this: its stated purpose is to "consistently generate the same tangent spaces, for a given mesh, in any tool in which it is used," via an internal welding step and order-independent evaluation, so the result does not depend on face order, vertex order, or degenerate primitives. The rationale is exact: **the normal map sampler must use the precise inverse of the pixel shader's transformation**, or you get distortions and unwanted hard edges in lighting. Blender, Unity, Unreal, and most bakers use mikktspace; a tool that does not is a seam generator. The visible symptom is a hard shading discontinuity along UV seams that no amount of padding fixes.

**Smooth-by-angle changed in 4.1.** The old per-mesh auto-smooth property was removed in favor of a **Smooth by Angle** modifier and the `shade_auto_smooth` operator. Configs and scripts written against the old property break silently -- the mesh just renders fully smooth or fully flat. Custom split normals still exist and still override everything; they are also silently destroyed by some editing operations, which is a common cause of "my shading broke and I did not touch the normals."

**Winding and flipped faces.** Backface culling is off in Blender's viewport by default and on in most engines. Enable it in the overlay while working, or the first time anyone sees the flipped faces will be in-engine.

## The modifier stack and the dependency graph

The modifier stack is ordered and non-commutative. Mirror-then-subdivide and subdivide-then-mirror produce different meshes; Subdivision before Bevel rounds the bevel, after it does not. Most "why does this look wrong" modifier questions are order questions.

Modifiers evaluate through the dependency graph. Two consequences that bite:

- **Python reading mesh data gets the pre-modifier state** unless you explicitly request the evaluated version through the depsgraph. `object.data.vertices` is the base mesh. This is a frequent source of scripts that produce correct-looking but wrong output.
- **Modifier evaluation order across objects** (a Boolean referencing another object, a Shrinkwrap targeting a surface) creates dependencies. Cycles are refused; near-cycles produce one-frame-late results that look like intermittent glitches.

Geometry Nodes is the procedural system and is genuinely non-destructive, which makes it the right home for anything parameterized. It is also a place where the "what does this actually output" question needs the Spreadsheet editor rather than reasoning about the graph.

## UVs and texel density

**Texel density** is texture pixels per unit of world space. Consistency matters more than the absolute number: an asset at 1024 px/m next to one at 256 px/m reads as a bug even though neither is wrong alone. Pick a project standard, express it in px/m, and check it rather than eyeballing.

**Overlapping UVs** are fine and desirable for tiling and mirrored detail in the base channel. They are fatal in a lightmap channel, where every face needs unique space. This is why lightmap UVs are a separate channel with different rules -- see `game-art-pipeline.md` for the engine-side requirements, including the important detail that Unreal's auto-generator repacks charts but never cuts or splits them, so bad channel-0 charts cannot be rescued automatically.

**Seams** are a tradeoff, not a defect. Every seam is a hard edge in tangent space and duplicated vertices at export. Fewer seams means more distortion; more seams means more vertices and more visible tangent discontinuity. Hide them where the eye does not go and where the geometry already breaks.

**UDIMs** are well supported in Blender and are the film convention. For real-time work a single atlas is usually still correct, since UDIM support in engines is uneven and the runtime cost is real.

## Rigging, skinning, animation

**Influence limits are the classic silent truncation.** Blender imposes no cap on bone influences per vertex. Engines do. Unity's `QualitySettings.skinWeights` sets the maximum bones per vertex used during skinning for all meshes -- and crucially **it does not change the mesh data, it only limits how many weights are used**, so excess weight data sits in the mesh unapplied with no error anywhere. Unreal's default mode uses 4 or 8 influences depending on platform, and its "Unlimited Bone Influences" mode is **capped at 12 in practice** because of how skeletal mesh source data is stored; Epic's own recommendation is to enable unlimited with the threshold set to 8. The result in both engines is the same class of bug: a rig that deforms correctly in Blender and shows collapsing or twitching vertices in-engine, with no warning at any step. Use Blender's Limit Total weight tool to the target's cap before export, and normalize after.

**Bone roll** is the rotation of a bone around its own axis. It has no visual effect on the rest pose and complete effect on how rotations compose and how IK behaves. Inconsistent roll on a symmetric rig produces limbs that bend correctly on one side and wrongly on the other. Recalculate roll deliberately rather than letting it accumulate.

**Rest pose versus bind pose.** The armature's rest pose is what skinning weights are relative to. Editing bones in Edit Mode after weighting changes that relationship and silently invalidates the weights. This is the "my character exploded" failure.

**Non-uniform and animated bone scale** is not portable. Many exporters and engines assume uniform bone scale; some drop it entirely. Avoid it in anything destined for a game engine.

**Rigify** is Blender's rig generator. It produces a control rig plus a deformation rig; only the deformation bones should export. Exporting the whole thing gives an engine a skeleton full of control bones, widgets, and constraint targets, which inflates bone counts past platform limits (Unreal caps mobile at **75 bones per Section**; exceeding max-bones-per-section splits the section into chunks and costs draw calls).

**Bake IK to FK before export.** Constraints are a Blender concept. glTF and FBX carry sampled transforms, not constraint relationships. Unbaked IK exports as a T-pose or as the FK values, depending on the exporter's mood.

**Slotted actions arrived in 4.4**, changing how actions bind to data-blocks. Scripts and add-ons that assumed the old one-action-per-object model need updating.

## Scene organization and linked data

Collections are the organizational unit and the export selection mechanism. **Linked collections** (from another .blend) are the asset-library workflow: edit once, propagate everywhere, at the cost of an indirection that makes local overrides awkward. Library overrides exist for exactly that and are the part most likely to behave unexpectedly.

The asset browser plus a marked-asset catalog is the built-in library system and is the right answer for a kit-based workflow.

**Naming is a contract.** Object names become node names in glTF, mesh names become asset names in engines, and material names are how engines match materials on reimport. Renaming a material in Blender after import setup exists downstream breaks the association silently. Decide the naming convention before the assets multiply.

## Python and headless automation

`bpy` is the API. Key facts for anyone scripting it:

- **`bpy.ops` is the operator layer and it depends on context** -- the active object, the current mode, the area type. Operators called from a script without the right context either fail with `RuntimeError: Operator bpy.ops.*.poll() failed, context is incorrect` or, worse, silently act on the wrong object. Prefer the data API (`bpy.data`, direct mesh manipulation, `bmesh`) over `bpy.ops` for anything non-interactive. This is the single biggest quality difference between fragile and robust Blender scripts.
- **`bmesh`** is the mesh-editing API and the right tool for topology manipulation from Python.
- **Headless runs**: `blender --background file.blend --python script.py`. Note that `--background` disables the GPU-dependent parts of the UI, which affects some rendering paths.
- **The `bpy` PyPI wheel** ("Blender as a Python module") lets Blender run inside a normal Python process, for pipelines and services. **Each Blender release pins exactly one CPython version**: releases up to 5.0 are cp311, and **5.1 onward are cp313 -- there is no cp312 line at all.** This is a hard continuous-integration constraint with a consequence teams hit late: **you cannot have 4.5 LTS and 5.2 in the same Python environment**, so supporting both means two environments, and a Blender upgrade forces a Python upgrade in lockstep.
- **The undo system does not extend to scripts** the way users expect. A script that runs a destructive operation has no reliable rollback. Save first.

## Rendering, briefly

Cycles is the path tracer; EEVEE is the rasterizer. **EEVEE Next** (4.2+) closed much of the gap by adding real shadows, ray-traced reflections, and proper volumetrics, and its speed changes what workflows are practical -- the director of *Flow* switched to Blender in 2019 specifically for EEVEE, and worked without storyboards by exploring scenes with the camera in real time.

The Principled BSDF v2 (4.0+) reorganized inputs and added coat, sheen, and thin-film layers. Old node setups and scripts that address inputs by index rather than name break across this boundary.

## Schools of thought

These are live disagreements among competent people. Both sides are recorded at strength; neither is the settled answer.

### Topology purism versus "triangles do not matter"

**The quad-discipline position.** All-quad topology with deliberate edge flow is not aesthetics, it is surface continuity control. Catmull-Clark subdivision is **C2-continuous at valence-4 vertices and only tangent-plane (G1) continuous at extraordinary vertices**. Edge-loop placement is valence control, and valence control is continuity control. Poles in the wrong place produce pinching that is visible under any smooth shading and impossible to fix by pushing vertices. For anything that deforms, edge loops following the deformation direction are what let a joint bend without collapsing. For anything that will be subdivided, quads are the input the algorithm was designed for.

**The it-all-triangulates position.** Every renderer triangulates before rasterizing. A static prop that will never be subdivided and never deform is not made better by quads; the effort spent on retopology is effort not spent on silhouette and texture, which is what viewers actually see. Nanite makes the polygon-budget conversation obsolete for static geometry: import the high-poly mesh and let virtualized geometry handle it. Insisting on quad discipline for a rock is cargo cult.

**Where they genuinely differ**: the quad camp is describing deforming and subdividing surfaces, the triangle camp is describing static props. Both are right in their domain, and the argument persists because people generalize their own domain.

### Blender versus Maya in professional pipelines

**The Blender-is-ready position.** *Flow* (2024) was made entirely in Blender on a EUR 3.5M budget, grossed EUR 50M, and won the Academy Award for Best Animated Feature -- the first independent film to do so. Sony Pictures Imageworks has adopted it; Ubisoft Animation Studio planned to replace internal software with it from 2020; Warner Bros. Animation has hired Blender artists since 2022. The Development Fund runs about $330k/month with roughly 7600 individual and 47 corporate members including Adobe, AMD, Microsoft, NVIDIA, Intel, Google, Meta, and Netflix Animation. The remaining objections are inertia and retraining cost, not capability.

**The Maya-still-wins position.** By 2015 all ten Best-VFX Oscar nominees used Maya, and every winner since 1997 has. The case is not rendering, it is the technical-director surface: the node-based dependency graph, the depth of rigging tooling, scene scale on very large shot files, and two decades of studio-internal tooling written against it. **Blender's own Foundation has cited "overly optimistic targets, over-scoping, unclear designs" in its roadmap self-critique of animation-layer work** -- the gap is conceded by its own developers, which is why dismissing this position as pure inertia is wrong. Maya is rental-only at $255/month or $2,010/year, with Indie at $330/year; the cost is real but is small next to a senior artist's salary.

**Note the scope**: *Flow* answers the feature-animation question. It does not answer the game-pipeline question, which is where the Maya case actually lives.

### One big scene versus asset library and linked collections

**Linked-library position.** Assets live in their own files, get linked into scenes, and a fix propagates everywhere. It is the only thing that scales past one person, and it is how version control on binary files becomes tractable.

**Single-scene position.** Linking adds indirection that makes overrides awkward and debugging harder, and for a solo artist or a small scene the coordination cost buys nothing. Unreal's One File Per Actor exists to solve the same collaboration problem at the engine level -- and has its own documented footgun, where Level Blueprints referencing actors mark them "Always Loaded" and defeat the selective cell streaming the system exists to enable.

### Modularity and kit-bashing versus bespoke modeling

**Modular position.** Trim sheets, tileable materials, and a kit of snapping pieces give texture resolution that bespoke unwrapping cannot match at the same memory budget, and they let a small team build large spaces. Repetition is manageable with decals and variation.

**Bespoke position.** Kits read as kits. Grid-snapped repetition is visible to players even when they cannot name it, and the silhouette variety that makes a place memorable comes from unique geometry. Modularity optimizes for the production constraint at the cost of the thing the production exists to make.

*This debate is thinner in the sourced material than the others; treat the framing as a skeleton and weigh a specific project's constraints over the general argument.*

## Anti-pattern catalog

| Pattern | Trigger | Consequence | Fix |
|---|---|---|---|
| Unapplied scale or rotation at export | Object-mode transforms, never applied | Wrong modifier widths, broken normals, wrong physics size, compounding on children | `Ctrl+A > Apply` before export; audit with a script |
| Negative scale | Mirroring by scaling -1 | Inverted winding, inside-out in-engine, invisible in Blender's two-sided viewport | Apply scale; enable backface culling in the overlay while working |
| Non-color data tagged sRGB | Default color-space on a roughness/normal/mask texture | Corrupted material response; artist compensates and bakes the error in | Set Non-Color on every non-albedo texture |
| Wrong green channel | OpenGL normal map into Unreal, or the reverse | Lighting inverted along one axis; looks fine until the light moves | Decide who owns the conversion; apply it once, consistently |
| Over-weighted vertices | No influence cap in Blender | Silent truncation in-engine; deformation differs from Blender with no error | Limit Total to the target cap, then normalize |
| Exporting the control rig | Rigify or any control-rig setup exported wholesale | Bone count blows past platform limits; sections split into chunks | Export deform bones only |
| Unbaked IK | Constraint-driven animation exported directly | Animation arrives as T-pose or FK values | Bake to keyframes before export |
| `bpy.ops` in a headless script | Porting interactive actions to automation | Context errors, or silent action on the wrong object | Use `bpy.data` / `bmesh`; reserve operators for interactive use |
| Reading mesh data without the depsgraph | Scripting against modified geometry | Script sees pre-modifier state; output silently wrong | Get the evaluated object from the dependency graph |
| Renaming materials after downstream setup | Housekeeping late in production | Engine-side material assignment breaks on reimport | Fix names before assets propagate |
| Judging textures through AgX | Default view transform since 4.0 | Authored against the wrong reference; saturation wrong in-engine | Switch to Standard when authoring for an external target |
| Editing bones after weighting | Late rig adjustment in Edit Mode | Weights silently invalidated; character deforms wrongly | Finish the rest pose before weighting |

## Severity rubric for this lens

- **blocker** -- data loss or a defect that ships broken: destructive script with no save, redaction-equivalent (exporting source-only data into a distributed asset), influence truncation on a hero character, negative scale reaching an engine, a licensing violation in a bundled asset.
- **major** -- wrong-looking output that will be caught late and cost rework: color-space errors, green-channel mismatch, unapplied transforms at an export boundary, unbaked constraints, control rig exported.
- **minor** -- inconsistency that costs time but not correctness: inconsistent texel density, seam placement, naming drift, modifier order producing an unintended but acceptable result.
- **nit** -- organizational preference: collection structure, panel layout, non-load-bearing naming style.
- **insight** -- a structural reframe: this belongs in Geometry Nodes rather than hand-modeled; this kit wants a trim sheet rather than unique unwraps; this scene should be a linked library.

## Authorities

- **Blender manual and Python API reference** (docs.blender.org) -- the primary source. The Python API reference is versioned; read the one matching the target release.
- **Blender Studio** (studio.blender.org) -- the Foundation's own production team, open-movie files and pipeline documentation. The closest thing to a canonical production reference.
- **mikktspace** (the header and its licence text, zlib-style) -- the tangent-space standard; the header comments state the design rationale precisely.
- **Khronos glTF specification and extension registry** -- for anything crossing the export boundary.
- **Andrew Price (Blender Guru)** -- beginner-to-intermediate craft, the donut tutorial as the standard on-ramp.
- **Ian Hubert** -- the lazy-tutorials school: photo-projection and texture-driven detail over modeled detail. Strong counterweight to modeling purism.
- **CG Cookie, Grant Abbitt** -- structured game-art-oriented instruction.
- **Polycount wiki** -- the accumulated game-art convention reference, particularly on normal maps and baking.
- **Gints Zilbalodis** -- *Flow*'s director; his EEVEE-driven, storyboard-free workflow is the strongest single data point for real-time rendering changing the production method, not just its speed.

## Changelog

- **2026-09-05** -- Initial version. Verified against primary sources: Blender 5.2 as current LTS, the 4.1 auto-smooth removal, the `bpy` wheel's one-CPython-per-release pinning (cp311 through 5.0, cp313 from 5.1, no cp312), mikktspace's stated rationale, Unity `skinWeights` semantics, Unreal's 12-influence practical cap and 75-bone mobile section limit, *Flow*'s production facts, and the Foundation's funding scale. Blender's own domains returned 403 throughout research, so Blender-side USD and MaterialX parity claims, headless EEVEE and baking behavior, and Blender Studio's production file structure are **unverified**. OpenPBR's README does not list Blender among implementations, so Blender OpenPBR adoption is unconfirmed in both directions.
