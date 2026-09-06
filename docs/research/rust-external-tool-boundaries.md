# Rust external-tool boundaries (2026-09-06)

Research pass on one question: when a Rust program drives tmux or git, what are
the real alternatives to `Command::new("tmux")` and `Command::new("git")`, and
is any of them better for this repo?

Extends `shell-to-rust-prior-art.md` (2026-09-04), which covered the testing
ecosystem and the sourcing boundary. That doc concluded shell is often correct
here. This pass does not overturn that conclusion. It sharpens it, and it
found the repo's actual performance problem somewhere neither doc was looking.

**Verdict up front.** `Command::new("git")` is not the bottleneck, and the
alternatives to it mostly do not help this repo. The measured bottleneck is
**spawn count**, and the largest single cost in the repo is not in the script
under discussion at all: it is `parse_git_dirty` in `.zshrc`, which runs a full
`git status` on every prompt and costs **274 ms** in a large worktree. The hot
script the plan targets costs 16 ms and is already faster than a Rust binary
doing the same job.

Three findings reorder the whole plan:

1. **The hot path is already fixed.** `tmux-update-window-names.sh` has been
   batched since the last doc. Its precmd/hook path is 16 ms, not 50-62 ms.
2. **A Rust rewrite of that path measures slower.** 18.84 ms against 17.13 ms,
   because the binary pays an 8.4 ms spawn that the shell script does not.
3. **The real 1.1-second cost is the `-a` path**, and the fix is available in
   shell: stop spawning git, read `.git/HEAD` directly. 1137 ms to 32 ms.

## Measurement method

macOS 26.5, Darwin 25.5.0, arm64. tmux 3.4 (homebrew), git 2.50.0. Live tmux
server: 59 windows, 144 panes. hyperfine is not installed, so every number
below comes from a zsh loop using `EPOCHREALTIME` with the module preloaded and
one warm-up call discarded:

```zsh
#!/bin/zsh
zmodload zsh/datetime
n=$1; shift; label=$1; shift
"$@" >/dev/null 2>&1                       # warm
start=$EPOCHREALTIME
for i in {1..$n}; do "$@" >/dev/null 2>&1; done
end=$EPOCHREALTIME
printf '%-48s %7.2f ms/call (n=%d)\n' "$label" $(( (end-start)*1000.0/n )) $n
```

Every number is labeled with its `n`. All numbers in this document are
**verified** (I ran them) unless the text says otherwise.

### The floor

| Operation | Per call | n |
|---|---|---|
| `/usr/bin/true` | 6.66 ms | 200 |
| **Rust release binary, no work** | **8.39 ms** | 200 |
| unix-socket client, daemon already running | 7.82 ms | 200 |
| `/bin/sh -c ':'` | 13.47 ms | 200 |

This corroborates the prior doc: a Rust binary starts faster than a shell
script and slower than `/usr/bin/true`. The ~8 ms is macOS fork+exec, not a
Rust tax.

**The socket number is the important one and it arrives early.** A unix-socket
client talking to a warm daemon costs 7.82 ms, which is the spawn floor.
Section 5 returns to this. It disqualifies every daemon design for a
hook-invoked script.

### Single operations

| Operation | Per call | n |
|---|---|---|
| `git rev-parse HEAD` (bare `.cfg` + worktree `$HOME`) | 12.13 ms | 100 |
| `git rev-parse --git-common-dir --abbrev-ref HEAD` | 12.51 ms | 100 |
| `git branch --show-current` | 9.63 ms | 100 |
| `tmux display-message -p -F` | 10.07 ms | 100 |
| `tmux list-windows -a -F` (59 windows) | 11.67 ms | 50 |
| `tmux list-panes -a -F` (144 panes) | 12.99 ms | 50 |

A tmux query for the whole server costs the same as a query for one window.
tmux's own work is a few milliseconds; the rest is spawn. That is the entire
reason batching wins, and it is why the alternatives below have so little room
to work in.

## 1. tmux control mode: real, and not for this

`tmux -C` is a persistent bidirectional protocol. One client, many commands,
guard-framed replies. I read the tmux source at commit
`578e07fcbc66dc60822b55b88ba12f518df57374` and measured against the installed
3.4.

**The protocol.** Guards come from exactly one function, `cmdq_guard` in
`cmd-queue.c:796-806`, which calls `control_write_guard` (`control.c:479-499`)
to emit `%<kind> <t> <number> <flags>`. The three arguments are epoch seconds
(`cmd-queue.c:742`), a **monotonic per-server command counter**
(`cmd-queue.c:743`, `item->number = ++number`), and a flag that is 1 for
client-originated commands (`cmd-queue.c:591`). The man page says the flags
field is "currently not used"; that is wrong, and iTerm2 relies on the real
meaning.

**The marginal per-command cost is genuinely spectacular and genuinely
irrelevant.** Inside an established control client, a command costs
0.033-0.115 ms against 11-13 ms for a fresh spawn: a 100-400x reduction. It
does not help here for four independent reasons, each sufficient alone.

| Approach | Cost | Verdict |
|---|---|---|
| 1 spawn, many fields in one format string | **11.0 ms** | best |
| 1 spawn, chained commands with `\;` | 12.5 ms | fine |
| control mode via zsh coproc, 13 commands | 17.0 ms | loses by 6 ms |
| control mode via python3 harness | 74 ms | worse than the status quo |

1. **Setup exceeds the saving.** Opening the client and reading the first guard
   block costs 12.7-22.5 ms. One plain `tmux` spawn costs 11 ms. You pay more
   to open the control client than to just run the command.
2. **A short-lived process cannot amortize.** Amortizing needs the client to
   outlive many commands. This process dies every prompt, with N=13, once.
3. **The parser costs more than it saves.** python3 startup alone is 49 ms.
   Via the pyenv shim it is 603 ms/call, consistent with the prior doc's 1.34 s
   finding. Any interpreted parser is disqualified before it parses anything.
4. **It perturbs server state, which is a correctness problem, not a speed
   one.** This is the finding that ends it.

**Control mode clients count as attached.** `resize.c:453-455` increments
`s->attached` for any client lacking `CLIENT_UNATTACHEDFLAGS`, and that mask
(`tmux.h:2311-2314`) is only `DEAD|SUSPENDED|EXIT`. `CLIENT_CONTROL` is not
excluded. Verified empirically: a `tmux -C attach-session` client reports
`session_attached=1` and appears in `list-clients`. It therefore affects
`destroy-unattached` collection (`server-fn.c:539`) and alerts
(`alerts.c:202`).

**Worse, bare `tmux -C` creates and leaks a session.** Verified on a private
server: sessions went 1 to 2, a shell spawned, and the session survived client
exit. The `-N` flag does not prevent it.

**And a session-less control client exits after one command.**
`server-client.c:2793-2800`: a control client without `CLIENT_ATTACHED` sets
`CLIENT_EXIT` once its first command completes. So "open a control client,
don't attach, run 13 queries" is not a thing that exists.

**Version hazard.** The man page promises "a notification will never occur
inside an output block." On HEAD that is enforced by a `guard_depth` counter
and a deferred queue (`control.c:502-540`). **Installed 3.4 has neither**;
grepping `guard_depth` in `3.4:control.c` returns nothing. Observed on 3.4:
notifications arrive after the blocks, reordered, and **duplicated**. Any
parser written against the documented guarantee would be wrong on the tmux
actually installed here.

**Who consumes it.** Terminal emulators embedding tmux panes, which is what it
was designed for: iTerm2 (`sources/tmux/TmuxGateway.m:655-692`) and WezTerm,
which carries the most complete published grammar
(`wezterm-escape-parser/src/tmux_cc/tmux.pest`, 26 notifications). The two
projects doing what this repo does both chose subprocess-per-command:
**libtmux (python) is `subprocess.Popen` per call** (`common.py:331-338`), and
its single `-C` use is a test fixture that parses nothing. **tmuxinator**
compiles config to a shell script and `Kernel.exec`s it (`cli.rb:252`).

**Rust crates.** The premise that `tmux_interface` merely shells out is wrong:
it has a real control-mode module. It is also unusable. Its changelog
self-labels the feature "not fully functional yet" (`CHANGELOG.md:83-84`);
`send()` discards the command number it just parsed and assumes the next event
is its reply (`control_mode.rs:209`), so correlation is broken; and
`control_mode.rs:270` does `output_block.data = Some(data)` -- assignment, not
append -- so a multi-line block keeps only its last line.

| Crate | Version | Published | Downloads | Control mode |
|---|---|---|---|---|
| `tmux_interface` | 0.4.0 | 2026-03-10 | 152,977 | yes, draft, 2 real bugs |
| `libtmux` (rust) | 0.1.0-alpha.9 | 2026-08-31 | 233 | yes, correct |
| `tmuxctl` | 0.1.0 | 2026-06-19 | 344 | yes, clean, sans-IO |
| `par-term-tmux` | 0.1.15 | 2026-08-21 | 485 | yes, emulator-internal |

The two correct implementations have effectively no users, so they have no user
base to have found bugs on this repo's behalf.

**Conclusion: drop control mode.** It is well-built for embedding a multiplexer
in a terminal emulator. For a script that answers "what should this window be
named," one batched `tmux` call is faster, has no parser, and does not touch
attach counts.

## 2. git without spawning git

### What the repo actually needs

`tmux-update-window-names.sh` needs two facts per window: the **common** git
dir (so a linked worktree reports its main repo) and the current branch or
short SHA. `config-stamp` needs `read-tree --empty`, `add`, `write-tree`, and
`rev-parse <tree>:<path>`. The `config-manifest` crate additionally uses
`ls-tree -r -z`, `rev-parse --short`, and `rev-parse --verify --quiet`.

### The measurement that matters: spawn amortization

The brief's central hypothesis was that one binary could do 13 git operations
for the cost of one spawn. That is true, and it is true for a reason that has
nothing to do with any git library.

I built a Rust binary that instruments its own inner work with `Instant`:

| Work done inside one process | Inner work | Total wall |
|---|---|---|
| nothing | -- | 8.39 ms |
| 19 `git rev-parse` **spawns** | 184.58 ms | 214.40 ms |
| 57 `git rev-parse` **spawns** | 717.54 ms | -- |
| 57 direct reads of `.git/HEAD` | **0.11 ms** | **7.86 ms** |

Reading the files git would read costs **0.11 ms for 57 windows**. Spawning
git 57 times costs 717 ms. That is a factor of 6,500, and the whole operation
finishes inside the 8.4 ms spawn floor.

The lesson is not "use gix." It is **"stop spawning."** A library is one way to
stop spawning. For the two facts this script needs, a plain file read is
another, and it is much cheaper than either.

### The `-a` path: a verified 35x win, available in shell

The script's `-a` mode is the expensive one:

| Implementation | Cost | n |
|---|---|---|
| **current shell** | **1137-1340 ms** | 5 |
| Rust: 1 tmux call + 59 file-read resolutions | 19.96 ms (10.46 ms inner) | 30 |
| **POSIX sh: same algorithm, fork-free** | **32.21 ms** | 20 |

Fork census of the current `-a` path, via `sh -x`:

```
95 git spawns    (57 primary + 38 fallback)
 1 tmux call
58 printf        (shell builtin, not a fork)
```

95 git spawns at 12.1 ms is 1150 ms of the measured 1340 ms. The cost is
entirely spawn count.

**Two of those spawns per window are avoidable, and 40 windows pay them for
nothing.** Of 59 live window paths, **40 are not git repositories at all**.
Each non-repo window spawns git, gets "not a git repository," then spawns git a
second time on the `--path-format` compatibility fallback, then falls back to
the directory basename. That fallback exists for git older than 2.31; git here
is 2.50, so it never helps. 40 wasted spawns is ~484 ms of the 1340 ms.

**The file-read replacement is correct, verified against real git.** Both my
Rust and shell implementations resolve the common dir and branch by reading at
most three small files: `.git` (a directory, or a file containing
`gitdir: <path>`), `<gitdir>/commondir`, and `<gitdir>/HEAD`. Compared against
`git rev-parse --path-format=absolute --git-common-dir --abbrev-ref HEAD` across
all 59 live windows:

```
agree=19  differ=0  both_not_repo=40
```

Zero disagreements, including the hard case. The live server has four linked
worktrees of the form `myrepo/1` whose `.git` is a file pointing at
`myrepo.git/worktrees/1`, whose `commondir` contains `../..`. Both
implementations resolve that to `myrepo.git` and
`user/feature-branch`, matching git exactly.

**Rust buys 12 ms of the 1105 ms win.** Shell captures 1137 to 32 ms; Rust
takes it to 20 ms. Credit the win to not spawning, not to the language. This is
the same shape as the prior doc's batching finding, and the same caution
applies: do not credit Rust for what algorithm change delivers on its own.

### The precmd path: Rust measures slower

This is the result that most directly contradicts the plan.

| Implementation | Cost | n |
|---|---|---|
| **current shell script, default path** | **17.13 ms** | 20 |
| Rust binary making one tmux call | **18.84 ms** | 100 |
| `/bin/sh -c 'tmux display-message ...'` | 22.07 ms | 100 |
| `tmux display-message` alone | 8.58 ms | 100 |

Any implementation must ask tmux for the window's state, and it cannot ask
without a process. So the floor is the binary's own spawn (8.4 ms) plus the
tmux spawn (~10 ms), which is already ~18 ms. The existing shell script does it
in 17 ms because the shell it runs in is already paid for and it spawns tmux
directly.

There is no Rust win available on this path. There is no gix win either: the
script's git calls are already only 1-2 per invocation, and on 40 of 59 windows
the correct answer involves no git at all.

### gix, git2, and what the ecosystem picked

**Starship uses `gix`, and finished that migration in 2022.** Verified by
cloning it myself at `125ec297`:

```toml
gix = { version = "0.87.1", default-features = false, features = [
  "max-performance-safe", "revision", "status", "sha1", "sha256",
] }
```

`git2` appears in the source only in stale comments
(`src/modules/git_state.rs:89` still complains about libgit2's state reading).
Starship's default git prompt spawns **zero** processes in a normal repo.

**But starship keeps the git executable as a documented fallback**, and the
four conditions are instructive (`src/modules/git_status.rs:342-347`, verified
by reading it):

```rust
    if config.use_git_executable
        || repo.fs_monitor_value_is_true
        || uses_reftables(&repo.repo.to_thread_local())
        || gix_repo.index_or_empty().ok()?.is_sparse()
    {
        let mut args = vec!["status", "--porcelain=2"];
```

The reasons come from the code itself. Sparse index is a stated gitoxide gap
(`git_status.rs:339`: "TODO: remove this special case once `gitoxide` can
handle sparse indices for tree-index comparisons"). `core.fsmonitor=true` is a
**correctness** fallback, not a capability one: gix would ignore the user's
filesystem monitor and do a full walk. reftables is a ref backend gix cannot
read.

That is the design worth copying, and it is not a purity position: **library
first, real tool as a config-visible escape hatch for the cases the library
gets wrong.** It is also a warning. Two of the four fallbacks are things this
repo could plausibly hit, and one of them (`core.fsmonitor`) is a setting
section 3 recommends turning on.

### gix is disqualified for this repo, twice, on tested grounds

I had flagged two questions as unverified. Both were then tested with running
code against fixtures built to mirror this repo (a bare `.cfg` plus a separate
worktree, a normal repo, and a linked worktree). Both came back negative.

**Blocker 1: gix has no index-to-tree operation, so `config-stamp` is
impossible.** An exhaustive grep across every `gix-*` crate for
`tree_from_index|write_tree|index_as_tree` returns zero hits. Only the reverse
exists (`gix-0.87.1/src/repository/index.rs:206`, `index_from_tree`).
gitoxide's own tracking document confirms it:

```
crate-status.md:184   * [ ] tree from index          (unchecked)
crate-status.md:185   * [x] index from tree          (checked)
crate-status.md:178   * [ ] add files with .gitignore handling   (unchecked)
crate-status.md:899   * [ ] add and remove entries   (gix-index)
```

The primitives that do exist are named `dangerously_push_entry`, which is
gitoxide's own warning label: you would hand-roll blob hashing, entry
construction, stat data, correct sort order, and the tree-building fold.
`gix::object::tree::Editor` (`src/object/tree/editor.rs:81`) is a separate
tree-to-tree path that never consults the worktree or `.gitignore`. This is
exactly the operation `config-stamp` is built on.

**Blocker 2: gix cannot open a bare repo with a separate work tree without
mutating process-global environment variables.** This is the
`git --git-dir=$HOME/.cfg --work-tree=$HOME` requirement. Tested four ways:

| Method | Result |
|---|---|
| `gix::open($HOME/.cfg)` | `workdir=None, is_bare=true` |
| API config override `core.worktree` | **ignored** |
| CLI config override `core.worktree` | **ignored** |
| `GIT_WORK_TREE` + `GIT_DIR` as process env | works |

The cause is in the source, not inferred. `gix-0.87.1/src/open/repository.rs:274`:

```rust
let may_use_configured_worktree = config.is_bare == Some(false) || worktree_from_environment;
```

On a bare repo `is_bare == Some(true)`, so `core.worktree` is discarded from
every source except `EnvOverride`. The function that takes exactly the pair
needed, `open_from_paths(git_dir, worktree_dir, options)`, is `pub(crate)`
(`src/open/repository.rs:160`) and unreachable. gitoxide documents the gap
(`crate-status.md:195-197`): "The delicate interplay between `GIT_COMMON_DIR`
and `GIT_WORK_TREE` isn't implemented."

Forcing a library to set process-global env vars to open its own repository is
hostile in a library and racy if anything else in the process touches git.

**Two further correctness deviations, both tested.**

- **`common_dir()` is not normalized.** This is load-bearing for window naming.
  In a linked worktree, git reports `/.../normal/.git`, git2 reports
  `/.../normal/.git/`, and gix reports
  `/.../normal/.git/worktrees/wt/../..`. `src/repository/location.rs:35` returns
  the stored field verbatim. `.canonicalize()` cleans it but resolves symlinks,
  which is not what `--path-format=absolute` means.
- **`core.quotePath` is not a modeled config key in gix at all.** Grepping
  `quotepath|quote_path` across `gix-0.87.1/src/` returns zero hits. git's
  ground truth differs materially (`"caf\303\251-r\303\251sum\303\251.txt"`
  against `café-résumé.txt`), so any path round-tripping diverges silently. In
  a dotfiles repo spanning `$HOME`, that is a latent correctness bug.

**Where gix is genuinely excellent, and why it does not help.** `gix-ignore` is
production grade. Tested against a fixture exercising all four precedence
sources, it matched git exactly on every path, including correct attribution of
which file caused the exclusion (repo `.gitignore`, nested `.gitignore`,
`.git/info/exclude`, and `core.excludesFile`). The irony is precise: gix's
ignore engine is excellent and unusable for `config-stamp`, because nothing in
gix consumes it into an index.

### git2 is technically sufficient, and still not worth it

git2 passed all eight operations. Notably, its temp-index write-tree was
verified **byte-identical to real git**: `Index::new()` + `set_workdir()` +
`add_all(DEFAULT)` + `write_tree_to()` produced the same OID
(`fe433dbac76fae0ac416d4bfdaf3ff006565e2f3`) as
`GIT_INDEX_FILE=tmp; read-tree --empty; add; write-tree`, and `.gitignore` was
honored correctly. It also does bare-plus-worktree cleanly via
`Repository::open_ext()` then `set_workdir(path, false)`, no env vars.

The cost is the problem:

| | cold `cargo build --release` | unique crates | binary |
|---|---|---|---|
| **gix** (starship's feature set) | 21.4 s | **123** | 1.74 MB |
| **git2** | 12.8 s | **13** | 1.25 MB |
| **current crate** | -- | **3** | -- |

gix would take this crate from 3 dependencies to 123, a 41x increase in
supply-chain surface, to replace a program already installed on every machine
this repo targets. git2's 13 is far better but drags in a vendored libgit2 C
build (`cc`, `pkg-config`, `vcpkg`, `libz-sys`).

### The in-process numbers, and what they actually buy

Measured with a C fork/exec harness rather than a shell loop (a shell loop was
inflating every figure by ~11 ms), n=150-300:

| Approach | Cost |
|---|---|
| 13x `git rev-parse` via shell loop | **188.5 ms** |
| 1x gix binary doing 13 operations inside | **10.7 ms** |
| 1x `git` spawn (reference) | 12.1 ms |
| `/usr/bin/true` (spawn floor) | 6.6 ms |

A 17.6x win, ~178 ms saved. The premise in the brief is real. **But read the
composition:** of that 10.7 ms, 6.6 ms is the spawn floor and only ~2.0 ms is
actual git work. In-process, gix opens a repository in ~350 µs and steady-state
per-operation cost is ~155 µs (git2: ~290 µs open, ~790 µs per operation).

So the purchase is not "gix is fast." It is **"do not spawn 13 processes,"**
and that is available without any library by batching `git` invocations. This
repo already knows the trick: the script's primary call fetches two facts in
one `rev-parse`.

### Prior art: what the Rust VCS projects picked

- **jj (jujutsu) migrated fully to gix and has zero git2.**
  `Cargo.toml:63-70` pins `gix = "0.87.1"`; grepping `git2` across all
  manifests returns no matches, and `Cargo.lock` has 0 `git2` entries against
  53 `gix-*` crates. The motive was libgit2 *limitations*, not speed:
  `CHANGELOG.md:2764` cites SSH bugs "due to `libgit2`s limitations."
  **The detail worth noting: jj's interim fix for those bugs was spawning an
  external `git` subprocess** (`git.subprocess = true`), later removed
  (`CHANGELOG.md:2109-2110`).
- **gitui remains a hybrid with git2 primary** after years.
  `asyncgit/Cargo.toml:20` has `git2 = "0.21"` and `:22` has `gix = "0.84.0"`,
  with gix scoped to status, mailmap, and revision: read paths, not index
  writing.
- **starship is gix-only and stays inside the subset gix does well.** No
  write-tree, no bare-plus-worktree. It uses gix for exactly the branch,
  short-SHA, and tree-reading shapes.

**There is no published "gix is N times faster than git" number.** gitoxide
has no consolidated benchmark suite: only per-crate criterion micro-benchmarks
and narrative monthly reports in `etc/reports/`. Grepping the README for speed
claims returns generic prose only. Worth recording, given how often that claim
circulates.

### What this means for the crate's git boundary

`crates/config-manifest/src/git.rs` already has the right shape. All git
invocation funnels through one `Git` struct carrying `prefix` and `env`, with a
single `command()` constructor (`src/git.rs:74-84`). The bare-repo case is
handled by prefix args, mirroring `config-stamp`'s
`git --git-dir="$ROOT/.cfg" --work-tree="$ROOT"`.

Measured cost of the commands that use it:

| Command | Cost | git spawns | n |
|---|---|---|---|
| `config check` | 71.21 ms | -- | 5 |
| `config stamp` | 100.99 ms | 8 | 5 |

Neither is on a hot path. `config stamp` runs from a build or a pre-push hook,
not a prompt. At 8 spawns and 101 ms, a port would save perhaps 90 ms on a
command a human runs deliberately and waits for, in exchange for either a
disqualifying capability gap (gix) or 10 extra dependencies and a vendored C
build (git2).

**The correctness asymmetry is the real argument, and it favors the
subprocess.** Real `git` is the reference implementation for `.gitignore`
precedence, `core.quotePath`, `core.worktree`, and common-dir resolution. Every
one of those is a place a library was measured deviating. For a personal
dotfiles repo, matching `git` exactly is worth more than 178 ms on a
hand-invoked command.

## 3. The finding neither doc was looking for

The brief asked about a 50-62 ms script. While measuring it I measured the rest
of the prompt, and the script is not the problem.

`.zshrc:219-222` defines the precmd. `PS1` (`.zshrc:224`) additionally calls
`parse_git_dirty` via command substitution, and `parse_git_dirty`
(`.zshrc:208-213`) runs a **full `git status`**.

Per prompt, in `~/code/myrepo/1` (21,895 tracked files):

| Component | Cost | n |
|---|---|---|
| **`parse_git_dirty` (`git status`)** | **274.53 ms** | 10 |
| `vcs_info` | 46.86 ms | 20 |
| `tmux-update-window-names.sh` | 16.70 ms | 20 |
| **total** | **~338 ms** | |

The naming script is **5%** of the prompt's git cost. The plan's target is the
smallest of the three terms.

Against zsh-bench's 10 ms command-lag threshold (cited in the prior doc), this
prompt is at 3,380%.

**Untracked-file scanning is the entire cost, and it finds nothing.**

| Variant | Cost | n |
|---|---|---|
| `git status --porcelain` (default `-unormal`) | 241.16 ms | 8 |
| `git status --porcelain --no-renames` | 243.12 ms | 8 |
| **`git status --porcelain -uno`** | **44.27 ms** | 8 |
| `core.untrackedCache=true` via env, default flags | 76.52 ms | 8 |

`git status --porcelain` in this repo reports **0 lines**. It spends 197 ms
proving there is nothing to report. Rename detection is free; untracked
scanning is everything.

Two independent fixes, neither involving Rust:

- **`core.untrackedCache=true`**: 241 to 77 ms, a 3x win from one config
  setting and no code change. Verified via `GIT_CONFIG_COUNT` env override so I
  did not write to the user's config. `core.untrackedCache` and
  `core.fsmonitor` are both currently **unset** in this repo.
- **`-uno`**: 241 to 44 ms, a 5.5x win, at the cost of no longer coloring the
  prompt for untracked files. That is a behavior change, so it is the user's
  call, not mine.

Note the interaction flagged in section 2: turning on `core.fsmonitor` is
exactly the condition that makes starship abandon gix and shell out to `git`.
If this repo ever adopts a gix-based prompt, enabling fsmonitor would push it
back onto the executable.

**This reframes the whole exercise.** A Rust rewrite of the naming script
targets 16 ms of a 338 ms prompt and measures slower. One git config setting
targets 197 ms and costs nothing.

## 4. The sourcing constraint, and what the ecosystem really does

The prior doc found the irreducible boundary is `zsh-git-widgets.sh`, 27 lines.
That holds, with one correction.

| Script | Alias | Lines | `return` | `exit` | Mutates caller? |
|---|---|---|---|---|---|
| `tmux-start.sh` | `s` | 38 | 4 | 0 | No |
| `tmux-setup.sh` | `se` | 82 | 4 | 0 | No |
| `tmux-split.sh` | `sp` | 89 | 3 | 1 | No |
| `tmux-close.sh` | `c` | 17 | 3 | 0 | No |
| `zsh-git-widgets.sh` | n/a | 27 | 0 | 0 | **Yes: `LBUFFER`, `zle`, `bindkey`** |
| **`platform.sh`** | n/a | -- | -- | -- | **Yes: `export DOTFILES_PLATFORM`** |

**Correction to the prior doc: there is a fifth sourced script.**
`.zshrc:171` sources `.scripts/platform.sh`, which genuinely mutates the
parent shell at line 31 (`export DOTFILES_PLATFORM`). Its own header states
the requirement: "Sourced, not executed: callers need $DOTFILES_PLATFORM set
in their [environment]."

That is a real sourcing requirement, and it is also the cheapest thing in this
document:

| Operation | Cost |
|---|---|
| `source ~/.scripts/platform.sh` | **0.272 ms**, zero forks |
| a binary that printed the same export | 8.39 ms + eval |

A 30x penalty to convert a 0.27 ms no-fork operation into a process. There is
no version of this that improves.

### The eval pattern, priced

The `eval "$(tool init zsh)"` dance costs one spawn at shell startup and
essentially nothing thereafter:

| Operation | Cost | n |
|---|---|---|
| `zoxide init zsh` (generate) | 8.04 ms | 50 |
| `eval` of that output | 0.081 ms | 100 |
| `eval` of a small function definition | 0.0021 ms | 1000 |

This repo already pays it once, at `.zshrc:227`.

### The readability objection is wrong, and this is the useful correction

The brief's concern was that this pattern "moves the shell code into a Rust
string literal, which may be worse for readability than a .sh file." **The
major projects do not do that.** I verified starship myself:

```rust
const ZSH_INIT: &str = include_str!("starship.zsh");   // src/init/mod.rs:264
```

`src/init/starship.zsh` is a real, syntax-highlightable 102-line zsh file on
disk. The only transformation is one placeholder replacement
(`init/mod.rs:243`, `script.replace("::STARSHIP::", path)`).

| Project | Storage | Real file? | Lines (zsh) |
|---|---|---|---|
| **starship** | `include_str!` + 1 placeholder | **yes** | 102 |
| **fzf** | `//go:embed` of the shipped files | **yes** | 203 |
| **atuin** | `include_str!`, no templating | **yes** | 255 |
| zoxide | askama template | template file | 186 |
| direnv | Go backtick string literal | **no** | 14 |

Three of five keep real shell files and embed them. Only direnv does the thing
the brief feared, and direnv is the worst-behaved of the set (section 5).
zoxide templates because it compiles flag variations into its output, and pays
with `{%- if %}` directives interleaved in its shell source.

So the honest answer: **the eval pattern does not force shell into string
literals.** `include_str!` gives you one source of truth, a real `.zsh` file,
and normal tooling. The prior doc's cited hazards
(direnv #650 quoting corruption, zoxide #953 syntax errors at line numbers in
generated code) are hazards of *generating* shell, not of embedding it.

That said, the prior doc's conclusion stands: **this repo does not need the
pattern.** Four of the five sourced scripts do not mutate caller state, and the
two that do are 27 lines of ZLE widgets and one `export`. Both are cheaper and
clearer as shell.

## 5. The other alternatives, priced

### Persistent daemon over a unix socket

I built one: a Rust `UnixListener` server and a client that connects, writes a
query, reads a reply, exits.

| Operation | Cost | n |
|---|---|---|
| socket client roundtrip, daemon warm | **7.82 ms** | 200 |
| Rust binary, no work at all | 8.39 ms | 200 |

**The daemon eliminates the work and not the spawn.** The client is still a
process, and the process is the cost. 7.82 ms against an 8.39 ms floor means
the socket roundtrip itself is nearly free and the measurement is dominated by
fork+exec. A daemon cannot beat 8 ms when reached by a command.

That is fatal for a tmux hook or a precmd, both of which invoke a command. A
daemon only pays off when the *caller* is already resident.

**Atuin's own posture supports this skepticism.** Atuin ships a daemon,
defaults it **off** (`crates/atuin-client/src/settings/daemon.rs:41-52`), and
speaks gRPC over HTTP/2 via tonic+prost over a unix socket
(`crates/atuin-daemon/build.rs:7-22`). Its docs claim it speeds up writes, but
there is no in-repo benchmark and no linked issue for that claim. The
defensible justification is the in-memory fuzzy index, which is a
capability, not a latency win. Meanwhile zoxide, direnv, fzf, and starship ship
no daemon at all.

Cost side: a socket path, a wire protocol, version negotiation, a lifecycle,
and stale-state failure modes. Against a measured saving of zero.

### tmux `run-shell` semantics: a free win, already taken

| Invocation | Cost |
|---|---|
| `tmux run-shell 'sleep 0.3'` | 337.2 ms |
| `tmux run-shell -b 'sleep 0.3'` | 17.0 ms |

`run-shell` blocks the tmux server for the command's full duration; tmux is
single-threaded, so that stalls every client. `-b` returns immediately.

**The repo already does this correctly.** All six hooks
(`.config/tmux/tmux.conf:124-129`) use `run-shell -b`, and all six pass
`-w #{window_id}`, so they take the 16 ms single-window path, not the 1137 ms
`-a` path. This closes a question the brief left open: the "50ms script invoked
7 ways" is really a 16 ms script invoked 7 ways, plus one 1137 ms path reached
only by the `re` alias (`.zshrc:85`) and `tmux-setup.sh:76`.

Also confirmed from the prior doc: anything defined by `eval` is invisible to
`run-shell`, which uses `/bin/sh`. Hooks must call a binary or script by path.

### Filesystem watching (kqueue) eliminates the wrong poll

A watcher on `.git/HEAD` would correctly invalidate a cached branch name, since
checkout rewrites that file. **But the naming script is not triggered by branch
changes.** It is triggered by `after-select-pane` and `after-select-window`:
tmux events, not filesystem events. A pane switch needs a fresh answer even
though nothing on disk moved.

A watcher could maintain a warm branch-name cache that a hook then reads, but
the hook still has to run, and running is the 8 ms cost. This is the daemon
problem again. Neither `fswatch` nor `watchman` is installed here.

### Zellij: same per-command cost, one genuinely better idea

`zellij action` reaches a running server over an `interprocess` local socket
(unix domain socket on unix), framed as length-prefixed protobuf via prost
(`zellij-utils/src/ipc.rs:514-525`). Structurally cleaner than tmux's protocol.

**It is not cheaper.** Each `zellij action` is a full process that also parses
KDL config and runs `create_config_and_cache_folders()` before its single
round trip. The CLI grammar admits one verb, so it cannot batch, which makes
it slightly *worse* than tmux, where `\;` chaining and multi-field format
strings both work. This was read from the code path, **not benchmarked**.

The genuinely better primitive is **`zellij subscribe`**: a long-lived push
stream, one process, then repeated updates. For watching state that is better
than polling outright. It has no tmux equivalent, and it does not apply to a
per-prompt script.

Its WASM plugin model is a real difference in *lifecycle*: plugins are resident
with state surviving across events (`plugin_map.rs:358-360`), and delivery is
push, so there is no spawn per event. A shell hook can do neither. But per-call
cost is IPC, not a function call, and the boundary is unflattering: the host
import is a zero-argument doorbell
(`extern "C" { fn host_run_plugin_command(); }`, `shim.rs:2982-2988`), and the
protobuf payload is JSON-stringified before crossing
(`shim.rs:2869-2873`), then parsed back with `serde_json`. The runtime is
wasmi, an interpreter, not a JIT. I would not assume it is cheap for
high-frequency chatter. Not benchmarked.

### Nushell and fish 4.x: no change to either constraint

**Nushell relocates the text-parsing problem, it does not solve it.** External
commands spawn a real process
(`crates/nu-command/src/system/run_external.rs:172`) and stdout arrives as raw
bytes with no metadata; the stream type is literally `Unknown`. Calling `git`
from nushell still means encoding every field name and delimiter assumption,
just in a typed pipeline stage. Its *plugin* protocol is structured, but a
plugin is also a separate process.

**Fish's Rust rewrite changes nothing about sourcing.** Builtins do receive
`&mut Parser`, the live session, but the builtin table is a compile-time
`const BUILTIN_DATAS` of static function pointers, binary-searched. There is no
registration function and no mutable registry; grepping for
`dlopen|libloading|LoadLibrary|dylib` returns zero matches. Third parties
cannot add builtins without recompiling fish. External binaries are `execve`d,
which ends the shared-address-space story by construction. Being written in
Rust confers nothing here.

### Corrections to the prior doc's build numbers

The brief cited 727 ms to 1.46 s for a no-op release build. Measured now:

| Invocation | Wall clock |
|---|---|
| `cargo build --release -q` (no-op, rustup shim) | 50-60 ms |
| same, shim bypassed (`rustup which cargo`) | 30 ms |
| `cargo run --release -q` (no-op) | 210-430 ms |

The no-op build is much cheaper than the brief assumed and matches the prior
doc's 60-80 ms. The prior doc's rule survives intact and for the same reason:
**never put `cargo run` in a shell rc or a tmux hook.** At 210-430 ms it
exceeds the 10 ms command-lag budget by 20-40x. Hooks must exec a built binary
by absolute path.

## 6. Patterns worth stealing regardless of language

From starship, verified by reading the source:

1. **Cheap detector gates expensive work.** Starship has 109 modules and 68 can
   spawn a subprocess. A prompt does not spawn 68, because each is gated behind
   a filesystem check first (`src/modules/rust.rs:165-174`). All 68 detectors
   share **one** memoized directory read
   (`dir_contents: OnceLock<...>`, `src/context/mod.rs:59`, populated by a
   single `fs::read_dir` at line 588). This repo's analogue: 40 of 59 windows
   are not repos, and a `test -d "$dir/.git"` costs nothing next to a 12 ms
   git spawn.
2. **Timeout every external call; render empty on expiry.** Defaults are
   `command_timeout: 500` and `scan_timeout: 30`
   (`src/configs/starship_root.rs:162-163`), enforced with
   `terminate_for_timeout()`. A slow prompt is a bug; a hung prompt is an
   outage. The current script has no timeout, so a hung git on a network mount
   stalls every prompt indefinitely.
3. **Ship your own profiler.** `starship timings` is ~10 lines of
   `Instant::now()` instrumentation (`src/modules/mod.rs:124,253`) and turns
   "which of my calls is slow" into one command instead of a bisect. This
   investigation needed a hand-built harness to find the 274 ms; the repo
   could have answered it itself.
4. **Reduce hook frequency before optimizing the hook.** zoxide defaults to
   `chpwd`, not `precmd` (`src/cmd/cmd.rs:142-144`), so it fires on directory
   change rather than every prompt. fzf installs no prompt hook at all. The
   naming script's answer only changes on directory change, branch change, or
   pane switch, and `precmd` is not the tightest available trigger for the
   first of those.
5. **Background writes; block only on reads.** atuin's `precmd` uses `( ... & )`
   and blocks only in `preexec`, where it needs an ID back.
6. **Direct exec, never `/bin/sh -c`.** starship resolves the binary with
   `which::which` then `Command::new(full_path)` (`src/utils/mod.rs:168-190`).
   Measured here: `/bin/sh -c ':'` is 13.47 ms against 6.66 ms for
   `/usr/bin/true`, so the shell wrapper doubles the floor.

And the cautionary case. **direnv is the closest thing to a negative
control**: 14 lines of Go string literal, registered in *both*
`precmd_functions` and `chpwd_functions` so a `cd` runs it twice, with no
shell-side guard at all. Its "nothing changed" fast path calls `LoadedRC()`
before the no-op check, which SHA-256s the entire `.envrc` on every prompt for
a hash the fast path never uses (`cmd_export.go:47` before `:61-77`,
`rc.go:343-362`). The team knows: the whole export is wrapped in a
`cmdWithWarnTimeout` that prints "is taking a while to execute." Being written
in a compiled language did not save it. Doing unnecessary work on every prompt
is the defect, in any language.

## Recommendation

Ranked by measured value per unit of work. The first two are not about Rust and
should happen regardless.

1. **Fix `parse_git_dirty`. This is the whole ballgame.** It costs **274 ms per
   prompt** and is 81% of the prompt's git cost. Two options, in order of
   cost to behavior:
   - Set `core.untrackedCache=true` (and consider `core.fsmonitor`). Measured
     241 to 77 ms, no code change, no behavior change. Both are currently
     unset.
   - Replace `git status` with `git status --porcelain -uno --no-renames`:
     44 ms. Also replaces the three `[[ =~ ]]` matches against human-readable
     output with a stable machine format, which is a correctness improvement
     as well. It drops untracked-file coloring, so this one is a behavior
     decision.

   Do this before anything else. It is a one-line change worth 20x what the
   entire port question is worth.

2. **Fix the `-a` path in shell. 1137 ms to 32 ms, verified.** Two changes,
   both small:
   - Delete the `--path-format` fallback. It exists for git older than 2.31,
     git here is 2.50, and it costs a second wasted spawn on each of the 40
     non-repo windows (~484 ms).
   - Replace the per-window `git rev-parse` with direct reads of `.git`,
     `<gitdir>/commondir`, and `<gitdir>/HEAD`. My POSIX sh implementation
     measured 32.21 ms and agreed with real git on all 59 live windows
     (19 agree, 0 differ, 40 correctly not-a-repo), including four linked
     worktrees. Guard it with `test -e "$dir/.git"` so non-repo windows cost
     no forks at all.

   Keep a `git rev-parse` fallback for shapes the file read does not cover
   (`gitdir:` chains, `core.worktree`, unusual ref backends), following
   starship's library-first-with-escape-hatch design. Build the equivalence
   harness first: my comparison ran against 59 live windows, which is evidence,
   not proof.

3. **Do not rewrite the precmd/hook path in Rust.** Measured: current shell
   17.13 ms, Rust binary making the same one tmux call 18.84 ms. The binary
   pays an 8.4 ms spawn the shell script does not, and the tmux call cannot be
   avoided by any implementation. This is the clearest measured
   anti-recommendation in the document.

   If a hot-path improvement is still wanted, the levers are frequency and
   guards, not language: move the naming trigger off `precmd` toward `chpwd`
   plus the existing tmux hooks, and add a cheap `test -e .git` gate.

4. **Drop tmux control mode from the plan.** Setup costs 12.7-22.5 ms against
   11 ms for one plain spawn, so it loses outright for a short-lived process.
   It also counts as an attached client (`resize.c:453-455`), bare `tmux -C`
   creates and leaks a session, a non-attached control client exits after one
   command (`server-client.c:2793-2800`), and installed tmux 3.4 lacks HEAD's
   notification-deferral machinery entirely, so it reorders and duplicates
   notifications. One batched `tmux` call with a multi-field format string is
   11 ms, has no parser, and perturbs nothing.

5. **Leave `config-stamp` and the `config-manifest` crate on
   `Command::new("git")`. gix is disqualified on tested grounds, not
   speculation.** Two blockers, each sufficient alone:
   - **No index-to-tree operation exists in any `gix-*` crate**, so
     `config-stamp`'s temp-index `write-tree` cannot be expressed.
     gitoxide's own `crate-status.md:184` has `* [ ] tree from index`
     unchecked, and `:178` has `* [ ] add files with .gitignore handling`
     unchecked.
   - **gix cannot open a bare repo with a separate work tree** except by
     mutating process-global `GIT_DIR`/`GIT_WORK_TREE`. `core.worktree` is
     discarded for bare repos at `src/open/repository.rs:274`, and the
     function taking the right argument pair is `pub(crate)`.

   Two further tested deviations: gix's `common_dir()` returns an
   un-normalized `.../worktrees/wt/../..`, and `core.quotePath` is not a modeled
   key in gix at all, which diverges silently on non-ASCII paths in a repo
   spanning `$HOME`.

   git2 does pass all eight operations, with its write-tree verified
   byte-identical to git's. It is still not worth it: 3 dependencies to 13
   plus a vendored libgit2 C build, to save ~90 ms on a command a human
   invokes and waits for. gix would be 3 to 123.

   The decisive argument is correctness, not speed. Real `git` is the
   reference implementation for `.gitignore` precedence, `core.quotePath`,
   `core.worktree`, and common-dir resolution, and a library was measured
   deviating on each.

   The existing boundary is already well-shaped for a swap if that ever
   changes: one `Git` struct, one `command()` constructor (`src/git.rs:74-84`),
   prefix args carrying the bare-repo case. That interface-boundary work is
   done, so the implementation behind it stays replaceable.

6. **Leave all five sourced scripts as shell.** The prior doc's conclusion
   holds, with the correction that there are five, not four: `platform.sh`
   genuinely exports `DOTFILES_PLATFORM` and is a real sourcing requirement. It
   costs **0.272 ms with zero forks**; a binary equivalent costs 8.39 ms plus
   an eval, a 30x penalty. `zsh-git-widgets.sh` remains irreducible at 27 lines
   of ZLE widgets.

   The brief's readability worry about the eval pattern is unfounded, and worth
   recording for the future: starship, fzf, and atuin all keep **real `.zsh`
   files** and embed them with `include_str!` or `//go:embed`. The pattern does
   not require shell in string literals. It is still not needed here.

7. **Adopt two of starship's habits, in shell.** Add a timeout to the git calls
   so a hung git on a network mount cannot stall every prompt (the script
   currently has none). And add a `--timings` flag that reports per-step
   `EPOCHREALTIME` deltas; this investigation needed a hand-built harness to
   find the 274 ms, and the repo should be able to answer that itself.

## Revisit if

- **A prompt component genuinely needs many git facts at once.** That is the
  one shape where a library wins big, and the shape starship is built for.
  Reading `.git/HEAD` covers two facts; it does not cover ahead/behind counts,
  stash depth, or a dirty-file summary. Note that this shape avoids both gix
  blockers: it needs no index write, and a prompt runs inside a normal
  worktree, not the bare `.cfg`. If the prompt grows to want those facts, gix
  becomes worth re-pricing.
- **gix gains index-to-tree and non-env bare-plus-worktree opening.** Those are
  the two blockers, both tracked as unchecked in gitoxide's own
  `crate-status.md`. If both land, re-price the `config-stamp` port. Starship's
  four executable fallbacks (sparse index, `core.fsmonitor`, reftables, opt-out)
  are the other honest map of gix's current edges.
- **The `-a` path starts running on an interactive trigger.** It is currently
  reached only by the `re` alias and `tmux-setup.sh`. If it lands on a hook,
  fix it first (item 2) and re-measure before considering a language change.
- **tmux gains a batched query that returns per-window git state**, or the
  installed tmux reaches a version whose control mode enforces the
  notification ordering its own man page promises. Neither changes the spawn
  arithmetic, but both would change the parser risk.
- **A caller appears that is already resident.** Every daemon and IPC option
  priced here failed for the same reason: the client is a process, and the
  process is the cost. A resident caller inverts that, and only then are
  sockets, control mode, and warm caches worth re-pricing.
