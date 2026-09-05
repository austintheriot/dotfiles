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
- **The quarterly exclusion (below $10,000) is a cliff, not a deductible.** Cross it and the whole quarter's revenue is subject to royalty, not just the excess. This is the detail that makes the arithmetic surprising.
- There is a set of royalty-alternative exclusions preventing double payment where an advance or other fee obligation to Epic already applied.
- **Epic Games Store sales are excluded from Royalty Revenue entirely**, which is a stronger position than "royalties are waived."
- **A 3.5% tier exists** (EULA update 20) for "Launch Everywhere with Epic" -- an EGS release on every platform with content, feature, and marketing parity. iOS is temporarily waived given Apple's terms.
- Reporting is due within 45 days per quarter; late fees moved from a flat 2% to 30-day SOFR.

Source access is via GitHub after linking accounts. **Non-game seats are $1,850 per seat per year**, free below **$1M trailing-twelve-month group revenue including funding** -- now folded into the main EULA rather than a separate program. This matters for architectural visualization, film, and simulation work.

**As of 2026-09-05**: stable is **5.8** (released 2026-06-23), and it is the **last planned UE5 release** -- there is no 5.9 and no LTS programme. **UE6 was announced at Unreal Fest Chicago**, with Verse and Scene Graph replacing Blueprints and Actors, early access targeted around the end of 2027 and full release around 2029. A UE6 stream is already public on GitHub. Treat the Blueprint and Actor deprecation as a long-horizon planning consideration for a project expecting a decade of life, not a present concern.

Two current-version notes worth carrying: **PSO precaching is on by default in 5.8**, which is the real answer to UE5's shader-compilation stutter; and **Mass Entity / ECS remains experimental** after four years, so plans that assume it is production-ready are wrong.

**Fab is 88/12.** Megascans' free-for-all ended 2024-12-31 and Quixel content is now paid. **Epic sold ArtStation and Sketchfab to KitBash on 2026-08-10**, partially unwinding the 2024 consolidation.

### Unity

**The Runtime Fee is cancelled** -- announced September 2023, and **fully cancelled 2024-09-12**, replaced with seat-price increases (Pro +8% and Enterprise +25% effective 2025-01-01). Any advice premised on install-based fees is stale.

**As of 2026-09-05, verified at unity.com/pricing**: Personal is **free below $200K** total finances trailing twelve months; **Pro is $210/month or $2,310/year per seat**; Enterprise is custom above $25M; Industry is custom for non-games above $1M. Current LTS is **6.3** (released 2025-12-03, supported to December 2027), with 6.4 as a Supported Update rather than an LTS. **Pro has been priced up twice in two years -- treat this as high volatility and re-verify before quoting.**

**The attribution trap is separate from the splash screen.** The "Made with Unity" *splash screen* became optional for Personal on Unity 6+. But the Editor Software Terms separately require a **credits attribution** with no tier exemption found. Disabling the splash does not discharge the attribution obligation, and teams routinely assume it does.

Structurally: subscription per seat by tier, with a revenue and funding ceiling on the free Personal tier. The technical facts that persist: IL2CPP for ahead-of-time compilation to native targets, the ECS/DOTS stack as a separate performance-oriented path with its own learning curve and maturity questions, and a large asset ecosystem whose licence terms are per-asset and worth reading -- **Unity Asset Store Extension Assets are single-seat and two-computer, and there is no guaranteed continued access if a publisher delists.**

**The migration wave did not happen, and this is worth stating plainly because the opposite is widely assumed.** GDC's State of the Game Industry shows Unity at 33% (2024), 32% (2025), 30% (2026); Godot moved 4% to 5%. GDC's own 2025 commentary: "it appears that few developers have moved on from Unity." The 2026 figure showing Unreal at 42% against Unity's 30% reflects a **narrowed sample** (hands-on roles only, 3,000 down to 2,300) rather than a measured shift. The real split is by segment: Global Game Jam Unity usage fell from 61% to 36%, so **hobbyists moved and professionals largely did not.** Money moved to Godot; developers did not.

### Godot

**MIT licensed**, which is the whole argument. No royalties, no seats, no revenue thresholds, no vendor able to change the terms. For a studio that has lived through a licensing shock, this is not a minor consideration.

**As of 2026-09-05**: 4.7.2 stable, with 3.6.3 still maintained. Governance sits with the **Godot Foundation** (Stichting Godot, a Dutch nonprofit formed 2022-08-23), which notably **does not own the Godot Project**. **W4 Games is a separate commercial entity** providing console porting as approved middleware -- the practical answer to Godot's console gap, since the engine itself cannot ship platform SDKs under an open licence.

**MIT's practical scope**: no royalties, no thresholds, close the source freely, modify without publishing, and **static versus dynamic linking is irrelevant**. The only obligation is reproducing the licence text, which a credits screen, a licences menu, an output log, or a link all satisfy. **The real compliance risk is bundled third-party dependencies, not Godot's own MIT.**

The real constraints, verified:

- **C# is second-class.** It is **not supported on web at all** (the tracking issue has been open since 2023-01-01, and the documentation suggests considering Godot 3 instead), and remains experimental on Android and iOS. Recent releases shipped little for C#.
- **The renderer split is a structural cost**: Forward+ is desktop-only, and web is Compatibility-only -- no screen-space reflections, volumetric fog, SDFGI, temporal antialiasing, or FSR2.
- **Consoles cannot be fixed.** MIT and platform NDAs are legally incompatible, so this is permanent rather than a roadmap item. **W4 Consoles is the path: $800/year per platform or $2,000/year for all platforms** below $300k revenue and 30 employees, rising to $4,000/$10,000 for Pro, **with no revenue share.** Switch, Switch 2 (beta), PS5, and Xbox Series X|S.
- Jolt became the default 3D physics engine in 4.6.
- Funding runs about **EUR 35,514/month across 1,828 members and 23 corporate sponsors**, roughly 2.3 times Bevy's funding with 8 times the donors.

**Redot** is a live fork -- a US nonprofit since February 2026 with quarterly LTS releases -- but it tracks Godot 4.5, two releases behind, while simultaneously running a hard fork and a from-scratch engine, on volunteer labour, with no commercial shipped game found. Treat it as a governance controversy with an uncertain technical future.

### Bevy

**The most important single fact: after six years and 19 releases there are no published 1.0 criteria** -- not in the release posts, the birthday post, or the README. If a team is waiting for 1.0 to signal stability, **there is no signal coming.**

The churn is the defining property and it is not tapering: the 0.18-to-0.19 migration guide runs roughly 80 to 90 breaking changes across 15,000-plus words, including Resources becoming Components and the render graph becoming ECS schedules. **Bevy's own documentation recommends Godot for production stability** -- take that at face value. There is still no editor ("very close," six years running), no console path (the Rust console toolchain does not exist), and experimental web support. Funding is about $16,901/month, and the Foundation states developers earn roughly 54% below market. The rendering work is genuinely impressive and much of it is experimental.

### Others worth knowing

**Bevy** (Rust, ECS-first) is pre-1.0 with a migration guide per release -- the churn is the defining fact, and it is a deliberate tradeoff for architectural freedom. **GameMaker** changed to free for non-commercial use in 2023; verify current commercial terms. **Defold** (King) is free. **O3DE** (Apache-2.0) is the Lumberyard successor. **Construct 3**, **RPG Maker**, **Ren'Py**, and **Twine** are genre-specific and often the correct answer for their genre. **LÖVE**, **raylib**, **SDL3**, **MonoGame**, **libGDX**, and **Heaps** are frameworks rather than engines -- no editor, more control, more work. **Phaser**, **PlayCanvas**, **Three.js**, and **Babylon.js** are web-first.

A licence correction worth carrying because it is routinely stated wrong: **MonoGame is Microsoft Public License, not MIT.** Ms-PL is file-level weak copyleft with a patent grant and is not interchangeable with MIT for compliance purposes. Similarly **Flax is source-available rather than OSI-licensed, with a 4% royalty above $250,000 per calendar quarter** -- per quarter, not lifetime, which is a materially different shape from Unreal's. **Ren'Py's core is MIT but its dependencies are LGPL**, so commercial shipping carries relinking obligations that are routinely skipped. **Construct 3 is subscription-only with no perpetual licence**, meaning project files are effectively rented. **PlayCanvas's engine is MIT but its cloud editor is proprietary, and private projects lock on cancellation** -- an MIT engine does not protect your data.

## Licensing traps

These sit outside the engine's own terms and are where projects actually get hurt.

**Audio middleware is per-title, and sequels need new licences.**

- **FMOD**: Indie free or $2,000, Basic $6,000, Premium $18,000 -- **per game, all platforms included, consoles free.** Logo waivers cost extra. The free tier has **two triggers**: under $600k budget **and** under $200k/year company revenue, with funding counting toward revenue.
- **Wwise**: Indie free, Pro $8,000, Premium $25,000, Platinum $45,000 -- but **priced per platform**, so Pro across three consoles is $24,000 rather than $8,000. A 1% gross royalty alternative exists. **The old 500-asset limit no longer exists** on commercial tiers; the 200 cap is trial-only.

**FFmpeg is the sharpest trap in the list.** Building with `--enable-gpl` or `--enable-libx264`/`libx265` flips the entire binary to GPL, and statically linking that into a console build violates on several axes at once, silently. FFmpeg's own position is that it "is not available under any other licensing terms... not even in exchange for payment."

**LGPL static linking is effectively unsatisfiable on consoles and iOS**, because you cannot grant relinking rights on a signed, locked platform. This is a structural incompatibility, not a paperwork problem.

**Fonts are the most-violated licence in games.** Desktop licences do not cover binary embedding -- that needs an app or embedding licence -- and an SDF atlas is a derivative work. The Open Font License is the safe default, with the caveat that its Reserved Font Name clause bites when you subset.

**Creative Commons** carries two specific hazards: BY-NC forbids commercial games outright, and BY-SA can infect an OST release if you arrange the track.

## Deployment reality

**Console is a gate, not a build target.** Shipping to Switch, PlayStation, or Xbox requires being an approved developer, signing NDAs, and obtaining devkits. The engine question is downstream of that. Unity and Unreal have first-party console support available to approved developers. **Godot's path is through commercial porting partners** such as W4 Games, operating as approved middleware -- workable, and a cost and dependency that an MIT licence does not remove.

Two console specifics worth knowing: **Nintendo was refusing Switch 2 devkits to indies as of August 2025**, directing them to Switch 1 backward compatibility, and **ID@Xbox provides two free devkits**, the lowest barrier of the three platform holders.

**Web is where scripting-language choice becomes a deployment constraint.** WebAssembly is the target; the gaps are specific and persistent. C# on the web is Godot's outright hole -- unsupported, not merely limited. Threads require SharedArrayBuffer, which requires COOP and COEP headers, a hosting constraint teams discover late. iOS Safari imposes its own limits, and download size gates conversion far more aggressively than on desktop.

**The 3D-on-web gap has genuinely closed, though**: WebGPU now ships in all four major browsers -- Chrome since 2023, **Safari 26 in June 2025, Firefox 141 in July 2025**. Advice written before mid-2025 understates what the web can do.

**Mobile economics were rewritten in 2025 and 2026, and this is the fastest-moving part of the deployment picture.**

- **Epic v Google settled 2026-03-04**: Google's cut is now **at most 20%**, third-party stores are permitted globally, and Fortnite returned to Play on 2026-03-19. **The flat 30% is gone on Android.**
- **Epic v Apple is unsettled.** An April 2025 contempt ruling barred Apple from taking a fee on external link-outs, and **the Supreme Court granted certiorari for the 2026 term.** Do not treat US iOS terms as stable, and verify Apple's Core Technology Fee position directly -- it was not confirmed in this pass.
- Apple's standard terms remain 30%, 15% for subscriptions after year one, and 15% under the Small Business Program below $1M.

Mobile also brings store-imposed deadlines that arrive whether or not the project is ready: target API levels, 64-bit mandates, size limits, privacy manifests. Those are release-engineering concerns (see `platform-release`) but they constrain engine and plugin choice early.

**Store terms**: Steam Direct is $100, refunded above $1,000, with the 30/25/20 tiers and the two-hour/fourteen-day refund window unchanged. Epic Games Store and Fab are both 88/12. **itch.io defaults to 10% and is creator-settable to zero**, but it delisted thousands of adult games on 2025-07-24 under payment-processor pressure -- a reminder that platform risk is not only about revenue splits.

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

- **2026-09-05** -- Initial version. Unreal royalty terms read directly from the EULA text: the 5% rate, the per-product $1,000,000 lifetime exclusion, the $5,000,000 Oculus Store exclusion, the quarterly exclusion as a cliff rather than a deductible, the 3.5% "Launch Everywhere with Epic" tier, and EGS revenue being excluded entirely. Unity pricing verified at unity.com/pricing (Personal free below $200K; Pro $210/month or $2,310/year; 6.3 LTS). Godot 4.7.2, its MIT scope, the C#-on-web gap, the renderer split, and W4 console pricing verified. Middleware, font, FFmpeg, and LGPL traps verified.

  Three corrections to widely-held premises are recorded in the body because they change advice: **the Unity-to-Godot migration wave did not happen** among professionals (GDC's own data shows Unity 33/32/30% across 2024-2026, with the apparent 2026 shift explained by a narrowed sample); **UE 5.8 is the last planned UE5 release** with UE6 targeting roughly 2029; and **MonoGame is Ms-PL, not MIT.**

  **Known gaps**: Apple's Core Technology Fee in its current form, Fab's Personal-versus-Professional revenue threshold, and Roblox's DevEx rate were not verifiable in this pass. Two research hazards worth remembering for the next refresh: gamedeveloper.com and pocketgamer.biz return HTTP 200 for nonexistent pages while serving unrelated articles, so verify page identity by headline rather than status code; and GDC Vault is not machine-fetchable, so talk citations need a different route.
