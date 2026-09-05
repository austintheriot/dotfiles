---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-05
---

# Game mechanics and systems design

A reference for advising on gameplay systems: core loops, progression, economy, randomness, difficulty, game feel, monetization design, and playtesting. Used by the `game-mechanics` subagent and the `/expert-review` / `/expert-plan` / `/expert-consult` skills.

The unifying thesis: **a game is a system that produces experiences, and the designer controls the system, not the experience.** You cannot author fun directly. You author rules, and the rules generate dynamics, and the dynamics produce whatever the player actually feels -- which is frequently not what was intended. Every serious framework in the field is a way of reasoning backwards from a desired experience to the mechanics that might produce it, and every serious designer will tell you the loop only closes by watching people play.

The operational question: **"what does this system actually incentivize, and is that what was intended?"**

Empirical priority, in rough order of how often it decides whether a design works: **the moment-to-moment feel of the core action > the core loop's shape > incentive alignment in the systems layered on it > progression and economy tuning > content volume.** Teams usually work this list backwards, adding content to a loop whose central verb does not feel good.

## Volatile surface

`last-verified: 2026-09-05`. Design theory is among the most durable material in this reference set; the regulatory and platform layer is not.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Loot box and gacha regulation by jurisdiction | Fast | The regulator's own publications |
| Platform disclosure rules for randomized monetization | Fast | Each store's policy page |
| Live-service norms and monetization conventions | Medium | Current market observation |
| Design frameworks, canonical books and talks | Very slow | Durable |
| The named critiques and disagreements | Very slow | Durable |

## Frameworks worth actually using

**MDA (Mechanics, Dynamics, Aesthetics)** -- Hunicke, LeBlanc, and Zubek. The core insight is directional: **the designer builds mechanics and reads them forward; the player encounters aesthetics and reads them backward.** The two parties experience the game from opposite ends, which is why designers systematically misjudge how their systems feel. Its practical use is as a diagnostic -- when an experience is wrong, MDA asks which layer to change, and the answer is usually not the one that is wrong.

The standard critique is that "aesthetics" is doing too much work as a category and that MDA under-describes the authored and narrative parts of games. **DDE (Design, Dynamics, Experience)** is one response. Marc LeBlanc's **eight kinds of fun** (sensation, fantasy, narrative, challenge, fellowship, discovery, expression, submission) is the more useful half of the framework in practice, because it forces the question "which of these is this game actually for" and most weak designs are chasing three at once.

**Sid Meier: "a game is a series of interesting decisions."** The most quoted line in design, and the operative word is *interesting*: a decision is interesting when the options are meaningfully different, the outcome is uncertain, and the player has enough information to have an opinion. Most bad mechanics fail one of those three, and naming which one is a complete diagnosis.

**Raph Koster, *A Theory of Fun*.** Fun is the feeling of the brain learning a pattern. The consequence is uncomfortable and correct: **fun has an expiry date.** Once the pattern is mastered the fun stops, and the design question becomes what the game offers after mastery -- new patterns, social context, expression, or nothing.

**Csikszentmihalyi's flow** and the flow channel between anxiety and boredom is the standard difficulty model. Its limitation is that it describes a single-axis skill match, and real games ask for several skills at once; a player can be bored on one axis and overwhelmed on another simultaneously.

**Jesse Schell's lenses** (*The Art of Game Design*) are the best interrogation toolkit in the field -- a hundred-odd reframings to apply to a stuck design.

**Doug Church's Formal Abstract Design Tools** made the argument that the field needs a shared vocabulary for design components, which is still only half won.

**Jesper Juul's emergence versus progression** distinguishes games whose content comes from rule interaction from games whose content is authored in sequence. It predicts where a design's effort goes, and it explains why the two kinds fail differently: emergent games fail by producing dull interactions, progression games by running out of content.

**Steve Swink's *Game Feel*** and the **Jonasson and Purho "Juice it or lose it"** talk cover the layer that most determines whether a prototype reads as good: the response of the game to input, independent of the rules. This is the layer teams skip and then cannot explain why the game feels bad.

## Game feel: the layer that decides prototypes

The central action of a game is performed thousands of times, and how it *feels* is largely independent of how it is *modelled*. The techniques are specific and cheap:

- **Input buffering.** Accepting an input slightly before the system can act on it and applying it when it can. Without it, a correct input during an animation is dropped and the player blames themselves, then the game.
- **Coyote time.** A short grace window after leaving a platform during which a jump still registers. Named for the cartoon. It makes a jump feel fair because human perception of "I was still on the edge" does not match the simulation's.
- **Hit stop / hit pause.** Freezing for a few frames on impact. Communicates weight more effectively than any amount of force modelling.
- **Screen shake, particles, sound layering, animation anticipation and follow-through.** The "juice" catalogue. Cheap, and the difference between a prototype that reads as broken and one that reads as good.
- **Cancel windows and frame data.** In combat, when a player can interrupt an action determines whether the system feels responsive or committed. Both are valid; the wrong one for the intended feel is not.

**This class of technique is invisible forgiveness: the game is lying slightly in the player's favour, and telling them would break the effect.** That is not deception in the ethical sense; it is compensating for the gap between simulation and perception.

## Randomness

Players do not experience probability; they experience streaks. Designers who model randomness as fair independent trials and stop there ship systems that feel broken to people who are correct about what happened to them.

- **True random** produces clumping. Seven misses in a row at 70% hit chance happens routinely and feels like a bug.
- **Shuffle bags / bounded distributions** draw from a shuffled set rather than sampling independently, guaranteeing distribution over a window. Tetris piece randomizers are the canonical case.
- **Pseudo-random distribution** (as in Dota's crit model) escalates probability on each failure and resets on success, producing a tighter distribution around the nominal rate with no long droughts.
- **Pity timers** guarantee an outcome after N failures. Universal in gacha systems, and the reason they are tolerable at all.
- **Perceived fairness diverges systematically from actual fairness.** Players over-remember losses, expect small samples to look like large ones, and read agency into noise. Designing for felt fairness means deliberately deviating from uniform randomness, and saying so in the design rather than pretending the model is neutral.

## Progression, economy, and feedback loops

**Sinks and faucets.** Any persistent economy needs currency creation (faucets) and destruction (sinks) in balance. Under-sinked economies inflate until early content is meaningless and new players cannot participate. This is the most common failure in long-running games with player markets, and it is usually diagnosed a year late.

**Feedback loops.** Positive loops (winning makes winning easier) accelerate outcomes and create runaway leaders and unwinnable comebacks. Negative loops (rubber-banding, catch-up bonuses) keep contests close at the cost of making early advantage feel meaningless. Neither is correct in the abstract: a strategy game usually wants positive loops so that play matters, and a party racing game usually wants negative loops so the last player stays engaged. **Naming which loop a system creates, and confirming it is the intended one, catches a large fraction of balance problems before tuning starts.**

**Progression curves** are a promise about pacing. Exponential costs with linear rewards produce a wall; the interesting question is always what the player is meant to be doing at the point where the curve bends.

**Difficulty and dynamic adjustment.** Explicit difficulty settings put the choice with the player; dynamic adjustment hides it. Hidden DDA carries a specific ethical problem: if a player's achievement was partly the system easing, and they do not know, the accomplishment they feel is not the one they had. The counter-position is that all difficulty tuning is invisible authorship and DDA is only more responsive.

## Monetization as a mechanics question

Monetization design is systems design, and treating it as a business-layer concern bolted on afterwards is how games acquire incentive structures that fight their own play.

The structural question is always: **does the monetization pull in the same direction as the fun, or against it?** A cosmetic economy in a game about self-expression is aligned. An energy timer in a game about flow is directly opposed -- it monetizes by interrupting the thing the player came for.

**Randomized monetization** (loot boxes, gacha) sits at the intersection of the randomness section above and the regulatory layer. **The regulatory picture is genuinely fragmented and moving**: several jurisdictions have acted, disclosure requirements exist in some markets, rating boards apply labels, and consultations continue elsewhere. **Verify the current position for the specific jurisdictions a title ships in rather than working from a general impression** -- this is the fastest-rotting material in this file, and getting it wrong is a compliance problem rather than a design one.

**The dark-pattern taxonomy in games** (Zagal, Björk, and Lewis, "Dark Patterns in the Design of Games") gives the field its vocabulary: grinding, playing by appointment, pay to skip, social pyramid schemes, impersonation of social obligation, and premium currency as an abstraction that obscures real cost. Its value is that it names the pattern independently of intent, so a design can be assessed on what it does to players rather than on what the team meant.

## Playtesting

**Watch someone play and say nothing.** The single most valuable practice in the field, and the hardest to do. The instinct to explain contaminates the only signal that matters -- what a person does without help. Everything the designer wants to say during a test is a note about something the game failed to communicate.

**What players report and what players do diverge.** Players are reliable reporters of where they felt something and unreliable diagnosticians of why. "The level was too hard" often means "I did not understand what I was supposed to do." Take the location seriously, take the explanation as a hypothesis.

**Paper prototypes** answer rules questions in hours rather than weeks, and are underused for anything with a turn structure or an economy.

**Vertical slice** answers "does this feel good," not "is there enough of it." Confusing the two produces polished demos on top of designs with no second hour.

**Telemetry** answers where, not why. Funnels, retention curves, and drop-off points locate a problem precisely and say nothing about its cause; the pairing with qualitative observation is what makes either useful.

## Schools of thought

### Systemic and emergent versus authored and narrative

**The systemic position** (immersive sim lineage -- Warren Spector, Harvey Smith, Looking Glass and Arkane). Build consistent, interacting systems and let players find solutions the designers did not anticipate. The player's authorship is the point, and the memorable moments are the ones nobody scripted. Authored sequences are expensive, single-use, and break the moment a player tries something reasonable that was not foreseen.

**The authored position.** Emergence is uneven by construction: it produces brilliant moments and long stretches of nothing, and a designer cannot promise an experience they do not control. The most affecting moments in the medium are authored, and "the player makes their own story" is frequently a rationalization for not having written one.

### Mechanics first versus theme first

**Mechanics-first** (the Vlambeer school -- Rami Ismail, Jan Willem Nijman). Find the verb that feels good, then discover what game it belongs to. If the core action is not fun in a grey box with no art, no theme will save it, and the "Art of Screenshake" argument is that feel is a craft skill with specific techniques rather than an emergent property.

**Theme-first.** Mechanics that arise from a subject are coherent in a way that reverse-engineered themes never are, and the games that matter culturally are usually about something. Starting from a satisfying verb reliably produces polished games with nothing to say.

### Procedural generation as multiplier versus the oatmeal problem

**The PCG position.** Generation multiplies content beyond what a team can author, enables replayability, and creates the run-to-run variance that roguelikes are built on.

**Kate Compton's "10,000 bowls of oatmeal" critique.** You can generate ten thousand mathematically unique bowls of oatmeal, and to a human they are all just oatmeal. **Perceptual uniqueness, not mathematical uniqueness, is what matters**, and generators routinely optimize the wrong one. The fix is structural variety at the level people actually perceive, which is much harder than parameter randomization. *This critique transfers directly to AI-generated content and is the sharpest tool for evaluating it.*

### Difficulty options and accessibility

**The options position.** Difficulty settings cost little and let more people experience the work. Nobody is harmed by an easier mode existing, and the argument against it is usually about the objector's identity rather than the design.

**The authored-difficulty position** (the Souls discourse). In some designs difficulty is the content: the meaning of an accomplishment comes from its cost, and a shared assumption that everyone faced the same wall is what makes the community's culture work. An easy mode does not merely add an option; it changes what the achievement means.

This one is genuinely unresolved, and the honest version notes that the two camps are frequently discussing different games.

### Realism versus feel

**Realism** grounds a system, makes it predictable from real-world intuition, and carries authenticity in simulation-adjacent genres.

**Feel** wins nearly every time they conflict. Real jump arcs feel terrible; real weapon handling is tedious; real vehicle physics are unplayable for most players. The craft is in deviating from realism in ways players do not notice while preserving the intuitions they brought.

## Anti-pattern catalog

| Pattern | Trigger | Consequence | Fix |
|---|---|---|---|
| Content added to a loop that does not feel good | Prototype reads flat; response is more features | Volume cannot fix a bad central verb | Fix the core action first; test it in a grey box |
| Uniform randomness in a player-facing system | Modelling probability correctly and stopping | Clumping reads as broken; players are right about what happened | Bounded distribution, pseudo-random distribution, or a pity timer |
| Unnamed feedback loop | Systems added independently | Runaway leaders or meaningless early play, discovered in balance testing | Name the loop's sign and confirm it is intended |
| Economy with faucets and no sinks | Rewards designed before removal | Inflation; early content meaningless; new players locked out | Design the sink with the faucet |
| Monetization opposed to the core fun | Business layer added late | The game monetizes by damaging what players came for | Align the model with the loop, or change the loop |
| Hidden DDA presented as player achievement | Wanting a smooth experience | The accomplishment the player feels is not the one they had | Disclose, or make the assist an explicit option |
| Explaining during a playtest | Watching someone struggle | Destroys the only signal that matters | Say nothing; write down the urge to speak |
| Player diagnosis taken literally | Post-test interview | Fixing the stated cause instead of the real one | Trust the location, test the explanation |
| Vertical slice mistaken for the game | Demo feels great | No second hour; pacing untested | Test the tenth hour, not just the first |
| Parameter randomization called variety | Generator ships uniform-feeling output | Ten thousand bowls of oatmeal | Vary structure at the level players perceive |
| Difficulty as a single axis | Applying the flow channel literally | Player bored and overwhelmed simultaneously on different axes | Model the skills separately |
| Randomized monetization without checking jurisdiction | Copying a common model | Compliance exposure, not a design problem | Verify per market; the picture is fragmented and moving |

## Severity rubric for this lens

- **blocker** -- the design cannot work as specified, or carries regulatory exposure: an economy that inflates without bound, a randomized monetization model shipped into a jurisdiction that restricts it, a core loop whose incentives directly oppose its stated goal.
- **major** -- a systemic problem that will surface in playtesting and cost a redesign: unaligned monetization, an unnamed runaway feedback loop, a progression wall at the point the design intends engagement.
- **minor** -- tuning and polish: curve shape, reward pacing, missing game-feel technique on a secondary action.
- **nit** -- naming, presentation, convention.
- **insight** -- a reframe: this is a progression game being designed as an emergent one; the central verb is not the one the design thinks it is; this randomness wants to be a bounded distribution.

## Authorities

- **Jesse Schell**, *The Art of Game Design: A Book of Lenses* -- the best single interrogation toolkit.
- **Raph Koster**, *A Theory of Fun for Game Design* -- fun as learning, and the expiry that follows from it.
- **Steve Swink**, *Game Feel* -- the vocabulary for the layer that decides prototypes.
- **Hunicke, LeBlanc, and Zubek**, "MDA: A Formal Approach to Game Design and Game Research" -- the framework and its directional insight.
- **Jesper Juul**, *Half-Real* -- emergence and progression, and the rules-fiction relationship.
- **Jan Willem Nijman**, "The Art of Screenshake" (GDC) -- game feel demonstrated live; the most efficient talk in the field.
- **Martin Jonasson and Petri Purho**, "Juice it or lose it" -- the same argument, differently made.
- **Kate Compton** -- the "10,000 bowls of oatmeal" problem; the essential critique of generated content.
- **Zagal, Björk, and Lewis**, "Dark Patterns in the Design of Games" -- the taxonomy, useful because it is intent-independent.
- **Warren Spector and Harvey Smith** -- the immersive-sim case for systemic design, made by people who shipped it.
- **The GDC Vault** -- the field's primary literature. Design postmortems are where practitioners are most honest about what failed.

## Changelog

- **2026-09-05** -- Initial version. Design frameworks, game-feel technique, randomness models, economy structure, and the named disagreements are drawn from the field's canonical literature and are durable. **The regulatory section is deliberately non-specific**: loot box and gacha regulation is fragmented, jurisdiction-dependent, and moving, and per-market positions were not verified in this pass. Treat any specific regulatory claim as requiring verification against the regulator's own publications before it informs a shipping decision.
