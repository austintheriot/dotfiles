# `config` command and `config-manifest` crate (2026-09-04)

Design for a `config` wrapper command with subcommands, and for
`crates/config-manifest`, a Rust program that owns the `.sync-manifest`
domain: `check` (branch drift) and `sync` (one-way branch sync). Replaces
`tests/check-branch-drift.sh` and the hand-run branch sync.

Status: design approved in conversation; awaiting spec review before an
implementation plan is written.

## 1. Goals

- One `config` front door: dedicated subcommands, everything else passes
  through to git against the bare repo. `config status` keeps working.
- `config sync` makes the other branch's shared paths match the current
  branch, without touching `$HOME`, without pushing.
- One parser for `.sync-manifest`. `check` and `sync` cannot disagree
  about what is shared.
- The Rust core is sans-IO and dependency-injectable. Tests of the core
  need no git and no filesystem. This is the standard for every crate under
  `crates/` (see `~/.claude/rules/rust.md`, "Sans-IO core").
- The Rust binary runs in every environment that runs the tests: host,
  pre-push Docker suite, CI. No environment falls back to shell.
- No compile step inside a git hook. No compile step on shell startup.

## 2. Non-goals

- Two-way sync. The plan type is the extension point; the planner for it
  is later work.
- git2 or any linked git library. Git is a subprocess.
- Rewriting the tmux orchestration scripts or the interactive-zsh tests.
  The research (`docs/research/shell-to-rust-prior-art.md`) rejects both.
- A release pipeline or committed binaries.

## 3. Decisions already made, with the evidence

| Decision | Chosen | Why |
|---|---|---|
| Invocation shape | `config` wrapper command | Git rejects `:` in alias names; a wrapper gives real `--help` and one home for Rust |
| Dispatcher style | git-style `config-<sub>` executables | Each subcommand is any executable; Rust replaces one at a time with no wrapper change |
| Sync semantics | One-way, current branch wins | The merge-base is ancient (`2e4f7f0`), so git 3-way merge is useless; one-way is what was done by hand three times on 2026-09-04 |
| Applier mechanism | Git plumbing into a temp index | `$HOME` is never touched; no scratch worktree; atomic ref update |
| Rust boundary | Rust owns the whole manifest domain | Two consumers of one format; the only place the research says Rust pays |
| Rust in Docker | Multi-stage build, binary copied into the Rust-free runtime image | Measured: 0.8s unchanged, 2.6s after a Rust edit, 100s once; runtime image 195MB |
| Host binary freshness | Tree-id stamp, never compile in the hook | Every source says hooks that compile get bypassed |
| Crate location | `crates/` | A crate is not a script |
| Git boundary model | Snapshot data in, plan data out; no trait | Four-agent panel, unanimous |
| Plan granularity | Domain edits (`Set`, `Remove`), not git ops | Core stays backend-agnostic; `apply_to` gives the same pure simulator |

## 4. Layout

```
.scripts/config/
  config                 dispatcher (sh)
  config-test            shell subcommands
  config-install
  config-install-hooks
  config-reload
  config-build
crates/
  config-manifest/       Rust crate
    Cargo.toml
    src/main.rs          CLI: parse args, gather inputs at the edge, call core, apply
    src/manifest.rs      parser and rules (pure)
    src/tree.rs          TreeListing and the ls-tree parser (pure)
    src/check.rs         drift check (pure)
    src/plan.rs          sync planner (pure)
    src/git.rs           the only module that spawns git
    tests/               integration tests on fixture repos
```

Path filters that must learn about `crates/`:

- `.sync-manifest`: add `crates/` as a shared rule.
- `tests/run-in-docker.sh` overlay list: add `crates`, so uncommitted Rust
  edits are what the container tests.
- `tests/pre-push` `TRIGGER_PATHS`: add `^crates/`.
- CI: no change. `test-suite.yml` and `branch-drift.yml` run on every push.

## 5. The dispatcher

`.scripts/config/config`, POSIX sh, under 40 lines.

```
config <sub> [args...]
```

1. Resolve its own directory (`dirname "$(readlink -f "$0")"`; `readlink -f`
   is present on Darwin 25 and on Linux).
2. If `<dir>/config-<sub>` exists and is executable, `exec` it with the
   remaining args.
3. Otherwise `exec git --git-dir="$HOME/.cfg" --work-tree="$HOME" "$@"`.

All paths derive from `$HOME`, so one tracked copy serves both branches.
The two per-branch aliases (`.zshrc-mac:2`, `.zshrc-linux:2`) are deleted.

Installed as a symlink `~/.local/bin/config` by `config install-hooks`
(`~/.local/bin` is already on PATH). Fresh-clone bootstrap is one call by
path: `~/.scripts/config/config install-hooks`.

Subcommand names must not shadow a git subcommand. A test asserts that
`git <sub>` is not a real command for every `config-<sub>` present.

## 6. `config-manifest`: types and modules

Dependency direction, arrows pointing at what a module depends on:

```
manifest (no deps)    tree (no deps)
      \                  /
       check ---- plan          pure; take values, return values
                    \
                    git         the IO edge; depends on tree and plan
                      \
                      main      orchestration, about 30 lines
```

### 6.1 `manifest`

```rust
pub enum Rule { Shared(PathPattern), Excluded(PathPattern), PerBranch(PathPattern) }
pub struct PathPattern { path: RelPath, is_dir: bool }   // is_dir = trailing slash
pub struct Manifest { rules: Vec<Rule> }

pub fn parse(text: &str) -> Result<Manifest, ManifestError>  // line number in the error
impl Manifest {
    pub fn classify(&self, path: &RelPath) -> Classification
}
pub enum Classification { Shared, Excluded, PerBranch, Unmatched }
```

Matching semantics, identical to today's shell:

- A rule with `is_dir` matches the exact path and everything beneath it.
- A rule without `is_dir` matches the exact path only. `DOTFILES.md` does
  not cover `DOTFILES.md.bak`.
- `Excluded` wins over `Shared` for any path it matches, regardless of line
  order.
- Comments (`#`) and blank lines are skipped.

### 6.2 `tree`

```rust
pub struct BlobId(String);
pub enum FileMode { Regular, Executable, Symlink }
pub struct TreeListing(BTreeMap<RelPath, (FileMode, BlobId)>);

pub fn parse_ls_tree(bytes: &[u8]) -> Result<TreeListing, TreeError>  // `ls-tree -r -z` output
```

`FileMode` is part of the listing because exec bits are load-bearing here:
`tests/scripts-dir-name.test.sh` asserts them, and a sync that dropped a
mode would break a hook. Symlinks are tracked in this repo and must round
trip.

### 6.3 `check`

```rust
pub fn check(manifest: &Manifest, a: &TreeListing, b: &TreeListing) -> CheckReport

pub struct CheckReport { findings: Vec<Finding>, shared_paths_checked: usize }
pub enum Finding {
    Diverged { rule: RelPath },                       // a shared rule whose contents differ
    Unmatched { path: RelPath, present_on: Vec<String> }, // a tracked file matching no rule; which refs carry it
}
```

Byte-identical means equal `BlobId` and equal `FileMode`. The core never
reads file bytes. `Unmatched` is computed over the union of both listings,
because a file added on one branch only must not escape.

Output format is a contract. `tests/pre-push` and
`.github/workflows/branch-drift.yml` grep for `^diverged: ` and
`match no .sync-manifest rule`. The binary preserves the current script's
stdout and stderr text exactly, and the equivalence harness (section 9.4)
enforces that.

### 6.4 `plan`

```rust
pub fn plan_sync(manifest: &Manifest, source: &TreeListing, target: &TreeListing,
                 target_head: CommitId) -> SyncPlan

pub struct SyncPlan { edits: Vec<TreeEdit>, planned_against: CommitId }
pub enum TreeEdit {
    Set { path: RelPath, mode: FileMode, blob: BlobId },
    Remove { path: RelPath },
}
impl SyncPlan {
    pub fn apply_to(&self, target: &TreeListing) -> TreeListing   // pure simulator
    pub fn is_empty(&self) -> bool
}
```

For every path classified `Shared` on either side (never `Excluded`, never
`PerBranch`):

- present on source, absent or different on target: `Set`.
- present on target, absent on source: `Remove`.

The planner is total. It returns no `Result`.

`planned_against` records the target commit the listing came from. The
applier's final step is a compare-and-swap against it (section 6.5).

### 6.5 `git`: the IO edge

```rust
pub struct Git { git_dir: PathBuf, work_tree: PathBuf }

impl Git {
    pub fn ls_tree(&self, rev: &str) -> Result<TreeListing>
    pub fn rev_parse(&self, rev: &str) -> Result<CommitId>
    pub fn current_branch(&self) -> Result<Option<String>>
    pub fn fetch(&self, remote: &str) -> Result<()>
    pub fn commit_plan(&self, plan: &SyncPlan, target_ref: &str, message: &str) -> Result<CommitId>
}
```

`commit_plan` is the only sequence that writes:

1. Create a temp file; set `GIT_INDEX_FILE` to it for every command below.
2. `git read-tree <planned_against>`.
3. For each `Set`: `git update-index --add --cacheinfo <mode>,<blob>,<path>`.
   For each `Remove`: `git update-index --force-remove <path>`.
4. `git write-tree` → tree id.
5. `git commit-tree <tree> -p <planned_against> -m <message>` → commit id.
6. `git update-ref refs/heads/<target> <commit> <planned_against>`.
7. Remove the temp index.

**Atomicity contract.** Steps 1 through 5 create only unreferenced objects.
Nothing observable changes until step 6, and step 6 fails if the ref no
longer points at `planned_against`. A failure at any step leaves the
branch untouched and at most some garbage-collectable objects. An
integration test kills the sequence after step 5 and asserts the ref did
not move.

`$HOME` and the working tree are never read or written by `sync`. The temp
index lives under the system temp dir.

Errors: `ManifestError` and `TreeError` are domain enums. `git` and `main`
use `anyhow`; a failed subprocess includes the command and its stderr.

### 6.6 `main`

Gathers inputs at the edge and passes them in. Resolves the current branch,
fetches, reads the manifest text from the source ref (`git show
<source>:.sync-manifest`), lists both trees, calls the core, applies.
Nothing in `main` decides anything about paths or rules.

## 7. Subcommand behaviour

### 7.1 `config check [ref-a] [ref-b]`

Defaults to `origin/mac` and `origin/linux`, as today. Exit 1 on any
finding. Output as in section 6.3.

### 7.2 `config sync [--dry-run] [--to <branch>]`

1. Current branch must be `mac` or `linux`; otherwise refuse with a
   message. `--to` defaults to the other one.
2. `git fetch origin`.
3. Refuse if local `<target>` is not at `origin/<target>`: the other
   machine has pushed work this would overwrite. Print the pull command.
4. Warn (do not refuse) if the working tree has uncommitted changes under a
   shared path, because sync copies committed content.
5. Compute the plan from `HEAD` and `<target>`. `--dry-run` prints it,
   one line per edit, and exits 0.
6. If the plan is empty, say so and exit 0.
7. Apply (section 6.5). Commit message:
   `Sync shared paths from <source> (<short sha>)`.
8. Run `check HEAD <target>`. It must pass; if it does not, that is a bug,
   and the message says so.
9. Print the two push commands. Never push.

### 7.3 `config test [-q] [--docker] [--watch] [suite]`

Wraps `tests/run-all.sh` (`-q` passes through) or `tests/run-in-docker.sh`
(`--docker`, optional suite name). `--watch` is a dependency-free 1s poll
over `git ls-files` mtimes that reruns on change; no watcher is installed
on either machine and a real one is a platform-split dependency.

### 7.4 `config install`

Wraps `.scripts/deps/check-deps.sh --fix`. Passes `--yes` and `--dry-run`
through.

### 7.5 `config install-hooks`

Symlinks `tests/pre-commit` and `tests/pre-push` into `.cfg/hooks/`, and
`.scripts/config/config` into `~/.local/bin/config`. Idempotent.
`tests/githooks-installed.test.sh` already asserts the hook half.

### 7.6 `config reload`

Runs `tmux source ~/.config/tmux/tmux.conf` when inside tmux, then prints
the `source ~/.zshrc` line for the caller to run (a child process cannot
re-source the caller's shell). This replaces the per-shell `tmux source`
at `.zshrc:174`, which costs 1.7s per pane (all of it `tpm` re-init) and
is removed in the same change.

### 7.7 `config build`

1. `cargo build --release --manifest-path ~/crates/config-manifest/Cargo.toml`
   with `CARGO_TARGET_DIR=~/.cache/config-manifest/target`. No `target/`
   inside the tracked tree, so no `.gitignore` question.
2. Copy the binary to `~/.local/bin/config-manifest`.
3. Write a stamp to `~/.cache/config-manifest/stamp`: the tree id of
   `crates/config-manifest` at `HEAD` (`git rev-parse HEAD:crates/config-manifest`),
   or the literal `dirty` if the crate directory has uncommitted changes.
4. Run the binary once (`--version`), so any first-run cost is paid here.

## 8. Build lifecycle by environment

| Environment | Binary source | Cost |
|---|---|---|
| Host | `config build` (section 7.7) | ~1s warm, ~6s cold |
| pre-push | Reads the stamp; compares to `git rev-parse <pushed-sha>:crates/config-manifest`. Missing, `dirty`, or mismatched: fail with `run config build`. Never compiles. | ~20ms |
| Docker suite | Builder stage `FROM rust:1.94-slim-bookworm@sha256:cf9dd0ec73e75f827fe59123fff9dc65af1a1c8363c3c31ee8d7f8ad0b6a5fb2`, deps-first layer, `cargo build --release`; runtime `COPY --from=builder` the binary only | 0.8s unchanged, 2.6s after a Rust edit, 100s once per machine |
| CI | Rust is preinstalled on `ubuntu-latest` and `macos-latest`. `cargo build --release && cargo test`, binary on PATH, then the suite | seconds |

The runtime image stays Rust-free and near its current size (measured 195MB
for the trial image against 259MB today). Cargo state never touches the
runtime image or the tracked tree.

All three environments find the binary by name on PATH. The dispatcher and
the shell tests use the same lookup.

## 9. Testing

### 9.1 Unit tests (pure core, no git, no filesystem)

- `manifest::parse`: every rule kind, comments, blank lines, trailing
  slash, error line numbers.
- `Manifest::classify`: tabular test of precedence, including `!` inside a
  shared directory and `~` overlap.
- `tree::parse_ls_tree`: recorded `ls-tree -r -z` output, all three modes,
  paths with spaces.
- `check`: each `Finding` kind from hand-built listings; the union rule.
- `plan_sync` then `apply_to`: result equals source filtered to shared
  paths; excluded and per-branch paths untouched.

### 9.2 Property tests (`proptest`)

- Idempotence: `plan_sync(m, s, apply_to(plan_sync(m, s, t), t))` is empty.
- Check after sync: `check(m, s, apply_to(plan, t))` has no `Diverged`.
- The plan never names a path classified `Excluded` or `PerBranch`.
- Parser round trip: `parse(print(m)) == m`.

### 9.3 Integration tests (`tempfile` + `assert_cmd`)

One helper builds a bare fixture repo with two branches. Three tests:

1. `check` is clean on identical shared paths and reports each `Finding`
   kind when they differ.
2. `sync` then `check` is clean; the working tree and untracked files are
   byte-identical before and after.
3. Atomicity: the applier is interrupted after `write-tree`; the target ref
   has not moved.

### 9.4 Equivalence harness during the port

`tests/check-branch-drift.test.sh` (224 lines) is parameterised by a
`CHECK_CMD` environment variable and run against both the shell script and
the binary until they agree on every assertion, including exact output
text. Then pre-push and CI switch to the binary and the shell script is
deleted.

### 9.5 Shell suite additions

- `tests/config.test.sh`: passthrough intact (`config rev-parse HEAD`
  equals git's), unknown subcommand falls through to git's own error, no
  `config-<sub>` shadows a git subcommand, `install-hooks` is idempotent
  and creates both symlinks.
- Stamp check: pre-push refuses on a stale stamp with the expected message.

## 10. Order of work

Each step lands green on its own and is a candidate for one commit series.

1. Scaffold: crate with `--version`, `crates/` in the manifest, overlay
   list, `TRIGGER_PATHS`, CI build step, Docker builder stage, `config
   build` and the stamp. Suite green on host, Docker, CI.
2. `manifest`, `tree`, `check`, TDD. Equivalence harness. Switch pre-push
   and CI to the binary. Delete `check-branch-drift.sh`.
3. `plan`, `git::commit_plan`, `sync`, TDD with the three integration tests.
4. Dispatcher, `install-hooks`, delete the two aliases, `test`, `install`.
5. `reload`, and remove the per-shell `tmux source` from both `.zshrc`
   copies.

Steps 2 and 3 are where the sans-IO standard is exercised; step 1 is
where the build lifecycle is proven before any logic exists.

## 11. Risks and open points

- **`docs/superpowers/` is labelled per-branch** (`~docs/superpowers/`),
  following d62cc13: planning docs describe code that must match; the docs
  themselves need not. This spec therefore lives on `mac` only.
- **Stamp and uncommitted edits.** `dirty` refuses the push until the crate
  is built from a clean tree. That is the intended behaviour; note it in
  the failure message.
- **`readlink -f` on older macOS.** Present since macOS 12.3. Not a concern
  on this machine (Darwin 25).
- **Docker cold pull** of the builder image is ~100s once per machine. CI
  does not run Docker, so it never pays it.
- **Two-way sync** is out of scope. The `SyncPlan` type and the
  planner/applier split are the extension point.

## 12. References

- `docs/research/shell-to-rust-prior-art.md`: the research this design
  rests on, including all measurements cited above.
- `~/.claude/rules/rust.md`, "Sans-IO core, injectable boundaries": the
  crate-wide standard.
- Firezone, "sans-IO: The secret to effective Rust for network services":
  https://www.firezone.dev/blog/sans-io
- `tests/check-branch-drift.sh`: the reference implementation of `check`.
