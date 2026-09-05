---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-05
---

# Game art pipeline

A reference for reviewing the boundary between authored 3D content and the engine that consumes it: interchange formats, export and import settings, texture compression, LOD and collision conventions, naming contracts, asset validation, and version control for binary assets. Used by the `game-art-pipeline` subagent and the `/expert-review` / `/expert-plan` / `/expert-consult` skills.

The unifying thesis: **the pipeline is a contract between two tools that were never designed together, and the contract is enforced by nothing.** No compiler checks that the exporter's tangent basis matches the shader's, that the material name in the .blend still matches the one the engine bound, or that a texture reached the GPU in the format the artist assumed. Every one of those is a convention held in someone's head, and the pipeline's job is to move as many of them as possible into something that fails loudly.

The operational question: **"what does this asset assume about its consumer, what does the consumer assume about it, and what checks that they agree?"**

Empirical priority, in rough order of how often it bites: **identity and reference stability (GUIDs, names, paths) > import-setting drift > color space and compression > influence and bone limits > LOD and collision conventions > determinism and caching.** Reference breakage is the worst because it is silent, unrecoverable, and destroys work that was already correct.

## Volatile surface

`last-verified: 2026-09-05`. Engine behavior changes per release, and much of this section is version-gated. The failure *classes* are durable; the settings, defaults, and menu paths are not.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Engine import behavior, setting names, menu paths | Fast (per release) | The engine's own current docs |
| Version-gated defaults (DDC backend, Interchange flags) | Fast | Engine release notes |
| glTF extension ratification status | Medium | KhronosGroup/glTF extensions README |
| Tool and add-on maintenance status | Fast | Repo commit recency, actual LICENSE file |
| Marketplace and asset-store ownership, terms | Fast | The store's own pages |
| Compression format hardware support | Slow | Vendor documentation |

**As of 2026-09-05**: Unreal Engine stable is **5.8**; **UE6 was announced 2026-05-24** with early access targeted "late 2027-ish" and Blueprints and Actors slated for eventual replacement by Verse and Scene Graph -- announced, not shipped, so 5.8 is the target for all present advice. Godot is at **4.7.2** with 3.6.3 still maintained. Blender is at **5.2 LTS**. **Epic divested ArtStation and Sketchfab to Kitbash in August 2026**; do not describe them as Epic properties. **RealityCapture was rebranded RealityScan 2.0** in June 2025 and is free below US$1M annual gross revenue.

## Identity: the thing that breaks worst

**Unity `.meta` files hold the GUID.** Every asset's identity lives in its `.meta` file, and **only** there -- never in the gitignored `Library` cache. An uncommitted `.meta` means the GUID regenerates per machine, which breaks every reference to that asset: scene references, prefab component connections, material assignments. The damage is not recoverable by re-importing, because the references pointed at an identifier that no longer exists. **Committing `.meta` files is not a preference, it is a correctness requirement.**

Unity's AssetDatabase v2 splits this: `Library/SourceAssetDB` tracks modification dates and file hashes to decide whether a source file changed; `Library/ArtifactDB` holds import results and their dependencies. Artifacts are explicitly disposable and regenerable. The importer version and the **target platform** are folded into the artifact hash, so switching build target can trigger a bulk reimport with zero asset changes -- a legitimate cause of "why is it importing everything again."

**Unity's Model Prefab is read-only and fully regenerated on every reimport.** You cannot edit it in the editor. The fix for "my customization vanished when the artist re-exported" is structural, not a setting: create a **Prefab Variant** from the Model Prefab and instantiate that. Variant overrides persist independently and keep inheriting base changes except where overridden. Unity's Prefab Variant documentation states no limitations, which is a documentation gap rather than evidence the feature is painless.

**Godot uses MD5 checksums, not timestamps**, to decide on reimport -- robust to clone and checkout timestamp churn. Commit source assets and their `.import` files; gitignore `.godot/` including `.godot/imported/`, which regenerates.

**Material names are the reimport binding.** In every engine, renaming a material after downstream setup exists breaks the association silently. Unreal's Interchange makes this sharper: on reimport its Conflicts Preview highlights material and skeleton structure changes, but **you can no longer choose to preserve the original material assignment or replace it from the conflict window** -- a documented regression from the legacy path. To change an assigned material you must fix the source file or use the Static Mesh Editor. This directly determines who owns the material: the DCC or the engine. Decide explicitly.

## Interchange formats

**glTF 2.0** is the modern default for real-time. PBR-native, extension-based, and structurally deterministic: the `asset` schema carries only version, copyright, generator, minVersion, extensions, and extras -- **there is no timestamp field in the spec**. Residual nondeterminism comes from Draco/meshopt settings and JSON key and float serialization order, not from the format itself.

Ratified Khronos extensions include `KHR_materials_*` (anisotropy, clearcoat, dispersion, emissive_strength, ior, iridescence, sheen, specular, transmission, unlit, variants, volume), `KHR_draco_mesh_compression`, `KHR_texture_basisu`, `KHR_texture_transform`, `KHR_mesh_quantization`, `KHR_lights_punctual`, `KHR_animation_pointer`, and **`KHR_gaussian_splatting`** -- the first vendor-neutral splat interchange. `KHR_materials_pbrSpecularGlossiness` and `KHR_xmp` are **archived**; using them is a legacy path.

`KHR_materials_variants` carries root-level named variants with per-primitive mappings, aimed at commerce use (colorways) and finite author-time variant sets. It is explicitly **not** for continuous or dynamic configurators.

**FBX** persists on inertia and tooling depth. **The binary format has no official documentation** -- the Blender Foundation published unofficial specs, and clean-room implementations descend from those. Autodesk ships C++ and Python SDKs whose licensing is a real constraint; Godot went to Assimp and then `ufbx` rather than use the SDK. Version numbers map to years (7.4 = 2014, 7.5 = 2016.1.2), and cross-version incompatibility is routine.

**FBX export is not byte-reproducible.** Established by reading Blender's `export_fbx_bin.py` directly: `CreationTimeStamp` comes from `datetime.datetime.now()` when the caller passes `time=None`, which is the operator default. Blender deliberately neutralized the other identity sources -- Original and LastSaved `DateTime_GMT` hardcoded to the epoch, `FileId` a dummy `b"FooBar"`, document URLs hardcoded to `/foobar.fbx` -- but `ApplicationNativeFile` still embeds the live `bpy.data.filepath`, which breaks cross-machine reproducibility independently of the timestamp. Net: same-machine repeat exports differ only in the timestamp header block, so a masked diff is nearly viable, but a naive hash or `git diff` is not. **This settles whether "regenerate exports rather than commit them" is workable: for FBX, not without masking; for glTF, structurally much closer.**

**USD / OpenUSD** is the composition-first format. Its strength is layering, and the composition order is **LIVRPS**, strongest to weakest: **L**ocal, **I**nherits, **V**ariants, **R**eferences, **P**ayloads, **S**pecializes. SubLayers are the foundational layer-stack mechanism and are not in the acronym. Payloads are deferrable heavyweight content; Specializes is the weakest, for fallback values. Formats: `usda` ASCII, `usdc` binary, `usdz` a **zero-compression** zip that may also hold PNG, JPEG, M4A, MP3, and WAV. Governed by AOUSD (formed 2023-08-01, Pixar, Adobe, Apple, Autodesk, NVIDIA, under the Linux Foundation).

**Alembic** carries baked geometry caches, not rigs. **OBJ/PLY/STL** carry geometry and little else; OBJ has no skeleton, no animation, and a material format nobody agrees on.

**MaterialX** (ASWF, Apache-2.0, v1.39.4) and **OpenPBR** (ASWF, spec v1.1.1 dated 2026-04-17, Adobe and Autodesk origin) are the shading-model interchange effort. OpenPBR's own README lists Arnold for Maya, MaterialX Web Viewer, OpenPBR-viewer, and Adobe's open-source BSDF as implementations -- **it does not list Blender**, so Blender adoption is unconfirmed in either direction.

**Godot's format support**: glTF 2.0 (recommended), `.blend`, DAE, OBJ, FBX. **Godot 4.3+ uses `ufbx` by default** for FBX; legacy projects keep FBX2glTF unless changed, and both importers still ship in master. `.blend` import transparently calls Blender's glTF exporter, then runs the standard glTF pipeline -- which means **every team member needs Blender installed**, at 3.0+ (3.5+ recommended), before opening Godot. Unavailable on Android and web editors.

**Unreal's Interchange framework**: Pipelines process imported data and expose options; a Pipeline Stack is an ordered list assigned per format in Project Settings > Interchange; Factories generate the assets. The plugins are enabled by default. If a format is not supported by Interchange, Unreal **falls back to the legacy import framework** -- and **FBX through Interchange is still experimental as of UE 5.8**, off by default, behind `Interchange.FeatureFlags.Import.FBX`. So FBX does not route through Interchange by default even in 5.8. Import Into Level currently works only with glTF and MaterialX. Runtime Blueprint import does not support skeletal mesh or animation data.

## Textures and compression

**Block compression is not optional** at scale. An uncompressed 4096x4096 RGBA texture is 64 MB; the same texture in BC7 is 16 MB, and in BC1 8 MB. Shipping uncompressed textures is the most common cause of a build that will not fit or a scene that will not stream.

The BC family, with the correct ratios:

- **BC1 / DXT1** -- 64 bits per 16 pixels. **6:1 for 24-bit RGB** (not 4:1, the commonly repeated figure). Endpoints are two 16-bit **5:6:5** RGB values, so green gets the extra bit. Optional 1-bit alpha.
- **BC2 / BC3** -- 128 bits per 16 pixels, 4:1. BC3 (DXT5) is the standard RGBA workhorse.
- **BC4** -- 64 bits per 16 pixels, single channel. Height, masks, roughness alone.
- **BC5** -- 128 bits per 16 pixels, two channels. **The normal-map format.** ATI's 3Dc was a DXT5 modification made specifically to fix S3TC's normal-map weakness; reconstruct Z in the shader.
- **BC6H** -- 128 bits per 16 pixels, HDR float16. Skyboxes, light probes.
- **BC7** -- 128 bits per 16 pixels, RGBA, a much-enhanced BC3. The modern default where the hardware supports it.

**ASTC** uses a fixed 128-bit block with a variable footprint: 4x4 through 12x12 in 2D gives **8.00 down to 0.89 bits per texel**; 3D 3x3x3 through 6x6x6 gives 4.74 to 0.59. It beats PVRTC, S3TC, and ETC2 on PSNR at 2 and 3.56 bits per texel, and its HDR mode is comparable to BC6H at 8 bpt. Hardware support: Apple LDR from A8-A12 and full from A13; Mali T620/720/820 onward; Intel Skylake through Gen12 and then **removed in Arc**; Adreno 4xx+ LDR; PowerVR Series6XT+. **ETC2** is the OpenGL ES 3.0 baseline.

**Channel packing** puts three grayscale maps in one texture's RGB. ORM (occlusion, roughness, metallic) is the glTF convention and is baked into the spec's `pbrMetallicRoughness` model. Unreal's community conventions differ, and Allar's ue5-style-guide explicitly **warns against packing four channels** except alpha-in-diffuse. The packed texture must be Non-Color / linear; tagging it sRGB corrupts all three channels at once.

**KTX2 with Basis Universal** (`KHR_texture_basisu`) is the transcodable path: ship one file, transcode to whichever block format the device supports at load. This is the answer to shipping a single build across desktop and mobile.

## LODs, collision, and naming

**Unreal's collision naming is a hard contract.** The prefix determines the primitive and the suffix must match the render mesh name exactly:

- `UBX_` box -- vertices cannot be moved or deformed.
- `UCP_` capsule -- cylinder with hemispherical caps.
- `USP_` sphere -- 8 segments is sufficient.
- `UCX_` convex -- **must be completely closed and genuinely convex**.

Multiples use `UCX_Tree_01_00`, `UCX_Tree_01_01`. Hulls work best non-intersecting. Non-convex shapes require manual decomposition; automatic decomposition is unpredictable and is a common source of collision that almost works.

Allar's ue5-style-guide is the de facto Unreal naming convention: `S_` static mesh, `SK_` skeletal, `BP_` blueprint, `M_` material, `MI_` material instance, `A_` anim sequence, `AM_` montage, `BS_` blend space, `T_` texture with `_D _N _R _O _E _M _A` suffixes.

**Lightmap UVs are conditional, not obsolete.** They are required only for static and stationary (baked, precomputed) lighting; **a project using only dynamic lighting needs none**. That is the precise scope of the "Lumen changed this" claim. Where they are needed, every face needs unique space with no overlap, plus padding scaled to the lightmap resolution. Unreal auto-generates one on import from UV channel 0, and the critical non-obvious detail is that **the repacking algorithm repacks charts but does not cut or split them**. If channel 0 has charts that are wrong for lightmapping, auto-generation cannot fix them -- you must split the charts in the DCC before import. This is the actual reason auto-generated lightmaps sometimes fail.

**Nanite's current scope** (UE 5.8): Opaque and Masked blend modes only; anything else falls back to the default material with an Output Log warning. **Skeletal meshes are fully supported as of 5.8** -- single draw call per mesh, virtual shadow map shadowing, animation LODs, instancing with animation banks. Still incompatible: translucent blend modes, **morph-target deformation**, and deformation beyond translate/rotate/non-uniform-scale. Projective decals work; mesh decals (translucent) do not. Hard limit of **16 million instances per scene**. LOD is automatic; manual LOD setup is no longer required.

**Bone limits.** Unreal's default mode uses 4 or 8 influences per vertex by platform, at fixed cost -- a vertex using one bone still fills the other slots with zero weights and still pays the computation. "Unlimited Bone Influences" is **capped at 12 in practice** because of how skeletal mesh source data is stored; Epic recommends enabling it with the threshold at 8, so 9-12 influence meshes take the unlimited path and 0-8 take the fixed path. Bone indices are **8-bit by default, supporting 256 bones per Section**; above that requires enabling "Support 16-bit Bone Index", which needs an editor restart **and a reimport of existing meshes**. Max Bones Per Section defaults to 65536 but **mobile caps at 75**; exceeding it splits the Section into Chunks and costs draw calls. The console command **`SkeletalMeshReport`** dumps per-mesh setup and memory stats and is a real audit hook.

Unity's `QualitySettings.skinWeights` takes One / Two / Four / Auto and **does not change mesh data** -- it only limits how many weights skinning uses, so excess weights ship unapplied and silently ignored. Per-mesh override via `SkinnedMeshRenderer.quality`. Unity's own documentation does not state the default value; "4 is the default" is widespread convention, not a documented fact.

## Version control for binary assets

**Git LFS** is the common answer and has sharp edges. `git lfs lock` marks a path locked against the LFS server and makes the server reject pushes that modify others' locked files -- the practical substitute for exclusive checkout on unmergeable binaries. `git lfs prune` deletes local copies that are both old **and** already pushed, preserving the current checkout, stashes, recent branches and commits, unpushed commits, and other worktrees. `lfs.pruneoffsetdays` defaults to 3. **It never deletes files whose local copy is the only one**, and it checks `origin` -- with no `origin` configured, everything looks unpushed and nothing is pruned.

**Perforce Helix Core** remains the studio standard for a reason: exclusive file locking on binaries, Streams for workflow structure, scaling to petabytes and millions of files, and delta transfers. Unity names Unity Version Control and Perforce as its two first-class integrations.

**Unreal's Derived Data Cache** is regenerable and should never be in source control. The hierarchy checks fastest cache first, copies hits into the fastest local cache, and asynchronously populates the shared caches on a miss. **Version boundary: UE 5.3 and earlier defaulted the local DDC to Filesystem in the install directory; UE 5.4+ defaults to Unreal Zen Store at `AppSettingsDir/Zen/Data`, with the legacy filesystem DDC set to delete-only with an 8-day cleanup.** The Zen Storage Server is unauthenticated -- restrict it to a trusted network or VPN. The S3 backend is **no longer recommended**; migrate to Unreal Cloud DDC. A shared filesystem DDC on a network drive is the standard co-located-team setup. **Epic explicitly advises against copying an entire DDC over the internet, backing it up, or restoring from remote backup** -- transferring it takes longer than regenerating locally; distribute a DDC Pak (`.ddp`) instead.

## Validation in continuous integration

The pipeline's leverage is turning conventions into checks that fail loudly.

**glTF-Validator** (Apache-2.0) is the format-level gate: `gltf_validator [options] <input>`, writing `<asset>.report.json` by default, `-o/--stdout` to print, `-r/--validate-resources` to validate buffer and image content, `-c/--config` for YAML config, `-h/--threads` for directory parallelism, and **a non-zero exit code on validation errors** -- the CI hook. Staleness signal: the latest tag is `2.0.0-dev.3.10` from 2024-10-22, still formally a dev pre-release track.

**Unreal Data Validation**: extend `UEditorValidatorBase` and implement `CanValidateAsset` plus `ValidateLoadedAsset`, which must call `AssetPasses` or `AssetFails` on **every** path. C++ and Blueprint validators are auto-discovered at editor startup; **Python validators require manual registration** via `UEditorValidatorSubsystem::AddValidator`. Alternatively override `IsDataValid` on a `UObject`, which gets access to private and protected members. Run in CI with `UnrealEditor-Cmd.exe <PROJECT>.uproject -run=DataValidation` -- which **by default runs only the C++ validation rules**, a trap for teams whose rules are in Blueprint or Python.

Checks worth automating, roughly in yield order: committed `.meta` files (Unity); texture color-space tagging; texture dimensions power-of-two and within budget; influence counts against the platform cap; bone counts against section limits; collision primitives present and correctly named; lightmap UV presence where baked lighting is used; naming convention conformance; polycount and LOD-chain presence; missing-material and default-material detection.

## Tool ecosystem health

Repository status verified 2026-09-05. Maintenance status is the fastest-rotting class of fact here.

**Healthy and active**: `assimp` (import library), `ufbx` (FBX parser, the Godot path), `KhronosGroup/glTF-Blender-IO` (Apache-2.0), `glTF-Transform` (MIT), `meshoptimizer` (MIT, ships `gltfpack`), `google/draco` (Apache-2.0 -- commits active but the latest tag is still 1.5.7 from January 2024, a tag-versus-commit lag), `OpenColorIO` (BSD-3), `KTX-Software` (Apache-2.0), `OpenUSD`, `MaterialX`, `MeshLab` (GPL-3.0) and `PyMeshLab` for headless batch mesh processing, `COLMAP` (BSD despite GitHub's NOASSERTION detection artifact), `Meshroom` (MPL-2.0, needs NVIDIA CUDA for the dense stage, which blocks AMD and Apple Silicon), `RenderDoc` (MIT, no macOS/Metal support), `Material Maker` (MIT), `Bforartists` (Blender fork with a reworked UI).

**Dormant or dead**: `MakeHuman` (last push 2024-08-19; the live path is the MPFB2 Blender add-on), `Compressonator` (2024-06-19), **NVIDIA Texture Tools** (archived, last push 2020-12-21), **Epic's BlenderTools** -- Send to Unreal's last release was 2.4.3 on 2023-11-09, roughly three years dormant. `xNormal` is still hosted, Windows-only, with no active development signal.

**License traps worth naming**: `Kitsu` is **AGPL-3.0**, whose network-use clause matters for a studio self-hosting a modified version. `OpenMVS` is **AGPL-3.0** -- it triggers source disclosure for hosted reconstruction services even with no binary distribution. `Prism` is LGPL-3.0. **ArmorPaint is zlib/libpng** (GitHub's NOASSERTION is wrong -- read `license.md`): source is free, official binaries are sold, which is legal precisely because zlib does not require free binary redistribution. It reached 1.0 on 2026-09-03 after a three-year gap. `RetopoFlow`'s code is GPL-3.0-or-later but its icons, matcaps, and docs are proprietary to CG Cookie / Orange Turbine -- a clean example of GPL-compliant add-on monetization. **Poly Haven is CC0**: any purpose including commercial, no credit needed, redistributable.

**The Gaussian-splatting licensing trap.** The original Inria/MPII 3D Gaussian Splatting reference implementation is a **non-commercial research licence** -- research and evaluation only, no commercial use without separate written consent, no sublicensing. It propagates through shared codebase lineage to **SuGaR** and **2DGS**, both mesh-extraction tools. Attribution does not cure it; a commercial splat-to-mesh pipeline needs a separate Inria licence. Clean alternatives: Niantic's **`.spz`** (MIT) and **`gsplat`** (Apache-2.0), plus the now-ratified `KHR_gaussian_splatting`. Splats currently replace photogrammetry for static capture; animated and rigged splats remain an open research problem with no production pipeline.

**Version-control-relevant packaging note**: Unity packages use Package Manager versioning, so judging a Unity package's health by its GitHub release tags gives a false staleness reading -- `com.unity.formats.fbx` shows a last release tag of v4.0.1 from 2021 despite active commits. Its docs also frame round-trip around Maya, Maya LT, and 3ds Max; **Blender is not a named first-class round-trip target**.

## Schools of thought

### Commit exports, or generate them

**Commit-the-exports position.** The engine consumes exported assets, so those are the real artifacts. Committing them makes checkouts reproducible, removes the DCC from the build machine's dependency list, and means a broken exporter cannot break the build retroactively. Godot's own guidance is to commit source assets plus their `.import` files.

**Generate-on-build position.** Committed exports are derived data that goes stale silently the moment someone edits the source and forgets to re-export. The `.blend` is the truth; anything else is a cache. Commit sources, generate in CI, and the two can never disagree.

**The evidence complicates both.** FBX export is not byte-reproducible (see above), so a generate-on-build policy produces a diff on every run and defeats caching unless you mask the header. glTF has no timestamp field and is structurally much closer to reproducible, which makes generate-on-build genuinely viable there and awkward for FBX. The format choice and the version-control policy are coupled, and teams that pick them independently get the worst of both.

### Who owns the material

**DCC-owns position.** The artist authors the material in Blender or Maya, and the engine mirrors it. One source of truth, and the artist can see what they are shipping.

**Engine-owns position.** Real materials use engine features the DCC cannot represent -- instances, parameter overrides, engine-specific nodes. The DCC material is a preview, and the engine material is the product. Unreal's Interchange regression, which removed the ability to preserve or replace material assignment from the conflict window on reimport, pushes hard toward deciding this explicitly and early: fix the source file or fix it in the Static Mesh Editor, with no middle path.

### Direct `.blend` import versus an explicit export step

**Direct-import position.** Godot imports `.blend` by calling Blender's glTF exporter transparently. Fewer steps, no stale intermediate, no forgotten re-export.

**Explicit-export position.** It puts Blender on every team member's machine and every build agent as a hard dependency, hides the export settings inside a black box, and breaks on Android and web editors. An explicit export step is visible, configurable, and diagnosable.

## Anti-pattern catalog

| Pattern | Trigger | Consequence | Fix |
|---|---|---|---|
| `.meta` files gitignored | Copying a generic Unity gitignore | GUIDs regenerate per machine; every reference breaks unrecoverably | Commit `.meta`; gitignore `Library/` only |
| Editing the Model Prefab | Customizing an imported FBX in place | Work silently destroyed on every reimport | Create a Prefab Variant and instantiate that |
| DDC in source control | Treating it as project data | Enormous repo, no benefit, regenerable anyway | Gitignore it; use a shared DDC or a `.ddp` pak |
| Uncompressed textures shipped | Never configuring per-platform compression | Build size and streaming blowout | BC7/BC5/BC1 by role, or KTX2+Basis for one cross-platform build |
| Packed ORM tagged sRGB | Default color space on import | Three channels corrupted at once | Linear / Non-Color on every packed and data texture |
| `UCX_` hull that is not closed or not convex | Hand-modeled collision | Collision that almost works; unpredictable auto-decomposition | Verify closure and convexity; decompose manually |
| Collision suffix mismatched | Renaming the render mesh | Collision silently not applied | Name-match exactly; validate in CI |
| Auto-lightmap on bad channel-0 charts | Trusting auto-generation | Repacking cannot cut or split charts; artifacts persist | Split charts in the DCC before import |
| Influences above the platform cap | No cap in the DCC | Silent truncation; deformation differs with no error | Limit in the DCC; validate with `SkeletalMeshReport` or equivalent |
| 16-bit bone index enabled without reimport | Flipping the setting alone | Existing meshes stay 8-bit and stay clamped to 256 | Restart the editor and reimport affected meshes |
| Python-only Unreal validators in CI | Writing validators in Python | `-run=DataValidation` runs only C++ rules by default; CI passes vacuously | Register Python validators explicitly, or write rules in C++ |
| Archived glTF extensions in new assets | Old exporter or old preset | `pbrSpecularGlossiness` and `KHR_xmp` are archived paths | Move to the ratified metallic-roughness model |
| Inria-lineage splat code in a product | Reaching for SuGaR or 2DGS | Non-commercial research licence violation | Use `.spz` (MIT) or `gsplat` (Apache-2.0), or licence separately |
| Judging Unity package health by GitHub tags | Checking releases | False staleness reading; Package Manager versions separately | Check the package registry, not the repo tags |

## Severity rubric for this lens

- **blocker** -- unrecoverable data or reference loss, or a licensing violation that ships: uncommitted `.meta`, non-commercial-licensed code in a commercial product, redistribution violating an asset licence, a validation gate that passes vacuously.
- **major** -- ships visibly wrong or costs significant rework: influence truncation, collision that does not apply, color-space corruption, missing lightmap UVs where baked lighting is used, uncompressed textures blowing the size budget.
- **minor** -- inconsistency or avoidable cost: naming drift, an unnecessary format conversion, LOD chain gaps, a suboptimal compression choice.
- **nit** -- cosmetic convention: folder layout, non-load-bearing naming style.
- **insight** -- a structural reframe: this pipeline wants glTF rather than FBX given its determinism requirement; this manual step wants to be a CI validator; the material ownership question is unresolved and should be decided before the asset count grows.

## Authorities

- **The engines' own current documentation** -- Epic's `dev.epicgames.com` (note it is JavaScript-rendered; plain fetching returns a table-of-contents shell), Unity's manual, Godot's docs. Version-specific; read the version you target.
- **Khronos glTF specification and extension registry** -- the format and its ratification status.
- **OpenUSD documentation and the AOUSD glossary** -- composition semantics, LIVRPS.
- **Allar's ue5-style-guide** -- the de facto Unreal naming and structure convention.
- **Polycount wiki** -- accumulated game-art convention, strongest on normal maps and baking.
- **`mikktspace.h`** -- the tangent-space standard and its stated rationale.
- **Blender's `io_scene_fbx` source** -- the authoritative answer on what FBX export actually writes, since the binary format has no official specification.
- **Git LFS and Perforce documentation** -- binary version-control mechanics.

## Changelog

- **2026-09-05** -- Initial version. Verified by primary fetch and by reading source: Unreal 5.8 Nanite scope including full skeletal support, Interchange FBX still experimental, the DDC 5.3-to-5.4 default change, bone influence and index limits with the practical 12 cap and 75-bone mobile section limit, lightmap auto-generation repacking without splitting, Unity AssetDatabase v2 and Model Prefab semantics, Godot MD5-based reimport and the ufbx switch, FBX export non-determinism read from `export_fbx_bin.py`, BC and ASTC ratios and hardware support, the Inria 3DGS licence lineage, Epic's August 2026 divestiture of ArtStation and Sketchfab, and repository health across the open-source tool set. Practitioner-forum sources (Polycount, Blender Artists, Reddit) were unreachable during research, so convention claims sourced to community practice are weaker than the vendor-documented ones.
