# Sharing one agent configuration across tools (2026-09-12)

Research pass on what it takes to run this repo's `~/.claude/`
configuration under Codex, OpenCode, Goose and the other agentic coding
tools, rather than maintaining a copy per tool.

The goal that shaped this: **one source of truth for rules, skills and
agents, shared across most tools, with a few tool-specific overrides.**
Everything below is judged against that, not against "can it be ported
at all".

## The short answer

**Mostly you do not port it. You point other tools at it.**

The brief assumed a conversion problem. That is not the shape of the
answer. Copilot, Cursor, Goose, OpenCode and Crush read `.claude/` paths
natively. Codex and Gemini CLI read `CLAUDE.md` after a one-line config
change. Codex ships a first-party Claude Code migrator. Claude Code's
layout has become the de facto interop substrate, and Cursor's own docs
say so: *"Cursor reads CLAUDE.md files the same way it reads
AGENTS.md... This ensures compatibility with projects that also use
Claude Code."*

One thing genuinely does not port: **`paths:` glob auto-loading.** That
is the real design problem, and it is narrow.

## What is actually here

Measured against the tracked tree, not the working directory.
`~/.claude/` on disk is ~3.5 GB. Almost all of that is untracked session
transcripts and project data. An early pass this session reported large
hardcoded-path counts from scanning those. The tracked configuration is
3.4 MB and contains **zero** hardcoded absolute paths.

| Category | Files | Size | What it is |
|---|---|---|---|
| `rules/` | 81 | 2.2 MB | Reference documents |
| `agents/` | 80 | 653 KB | Dispatch routers into the rules |
| `skills/` | 31 | 381 KB | Workflows |
| `scripts/` | 3 | 30 KB | STE linter, plain Python |
| `hooks/` | 1 | 3 KB | macOS notifier |
| `CLAUDE.md` | 1 | 19 KB | Global instructions |

Agents declare four frontmatter keys (`name`, `description`, `tools`,
`skills`), rules declare two (`paths`, `last-verified`), skills two or
three. The whole declared tool surface is eight names: Read, Grep, Glob,
Bash, Write, Edit, WebFetch, WebSearch.

### The split that decides everything

Only **15 of 83 rules auto-load** by path glob (280 KB). The other **68
are agent-only** (1.88 MB), marked with a sentinel glob
(`__agent_only_never_match_at_startup__/**`) that never matches, and
read explicitly by an agent naming the file path in prose.

| Layer | Size | Bytes identical across tools? |
|---|---|---|
| Agent-only rules (68) | 1.88 MB | Yes |
| Skill bodies (34) | 429 KB | Yes |
| Scripts (3) | 29 KB | Yes |
| Agent files (80) | 653 KB | No, frontmatter schema differs |
| Auto-load rules (15) | 280 KB | No, `paths:` has no equivalent |

**2.3 MB of 2.6 MB is byte-identical across tools.**

### Three facts that make this cheap

1. **No Claude API coupling in any agent body.** Checked all 80 for
   `Task(`, `subagent_type`, `Agent tool`, `SlashCommand`, `TodoWrite`:
   zero hits. The panel coordinates through `rules/panel-contract.md`,
   a markdown protocol in this repo, not a platform feature. 62 agents
   cite it.

2. **The agent-to-rules contract is just file reading.** 76 of 80 agents
   load rules by naming a path in prose ("Read `~/.claude/rules/rust.md`
   first"). Any harness with a read tool runs that unchanged.

3. **Only 5 tracked files mention "Claude Code", and 4 mention it as
   subject matter.** `agent-sandboxing.md` and `agent-orchestration.md`
   already treat Claude Code, Codex and Cursor as peers. The genuine
   machinery is `settings.json`, `hooks/notify.sh`, and one line in
   `skills/monitor-ci/SKILL.md`.

## What each tool reads natively

V = verified against source or primary docs. GitHub *code search*
produced false negatives twice during this pass. Fetching the file is
decisive, search is not.

| Tool | `CLAUDE.md` | `AGENTS.md` | `.claude/agents/` | `.claude/skills/` | `.claude/rules/` | Glob auto-load |
|---|---|---|---|---|---|---|
| **Copilot** | yes V | yes | yes V | yes V | **yes** V | `applyTo` |
| **Goose** | opt-in V | yes | **yes** V | yes V | no | absent |
| **Cursor** | yes V | yes | yes V | yes V | no | `globs` |
| **OpenCode** | yes V | yes | **no** V | yes V | no | absent |
| **Crush** | yes V | yes | no | yes V | no | absent |
| **Codex CLI** | opt-in V | yes V | no V | no | no | absent |
| **Gemini CLI** | opt-in V | opt-in | no | — | no | absent |
| **Kilo Code** | yes V | yes | no V | yes V | no | absent |
| **Continue.dev** | no V | no V | no | — | no | `globs`+`regex` |
| **pi** | yes V | yes | no V | opt-in | no V | absent |
| **Aider** | no V | **no** V | no | no | no | absent |

**Copilot and Goose consume the most of this tree unmodified.**

### Goose reads `.claude/agents/` directly

`aaif-goose/goose` (54k stars, active, transferred from `block/goose` to
the Linux Foundation). From
`crates/goose/src/agents/platform_extensions/summon.rs:398-412`, fetched
and read:

```rust
local:  .goose/agents, .claude/agents, .agents/agents
global: ~/.goose/agents, ~/.agents/agents, config/agents, ~/.claude/agents
```

The only tool found that runs `.claude/agents/` subagents without
conversion. Its docs call `.agents/skills/` "the recommended standard".

### Codex is closer than expected

`openai/codex` (124k stars, pushed 2026-09-12), verified at source:

- **`project_doc_fallback_filenames = ["CLAUDE.md"]`** makes it read
  this repo's instruction file by configuration, no symlink.
- **Its skills are already this format.** `.codex/skills/<name>/SKILL.md`
  with frontmatter `name` + `description`, byte-identical to the 28
  skills here. The `agents/openai.yaml` sidecar beside it is optional
  interface metadata.
- **It ships a first-party migrator** (`codex-rs/external-agent-migration/`)
  that reads `.claude/settings.json` and converts agents, skills, MCP
  servers and commands.
- Its subagents are **TOML**, not markdown: `name`, `description`,
  `developer_instructions`.
- Instructions load by **directory position** (project root down to
  cwd), not by which files you touch.

## The `paths:` gap

Glob-based conditional loading exists in four tools, under four field
names: Claude Code (`paths`), Cursor (`globs`), Copilot (`applyTo`),
Continue.dev (`globs` plus a `regex` that matches file *content*, which
exceeds every other implementation). Everywhere else it is **absent**.

This is by design. The Agent Skills spec defines no glob field, so the
portable standard everyone converged on deliberately omits conditional
loading.

Substitutes exist and are worth knowing, ranked by fidelity: Goose's
runtime directory-proximity lazy loading, then Gemini's just-in-time
subdirectory loading, then OpenCode's always-load globs. Worst is
Crush, which ingests `.cursor/rules/` while doing **zero** frontmatter
parsing, flattening the directory and passing YAML through as literal
prompt text. Reading the format without honouring the contract is worse
than not reading it.

**The 68 agent-only rules are unaffected.** They were never auto-loaded.
An agent reads them on demand, and that works everywhere.

## The real standards picture

**AGENTS.md is weaker than its profile.** Stewarded by the Agentic AI
Foundation under the Linux Foundation (platinum members include AWS,
Anthropic, Google, Microsoft, OpenAI), 60,000+ repos. But it is
**deliberately schema-less**: *"AGENTS.md is just standard Markdown."*
No SPEC file, no JSON schema. It covers instructions only, not agents,
skills, hooks, permissions or MCP. It replaces roughly a top-level
`CLAUDE.md` and nothing else. Its own adoption list is also not
reliable: it lists Aider, whose source has zero references to it.

**Agent Skills is the real spec.** `agentskills.io`, Apache-2.0, 25.2k
stars, Anthropic-originated and released as an open standard, ~45
implementers, with a `skills-ref validate` reference validator.
Required frontmatter is `name` and `description`, exactly what this
repo's skills already carry. Claude Code's SKILL.md is a superset, so
extra keys degrade rather than break.

**`~/.agents/` is convention-by-convergence, not a spec.** No governing
body defines a discovery path. But Codex, Gemini CLI, Cursor, OpenCode,
Crush, Goose and the GitHub CLI all adopted it independently.

## `~/.agents/` already exists here

Three skills under `~/.claude/skills/` are already symlinks into
`~/.agents/skills/`, tracked as mode `120000`:

```
find-skills -> ../../.agents/skills/find-skills
papercuts   -> ../../.agents/skills/papercuts
show-me     -> ../../.agents/skills/show-me
```

`~/.agents/.skill-lock.json` (`"version": 3`) records GitHub sources and
folder hashes. The tool that wrote it is **`npx skills`**
(`vercel-labs/skills`, MIT, 31.5k stars, active): it installs one
canonical copy and symlinks it into every agent's directory across 79
supported agents, with `--copy` as a fallback.

The format is a genuine cross-vendor commitment. The **official GitHub
CLI** hard-codes it, verified in `cli/cli`,
`internal/skills/lockfile/lockfile.go`:

```go
// lockVersion must match Vercel's CURRENT_LOCK_VERSION for interop.
lockVersion = 3
agentsDir   = ".agents"
lockFile    = ".skill-lock.json"
```

## Symlinks: prefer native directives instead

Tested locally and cross-checked against tool source.

| Approach | New shared files appear | Found by plain `find` |
|---|---|---|
| Directory symlink | yes | **no** (needs `-L`) |
| Per-file symlinks | no (one link each) | yes |

The failure mode is silence. **Continue.dev drops symlinked skill
directories outright**, verified in
`extensions/cli/src/util/loadMarkdownSkills.ts`: it filters `readdir`
results on `dir.isDirectory()`, which is false for a symlink. No error,
no log. Cursor had the same class of bug (staff-answered forum reports
of rules showing active while silently not loading), claimed fixed in
2.5. Codex's policy is scope-dependent (`User|Repo|Admin => Follow`,
`System => Ignore`) and it has an open bug ignoring symlinked
*`SKILL.md` files*, so **symlink directories, never individual files.**
Windows needs Admin or Developer Mode.

**Because of that, prefer the native include directives where they
exist:**

| Tool | Directive |
|---|---|
| Claude Code | `@path/to/file` |
| Codex | `project_doc_fallback_filenames = ["CLAUDE.md"]` |
| Gemini CLI | `context.fileName` array, `@./path`, `gemini skills link` |
| Goose | `CONTEXT_FILE_NAMES` |
| OpenCode | `instructions` array (globs, `~/`, URLs) |
| Crush | `option global-context-path`, `skills_paths` |

A symlink is also the same bytes everywhere. It serves the 2.3 MB that
is byte-identical. It cannot give two tools different frontmatter over
one body.

## Recommendation

1. **Keep `~/.claude/` as the canonical tree.** Do not restructure. It
   is the format the ecosystem converged on, and moving it costs
   compatibility rather than buys it.

2. **Expose `CLAUDE.md` as `AGENTS.md`** and switch tools on by config,
   not by copying: one line each for Codex, Gemini CLI and Goose. The
   file is 18 directives with no Claude-specific tool names.

3. **Use `npx skills` for skills.** Already installed here, 79 agents,
   and it is what wrote the existing `.skill-lock.json`. Use `--copy`
   for Continue.dev, and verify Cursor before trusting symlinks there.

4. **Take the free wins.** Goose and Copilot read `.claude/agents/` as
   is. For Codex, run its own migrator rather than writing a converter.

5. **Decide the 15 auto-load rules deliberately, per tool.** Cursor,
   Copilot and Continue.dev get a real translation (`globs`, `applyTo`).
   Everywhere else, choose per rule between always-on and
   agent-summoned, and record which.

6. **If a converter is needed, use `rulesync`** (`dyoshikawa/rulesync`,
   MIT, 1.4k stars, active, ~42 tools). It is the only one with true
   bidirectional `convert --from X --to Y` covering subagents, skills
   and hooks. Note that `ruler` has 2× the stars and does strictly less:
   no import, no commands, no hooks, subagents on 4 targets and off by
   default, and it admits unmappable tools are "dropped silently on a
   normal apply". Stars invert capability here. No converter was
   executed. These are README claims.

The expensive option, a build step generating 80 agent files per tool,
can wait until something actually needs it. Points 1 through 4 need
no tooling and cover most of the value.

## Not verified

- **Conflicting primary sources on `CLAUDE.md` in VS Code Chat.**
  GitHub's support matrix says No. VS Code's docs say always-on with
  `chat.useClaudeMdFile` defaulting true. Unresolved.
- Copilot's precedence among `copilot-instructions.md`, `AGENTS.md` and
  `CLAUDE.md` is genuinely undocumented.
- Whether Cline's documented `paths:` frontmatter fires in a shipping
  build. Source reading found no non-test caller. Needs a hands-on test.
- Goose hook events and recipe schema (docs moved during the Linux
  Foundation transfer, and some URLs returned 404).
- Whether `agents/<tool>.yaml` is a family or OpenAI-only with a
  generic-looking name. No second implementation found.
- No tool and no converter was executed. Behavioural claims come from
  source and docs reading.

## What surprised me

**The port is mostly unnecessary.** The expected deliverable was
conversion tables. The actual answer for Copilot, Cursor and Goose is
"point them at `.claude/`", and for Codex "run the migrator OpenAI
already wrote".

**The configuration was already portable and nobody designed it that
way.** The portability comes from a style choice: rules are prose with a
path reference, not data in a platform schema. An agent that says "read
`~/.claude/rules/rust.md` first" works under any harness with a read
tool. Expressed as platform metadata, all of it needs rewriting.

**The panel protocol survives intact.** 62 agents coordinate through
`panel-contract.md`, a markdown document in this repo. The most
sophisticated part of the setup costs nothing to move, because it was
never a platform feature.

**Coordination happens in code, not committees.** The GitHub CLI
hard-codes Vercel's lock format with a comment saying why. Meanwhile the
committee-backed standard, AGENTS.md, has no schema and an adoption list
with at least one entry its own source contradicts.

**Reading a format without honouring it is a real failure mode.** Crush
ingests `.cursor/rules/` and discards the frontmatter semantics. The
YAML reaches the model as literal prompt text. Continue.dev drops
symlinked directories with no error. Both look like support from
outside.
