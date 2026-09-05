---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-05
---

# Level design and spatial gameplay

A reference for advising on level layout, encounter design, pacing through space, player guidance, and the relationship between architecture and play. Used by the `level-design` subagent and the `/expert-review` / `/expert-plan` / `/expert-consult` skills.

The unifying thesis: **a level is an argument made in space about how the game should be played.** It cannot instruct, and anything it says explicitly it has already failed to say through form. The designer's tools are sightlines, thresholds, light, scale, silhouette, and the ordering of what a player can see and reach -- and the player reads all of them without noticing they are reading.

The operational question: **"where does the player look, where do they go, and did the space cause both without telling them?"**

Empirical priority, in rough order of how often it decides whether a space works: **readability and orientation > pacing and rhythm > encounter composition > navigation affordance > set dressing.** A beautiful space players get lost in has failed at the first item, and no amount of the last four repairs it.

## Volatile surface

`last-verified: 2026-09-05`. Level design principles are among the most durable material in this reference set, because they derive from human perception and locomotion rather than from technology. Engine-specific tooling is the exception.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Engine-specific tooling (World Partition, streaming, nav mesh) | Medium | The engine's docs; see `game-engines` |
| Genre conventions and current practice | Medium | Current shipped work |
| Perceptual and locomotion principles | Very slow | Durable |
| The named case studies | Very slow | Durable |

## Readability: the first obligation

A player who does not know where they are cannot enjoy anything else. Kevin Lynch's *The Image of the City* supplies the vocabulary, and it transfers to game space almost unchanged: **paths** (routes of travel), **edges** (boundaries), **districts** (regions with character), **nodes** (decision points and gathering places), and **landmarks** (external reference points). A space that gives players all five is navigable; one that gives only paths is a maze regardless of its size.

**Landmarks are the strongest single tool and the most underused.** A visible distant object that persists across the level gives players an absolute reference and converts wayfinding from memory into perception. This is why so many good open worlds have a mountain, a tower, or a spire visible from most of the map.

**Districts must differ perceptibly, not statistically.** Two regions with different asset sets but the same palette, silhouette language, and lighting read as the same place. This is the spatial form of Kate Compton's oatmeal problem: variation the designer can measure but the player cannot perceive.

**Landmark occlusion is the failure to check for.** A landmark that disappears behind geometry for most of the route is not doing its job. Verify from the actual player path and player height, not from a top-down editor view -- almost every orientation failure survives review because it was reviewed from above.

## Guidance without instruction

The craft is making players choose what the designer intended while believing they chose freely.

- **Light draws the eye.** The most reliable attractor there is. Players move toward brighter regions almost involuntarily, which is why a lit doorway needs no marker and why an unintentionally bright dead end is a real bug.
- **Contrast, motion, and saturation** work the same way. A moving element in a static scene captures attention absolutely; the most saturated object in a desaturated palette reads as important whether or not it is.
- **Leading lines and framing.** Architecture points. Corridors, railings, beams, and roads aim the eye, and a doorway frames what is beyond it as a subject.
- **The affordance must match the function.** A ledge that looks climbable and is not teaches players to distrust every ledge -- one inconsistency degrades the whole vocabulary. Consistency of affordance across a game matters more than the specific convention chosen.
- **Breadcrumbing and denial.** Showing a destination before granting access creates a goal; the classic form is a vista of a place the player will reach in twenty minutes.
- **Negative guidance.** What blocks, darkens, or narrows steers as strongly as what beckons, and more cheaply. A designer who only thinks in attractors ends up with a level of glowing arrows.

**Explicit markers are a confession.** Waypoints and objective arrows are sometimes the right choice, especially for accessibility and for open worlds where memorized layout is not the goal. But each one marks a place where form failed to communicate, and a level that needs them everywhere has a layout problem that markers only conceal.

## Pacing

A level is experienced in time, and a sequence of individually good encounters can still produce a bad hour.

**Intensity should vary deliberately.** The standard shape is a sawtooth -- rising tension, release, rising higher -- because sustained peak intensity stops registering as intense. Compression without release produces exhaustion and then indifference; the quiet section exists to make the loud one work.

**Rest is content, not the absence of it.** The corridor after the fight is where players process what happened, notice the environment, and reload their attention. Cutting the quiet parts to raise the density is the most common pacing error in level design.

**Rhythm at every scale.** Within an encounter (approach, engagement, resolution), between encounters, and across the level. A design that only paces one scale reads as monotonous at the others.

**Introduce, develop, twist, conclude.** The standard teaching structure for a mechanic, and the reason a well-designed level often feels like it was teaching without seeming to. Introduce a mechanic safely, develop it under pressure, combine it with something known, then test mastery.

**Player time is not designer time.** The designer knows the layout and traverses it in a third of the time. Every pacing judgment made in the editor is wrong by that factor.

## Encounter design

**Composition beats count.** An encounter is defined by the space it happens in, the sightlines available, the cover, the entry and exit points, the elevation, and the enemy composition -- roughly in that order. Placing the same enemies in a different room makes a different encounter; adding more of them usually does not.

**The space should suggest a plan.** Good encounter spaces let the player form an intention on approach -- flank left, take the high ground, break line of sight. Spaces that offer no readable plan produce reactive play, which is fine occasionally and dull as a default.

**Sightline management is the core skill.** What the player can see when they enter determines whether they get to plan or get ambushed, and both are valid choices that should be choices. A pre-entry vantage point converts an encounter from reactive to tactical.

**Cover geometry teaches the enemy's threat model.** Chest-high walls in a shooter are ridiculed and persist because they communicate "this is a ranged fight" instantly. Whatever the convention, it must be consistent.

**Entry and exit control the encounter's shape.** One entrance and one exit is a gauntlet; multiple entrances is an ambush; multiple exits gives the player agency about disengagement. Choose deliberately rather than inheriting whatever the geometry produced.

**Verticality multiplies options** and multiplies cost -- it complicates navigation meshes, breaks sightline assumptions, and makes readability harder. Worth it when the game's verbs use it, expensive when they do not.

## The blockout discipline

**Grey-box first, always.** Layout, scale, sightlines, and pacing are testable with no art, and testing them with art is more expensive and less honest -- art makes a bad layout feel better in review while playing exactly as badly.

**Metrics before geometry.** Character height, jump height and distance, crouch height, walk and sprint speed, weapon ranges, camera field of view. These define what a space means: a three-metre gap is a wall or a stride depending on the jump. **A level built before the metrics are locked will be rebuilt.**

**Scale is the most common blockout error, in both directions.** Spaces that read well in an editor are frequently too large at eye level, because the designer views them from above and from outside. First-person spaces in particular are usually built too big, and the correction is always to walk them.

**Playtest the blockout with people who did not build it.** The designer cannot get lost in their own level, and cannot un-know where the exit is.

## Architecture and games, honestly

The relationship is real and routinely overstated.

**What transfers.** Wayfinding and legibility -- Lynch was writing about cities and applies almost unchanged. Threshold and transition, compression and release (the low dark corridor into the high bright hall is the oldest trick in both fields). Sightline and framing. Prospect and refuge as a description of where people feel safe and where they feel exposed. Human-scale proportion as the reference against which everything else reads.

**What does not transfer.** Structural truth: game architecture carries no loads and can lie freely. Program and use: a real building serves daily life; a level serves one traversal. Code compliance: egress, fire ratings, and accessibility law govern buildings and do not govern levels, and applying them produces spaces that are realistic and dull. Durability and maintenance: a level is experienced once.

**The strongest form of the connection** is that both disciplines organize human movement and attention through space, and both are read by people who are not consciously reading. The weakest form is importing architectural vocabulary as decoration for design documents. Route the actual built-environment questions to `architectural-design`; borrow the perceptual principles freely.

## Schools of thought

### Guided versus open

**The guided position.** A designed sequence lets the designer control pacing, teach reliably, and land intended moments. Player freedom in the layout mostly means the ability to miss the good parts. Craft is the ability to make a linear space feel like a chosen path.

**The open position.** Space the player navigates by their own reading is the medium's distinctive capacity, and discovery cannot be authored -- only permitted. Guided levels produce competent experiences nobody remembers as *theirs*.

### Readability versus mystery

**The readability position.** Confusion is a failure state. A player who is lost is not exploring, they are stuck, and frustration reads as bad design rather than as depth.

**The mystery position.** Total legibility eliminates discovery. Not knowing what is around the corner is the engine of exploration, and a space that explains itself completely has nothing left to offer. Some of the most admired spaces in the medium are deliberately disorienting.

The reconciliation people reach for -- "legible at the macro scale, mysterious at the micro" -- is a useful default and not a resolution. The camps genuinely differ on whether being lost is a bug.

### Modular kits versus bespoke space

**Modular.** A snapping kit lets a small team build large spaces with consistent metrics and consistent texture density, and iteration is cheap because pieces are interchangeable.

**Bespoke.** Kits read as kits. Grid repetition is perceptible to players who cannot name it, and the memorable places in games are almost always the ones built once for one purpose.

### Realism versus playability in layout

**Realism.** Believable spaces -- offices with plausible floor plans, towns with sensible streets -- ground the fiction and make the world feel like a place rather than a set.

**Playability.** Real buildings have terrible sightlines, redundant corridors, and rooms that serve no play purpose. A realistic hospital is a bad level. The convention of the "gameplay space that reads as plausible" is a deliberate compromise, and the games that go furthest toward realism usually pay for it in navigability.

## Anti-pattern catalog

| Pattern | Trigger | Consequence | Fix |
|---|---|---|---|
| Reviewing from a top-down editor view | The natural way to look at a level | Scale, sightlines, and landmark occlusion all misjudged | Walk it at player height and speed |
| Art before layout is proven | Wanting it to look right early | Bad layouts survive review; rework is expensive | Grey-box and playtest first |
| Building before metrics are locked | Starting layout in parallel with mechanics | Every space is the wrong size; the level gets rebuilt | Lock character metrics first |
| Landmark occluded on the actual path | Placing it from an overview | Orientation fails exactly where it was meant to work | Verify visibility along the real route |
| Districts that differ only in assets | Reusing palette and silhouette | Regions read as the same place; players get lost | Vary silhouette, palette, and light |
| Waypoints compensating for layout | Playtesters get lost; a marker is added | The layout problem is hidden, not fixed | Fix the guidance; keep markers as an accessibility option |
| Unintentional attractor | A bright or high-contrast dead end | Players are drawn away from the intended route and blame themselves | Audit light and contrast as guidance, not just as mood |
| Inconsistent affordance | One climbable-looking ledge that is not | Players distrust the whole vocabulary thereafter | Consistency across the game, whatever the convention |
| Sustained peak intensity | Cutting the quiet parts for density | Intensity stops registering; exhaustion then indifference | Restore the rest; it is what makes the peaks work |
| More enemies instead of a different space | Encounter feels flat | Composition unchanged; the encounter is still flat | Change sightlines, cover, elevation, entries |
| Designer-time pacing judgments | Traversing a known layout | Everything is judged a third too fast | Test with players who do not know the space |
| Realistic floor plan as a level | Grounding the fiction | Redundant corridors, dead sightlines, no play | Plausible-reading gameplay space |

## Severity rubric for this lens

- **blocker** -- the space does not function: players cannot find the objective, a required route is not discoverable, a traversal is impossible with the actual character metrics, or the layout admits a sequence break that skips required content.
- **major** -- a systemic experience failure: orientation breaks down in a region, pacing produces a dead stretch, an encounter space contradicts the encounter's intent.
- **minor** -- local friction: one ambiguous affordance, a single awkward sightline, a landmark that occludes briefly.
- **nit** -- polish: set dressing, prop placement, cosmetic inconsistency.
- **insight** -- a reframe: this level is teaching a mechanic it never tests; this wants a landmark rather than a marker; this open area wants to be three connected spaces.

## Authorities

- **Kevin Lynch**, *The Image of the City* -- paths, edges, districts, nodes, landmarks. The most directly transferable text from urbanism to level design, and the vocabulary is worth using precisely.
- **Christopher Alexander**, *A Pattern Language* -- pattern thinking about space and its critiques; see `architectural-design` for the full argument, including the Alexander-Eisenman split.
- **The GDC Vault's level-design track** -- the field's primary literature; the postmortems are where practitioners are honest about what did not work.
- **Valve's developer commentary** (Half-Life 2, Portal) -- the most detailed public record of playtest-driven iteration on guidance and pacing, with the reasoning attached.
- **Naughty Dog, Arkane, and FromSoftware GDC talks** -- respectively the strongest public material on authored pacing, systemic space with multiple solutions, and interconnected world topology.
- **Prospect-refuge theory** (Jay Appleton, *The Experience of Landscape*) -- the underlying account of why certain vantage points feel safe and others exposed.
- **Steve Lee, Max Pears, and the practising level-design community** -- current craft discussion, closer to production reality than the academic material.

## Changelog

- **2026-09-05** -- Initial version. Principles are drawn from the field's canonical literature and from perceptual and locomotion constraints that do not rot. Lynch's categories are quoted precisely because they are frequently paraphrased into uselessness. Engine-specific tooling is deliberately excluded and routed to `game-engines`. The architecture relationship is stated with its limits, since the transfer is real but routinely overclaimed.
