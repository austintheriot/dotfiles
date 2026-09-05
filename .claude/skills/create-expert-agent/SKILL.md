---
name: create-expert-agent
description: Build one or more new deep subject-matter-expert subagents (or refresh existing ones) that integrate with `/expert-review`, `/expert-plan`, `/expert-consult`, and `/consult`. Researches the domain online for best practices, thought-leader positions, canonical references, and major pitfalls; splits over-broad subjects into focused sub-agents; captures competing schools of thought as-is without reconciling them; marks volatile facts for later refresh; and wires the new lenses into every consuming skill. Triggers -- "create an expert agent for X", "add a subagent for X", "make X an expert lens", "refresh the X agent", "update these agents".
---

# Create expert agent

Two modes. **Create** builds new expert lenses. **Refresh** re-verifies the volatile facts in existing ones. The mode is usually obvious from the request; when it is not, ask.

The output of create mode is, per agent: one rules file, one agent file, and edits to every consuming skill. An agent without its rules file is a generic model with a costume on. The rules file is the actual deliverable.

## Mode: create

### Step 1 -- Decompose the subject

The user names subjects, not agents. One subject often deserves several agents.

Split when the subject has **distinct failure modes, distinct literatures, or distinct moments of use**. Do not split on mere size. Three tests:

- **Different failure modes.** If a mistake in area A looks nothing like a mistake in area B, the lenses are different.
- **Different canon.** If the authorities you would cite do not overlap, the lenses are different.
- **Different trigger moment.** If area A fires during design and area B during integration, the lenses are different.

Do not split when the result would be two agents that always fire together and cite the same sources. An agent that never fires alone should have been a section.

Aim for 1-4 agents per subject. Present the proposed split to the user before writing anything, with a one-line justification per agent. This is a cheap checkpoint before expensive work.

Name agents for the lens, not the tool, unless the tool IS the domain (`neovim` is fine; `blender-3d` is fine because Blender is the ecosystem). Prefer `noun-noun` kebab-case matching the existing roster's feel.

### Step 2 -- Research, in parallel

Dispatch one research subagent per subject (not per agent -- related agents share a research pass). Give each a prompt that demands:

- **Current information, with version boundaries.** State today's date in the prompt. Volatile domains reward a search over a recollection.
- **Named attribution.** Real people, books, talks, papers, repos, standards by number. "Some argue" is worthless; "Christopher Alexander argues, and Peter Eisenman rejects it in their 1982 Harvard debate" is citable.
- **Concrete failure modes with trigger and symptom.** Not "be careful with scale" but "an unapplied non-uniform scale exports as baked vertex positions with a residual transform, and the symptom is a mesh that looks right in Blender and arrives in Unity with broken normals".
- **Both sides of live disagreements**, with each side's strongest argument, explicitly unreconciled.
- **A volatility judgment per fact class.** Which claims rot, how fast, and the authoritative URL to re-check.
- **Explicit confidence marking.** Verified from a primary source now / found but unverified / inferred.

Tell the researcher to prioritize the non-obvious -- things a general model would not already produce. The value of the file is the delta over baseline knowledge.

Run these concurrently. If you hit the concurrent-subagent cap, queue and dispatch as slots free.

### Step 3 -- Write the rules file

`~/.claude/rules/<agent-name>.md`. This is the deep reference; length is expected (existing files run 15k-40k words for broad domains).

Frontmatter:

```markdown
---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: YYYY-MM-DD
---
```

The sentinel path keeps the file out of startup auto-load; the agent loads it explicitly. Use a real glob only if the domain has file extensions that should pull the rules in automatically during ordinary editing.

Structure:

1. **Title and thesis.** One paragraph naming the unifying mental model, and the agent's single operational question. Then the empirical priority order -- which categories of problem actually bite most often. This is what makes the agent triage rather than enumerate.
2. **Volatile surface.** A short table near the top: what rots, how fast, and where to re-verify. See "Volatility marking" below.
3. **The domain body.** Organized by how a practitioner thinks, not by how a textbook indexes. Concrete throughout: real API names, real numbers, real settings, real symptoms.
4. **Schools of thought.** A dedicated section per live disagreement. State each position in its own strongest terms, name its adherents, and stop. Do not adjudicate. A note on when each camp's answer is the right one is fine; a verdict is not.
5. **Anti-pattern catalog.** Each entry: the pattern, the trigger, the consequence, the fix.
6. **Authorities.** Named people, books, talks, specs, repos, with what each is good for.
7. **Severity rubric.** What counts as blocker / major / minor / nit / insight *in this domain specifically*. Generic rubrics produce generic findings.

### Step 4 -- Write the agent file

`~/.claude/agents/<agent-name>.md`. Short -- 100-200 lines. It is a router into the rules file, not a second copy of it.

```markdown
---
name: <agent-name>
skills:
  - agent-modes
description: <see "Descriptions stay bare bones" below>
tools: Read, Edit, Write, Bash, Grep, Glob, WebFetch
---
```

Add `WebSearch` to `tools` when the domain is volatile enough that the agent should verify current facts at call time rather than trust the rules file. Pricing, licensing, and fast-moving tool ecosystems qualify.

Body sections, in this order:

- **Identity and mental model.** Restate the thesis compactly. Name the operational question.
- **What to read.** The rules file first, then `~/.claude/rules/panel-contract.md`, then project-local docs worth checking.
- **When you fire.** Concrete signals. Then an explicit **Do NOT fire** list routing to neighbor agents by name. Boundary clarity is what keeps a panel from producing five copies of one finding.
- **How to scan.** A numbered procedure. This is the agent's actual method and the highest-leverage section.
- **Findings name the consequence.** Two to four worked examples of a real finding, written out in full. Show, do not describe. These examples do more to set output quality than any instruction.
- **Routing to other lenses.** `See also: <agent>` lines for adjacent concerns.
- **Don't.** Failure modes of the agent itself: what not to flag, what not to reflexively recommend, where not to speculate.

### Step 5 -- Wire it in

Every consuming skill needs the new lens, or the agent will never be dispatched. Edit all of these:

- `~/.claude/skills/expert-review/SKILL.md` -- add to the roster list (usually the **Cross-cutting** line, or a new category line for a genuinely new family) AND add a row to the trigger-signal table. The table row is what actually drives dispatch; a roster mention alone is inert.
- `~/.claude/skills/expert-consult/SKILL.md` -- add to the eligible-agent list, and to the routing-examples table if there is a natural example question. Update the agent count if the file states one.
- `~/.claude/skills/consult/SKILL.md` -- add to the consult-capable roster with a one-line capability summary.
- `~/.claude/skills/expert-plan/SKILL.md` -- add only if the lens has something to say about a spec before code exists. Many domain lenses do; some (pure code-shape lenses) do not.

Also check for any **skill whose description enumerates agent names** -- `expert-review`'s description lists its panel. Long enumerations in a description are already at their useful limit; add the new names only where the list is short enough to stay readable, and prefer a category word over a growing list.

Then update `~/.claude/CLAUDE.md` if the new lens introduces a capability the user would not guess exists. Keep it to the minimum. See below.

### Step 6 -- Verify

- `ls ~/.claude/agents/ ~/.claude/rules/` shows both new files.
- Frontmatter parses: `name` matches the filename stem, `description` is one line, `skills: [agent-modes]` present for panel participants.
- Grep each consuming skill for the new agent name and confirm a hit in the roster AND (for expert-review) the trigger table.
- Re-read the new agent's **Do NOT fire** list against the neighbors it names, and confirm those neighbors do not claim the same territory. Overlapping claims produce duplicate findings.

## Mode: refresh

Agents rot. Pricing changes, licenses change, APIs deprecate, tools get abandoned, standards get revised. Refresh mode re-verifies without rewriting.

1. **Pick the targets.** Named agents, or all agents whose `last-verified` date is older than a threshold the user names (default: six months for volatile domains, eighteen for durable ones).
2. **Extract the volatile surface.** Read the rules file's volatility table and grep the body for `**VOLATILE**` markers. That set is the work list; the rest of the file is presumed durable.
3. **Re-verify in parallel.** One subagent per file, or per cluster of related files. Give each the current claims verbatim and ask it to confirm, correct, or mark unverifiable against primary sources. Demand the URL it checked.
4. **Patch surgically.** Change the facts that moved. Do not rewrite prose that is still correct -- a refresh that churns the whole file loses the review history and wastes tokens.
5. **Bump `last-verified`.** And add a dated line to a short changelog at the bottom of the rules file naming what moved. The next refresh reads that to know what is churning fastest.
6. **Report what changed.** The user needs to know which of their assumptions expired, not just that the file was touched.

A fact that has moved twice in two refreshes is a fact that should be looked up at call time instead of stored. Consider giving that agent `WebSearch` and marking the section "verify before citing".

## Volatility marking

Two mechanisms, both required in every rules file.

**The volatility table**, near the top of the rules file:

```markdown
## Volatile surface

`last-verified: 2026-09-05`. These rot; the rest of this file is comparatively durable.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Engine pricing and revenue thresholds | Fast (months) | Each vendor's pricing page |
| License terms for named tools | Medium (yearly) | The project's LICENSE file |
| API surface gated on version N | Medium | Upstream release notes |
| Maintenance status of named projects | Fast | Repo commit recency |
```

**Inline markers**, at the point of claim:

```markdown
Unity Personal is free below a **VOLATILE** (2026-09-05) revenue threshold; confirm at unity.com/pricing before relying on the number.
```

Mark a claim volatile when it is a price, a license term, a version-gated API, a maintenance status, a legal or regulatory state, a "current best tool" judgment, or anything with a named vendor attached. Do not mark durable theory -- proportional systems, algorithmic complexity, and design canon do not expire.

The agent file gets one line in its **Don't** section: do not state a volatile fact as current without checking it, and say plainly when a number is as-of rather than now.

## Descriptions stay bare bones

The `description` frontmatter field is loaded into context at every session start, for every agent. It is the most expensive prose in the system. Keep it dense and short.

It must carry: what the agent reviews or advises on, the two or three highest-signal coverage areas, the neighbors it is distinct from, and "Works in its own context."

It must NOT carry: the full topic list, the methodology, the canon, example findings. All of that belongs in the agent body or the rules file, where it is loaded only on dispatch.

The same restraint applies to `~/.claude/CLAUDE.md`. Add a new agent there only when its existence is genuinely unguessable, and then in a handful of words. The specialist-delegation instruction already tells the model to prefer specialists; it does not need each one enumerated.

## Capture disagreement, do not resolve it

The instruction that most changes these files: **when two respected camps disagree, record both at full strength.**

The reason is not diplomacy. It is that the two camps are usually looking at different failure modes, and each one's critique is load-bearing. Alexander's critique of Eisenman and Eisenman's of Alexander identify different real problems. Collapsing them into a moderate middle loses both diagnoses and produces advice that fails in both directions.

So: name the camps, name their adherents, state each argument in the form its own advocates would recognize, and say when each is the right answer if that is knowable. Then stop. An agent that hands the user a genuine unresolved tension has done its job; one that hands over a synthesized average has destroyed information.

Flag the failure mode explicitly in the rules file when a domain has a dominant orthodoxy with a serious minority critique -- the minority position is the one the model's baseline knowledge will under-represent, so it needs more words, not fewer.

## Anti-patterns in this workflow

- **Writing the agent file before the rules file.** The agent is a router; routing to nothing produces a confident generalist. Rules first.
- **Splitting a subject into agents that always co-fire.** That is one agent with sections.
- **A rules file that is a tutorial.** These are references for an expert, not lessons for a novice. Cut the introductory scaffolding; start at the level where the interesting content lives.
- **Skipping the trigger-table row.** The most common wiring miss. The agent exists, the roster lists it, and it never runs.
- **Letting descriptions grow.** Every word is paid at every session start forever.
- **Stating a 2026 price as a timeless fact.** Mark it, date it, or look it up.
- **Reconciling a live disagreement into a moderate recommendation.** Covered above; it is the highest-cost mistake here.
- **Research from memory.** In volatile domains the model's recollection is confidently wrong at a rate that makes the file worse than nothing. Search.
