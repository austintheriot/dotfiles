---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-05
---

# Game engines and game-making tools

A reference for advising on engine and tool selection, licensing and cost structure, deployment targets, and migration risk. Used by the `game-engines` subagent and the `/expert-review` / `/expert-plan` / `/expert-consult` skills.

The unifying thesis: **engine choice is a procurement decision wearing a technical costume.** The technical differences between mature engines are real but rarely decisive; what is decisive is the licence you can live with, the platforms you can actually ship to, the talent you can hire, and how much of your work is portable when the answer changes. Teams argue about renderers and then get hurt by a royalty threshold, a console-certification gate, or a scripting language with no web export.

The operational question: **"what does this choice cost, what does it foreclose, and what survives if we have to change it?"**

Empirical priority, in rough order of how often it hurts: **licence and cost structure > deployment-target reality > team capability and hiring > lock-in and migration cost > engine capability.** Capability is last because for most projects every mature engine is capable enough, and it is where teams spend their argument.

## Volatile surface

`last-verified: 2026-09-05`. **Pricing and licensing are the fastest-rotting facts in this file and also the most consequential.** The Unity Runtime Fee episode is the cautionary case: announced September 2023, walked back, and fully cancelled in September 2024 in favour of seat-price increases. Anyone advising from a 2023 recollection would have been confidently wrong for a year.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Pricing tiers, seat costs, revenue thresholds | **Very fast** | Each vendor's own pricing page |
| Royalty terms and exclusions | Medium | The current EULA text, not a summary |
| Engine versions and feature status | Fast | Release notes |
| Console support paths and middleware partners | Medium | The porting partner and platform holder |
| Store revenue splits and policies | Medium | Each store's terms |
| Governance and ownership | Slow but consequential | Primary reporting |

**Always read the actual licence text rather than a summary**, including a summary in this file. The numbers below were read from primary sources on the verification date and are reproduced with that caveat.

## The licence and cost axis

### Unreal Engine

**Verified from the EULA text (2026-09-05).** The royalty is **5% of Royalty Revenue**, defined as worldwide gross revenue attributable to each Royalty Product minus allowed exclusions, "regardless of whether that revenue is received by you or any other person or entity."

The exclusions matter as much as the rate:

- **The first $1,000,000 in lifetime gross revenue for each Royalty Product is excluded.** Per product, not per company. The EULA states plainly that "you will never owe us royalty payments under this Agreement unless a Product directly generates more than $1,000,000 USD in gross revenue."
- **The first $5,000,000 in lifetime gross revenue from the Oculus Store** (or its successor) is excluded. This is a substantially larger threshold that most summaries omit, and it materially changes the economics of a VR title.
- There is also a quarterly exclusion and a set of royalty-alternative exclusions preventing double payment where an advance or other fee obligation to Epic already applied.
- Royalties are waived for games published exclusively on the Epic Games Store.

Source access is via GitHub after linking accounts. Since 2023 the terms differentiate non-game industries, which matters for architectural visualization, film, and simulation work.

**As of 2026-09-05**: stable is **5.8**. **UE6 was announced 2026-05-24**, with early access targeted "late 2027-ish" and full release 12 to 18 months after. Rocket League is the first confirmed title moving to it. **Blueprints and Actors are slated for eventual replacement by Verse and Scene Graph** -- a long-horizon planning consideration rather than a present concern, but a real one for a project starting now that expects a decade of life.

### Unity

**The Runtime Fee is cancelled.** Announced September 2023, it triggered the largest developer backlash in the engine's history, was revised, and was **fully cancelled in September 2024**, replaced with seat-price increases. Any advice premised on install-based fees is stale. **Re-verify current seat pricing, the Personal tier's revenue cap, and splash-screen requirements at unity.com/pricing before quoting numbers** -- these are the fastest-moving figures in this file.

Structurally: subscription per seat by tier, with a revenue and funding ceiling on the free Personal tier. The technical facts that persist: IL2CPP for ahead-of-time compilation to native targets, the ECS/DOTS stack as a separate performance-oriented path with its own learning curve and maturity questions, and a large asset ecosystem whose licence terms are per-asset and worth reading.

### Godot

**MIT licensed**, which is the whole argument. No royalties, no seats, no revenue thresholds, no vendor able to change the terms. For a studio that has lived through a licensing shock, this is not a minor consideration.

**As of 2026-09-05**: 4.7.2 stable, with 3.6.3 still maintained. Governance sits with the **Godot Foundation** (Stichting Godot, a Dutch nonprofit formed 2022-08-23), which notably **does not own the Godot Project**. **W4 Games is a separate commercial entity** providing console porting as approved middleware -- the practical answer to Godot's console gap, since the engine itself cannot ship platform SDKs under an open licence.

The real constraints: **C# web export has been the persistent gap**, GDExtension is the native-extension path, and the mobile and .NET story is worth verifying against the current release rather than assumed. The "Redot" fork exists and is a governance controversy rather than a technical one.

### Others worth knowing

**Bevy** (Rust, ECS-first) is pre-1.0 with a migration guide per release -- the churn is the defining fact, and it is a deliberate tradeoff for architectural freedom. **GameMaker** changed to free for non-commercial use in 2023; verify current commercial terms. **Defold** (King) is free. **O3DE** (Apache-2.0) is the Lumberyard successor. **Construct 3**, **RPG Maker**, **Ren'Py**, and **Twine** are genre-specific and often the correct answer for their genre. **LÖVE**, **raylib**, **SDL3**, **MonoGame**, **libGDX**, and **Heaps** are frameworks rather than engines -- no editor, more control, more work. **Phaser**, **PlayCanvas**, **Three.js**, and **Babylon.js** are web-first.

**Middleware licensing is a frequently-missed cost.** FMOD and Wwise both have their own tiers and thresholds, and a project can clear an engine's royalty threshold while tripping a separate audio-middleware one.

## Deployment reality

**Console is a gate, not a build target.** Shipping to Switch, PlayStation, or Xbox requires being an approved developer, signing NDAs, and obtaining devkits. The engine question is downstream of that. Unity and Unreal have first-party console support available to approved developers. **Godot's path is through commercial porting partners** such as W4 Games, operating as approved middleware -- workable, and a cost and dependency that an MIT licence does not remove.

**Web is where scripting-language choice becomes a deployment constraint.** WebAssembly is the target; the gaps are specific and persistent. C# on the web has been Godot's notable hole. Threads require SharedArrayBuffer, which requires COOP and COEP headers, which is a hosting constraint many teams discover late. iOS Safari imposes its own limits, and download size gates conversion far more aggressively than on desktop.

**Mobile** brings store-imposed deadlines that arrive whether or not the project is ready: target API level requirements, 64-bit mandates, size limits, and privacy-manifest obligations. These are release-engineering concerns (see `platform-release`) but they constrain engine and plugin choice early.

**Store terms** are a real line in the model: the conventional 30% platform cut, with variations and exceptions worth checking per store rather than assumed.

## Lock-in and what survives

The honest inventory. **Portable**: art assets in interchange formats, audio, design documents, level layouts if authored in a neutral format, algorithms as ideas, and the team's understanding of the design. **Not portable**: engine-specific code, editor tooling, Blueprints and visual scripts, prefab and scene structure, physics tuning, shader graphs, and every integration with an engine-specific service.

**The practical consequence is that porting a shipped game between engines is usually a rewrite with an asset library attached.** Teams underestimate this consistently because the assets are the visible part. The mitigation is architectural rather than procedural: keeping game logic in plain code with a thin engine adapter costs discipline up front and is the only thing that makes a later move tractable. Few teams do it, and most who did not, regret it only once.

## Schools of thought

### Engine choice matters versus engine choice is not the bottleneck

**The it-matters position.** The engine determines your iteration speed, your platform reach, your hiring pool, your cost structure, and what is even possible in your renderer. Choosing wrong means either shipping something worse or paying a rewrite. The decision deserves the scrutiny it gets.

**The not-the-bottleneck position.** Almost every failed project failed on scope, design, or funding, not on engine capability. Every mature engine can ship every common genre. The time spent on engine comparison is time not spent on the prototype that would have revealed the design problem. Pick the one your team already knows and start.

Both are arguing from real evidence. The it-matters camp is usually thinking about licence and platform reach, which genuinely bite; the not-the-bottleneck camp is usually thinking about renderer features, which usually do not.

### Custom engine versus off-the-shelf

**The custom position.** You own the whole stack, there are no royalties, no vendor can change terms under you, and you can build exactly the tool your game needs. For a studio making several games in one genre, the tooling investment amortizes. The case is strongest where the game's core mechanic is something no commodity engine does well.

**The off-the-shelf position.** Writing an engine is writing an editor, an asset pipeline, a renderer, a physics integration, a platform layer, and console ports -- years of work that ships no game, and all of it must be maintained. The custom-engine studios people cite are studios that could afford it. Most who try it ship an engine and no game.

### Open source versus commercial licence

**The open-source position.** The Unity Runtime Fee episode proved the risk is not theoretical: a vendor can change the terms of a project already in production. An MIT licence cannot be revoked, and the source is there when the engine is wrong.

**The commercial position.** The licence is not what you are buying. You are buying console support, a battle-tested renderer, middleware integrations, a hiring pool, and someone to call. Godot's console story runs through a commercial third party anyway, so the difference is which company you depend on and under what terms.

### Visual scripting versus code

**The visual-scripting position.** Blueprints let designers and artists iterate without an engineer in the loop, which is where iteration speed actually comes from on a content-heavy game.

**The code position.** Visual scripts diff badly, merge worse, and resist the refactoring that any long project needs. Beyond a threshold they become unmaintainable graphs that only their author understands. Epic's own long-horizon plan to replace Blueprints with Verse is a data point neither camp can ignore.

## Anti-pattern catalog

| Pattern | Trigger | Consequence | Fix |
|---|---|---|---|
| Quoting stale pricing | Advising from memory | Confidently wrong on the most consequential axis | Read the vendor's current page; the Runtime Fee saga is the cautionary case |
| Reading a royalty summary, not the EULA | Reasonable shortcut | Missing exclusions (the per-product $1M, the $5M Oculus Store) that change the model | Read the licence text |
| Treating console as a build target | Planning from the engine's feature matrix | Platform-holder approval and devkits gate everything; timelines slip by months | Establish the approval path before committing |
| Ignoring middleware licensing | Focusing on the engine's terms | A separate audio or physics threshold trips independently | Inventory every licensed component |
| Assuming web export works | Seeing "web" in the feature list | Scripting-language gaps, header requirements, size limits appear late | Prototype the actual target early |
| Engine-coupled game logic | Natural way to build in an editor | A later engine change becomes a rewrite | Thin adapter; keep logic in plain code |
| Choosing for a feature the project will not use | Comparing renderer feature matrices | Optimizing the axis that does not bind | Decide on licence, platforms, and team first |
| Free tier without reading the ceiling | Prototyping under the free licence | Revenue or funding threshold crossed mid-project; forced migration under pressure | Know the ceiling before it matters |
| Splash-screen and attribution assumptions | Assuming the free tier is unbranded | Late discovery of a branding requirement | Verify current terms |

## Severity rubric for this lens

- **blocker** -- a licence violation, a threshold already crossed without compliance, or a deployment target that is structurally impossible with the chosen stack.
- **major** -- a cost or platform constraint that will force a painful change: an unread revenue ceiling, a console path with no partner, a scripting choice that forecloses a required target.
- **minor** -- avoidable cost or friction: middleware overlap, an inefficient tier choice, tooling that will need replacing.
- **nit** -- preference: editor ergonomics, language taste where both work.
- **insight** -- a structural reframe: this project does not need an engine; this logic should sit behind an adapter; this genre has a purpose-built tool that would halve the work.

## Authorities

- **The actual EULA and pricing pages** of each vendor. Nothing else is authoritative, including this file. Read them at decision time.
- **Epic's Unreal Engine EULA and Royalty Addendum** -- the royalty structure and its exclusions are stated precisely and are widely misquoted.
- **Godot's documentation and the Godot Foundation's own statements** -- for the governance structure, which is regularly misdescribed.
- **Platform holders' developer programmes** -- the authoritative answer on console access, and not something a third party can summarize reliably.
- **Primary reporting on the Unity Runtime Fee episode (2023-2024)** -- the case study in vendor-terms risk, and the reason "read the current licence" is the first rule here.

## Changelog

- **2026-09-05** -- Initial version. Unreal royalty terms verified by reading the EULA text directly, including the 5% rate, the per-product $1,000,000 lifetime exclusion, the $5,000,000 Oculus Store exclusion, and the quarterly and royalty-alternative exclusions. Unreal 5.8 current with UE6 announced 2026-05-24; Godot 4.7.2 with 3.6.3 maintained; Godot Foundation governance and its separation from W4 Games verified. **Unity's current seat pricing and Personal-tier ceiling were not re-verified in this pass and must be checked before quoting** -- the file records the Runtime Fee's cancellation (September 2024) but not current figures. Store revenue splits and middleware thresholds are stated structurally rather than numerically for the same reason.
