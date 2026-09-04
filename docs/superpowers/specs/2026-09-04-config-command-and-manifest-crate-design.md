# `config` command and `config-manifest` crate (2026-09-04)

Design for a `config` wrapper command with subcommands, and for
`crates/config-manifest`, a Rust program that owns the `.sync-manifest`
domain: `check` (branch drift) and `sync` (one-way branch sync). Replaces
tests/check-branch-drift.sh and the hand-run branch sync.

Status: approved. Reviewed by two expert panels (structure:
`oo-architecture`, `oo-patterns`, `data-flow`, `fp-effects`; edges:
`ci-pipeline`, `api-design`, `security`, `fp-types`) in place of an owner
read. Their accepted findings are folded in below; the two decisions they
left open were settled on 2026-09-04 and are recorded in section 3.

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
- Illegal states are unrepresentable where a type can carry the proof:
  paths, object ids, the plan's provenance, and the set of paths a plan
  may name.

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
| Host binary freshness | Content-id stamp, never compile in the hook | Every source says hooks that compile get bypassed; a HEAD-based stamp false-refuses in the `commit -a` workflow |
| Crate location | `crates/` | A crate is not a script |
| Git boundary model | Snapshot data in, plan data out; no trait | First panel, unanimous |
| Plan granularity | Domain edits (`Set`, `Remove`), not git ops | Core stays backend-agnostic; `apply_to` gives the same pure simulator |
| Planner input | A pre-partitioned `SharedPaths`, not a raw manifest | The planner cannot name an excluded path; `check` and `plan` consume one partition |
| Leak gate for plumbing commits | `leak-check.sh` gains a range mode; pre-push scans `<remote-sha>..<local-sha>` | `commit-tree` runs no hooks, so `sync` commits and `--no-verify` commits reached a public branch unscanned; verified that today only pre-commit scans, staged content only |
| Compiler pin | None; `Cargo.lock` committed, CI builds `--locked` | The lockfile fixes the dependency tree, which is where host-vs-CI divergence comes from; a toolchain pin costs a rustup download per CI run |

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
    Cargo.lock           committed; the crate is a binary
    src/main.rs          CLI: parse args, gather inputs at the edge, call core, apply
    src/path.rs          RelPath, BlobId, CommitId smart constructors (pure)
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
  `deps-check.yml` is path-filtered to `.scripts/deps/**` and is unrelated.

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

**Name rules.** A `config-<sub>` must not shadow a git subcommand, and a
name that has ever passed through to git must not later be claimed by a
`config-<sub>`, because existing callers of `config <name>` would silently
change meaning. Two tests: for every `config-<sub>` present, `git <sub>` is
not a real command; and the set of `config-<sub>` names equals an allowlist
in the test, so an unexpected executable beside the dispatcher fails the
suite.

**Trust assumptions, checked at install.** `config install-hooks` refuses
if `~/.local/bin` or `.scripts/config/` is group- or world-writable or not
owned by the invoking user. The dispatcher execs whatever is beside it, so
the directory's permissions are the trust boundary.

## 6. `config-manifest`: types and modules

Dependency direction, arrows pointing at what a module depends on:

```
path (no deps)
  |
manifest    tree                 pure; parse text into values
      \      /
       check -- plan             pure; take values, return values
                 \
                 git             the IO edge; depends on tree and plan
                   \
                   main          orchestration, about 30 lines
```

### 6.1 `path`: the proof-carrying primitives

```rust
pub struct RelPath(String);      // repo-relative, forward slashes, no leading '/', no '..' segment, non-empty
pub struct BlobId(String);       // 40 or 64 lowercase hex
pub struct CommitId(String);     // 40 or 64 lowercase hex

impl RelPath  { pub fn parse(raw: &str) -> Result<Self, PathError>; pub fn as_str(&self) -> &str }
impl BlobId   { pub fn parse(raw: &str) -> Result<Self, IdError> }
impl CommitId { pub fn parse(raw: &str) -> Result<Self, IdError> }
```

Fields are private. The only constructors are the fallible `parse`
functions, called at exactly the boundaries where text enters:
`manifest::parse` and `tree::parse_ls_tree` for paths, `parse_ls_tree` and
`git::rev_parse` for ids. Everything downstream receives the proof and
never re-validates. A hand-edited manifest line with a leading slash or a
`..` is a parse error with a line number, not a runtime surprise in
`update-index --cacheinfo`.

All types in this crate derive `PartialEq`, `Eq`, and `Debug`; the
ordered ones derive `Ord`.

### 6.2 `manifest`

```rust
pub enum Rule { Shared(PathPattern), Excluded(PathPattern), PerBranch(PathPattern) }
pub struct PathPattern { path: RelPath, is_dir: bool }   // is_dir = trailing slash
pub struct Manifest { rules: Vec<Rule> }

pub fn parse(text: &str) -> Result<Manifest, ManifestError>   // line number in the error
pub fn print(manifest: &Manifest) -> String                    // canonical form; parse(print(m)) == m

impl Manifest {
    pub fn classify(&self, path: &RelPath) -> Classification
    pub fn partition(&self, paths: impl Iterator<Item = &RelPath>) -> Partition
}
pub enum Classification { Shared, Excluded, PerBranch, Unmatched }
pub struct Partition { shared: SharedPaths, unmatched: Vec<RelPath> }
pub struct SharedPaths(BTreeSet<RelPath>);   // constructible only by Manifest::partition
```

Matching semantics, identical to today's shell:

- A rule with `is_dir` matches the exact path and everything beneath it.
- A rule without `is_dir` matches the exact path only. `DOTFILES.md` does
  not cover `DOTFILES.md.bak`.
- `Excluded` wins over `Shared` for any path it matches, regardless of line
  order.
- Comments (`#`) and blank lines are skipped.

`partition` runs `classify` once over the union of both trees' paths and
returns the shared set and the unmatched list. `check` and `plan` both
consume that one `Partition`, so they cannot disagree about a path, and the
planner cannot ask about an excluded path because it is never handed one.

### 6.3 `tree`

```rust
pub enum FileMode { Regular, Executable, Symlink }
pub struct TreeListing(BTreeMap<RelPath, (FileMode, BlobId)>);

pub fn parse_ls_tree(bytes: &[u8]) -> Result<TreeListing, TreeError>  // `ls-tree -r -z` output
```

`FileMode` is part of the listing because exec bits are load-bearing here:
`tests/scripts-dir-name.test.sh` asserts them, and a sync that dropped a
mode would break a hook. Symlinks are tracked in this repo and must round
trip.

### 6.4 `check`

```rust
pub fn check(partition: &Partition, a: &TreeListing, b: &TreeListing) -> CheckReport

pub struct CheckReport { findings: Vec<Finding>, shared_paths_checked: usize }
pub enum Finding {
    Diverged { rule: RelPath },                    // a shared rule whose contents differ
    Unmatched { path: RelPath, present_on: PresentOn },
}
pub enum PresentOn { A, B, Both }                 // exactly two trees are compared
```

Byte-identical means equal `BlobId` and equal `FileMode`. The core never
reads file bytes. `Unmatched` comes from the partition over the union of
both listings, because a file added on one branch only must not escape.

**Output format is a contract.** `tests/pre-push` and
`.github/workflows/branch-drift.yml` grep for `^diverged: ` and
`match no .sync-manifest rule`. The binary preserves the current script's
stdout and stderr text exactly, including which stream each line goes to,
and the equivalence harness (section 9.4) enforces that.

### 6.5 `plan`

```rust
pub struct TargetSnapshot { listing: TreeListing, head: CommitId }   // one value, cannot drift apart

pub fn plan_sync(shared: &SharedPaths, source: &TreeListing, target: &TargetSnapshot) -> SyncPlan

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

For every path in `shared` (which by construction contains no `Excluded`
or `PerBranch` path):

- present on source, absent or different on target: `Set`.
- present on target, absent on source: `Remove`.

`TreeEdit`'s fields are private and its only constructor is inside
`plan.rs`, so nothing outside the planner can fabricate a `Set` naming a
blob that is not in `source`. The planner is total over valid inputs and
returns no `Result`.

`planned_against` is copied from `target.head`, so the plan cannot carry a
provenance that disagrees with the listing it was computed from. The
applier's final step is a compare-and-swap against it (section 6.6).

### 6.6 `git`: the IO edge

```rust
pub struct Git { git_dir: PathBuf, work_tree: PathBuf }

impl Git {
    pub fn discover(root: &Path) -> Result<Git>     // bare repo at root/.cfg, or a normal repo at root
    pub fn ls_tree(&self, rev: &str) -> Result<TreeListing>
    pub fn rev_parse(&self, rev: &str) -> Result<CommitId>
    pub fn current_branch(&self) -> Result<Option<String>>
    pub fn fetch(&self, remote: &str) -> Result<()>
    pub fn commit_plan(&self, plan: &SyncPlan, target_ref: &str, message: &str) -> Result<CommitId>
}
```

`discover` preserves the shell script's two repo shapes: the real dotfiles
repo is bare at `<root>/.cfg` with `<root>` as the worktree, and the test
fixtures are normal repos with `.git` inside. `root` comes from
`$DOTFILES_ROOT` when set, else `$HOME`, exactly as today. The equivalence
harness exercises the fixture shape; a dedicated test exercises the bare
shape against a fixture built as a bare repo.

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

**Hooks do not run.** `commit-tree` and `update-ref` are plumbing and
invoke no hook, so the pre-commit leak check never sees a `sync` commit.
The gate that covers it is at push time: pre-push runs `leak-check.sh`
in range mode over every commit being pushed (section 8), which also
covers `--no-verify` commits. `sync` additionally copies only blobs
already committed on the source branch, but that is a property, not the
guarantee.

`$HOME` and the working tree are never read or written by `sync`. The temp
index lives under the system temp dir.

Errors: `PathError`, `IdError`, `ManifestError`, and `TreeError` are domain
enums. `git` and `main` use `anyhow`; a failed subprocess includes the
command and its stderr.

### 6.7 `main`

Gathers inputs at the edge and passes them in. Resolves the root, discovers
the repo, resolves the current branch, fetches, reads the manifest text
from the source ref (`git show <source>:.sync-manifest`), lists both trees,
partitions, calls the core, applies. Nothing in `main` decides anything
about paths or rules.

## 7. Subcommand behaviour

Exit codes are a contract for every subcommand. `0` success, `1` a finding
or a refusal the user can act on, `2` usage error (bad flag, bad value),
`3` an internal inconsistency that indicates a bug. Diagnostics, warnings,
and refusals go to stderr. Results and plans go to stdout. `--dry-run`
prints the plan to stdout and any warning to stderr, so a script capturing
stdout gets only the plan.

### 7.1 `config check [ref-a] [ref-b]`

Defaults to `origin/mac` and `origin/linux`, as today. Exit `1` on any
finding (the shell script's two failure exits collapse to one; the text
still distinguishes them and no consumer branches on the code). Output as
in section 6.4.

### 7.2 `config sync [--dry-run] [--to <branch>]`

1. Current branch must be `mac` or `linux`; otherwise refuse, exit `1`.
   `--to` defaults to the other one. `--to` must name `mac` or `linux` and
   must differ from the current branch; otherwise exit `2`.
2. `git fetch origin`.
3. Refuse (exit `1`) if local `<target>` is not at `origin/<target>`: the
   other machine has pushed work this would overwrite. Print the pull
   command on stderr.
4. Warn on stderr (do not refuse) if the working tree has uncommitted
   changes under a shared path, because sync copies committed content.
5. Compute the plan from `HEAD` and `<target>`. `--dry-run` prints it to
   stdout, one line per edit, and exits `0`.
6. If the plan is empty, say so on stdout and exit `0`.
7. Apply (section 6.6). Commit message:
   `Sync shared paths from <source> (<short sha>)`.
8. Run `check HEAD <target>`. It must pass; if it does not, print that this
   is a bug and exit `3`.
9. Print the two push commands on stdout. Never push.

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
`.scripts/config/config` into `~/.local/bin/config`. Checks the permission
assumptions in section 5 first and refuses if they fail. Idempotent.
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
3. Write the stamp to `~/.cache/config-manifest/stamp`: the tree id of the
   crate directory **as it is in the worktree**, computed with a temp
   index (`GIT_INDEX_FILE=<tmp> git add crates/config-manifest`, then
   `git write-tree`, then the `crates/config-manifest` subtree id). This
   is the content the binary was built from, whether or not it is
   committed yet.
4. Run the binary once (`--version`), so any first-run cost is paid here.

The stamp is a staleness check, not a trust boundary. It lives in a
user-writable cache and asserts nothing about who built the binary.

## 8. Build lifecycle by environment

| Environment | Binary source | Cost |
|---|---|---|
| Host | `config build` (section 7.7) | ~1s warm, ~6s cold |
| pre-push | First, `leak-check.sh --range <remote-sha>..<local-sha>` over the pushed commits; any hit blocks the push. Then compares the stamp to `git rev-parse <pushed-sha>:crates/config-manifest`. Equal: proceed. Missing: `config-manifest has not been built; run config build`, exit 1. Mismatch: `config-manifest is stale for <sha>; run config build`, exit 1. Never compiles. | leak scan: one diff pass; stamp: ~20ms |
| Docker suite | Builder stage `FROM rust:1.94-slim-bookworm@sha256:cf9dd0ec73e75f827fe59123fff9dc65af1a1c8363c3c31ee8d7f8ad0b6a5fb2`; layer order: `COPY Cargo.toml Cargo.lock`, build with a stub `main.rs` to cache dependencies, then `COPY src`, then build. Runtime stage is the existing digest-pinned `debian:bookworm-slim`; `COPY --from=builder` the binary only. | 0.8s unchanged, 2.6s after a Rust edit, 100s once per machine |
| CI | Rust is preinstalled on `ubuntu-latest` and `macos-latest`. `cargo build --release --locked && cargo test`, binary on PATH, then the suite | seconds |

Because the stamp is content-based, "committed the same content after
building" matches, and only genuinely different source refuses. The
Docker layer order is part of the contract: a single `COPY` of the whole
crate would silently defeat the dependency cache the measurements rest on.

The runtime image stays Rust-free and near its current size (measured 195MB
for the trial image against 259MB today). Cargo state never touches the
runtime image or the tracked tree.

All three environments find the binary by name on PATH. The dispatcher and
the shell tests use the same lookup. `Cargo.lock` is committed and CI
builds with `--locked`, so the host, Docker, and CI binaries resolve the
same dependency tree.

## 9. Testing

### 9.1 Unit tests (pure core, no git, no filesystem)

- `path`: `RelPath::parse` rejects absolute, `..`, and empty; accepts
  spaces and unicode. `BlobId` and `CommitId` reject wrong length and
  non-hex.
- `manifest::parse`: every rule kind, comments, blank lines, trailing
  slash, error line numbers.
- `Manifest::classify` and `partition`: tabular test of precedence,
  including `!` inside a shared directory and `~` overlap.
- `tree::parse_ls_tree`: recorded `ls-tree -r -z` output, all three modes,
  paths with spaces.
- `check`: each `Finding` kind and each `PresentOn` from hand-built
  listings; the union rule.
- `plan_sync` then `apply_to`: result equals source filtered to shared
  paths; excluded and per-branch paths untouched.

### 9.2 Property tests (`proptest`)

- Idempotence: `plan_sync(s, src, apply_to(plan_sync(s, src, t), t))` is
  empty.
- Check after sync: `check(p, src, apply_to(plan, t))` has no `Diverged`.
- The plan never names a path outside `shared` (redundant once
  `SharedPaths` exists; kept as a regression test).
- Every `Set` names a blob present in `source` at that path.
- Parser round trip: `parse(print(m)) == m`.

### 9.3 Integration tests (`tempfile` + `assert_cmd`)

One helper builds a fixture repo with two branches, in both shapes (normal
and bare-with-worktree). Four tests:

1. `check` is clean on identical shared paths and reports each `Finding`
   kind when they differ.
2. `sync` then `check` is clean; the working tree and untracked files are
   byte-identical before and after.
3. Atomicity: the applier is interrupted after `write-tree`; the target ref
   has not moved.
4. Exit codes: each refusal in 7.2 returns `1`, a bad `--to` returns `2`.

### 9.4 Equivalence harness during the port

`tests/check-branch-drift.test.sh` (224 lines) is parameterised by a
`CHECK_CMD` environment variable and run against both the shell script and
the binary until they agree on every assertion, including exact output
text and stream. Then pre-push and CI switch to the binary and the shell
script is deleted.

### 9.5 Shell suite additions

- `tests/config.test.sh`: passthrough intact (`config rev-parse HEAD`
  equals git's), unknown subcommand falls through to git's own error, the
  `config-<sub>` set equals the allowlist, no name shadows a git
  subcommand, `install-hooks` is idempotent, creates both symlinks, and
  refuses on a world-writable directory.
- Stamp check: pre-push refuses on a missing and on a stale stamp with the
  two expected messages, and passes when the same content was committed
  after the build.

## 10. Order of work

Each step lands green on its own and is a candidate for one commit series.

0. Leak gate first, before any plumbing commit path exists:
   `tests/leak-check.sh` gains `--range <a>..<b>` (scan the diff of the
   range with the same patterns and allow list as the staged mode), pre-push
   calls it before the drift check, and `tests/leak-check.test.sh` covers
   both modes in both directions: each pattern catches a planted fake
   secret, and the allow list never suppresses a real one. Fixtures live
   under the per-run temp dir; no real credentials. This is the queued
   TODO, done first because step 3 depends on it.
1. Scaffold: crate with `--version` **and the full dependency set already
   in `Cargo.toml` and `Cargo.lock`** (`anyhow`, `proptest`, `assert_cmd`,
   `tempfile`), so the lifecycle is proven against the real dependency
   tree, not a hello-world. `crates/` in the manifest, overlay list,
   `TRIGGER_PATHS`, CI build step with `--locked`, Docker builder stage
   with the stated layer order, `config build` and the content stamp.
   Suite green on host, Docker, CI.
2. `path`, `manifest`, `tree`, `check`, TDD. Equivalence harness. Switch
   pre-push and CI to the binary. Delete check-branch-drift.sh.
3. `plan`, `git::commit_plan`, `sync`, TDD with the four integration tests.
   Requires step 0 merged: `sync` must never exist without the push-time
   leak gate.
4. Dispatcher with name and permission checks, `install-hooks`, delete the
   two aliases, `test`, `install`.
5. `reload`, and remove the per-shell `tmux source` from both `.zshrc`
   copies.

Steps 2 and 3 are where the sans-IO standard is exercised; step 1 is
where the build lifecycle is proven before any logic exists.

## 11. Risks

- **`docs/superpowers/` is labelled per-branch** (`~docs/superpowers/`),
  following d62cc13: planning docs describe code that must match; the docs
  themselves need not. This spec therefore lives on `mac` only.
- **`readlink -f` on older macOS.** Present since macOS 12.3. Not a concern
  on this machine (Darwin 25).
- **Docker cold pull** of the builder image is ~100s once per machine. CI
  does not run Docker, so it never pays it.
- **Digest pins rot.** The builder and runtime digests are correct today
  and have no renewal mechanism. Revisit when either base publishes a
  security fix; a Dependabot config for Dockerfiles is the standard answer
  if it becomes a chore.
- **Compiler version floats with the runner image** (no toolchain pin, by
  decision). A compiler-version bug surfaces as a build error in CI, not as
  a silent behaviour change, because `Cargo.lock` fixes the dependencies.
  Add `rust-toolchain.toml` if that ever happens.
- **`plan_sync`'s signature is contract-open.** Two-way sync will add a
  conflict rule as input; callers are `main` and the tests only, so that
  change is expected and not a breaking-change concern.

## 12. References

- `docs/research/shell-to-rust-prior-art.md`: the research this design
  rests on, including all measurements cited above.
- `~/.claude/rules/rust.md`, "Sans-IO core, injectable boundaries": the
  crate-wide standard.
- Firezone, "sans-IO: The secret to effective Rust for network services":
  https://www.firezone.dev/blog/sans-io
- The former `check-branch-drift` shell script, replaced by `config-manifest check`: the reference implementation of `check`.
