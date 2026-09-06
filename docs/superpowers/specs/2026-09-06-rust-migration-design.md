# Shell-to-Rust migration: phase boundary, workspace, and the deps port (2026-09-06)

Follows `2026-09-04-config-command-and-manifest-crate-design.md`, which
established `crates/config-manifest` as a standalone crate with an embedded
build stamp. That crate is the precedent this document generalizes.

This design was developed through a `/brainstorming` session informed by two
expert panels: an `/expert-consult` round on whether to migrate at all, and a
second round on the stale-binary problem the migration introduces. Every cost
figure below was measured on this machine (darwin, Apple Silicon) rather than
estimated. The measurements are recorded with the decisions they drove,
because several of them overturned the intuition that prompted the question.

## 1. Goals

- Establish one testable rule for which language owns which script, so the
  question stops being re-litigated per file.
- Move the logic the maintainer actually edits into a language the maintainer
  reads fluently. This is the primary goal and it is a maintainability goal,
  not a performance goal.
- Keep a fresh machine bootstrappable with nothing but `sh` and `git`.
- Grow from one crate to several without weakening the pre-push stamp gate.
- Make the correct rebuild path easy to run and hard to forget.

## 2. Non-goals

- Performance. The panel found no runtime case for any rewrite, and the
  measurements in section 3.1 are recorded so this is not revisited as a
  speed project. A Rust port of the hot path recovers roughly 13ms of 62ms
  against a hard process-spawn floor of 8ms.
- Porting the test harness. `tests/` stays bash. It drives real tmux servers
  and real git repositories through their process interfaces, `assert_cmd`
  offers nothing for tmux server lifecycle, and the suite must keep running
  in a container with no Rust toolchain.
- Collapsing the mac/linux branch model. Rejected with evidence; see
  `TODO-AGENTS.md` under QUESTIONS.
- Runtime freshness checking. Rejected on measured cost; see section 3.3.

## 3. Decisions already made, with the evidence

### 3.1 Performance is not the reason

Measured, per invocation:

| What | Cost |
|---|---|
| `/usr/bin/true` (compiled, zero work) | 8ms |
| `/bin/sh -c ':'` | 14ms |
| `config-manifest --stamp` (Rust binary) | 8ms |
| one `git rev-parse` | 15-18ms |
| one `tmux display-message` | 11ms |
| `tmux-update-window-names.sh`, full run | 50-62ms |

The hot script has 13 subprocess call sites. Rust removes interpreter startup
and leaves every `tmux` and `git` roundtrip untouched, so the recoverable
share is about 21%, against a floor no out-of-process design can beat.

Two independent expert lenses concluded that the hot path wants a trigger
change (a git `post-checkout` hook, or `chpwd`, instead of a `precmd` that
fires on every prompt), not a language change. That remains true and is
tracked separately in `TODO-AGENTS.md`.

### 3.2 Maintainability is the reason

The maintainer reads Rust fluently and shell poorly. A correct shell script
that cannot be confidently modified is worse than a marginally slower Rust
one that can. This outranks the runtime finding above, and it is why the tmux
scripts are in scope despite owning no data model: clarity for the person who
has to change them is the deliverable.

### 3.3 No runtime freshness check

A second expert panel priced the two runtime options and both are
disqualified by measurement, not by preference:

- **Binary self-checks its stamp on startup.** The stamp is a git tree id
  computed through a temp index, measured at 124-131ms. That is two to three
  times the entire budget of a 50-62ms script invoked from a `precmd` and
  from six tmux hooks. Rejected.
- **Auto-rebuild when sources are newer.** A *no-op* `cargo build --release
  --locked` measures 727ms to 1.46s, and cargo takes a global package-cache
  lock. Triggered from a prompt hook across roughly 105 panes, the panes
  serialize on that lock and a prompt hangs. Rejected.

A content-addressed binary path (build to `~/.cache/config/bin/<tree-id>/`
so a stale binary is *absent* rather than silently wrong) was proposed and
also rejected: resolving the path requires computing the tree id, which is
the same 124ms, and memoizing per shell session reintroduces staleness across
the long-lived panes that are the normal case here.

### 3.4 Freshness is guaranteed uniformly at pre-push

Every ported binary is stamped and verified at push. Nothing checks at
runtime. This is uniform by construction, so there is no exception list and
no per-script judgment about whether staleness is self-healing.

### 3.5 Manifest format is TOML

Nothing outside `check-deps.sh` parses the dependency conf format. The
Dockerfiles and CI invoke the script with `--only` and `DEPS_LOCAL_CONF`;
they never read the file. So the format is an internal contract and changing
it is cheap.

The current pipe-delimited format has a documented defect in its own header:
a `check_command` containing a literal `|` truncates and leaks the remainder
into `docs_url`. That constraint has already deformed the data.
`deps.conf:24` is written as `if test -d ...; then true; else ...; fi`
instead of `test -d ... || command -v ...` solely to avoid the delimiter, and
`check-deps.test.sh:302` exists to enforce the constraint from outside.

### 3.6 Workspace with per-crate stamps

A single workspace-wide stamp would make the stamp coarser than any one
binary's actual input set. Editing the tmux crate would mark
`config-manifest` stale and `pre-push` would refuse a push over a binary
byte-identical to what its own sources produce. False refusals are how a gate
gets bypassed, which is the same reasoning `config-stamp`'s header uses to
reject a HEAD-based stamp.

Per-crate stamping was verified working before this document was written:
one `write-tree` over `crates/` produced root tree
`623787b34617522f3db88c908d4d3e20a8d75323`, and
`rev-parse "${root}:crates/config-manifest"` produced
`e7b9aa22ab52dd83fd6d1346708a3fd921637b36`, which is byte-identical to what
`.scripts/config/config-stamp` prints today. The extraction costs one extra
`rev-parse` per crate, not a second tree walk.

## 4. The governing rule

> Shell owns bootstrap up to the point a toolchain exists. Past that line,
> Rust owns everything it practically can.

This is a phase boundary rather than a per-script taste judgment, so it is
decidable by one question: **can this run before `rustup` is installed?**

The question has a concrete answer because `rustup` is itself a dependency
entry, and `config-init` documents the ordering contract at lines 28-46:
install-hooks first (it puts `config` on PATH), install second (it places the
toolchain), build last (it needs the cargo that install just placed).

### 4.1 Phase 1: shell, permanently

| Script | Why it cannot be Rust |
|---|---|
| `setup.sh` | The entry point on a machine with nothing installed. |
| `config-init` | Owns the ordering that ends with a toolchain existing. |
| `config-install-hooks` | Runs before anything is built; puts `config` on PATH. |
| `platform.sh` | Sourced by `.zshrc`, `setup.sh`, and tmux config. Must work as `sh`. |
| `.scripts/config/config` | Must locate and exec binaries *including when they are absent*. 34 lines, on the interactive path. |
| `bootstrap-deps.sh` (new) | Installs `rustup` and its prerequisites. See section 7.1. |

### 4.2 Phase 2: Rust

`config-check`, `config-sync`, `config-stamp`, `config-push-all`,
`config-test`, `config-reload`, `config-help`, the full dependency logic
beyond bootstrap, and all tmux orchestration.

Each keeps a thin shell wrapper under `.scripts/config/` that supplies the
subcommand name and execs the binary, preserving the dispatcher's existing
filesystem-registry contract.

## 5. The dispatcher's two contracts

The dispatcher currently enforces two different contracts under one name, and
the migration breaks the second one on its first step.

**Execution contract** (`.scripts/config/config:27-32`): a subcommand is a
file at `$here/config-<name>`, executable, name matching `[A-Za-z0-9-]+`. It
is exec'd with the remaining argv. A compiled binary satisfies this today.

**Introspection contract** (`.scripts/config/config-help:38`): the
description is read with `sed -n 's/^# help: //p'` over the file's *source
text*. Verified against the installed `config-manifest`: the sed yields
nothing, so a ported subcommand would list as `(undocumented)`.

### 5.1 Fix: `--describe` as an execution contract

`config-help` gains a resolution order per sibling:

1. Run `config-<name> --describe`. On exit 0, use the single line printed.
2. Otherwise fall back to the existing `sed` read of `# help:`.

Shell subcommands need no change: `usage.sh` gains a `describe_if_requested`
helper that prints the file's own `# help:` line, so the source text stays
the single source of truth for shell. Rust subcommands implement
`--describe` directly.

The fallback is what keeps the migration incremental. There is never a flag
day where `config help` is half broken.

Cost: `config help` performs N forks instead of N file reads. It is not on
any hot path.

## 6. Workspace layout and the stamp

```
crates/
  Cargo.toml          # [workspace], members = [...]
  Cargo.lock          # shared, one resolution
  rust-toolchain.toml # exact pin, see 6.2
  config-manifest/    # existing: .sync-manifest domain
  config-deps/        # new: dependency manifest domain
  config-tmux/        # new: tmux orchestration
```

### 6.1 Per-crate stamp

`config-stamp` changes shape. Today it prints one tree id; it becomes:

```
config stamp                 # every crate, as "<name> <tree-id>" lines
config stamp <crate>         # one crate's tree id, for scripting
```

Computation, verified in section 3.6:

1. Read `crates/` into a temp index, `write-tree` for the root tree id.
2. Per crate, `rev-parse "${root}:crates/<name>"`.
3. Fold the shared `Cargo.lock` and workspace `Cargo.toml` blob ids into
   each crate's stamp, so a dependency change invalidates every binary it
   could affect.

Step 3 is what keeps the stamp honest under a shared lockfile. Without it a
lockfile bump would leave every binary claiming currency.

### 6.2 Toolchain pin

`crates/rust-toolchain.toml` pins an exact toolchain. This closes a gap
found during the survey: `Cargo.toml` carries `edition = "2024"` and no
`rust-version`, and `edition` is not a pin. Every toolchain from 1.85 onward
compiles edition 2024, so mac and linux can produce matching stamps from
different compilers.

The file lives inside `crates/` so the existing `git add -- crates` in the
stamp computation covers it automatically.

### 6.3 pre-push

`tests/pre-push` iterates installed binaries rather than checking one:

```
for each workspace crate:
    installed=$(<binary> --stamp)
    pushed=$(git rev-parse "<ref>:crates/<name>")   # plus lockfile fold
    refuse on mismatch, naming the crate and the rebuild command
```

The hook still never compiles. That property is load-bearing: a hook that
compiles gets bypassed.

## 7. The dependency split

`check-deps.sh` is the only script that runs in both phases: at bootstrap it
places the toolchain, and on an inited machine it runs from shell startup via
`depcheck-hook.sh` (throttled to once per 24h) and from `config install`.
It splits along the phase line.

### 7.1 `bootstrap-deps.sh` (Phase 1, shell, small)

Sole job: get a machine to the point where `cargo` exists. Presence-checks
and installs `rustup` plus the handful of prerequisites it needs (`cc`,
`git`, `curl`). Hardcodes that short list rather than parsing a manifest,
because a manifest parser is the thing being avoided at this altitude.

This file should change roughly never. It is the one piece of shell whose
correctness cannot be observed on an inited machine, so it stays as small as
a correct implementation allows.

### 7.2 `config-deps` crate (Phase 2, Rust)

Owns everything else: manifest parsing, platform merge, `--only` selection,
install-command resolution, verdict, and execution.

Module shape follows `config-manifest`'s, which the survey rated well above
median and identified as the template:

- `manifest`: parse TOML into a `Dependency` model. Pure.
- `platform`: merge shared plus per-platform plus local manifests. Pure.
- `select`: apply `--only`, and report selectors that matched nothing. Pure.
  This replaces the current nested-`IFS` comma splitting, which walks the
  selector list twice with two hand-rolled splitters, with a set difference.
- `verdict`: given a manifest and observed presence, decide what is missing
  and what is installable. Pure.
- `exec`: run check commands and install commands. The IO edge.
- `render`: produce `Rendered { stdout, stderr, exit_code }` as a *value*,
  so exit codes and stream routing are testable without a subprocess.

### 7.3 Manifest format

```toml
[[dependency]]
name  = "gh"
check = "command -v gh"
docs  = "https://cli.github.com/"

[dependency.install]
brew = "brew install gh"
apt  = "..."      # the keyring plus apt-source case, which has no Brewfile form
```

This moves the install command out of `install_cmd_for`'s 235-line `case`
statement and into data. That case statement is the single largest block of
logic in `.scripts/`, and roughly 200 of its lines are per-dependency
rationale comments recording real production failures (the `gh` keyring
story, the `zoxide` rate-limit story, the `oh-my-zsh --keep-zshrc` story).
Those comments move with their entries, attached to the dependency they
explain rather than buried in a dispatch arm.

Two entries need conditional logic rather than a fixed command: `node`
checks for `nvm.sh` before emitting anything, and `zsh-autosuggestions`
checks `$ZSH_CUSTOM`. They get a `when` field. Two exceptions to a data model
is acceptable; twenty entries living in code is what this replaces.

### 7.4 Two verbs, not one

The current `--fix` exit code answers neither "is this machine ready" nor
"did every install succeed". It answers "did any attempted install fail",
and dependencies with no automated install path never make it non-zero. So
`config-init` can report success on a machine that is not ready.

The port splits them:

```
config deps check     # is this machine ready?    non-zero if anything missing
config deps install   # install what can be       non-zero if an attempt failed
                      # then names what remains manual
```

`config install` keeps working as an alias for `config deps install`.

## 8. Freshness ergonomics

No runtime checks, per 3.3 and 3.4. The rebuild path is made easy instead.

### 8.1 `config build`

With no arguments, builds every workspace crate and re-stamps all of them.
One command after any crate edit. With a crate name, builds just that one.

### 8.2 `config doctor`

Reports which installed binaries do not match their crate, and prints the
exact command to fix it. Silent when everything is current, so it is safe to
run habitually.

`doctor` was chosen after checking for collisions: `status` shadows a git
verb, and `check` is already the drift check. `doctor` collides with neither
and follows an established convention (`brew doctor`). It has room to grow
into adjacent health checks (hooks installed, dependencies present) without
another name.

### 8.3 Documentation

The edit-build-test loop is documented in:

- `.claude/rules/dotfiles-tests.md`, beside the existing pre-push rules.
- `.scripts/deps/README.md` for the dependency manifest specifically.
- `config doctor`'s own `# help:` line and `--help` output.

## 9. Testing

Per `~/.claude/rules/testing.md` and the TDD default: red tests first,
against a named spec, for every item below.

### 9.1 Equivalence harness during each port

Each ported subcommand gets a temporary harness asserting the Rust output
matches the shell output byte for byte across the existing fixture set,
before the shell version is deleted. `check-branch-drift.test.sh` already has
the shape (a `CHECK_CMD` variable selecting the implementation under test),
and this is what that variable was originally for.

The harness is deleted with the shell implementation it validated. Leaving it
behind is how `check-branch-drift.test.sh` became a 272-line suite testing
the same binary as `check_cli.rs`.

### 9.2 Pure-core unit tests

`manifest`, `platform`, `select`, `verdict`, and `render` take no IO, so
every case is a table test. The `--only` selector logic in particular has
error paths (a selector matching nothing) that currently require a
subprocess to exercise.

### 9.3 Property tests

- TOML parse and print round-trips.
- Platform merge is associative over the shared/platform/local layering.
- A selector set is satisfied if and only if every name matches some entry.

### 9.4 Shell suite additions

- `config help` lists a Rust subcommand by its `--describe` output, and a
  shell subcommand by its `# help:` line, in the same run.
- `config doctor` is silent when current and names the crate when stale.
- `pre-push` refuses a push when any one binary is stale, and names which.
- `pre-push` does *not* refuse when a different crate was edited. This is the
  false-refusal regression test that justifies per-crate stamping.
- `bootstrap-deps.sh` places a toolchain on a machine with none. The existing
  Docker bootstrap harness covers this shape.

### 9.5 TRIGGER_PATHS

Every new path added by this work must match `tests/pre-push`'s
`TRIGGER_PATHS`, or the suite that reads it will not run at pre-push. The
survey found this regex already missing `.scripts/config/*` (extensionless
files fail its `\.sh$` requirement), `.zshrc*`, `setup.sh`, `.profile`,
`.config/alacritty/`, and `.scripts/deps/*.conf`.

Those gaps are recorded in `TODO-AGENTS.md` and should be fixed before step 2
of section 10, because step 2 edits `pre-push` anyway.

## 10. Order of work

Sequenced so infrastructure is proven before new logic lands on it.

**Step 0. The four outstanding blockers.** Three leak-guard fail-open bugs
and the zero-assertion PASS, all recorded in `TODO-AGENTS.md` with
reproductions. Step 2 edits `tests/pre-push`, which is where the leak guard
runs, so doing these first avoids touching that file twice. Independent of
the migration, and higher priority than any of it: this repo is public and
the guard can currently report a clean scan on a scan that failed.

**Step 1. `--describe` on the dispatcher.** Hard prerequisite. Without it the
first ported subcommand lists as `(undocumented)`. Cheap, and the `sed`
fallback makes it incremental.

**Step 2. Workspace, per-crate stamp, toolchain pin, `config build`,
`config doctor`.** Pure infrastructure against the *existing* crate, adding
no new logic. This is the safe place to discover that the stamp redesign is
harder than it looks. Also untracks
`crates/config-manifest/proptest-regressions/plan.txt`, which currently sits
inside the stamped tree so a proptest seed forces a no-op rebuild.

**Step 3. `bootstrap-deps.sh` plus the `config-deps` crate.** The largest
logic win: TOML manifest, install-as-data, the two-verb split, and the
selector set logic. Also the largest single change, so it lands after the
build infrastructure is trusted.

**Step 4. Remaining `config-*` subcommands.** `config-stamp` (folded into the
workspace stamp work), `config-push-all`, `config-test`, `config-reload`,
`config-help`.

**Step 5. `config-tmux` crate.** Last. Largest orchestration surface, least
logic per line, and it benefits from every pattern established above. Fold in
the two known tmux bugs while porting: the non-atomic pointer write in
`alacritty-platform.sh` and the substring session match in `tmux-start.sh`.

Each step is independently shippable and independently revertible. None
depends on a later step.

## 11. Open questions

- **The orphan `!` rule in `config-manifest`.** A `!` pattern with no
  enclosing shared rule silently disables drift checking for the paths it
  names. Two fixes were proposed: make exclusions structurally nested
  (`SharedRule { pattern, exceptions }`) so an orphan is unrepresentable, or
  reject orphans at parse time. The first is the better shape and costs a
  parse-time tree assembly plus a print-time re-flatten. Decide before
  step 3, because the next crate copies this one's error-handling shape.
- **Exit codes as a closed sum.** `config-manifest` returns bare `u8` from
  nine sites and status 1 currently means eight different things, with
  `pre-push` and two other consumers discriminating by grepping message
  text. One `Outcome` enum with a single exhaustive `exit_code()` match
  fixes it without changing any observable number. Worth doing in step 2,
  before the shape is copied twice.
- **The hot-path trigger change.** Independent of this migration:
  `tmux-update-window-names.sh` runs from a `precmd` on every prompt when
  its input changes only on `git checkout` and `cd`. Two lenses independently
  recommended a git `post-checkout` hook plus `chpwd`. Tracked in
  `TODO-AGENTS.md`; do not fold it into step 5, since it is a behavior change
  and step 5 should be a faithful port.
