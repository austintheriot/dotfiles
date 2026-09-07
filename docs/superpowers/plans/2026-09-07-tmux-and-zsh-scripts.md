# tmux and zsh Scripts Port Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the computation in five shell scripts into a Rust binary,
leaving in shell only what a subprocess genuinely cannot do: `tmux attach`,
which must own the caller's TTY, and `LBUFFER=`, which must run inside the
zsh line editor.

**Architecture:** One new crate, `tmux-core`, holding the pure logic (window
naming, layout selection, branch listing) with no IO, plus a thin
`tmux-tools` binary that performs the tmux and git calls and drives it. The
existing `.scripts/*.sh` files either become shims that exec the binary, or
keep their `source` contract while delegating computation to it. This mirrors
the pure-core split `deps-core` and `config-manifest` already use.

**Tech Stack:** Rust 2024 edition, toolchain pinned to 1.94.0 by
`crates/rust-toolchain.toml` (rustup honours it only when the working
directory is under `crates/`). POSIX sh and zsh for the surviving shims. No
new third-party dependencies.

**Spec:** `docs/superpowers/specs/2026-09-07-tmux-and-zsh-scripts-design.md`.
Read sections 2, 3.1, 3.2 and 5 before Task 1: section 3 carries the two
decisions that unblocked this step, and section 5 names three things that
must not break.

## Global Constraints

Copied verbatim from the spec and from `~/.claude/CLAUDE.md`.

- **`tmux-core` performs NO IO.** No `std::fs`, `std::process`, `std::env`,
  `std::io`, or `Command::new`. Follow `deps-core`'s enforcement exactly: a
  purity test that reads its own sources with `include_str!` and asserts none
  of those paths appears. **No forbidden path may appear as a literal in any
  scanned file, INCLUDING inside a doc comment**, because the test scans the
  file holding its own needles. Assemble needles from runtime segments, as
  `deps-core/src/lib.rs` does. Add every new module to the array in the same
  commit that creates it.
- **`tmux-update-window-names.sh` runs in `precmd`** (`.zshrc:236`), so it
  fires on **every prompt draw**, not once at startup. Measured on this
  machine before planning: ~18ms outside tmux (early exit, no `$TMUX`), ~84ms
  doing real work on a one-window session, and **442ms for `-a` across the 21
  live windows of the `code` session**, with 17 spawn sites in the script.
  The port must not make the per-prompt path slower. Measure before and after
  and report both numbers.
- **`zshrc-startup-budget.test.sh` times interactive startup** against a
  budget set well above the measured ~180ms. No script here may add a spawn
  to `.zshrc` startup. `zsh-git-widgets.sh` is *sourced* at init and only its
  widget *body* gains a spawn, which runs on Ctrl+G, not at startup.
- **`tmux-update-window-names.test.sh` asserts `$HOME` renders as `~`.** The
  spec warns this is agreement rather than evidence: `/Users/austin` is
  itself a bare-repo worktree with no `.git`, so both the old and new
  implementations skip the repository branch identically. **A converted
  implementation needs its own explicit case for the bare-repo shape**, and a
  test for it, or the `~` assertion passes for the wrong reason.
- **The six `after-*` tmux hooks in `.config/tmux/tmux.conf` fire the naming
  script asynchronously**, which is why the tmux suites are flaky on a
  developer machine and stable on a runner. Do not add a hook, and do not
  make an existing hook synchronous.
- **`tmux attach` must stay in shell.** A subprocess that attaches attaches
  itself. `tmux-setup.sh` has two such calls and `tmux-start.sh` has two
  (`new-session -A` and `attach`). The attach moves into the alias; the
  binary computes and the alias attaches.
- **`LBUFFER=` must stay in the zsh widget.** Assignment into the line editor
  is impossible from another process. `zle -N` and the three `bindkey` calls
  stay shell too.
- **Every Rust invocation runs from inside `crates/`, never with
  `--manifest-path`.** rustup honours the toolchain pin only when the working
  directory is under `crates/`.
- **Any change under `crates/` moves the workspace build stamp**, which makes
  the installed binaries stale, which the live pre-push gate BLOCKS. After
  each task run `config build`, then confirm `config doctor` exits 0 and
  prints nothing.
- **`tests/container.test.sh` asserts the Docker builder's `COPY` list
  matches cargo's workspace members.** Adding `tmux-core` and `tmux-tools`
  without adding their `COPY` lines to `tests/docker/Dockerfile` fails that
  test rather than the Docker build. Add all of: the two
  `COPY crates/<name>/Cargo.toml <name>/` lines, the two stub-source lines in
  the dependency-cache `RUN`, and the two `COPY crates/<name>/src` lines.
- **`.scripts/config/config-stamp` reads the workspace members with `sed`**
  and requires `members` to stay on ONE line in `crates/Cargo.toml`. A
  multi-line array silently yields no members.
- **Use `config commit -F <file>` with a heredoc, NEVER `config commit -m`.**
- **Do NOT run `config status -uall` and do NOT run `config stash`.** Both
  walk all of `$HOME` and can leave `.cfg/index.lock` held.
- **Clear git's environment before running `cargo test` from any hook or
  script**: `env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE -u GIT_PREFIX`.
  See `.agents/PAPERCUTS.md`: a test fixture's `git add .` will otherwise
  operate on the real dotfiles repo and take `~/.cfg/index.lock`.
- NEVER `--no-verify`. NEVER disable a test instead of fixing it. NEVER
  commit a red suite.
- NO em dashes anywhere. NO emoji.
- NO single-letter variable names except numeric loop indices `i`, `j`, `k`.
  Closure parameters included: write `.map(|window| window.name)`, never
  `|w|`.
- Comment WHY not WHAT. Doc comments on public items, with `# Errors` where
  they apply. No `unwrap()`/`expect()` in non-test code without a proven
  invariant.
- Every empty-expected assertion needs a positive control FIRST.
- **An assertion is not a test until you have observed it fail against the
  unfixed code.** Every task has an explicit red step; report both
  observations with real output.

## Repository Context

Bare git repo at `~/.cfg` with `$HOME` as the worktree. Plain `git` does NOT
work in `~/crates`. The `config` command is git against that bare repo, and
tracked paths are home-relative (`.scripts/tmux-close.sh`,
`crates/tmux-core/src/lib.rs`).

The alias contracts, which are the public surface being preserved:

| Alias | `.zshrc` line | Invocation |
|---|---|---|
| `s` | 60 | `source ~/.scripts/tmux-start.sh` |
| `se` | 67 | `source ~/.scripts/tmux-setup.sh` |
| `sp` | 76 | `source ~/.scripts/tmux-split.sh` |
| `c` | 81 | `source ~/.scripts/tmux-close.sh` |
| `re` | 85 | `~/.scripts/tmux-update-window-names.sh` (executed) |
| (precmd) | 236 | `~/.scripts/tmux-update-window-names.sh` (executed, every prompt) |
| (init) | 264 | `source ~/.scripts/zsh-git-widgets.sh` |

## File Structure

| File | Lines now | Responsibility after this plan |
|---|---|---|
| `crates/tmux-core/src/lib.rs` | **new** | Crate root, re-exports, purity test with positive control. |
| `crates/tmux-core/src/naming.rs` | **new** | Window-name computation: precedence, label, bare-repo glob matching. Pure. |
| `crates/tmux-core/src/layout.rs` | **new** | Layout selection and the "not a layout" decision from spec 3.1. Pure. |
| `crates/tmux-core/src/branches.rs` | **new** | Branch-list formatting for the zsh widget, per spec 3.2. Pure. |
| `crates/tmux-tools/src/main.rs` | **new** | Subcommand dispatch, exit codes. |
| `crates/tmux-tools/src/tmux.rs` | **new** | The tmux calls. The only place `Command::new("tmux")` appears. |
| `crates/tmux-tools/src/repo.rs` | **new** | Git inspection: direct `.git`/`commondir`/`HEAD` reads with `git rev-parse` fallback. |
| `.scripts/tmux-update-window-names.sh` | 241 | Shim: execs `tmux-tools name-windows "$@"`. |
| `.scripts/tmux-close.sh` | 17 | Shim, keeps its `source` contract. |
| `.scripts/tmux-worktree-config.sh` | 8 | Shim. |
| `.scripts/zsh-git-widgets.sh` | 27 | Keeps `LBUFFER=`, `zle -N`, `bindkey`; delegates the listing. |
| `.scripts/tmux-split.sh` | 89 | Explicit argument per 3.1; keeps `source`. |
| `.scripts/tmux-start.sh` | 38 | Attach moves into the alias; calls `tmux-split "$1"` explicitly. |
| `.scripts/tmux-setup.sh` | 82 | Two attach calls move into the alias. |

**Why two crates rather than one.** `tmux-core` must hold zero IO so its
purity test means something, and the tmux and git calls are irreducibly IO.
That is the same split `deps-core` (pure) and `config-manifest` (IO) already
use, and the same reason.

**Why `naming.rs`, `layout.rs` and `branches.rs` are separate files.** They
share no types and change for unrelated reasons: naming changes when tmux
option handling changes, layout when a layout is added, branches when the
widget's display changes. One file per responsibility keeps each holdable in
context.

---

## Task 1: `tmux-core` and `tmux-tools` skeletons, with the purity gate

Two empty crates that build, are registered everywhere they must be, and
whose purity test already works. Nothing converts yet. This task exists on
its own because registration has five separate places to miss, and finding
out during a later task which one was forgotten is the expensive path.

**Files:**
- Create: `crates/tmux-core/Cargo.toml`, `crates/tmux-core/src/lib.rs`
- Create: `crates/tmux-tools/Cargo.toml`, `crates/tmux-tools/src/main.rs`
- Modify: `crates/Cargo.toml` (members, on ONE line)
- Modify: `tests/docker/Dockerfile` (three places, per Global Constraints)

**Interfaces:**
- Consumes: nothing; this is first.
- Produces: `tmux_core` as a library crate with a passing purity test, and a
  `tmux-tools` binary that runs and exits 2 on an unknown subcommand. Tasks 2
  through 6 add modules to `tmux-core` and subcommands to `tmux-tools`.

- [ ] **Step 1: Write the failing test**

Create `crates/tmux-core/src/lib.rs` with only the purity module, copied in
shape from `crates/deps-core/src/lib.rs:44-121`. Read that file first and
match it; the needle assembly is the part that must not drift.

```rust
//! Pure computation for the tmux and zsh helper scripts.
//!
//! Holds no capabilities: the window-name precedence, the layout choice and
//! the branch-list formatting are all functions of injected values. The
//! `tmux-tools` binary performs the tmux and git calls and hands the results
//! in. That split is what makes this crate's behavior testable without a
//! tmux server, and it is the same split `deps-core` uses.

#[cfg(test)]
mod purity {
    /// The crate source must name no IO capability.
    ///
    /// `include_str!` reads at compile time, so this test spawns nothing and
    /// opens nothing at runtime.
    ///
    /// Each later task adds its own module to `sources` in the same commit
    /// that adds the module. A module absent from this array is unchecked.
    #[test]
    fn no_module_names_an_io_capability() {
        let sources = [("lib.rs", include_str!("lib.rs"))];
        for (file_name, source) in sources {
            for forbidden in forbidden_capabilities() {
                assert!(
                    !source.contains(&forbidden),
                    "{file_name} names {forbidden}; tmux-core performs no IO"
                );
            }
        }
    }

    /// Positive control for the assertion above.
    ///
    /// The purity test asserts an absence, so it passes vacuously if
    /// `include_str!` ever yields empty text or a needle stops matching.
    #[test]
    fn the_purity_check_detects_a_forbidden_string() {
        for forbidden in forbidden_capabilities() {
            let planted = format!("fn reach_out() {{ {forbidden}::whatever() }}");
            assert!(
                planted.contains(&forbidden),
                "a needle the purity test relies on does not match a source that names it"
            );
        }
        for source in [include_str!("lib.rs")] {
            assert!(
                !source.is_empty(),
                "include_str! yielded empty text, so the purity test proves nothing"
            );
        }
    }

    /// The capability paths no module may name.
    ///
    /// Assembled from segments rather than written as literals, because this
    /// file is one of the files the check scans and a literal here would make
    /// the crate fail its own purity test.
    fn forbidden_capabilities() -> Vec<String> {
        ["fs", "process", "env", "io"]
            .into_iter()
            .map(|capability| format!("std::{capability}"))
            .collect()
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

```sh
cd ~/crates && cargo test --locked -p tmux-core
```

Expected: FAIL, because `tmux-core` is not a workspace member yet, with
`error: package ID specification 'tmux-core' did not match any packages`.
That is the right red: the crate genuinely does not exist to cargo. Record
the actual message.

- [ ] **Step 3: Create both manifests and register the members**

`crates/tmux-core/Cargo.toml`:

```toml
[package]
name = "tmux-core"
version = "0.1.0"
edition = "2024"

[dependencies]
```

`crates/tmux-tools/Cargo.toml`:

```toml
[package]
name = "tmux-tools"
version = "0.1.0"
edition = "2024"

[dependencies]
tmux-core = { path = "../tmux-core" }
```

In `crates/Cargo.toml`, extend `members` **keeping it on one line**, because
`.scripts/config/config-stamp` reads it with `sed` and a multi-line array
silently yields no members:

```toml
members = ["config-manifest", "deps-core", "dotfiles-path", "tmux-core", "tmux-tools"]
```

`crates/tmux-tools/src/main.rs`:

```rust
//! The IO half of the tmux and zsh helpers.
//!
//! Performs the tmux and git calls and hands their results to `tmux_core`,
//! which decides. Exit 2 is every usage error, matching the repo-wide
//! convention `check-deps.sh` and `config-manifest` already use.

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    match arguments.next().as_deref() {
        Some(other) => {
            eprintln!("tmux-tools: unknown subcommand {other}");
            ExitCode::from(2)
        }
        None => {
            eprintln!("tmux-tools: a subcommand is required");
            ExitCode::from(2)
        }
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

```sh
cd ~/crates && cargo test --locked -p tmux-core
cd ~/crates && cargo run --locked -p tmux-tools -- bogus; echo "exit=$?"
```

Expected: two purity tests PASS; the binary prints the unknown-subcommand
message and `exit=2`.

- [ ] **Step 5: Verify the purity test actually scans the new file**

The array is the one part that fails silently when incomplete.

```sh
cd ~/crates
printf 'const _: &str = "std::process";\n' >> tmux-core/src/lib.rs
cargo test --locked -p tmux-core purity 2>&1 | grep -cE 'FAILED|panicked'
sed -i '' -e '$d' tmux-core/src/lib.rs
cargo test --locked -p tmux-core purity 2>&1 | tail -1
```

Expected: a nonzero count from the sabotaged run, proving `lib.rs` is
scanned; the restored run passes. Note the needle above is a real forbidden
path, so it must be removed before committing; confirm with
`grep -c 'std::process' tmux-core/src/lib.rs` returning 0.

- [ ] **Step 6: Register both crates in the Docker builder**

`tests/container.test.sh` asserts cargo's member list matches
`tests/docker/Dockerfile`'s `COPY` lines. Add, matching the existing shape
exactly:

```dockerfile
COPY crates/tmux-core/Cargo.toml tmux-core/
COPY crates/tmux-tools/Cargo.toml tmux-tools/
```

Extend the stub-source `RUN` so the dependency cache still builds:

```dockerfile
RUN mkdir config-manifest/src dotfiles-path/src deps-core/src tmux-core/src tmux-tools/src \
    && echo 'fn main() {}' > config-manifest/src/main.rs \
    && echo '' > dotfiles-path/src/lib.rs \
    && echo '' > deps-core/src/lib.rs \
    && echo '' > tmux-core/src/lib.rs \
    && echo 'fn main() {}' > tmux-tools/src/main.rs \
    && cargo build --release --locked --quiet \
    && rm -rf config-manifest/src tmux-tools/src target/release/config-manifest \
        target/release/tmux-tools target/release/deps/config_manifest* \
        target/release/deps/tmux_tools*
```

Read the real `RUN` block before editing and preserve whatever it already
removes; the block above shows the shape, not necessarily the current text.
Then add the two source copies beside the existing ones:

```dockerfile
COPY crates/tmux-core/src ./tmux-core/src
COPY crates/tmux-tools/src ./tmux-tools/src
```

- [ ] **Step 7: Verify the container guard passes**

```sh
cd ~ && bash tests/container.test.sh 2>&1 | tail -3
```

Expected: PASS, with the member-list assertion satisfied. If it fails naming
a missing crate, a `COPY` line is absent; if it fails naming an extra one,
the members line has a typo.

- [ ] **Step 8: Verify the stamp reader still sees every member**

```sh
cd ~ && DOTFILES_ROOT="$HOME" .scripts/config/config-stamp | sort
```

Expected: five lines, one per member, including `tmux-core` and
`tmux-tools`. If it prints three, the `members` array went multi-line and
`sed` stopped matching.

- [ ] **Step 9: Rebuild, check the stamp, and commit**

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
```

Expected: `doctor=0`, no output.

```sh
cd ~
cat > /tmp/msg.txt <<'MSG'
Add the tmux-core and tmux-tools crate skeletons

Two crates rather than one, for the reason deps-core and config-manifest are
already two: tmux-core must hold zero IO for its purity test to mean
anything, and the tmux and git calls are irreducibly IO.

Nothing converts yet. Registration has five separate places to miss (the
members line, two Dockerfile COPY groups, the stub-source RUN, and the stamp
reader), and discovering a missed one during a conversion task is the
expensive path.

The purity test ships with its positive control and its coverage is verified
rather than assumed: a planted forbidden literal fails the check, proving
lib.rs is actually scanned.

members stays on one line because config-stamp reads it with sed and a
multi-line array silently yields no members.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/ tests/docker/Dockerfile
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 2: Window-name computation in `tmux-core::naming`

The pure half of the largest script. `tmux-update-window-names.sh` is 241 of
the 502 lines in scope and is already *executed* rather than sourced, so it
has no sourcing contract to preserve. The spec puts it first for that reason.

Measured before planning, on this machine: ~18ms outside tmux, ~84ms on a
one-window session, **442ms for `-a` across 21 live windows**, with 17 spawn
sites. The conversion is a latency win, and Task 4 is where that gets
measured again.

**Files:**
- Create: `crates/tmux-core/src/naming.rs`
- Modify: `crates/tmux-core/src/lib.rs` (declare the module, re-export, and
  add `naming.rs` to BOTH purity source lists)

**Interfaces:**
- Consumes: nothing from Task 1 beyond the crate existing.
- Produces:

```rust
pub struct WindowFacts {
    pub directory_basename: String,
    pub is_home: bool,
    pub repository: Option<RepositoryFacts>,
    pub manual_name: Option<String>,
    pub label: Option<String>,
}
pub struct RepositoryFacts {
    pub main_repo_name: String,
    pub head: HeadState,
}
pub enum HeadState { Branch(String), Detached { short_sha: String } }
pub fn window_name(facts: &WindowFacts, bare_repo_patterns: &[String]) -> String;
```

Task 4's binary gathers `WindowFacts` and calls `window_name`.

- [ ] **Step 1: Write the failing test**

Read `.scripts/tmux-update-window-names.sh` in full first: its header
documents the precedence, the `@wname_label` behavior and the
`@wname_bare_repos` glob list, and those are the contract these tests pin.

Create `crates/tmux-core/src/naming.rs` with only its test module, so the red
run fails on missing implementation rather than a missing file.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn a_repo_window() -> WindowFacts {
        WindowFacts {
            directory_basename: "crates".to_string(),
            is_home: false,
            repository: Some(RepositoryFacts {
                main_repo_name: "dotfiles".to_string(),
                head: HeadState::Branch("main".to_string()),
            }),
            manual_name: None,
            label: None,
        }
    }

    /// A repository window is named "repo/branch", which is precedence
    /// level 2 from the script's own header.
    #[test]
    fn a_repository_window_is_named_repo_slash_branch() {
        let name = window_name(&a_repo_window(), &[]);

        // Positive control: the name must not be empty, or the equality
        // below would hold for a function that returns nothing at all.
        assert!(!name.is_empty(), "a window must get a name");
        assert_eq!(name, "dotfiles/main");
    }

    /// A manual name set with `prefix ,` outranks everything, which is
    /// precedence level 3.
    #[test]
    fn a_manual_name_outranks_the_repository() {
        let mut facts = a_repo_window();
        facts.manual_name = Some("Reviews".to_string());

        assert_eq!(window_name(&facts, &[]), "Reviews");
    }

    /// An empty manual name drops back to automatic naming, which the
    /// script's header states explicitly.
    #[test]
    fn an_empty_manual_name_falls_back_to_automatic() {
        let mut facts = a_repo_window();
        facts.manual_name = Some(String::new());

        assert_eq!(window_name(&facts, &[]), "dotfiles/main");
    }

    /// A directory that is not a repository is named by basename, which is
    /// precedence level 1.
    #[test]
    fn a_plain_directory_is_named_by_basename() {
        let facts = WindowFacts {
            directory_basename: "Downloads".to_string(),
            is_home: false,
            repository: None,
            manual_name: None,
            label: None,
        };

        assert_eq!(window_name(&facts, &[]), "Downloads");
    }

    /// $HOME renders as "~".
    ///
    /// The existing shell suite asserts this, and the spec warns the
    /// assertion currently passes for the wrong reason: /Users/austin is
    /// itself a bare-repo worktree with no .git, so the old and new
    /// implementations skip the repository branch identically. This test
    /// pins the tilde on its own facts rather than on that coincidence.
    #[test]
    fn home_renders_as_a_tilde() {
        let facts = WindowFacts {
            directory_basename: "austin".to_string(),
            is_home: true,
            repository: None,
            manual_name: None,
            label: None,
        };

        assert_eq!(window_name(&facts, &[]), "~");
    }

    /// A bare-repo worktree at $HOME must still render as "~", even though
    /// it IS a repository.
    ///
    /// This is the case the spec says a converted implementation needs of
    /// its own: the shell version skips it only because it looks for `.git`
    /// and a bare-repo worktree has none, so a Rust version that inspects
    /// repositories more thoroughly would start naming $HOME "dotfiles/main"
    /// and break the assertion for a new reason.
    #[test]
    fn a_bare_repo_worktree_at_home_still_renders_as_a_tilde() {
        let facts = WindowFacts {
            directory_basename: "austin".to_string(),
            is_home: true,
            repository: Some(RepositoryFacts {
                main_repo_name: "dotfiles".to_string(),
                head: HeadState::Branch("main".to_string()),
            }),
            manual_name: None,
            label: None,
        };

        assert_eq!(
            window_name(&facts, &[]),
            "~",
            "is_home outranks the repository, or $HOME gets named after the dotfiles repo"
        );
    }

    /// A detached HEAD shows a short sha in parentheses.
    #[test]
    fn a_detached_head_shows_a_short_sha_in_parentheses() {
        let mut facts = a_repo_window();
        facts.repository = Some(RepositoryFacts {
            main_repo_name: "dotfiles".to_string(),
            head: HeadState::Detached { short_sha: "16c874a5".to_string() },
        });

        assert_eq!(window_name(&facts, &[]), "dotfiles/(16c874a5)");
    }

    /// A repository matching @wname_bare_repos is named by branch alone.
    #[test]
    fn a_bare_repo_pattern_drops_the_repo_prefix() {
        let patterns = vec!["dotfiles".to_string()];

        assert_eq!(window_name(&a_repo_window(), &patterns), "main");
    }

    /// The glob list is glob patterns, not literals.
    #[test]
    fn a_bare_repo_pattern_matches_as_a_glob() {
        let patterns = vec!["dot*".to_string()];

        // Positive control: a pattern that cannot match must leave the
        // prefix on, or this test would pass for a function that always
        // drops it.
        assert_eq!(window_name(&a_repo_window(), &["nomatch*".to_string()]), "dotfiles/main");
        assert_eq!(window_name(&a_repo_window(), &patterns), "main");
    }

    /// A label keeps a stable prefix in front of the automatic part, so
    /// `select-window -t <label>` keybindings keep working.
    #[test]
    fn a_label_prefixes_the_automatic_name() {
        let mut facts = a_repo_window();
        facts.label = Some("Reviews".to_string());

        assert_eq!(window_name(&facts, &[]), "Reviews - dotfiles/main");
    }
}
```

Verify each expected string against the real script before running: read how
it formats the detached case, the label separator, and the bare-repo case,
and correct any literal above that disagrees with the shell. **The shell is
the contract here**; if this plan's expected string differs from what the
script produces, the script wins and you must say so in your report.

- [ ] **Step 2: Run the tests to verify they fail**

```sh
cd ~/crates && cargo test --locked -p tmux-core naming
```

Expected: FAIL to build, because the module is not declared in `lib.rs`. Add
`mod naming;` first, then the failure becomes `cannot find function
'window_name' in this scope` plus unresolved types, which is the right red.
Record the actual message.

- [ ] **Step 3: Implement `naming.rs`**

Write the types from the Interfaces block and `window_name`. The precedence,
from the script's header, lowest to highest:

1. basename of the working directory, `~` for `$HOME`
2. `repo/branch` when the directory is a repository
3. a manual name set with `prefix ,`

with `@wname_label` prefixing the automatic part, and
`@wname_bare_repos` dropping the `repo/` prefix.

Implementation notes, all of them decisions the tests above pin:

- `is_home` outranks `repository`, per
  `a_bare_repo_worktree_at_home_still_renders_as_a_tilde`. Comment WHY: the
  home directory is itself a bare-repo worktree, so without this the
  dotfiles repo's name would replace the tilde.
- An empty `manual_name` is not a manual name. Treat `Some("")` as `None`.
- Glob matching is a small hand-rolled matcher over `*` and `?`, or a
  dependency-free character walk. **Do NOT add a glob crate**: the workspace
  takes no new third-party dependencies in this plan, and the patterns come
  from a tmux option the user writes. Write the matcher, unit-test it through
  the two glob tests above, and keep it private to this module.
- The pattern list arrives already split on `|` by the caller. Splitting is
  the binary's job because the raw option string is IO.

- [ ] **Step 4: Declare the module and extend BOTH purity lists**

In `lib.rs`, add `mod naming;` and the re-export:

```rust
pub use naming::{HeadState, RepositoryFacts, WindowFacts, window_name};
```

Then add `naming.rs` to **both** source lists in the purity module: the
scanned array in `no_module_names_an_io_capability`, and the non-empty list
in `the_purity_check_detects_a_forbidden_string`. A file in the first but not
the second is scanned but unproven, and the sabotage check in Step 6 only
exercises the first.

- [ ] **Step 5: Run the tests to verify they pass**

```sh
cd ~/crates && cargo test --locked -p tmux-core
cd ~/crates && cargo clippy --locked --all-targets -- -D warnings; echo "clippy=$?"
```

Expected: all naming tests PASS, both purity tests PASS, `clippy=0`.

- [ ] **Step 6: Verify the tests bind to the implementation**

Sabotage the precedence rather than deleting code, so the test proves it
catches a wrong ANSWER rather than a missing symbol.

```sh
cd ~/crates
cp tmux-core/src/naming.rs /tmp/naming.rs.good
python3 - <<'PY'
import pathlib
path = pathlib.Path("tmux-core/src/naming.rs")
text = path.read_text()
# Make is_home lose to the repository, which is the bug the spec warns about.
needle = "if facts.is_home"
assert text.count(needle) >= 1, "adjust this sabotage to the real control flow"
path.write_text(text.replace(needle, "if false && facts.is_home", 1))
PY
cargo test --locked -p tmux-core naming 2>&1 | grep -cE 'FAILED|panicked'
cp /tmp/naming.rs.good tmux-core/src/naming.rs && rm /tmp/naming.rs.good
cargo test --locked -p tmux-core naming 2>&1 | tail -1
```

Expected: the sabotaged run fails, naming
`a_bare_repo_worktree_at_home_still_renders_as_a_tilde`; the restored run
passes. If the sabotage does not compile because the control flow differs,
adapt it to the real code and say how in your report.

- [ ] **Step 7: Rebuild and commit**

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
cat > /tmp/msg.txt <<'MSG'
Compute window names in tmux-core, purely

The naming script is 241 of the 502 lines in this step's scope and is
already executed rather than sourced, so it has no sourcing contract to
preserve. That is why the spec puts it first.

Measured before the port, on 21 live windows: 442ms for the --all path,
with 17 spawn sites in the script. It also runs in precmd, so it fires on
every prompt draw rather than once at startup.

is_home outranks the repository, and that is a real case rather than a
tidiness choice. $HOME is itself a bare-repo worktree, so the shell version
renders it "~" only because it looks for .git and finds none. A Rust version
that inspects repositories properly would start naming $HOME after the
dotfiles repo and break the existing suite's tilde assertion for an entirely
new reason. The test pins the tilde on its own facts rather than on that
coincidence.

The glob matcher is hand-rolled rather than a new dependency: the patterns
come from a tmux option, the syntax needed is * and ?, and this plan adds no
third-party crates.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/tmux-core/
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 3: Repository inspection in `tmux-tools::repo`

The IO half of naming, and where the parent spec's `--all` fix lands: guard
on `.git` existing, then read `.git`, `commondir` and `HEAD` directly,
keeping `git rev-parse` as the fallback. The parent spec re-verified that
against 21 live window directories with zero fallbacks needed.

**Files:**
- Create: `crates/tmux-tools/src/repo.rs`
- Modify: `crates/tmux-tools/src/main.rs` (declare the module)

**Interfaces:**
- Consumes: `tmux_core::{RepositoryFacts, HeadState}` from Task 2.
- Produces:

```rust
pub fn inspect(directory: &Path) -> Option<RepositoryFacts>;
```

`None` when `directory` is not in a repository. Task 4 calls it per window.

- [ ] **Step 1: Write the failing test**

Integration-style, in `repo.rs`'s own test module, building real fixture
repositories with `tempfile`. `tempfile` is already a workspace dependency,
so add it to `tmux-tools` as `tempfile = { workspace = true }` under
`[dev-dependencies]`.

**Read `.agents/PAPERCUTS.md` before writing these tests.** Fixture tests
that shell out to `git` will operate on the real dotfiles repo if `GIT_DIR`
is set in the environment. Every `Command::new("git")` in these tests must
clear it:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Runs git against a fixture, with git's ambient environment cleared.
    ///
    /// Without the removals, an exported `GIT_DIR` from the calling shell
    /// makes every fixture command operate on the real dotfiles repository:
    /// observed one taking that repo's index lock and blocking every
    /// subsequent command. See `.agents/PAPERCUTS.md`.
    fn git_in(directory: &Path, arguments: &[&str]) {
        let status = std::process::Command::new("git")
            .args(arguments)
            .current_dir(directory)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_PREFIX")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .status()
            .expect("git runs");
        assert!(status.success(), "git {arguments:?} failed");
    }

    fn a_repo_with_one_commit() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("tempdir");
        git_in(directory.path(), &["init", "-q", "-b", "main"]);
        std::fs::write(directory.path().join("marker.txt"), "hello\n").expect("write");
        git_in(directory.path(), &["add", "marker.txt"]);
        git_in(
            directory.path(),
            &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "first"],
        );
        directory
    }

    /// A repository on a branch reports its name and its branch.
    #[test]
    fn a_repository_reports_its_name_and_branch() {
        let repo = a_repo_with_one_commit();

        let facts = inspect(repo.path()).expect("a git repository is inspectable");

        // Positive control: a non-repository must yield None, or the
        // assertions below would hold for a function that always answers.
        let plain = tempfile::tempdir().expect("tempdir");
        assert!(inspect(plain.path()).is_none(), "a plain directory is not a repository");

        assert_eq!(facts.head, HeadState::Branch("main".to_string()));
        assert_eq!(
            facts.main_repo_name,
            repo.path().file_name().expect("a name").to_string_lossy()
        );
    }

    /// A linked worktree reports the MAIN repository's name, not its own
    /// directory name. That is what `commondir` is read for.
    #[test]
    fn a_linked_worktree_reports_the_main_repository_name() {
        let main = a_repo_with_one_commit();
        let worktree_parent = tempfile::tempdir().expect("tempdir");
        let worktree = worktree_parent.path().join("feature-checkout");
        git_in(
            main.path(),
            &["worktree", "add", "-q", worktree.to_str().expect("utf-8 path"), "-b", "feature"],
        );

        let facts = inspect(&worktree).expect("a linked worktree is inspectable");

        assert_eq!(
            facts.main_repo_name,
            main.path().file_name().expect("a name").to_string_lossy(),
            "a worktree must report the main repo's name, not its own directory"
        );
        assert_eq!(facts.head, HeadState::Branch("feature".to_string()));
    }

    /// A detached HEAD reports a short sha rather than a branch.
    #[test]
    fn a_detached_head_reports_a_short_sha() {
        let repo = a_repo_with_one_commit();
        git_in(repo.path(), &["checkout", "-q", "--detach"]);

        let facts = inspect(repo.path()).expect("a detached repository is inspectable");

        match facts.head {
            HeadState::Detached { short_sha } => {
                assert!(!short_sha.is_empty(), "a detached head must carry a sha");
                assert!(
                    short_sha.len() >= 7 && short_sha.len() <= 12,
                    "a short sha, not a full one: got {short_sha:?}"
                );
            }
            other => panic!("expected a detached head, got {other:?}"),
        }
    }

    /// A bare-repo worktree, which is the shape $HOME has, reports facts
    /// rather than panicking or reporting None for the wrong reason.
    ///
    /// tmux-core's `is_home` check is what keeps $HOME rendering as "~"; this
    /// test only pins that inspection itself handles the shape.
    #[test]
    fn a_bare_repo_worktree_is_inspectable() {
        let home = tempfile::tempdir().expect("tempdir");
        let bare = home.path().join(".cfg");
        git_in(home.path(), &["init", "-q", "--bare", "-b", "main", ".cfg"]);
        std::fs::write(home.path().join("tracked.txt"), "hi\n").expect("write");
        let bare_path = bare.to_str().expect("utf-8 path");
        git_in(
            home.path(),
            &[
                "--git-dir", bare_path, "--work-tree", ".",
                "-c", "user.email=t@t", "-c", "user.name=t",
                "add", "tracked.txt",
            ],
        );

        // No .git in the work tree, so inspection must not claim a
        // repository from a directory that has none of the usual markers.
        assert!(
            inspect(home.path()).is_none(),
            "a bare-repo worktree has no .git, so inspection reports None and \
             tmux-core's is_home rule is what renders the tilde"
        );
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```sh
cd ~/crates && cargo test --locked -p tmux-tools repo
```

Expected: FAIL to build, `cannot find function 'inspect' in this scope`.
Record it.

- [ ] **Step 3: Implement `repo.rs`**

Direct file reads with a `git rev-parse` fallback, per the parent spec's 7.4:

1. If `directory/.git` does not exist, walk up to the filesystem root looking
   for one. If none exists, return `None`. That guard is what makes the
   common non-repository case cost one `stat` per ancestor rather than a
   process spawn.
2. `.git` is either a directory (ordinary clone) or a file containing
   `gitdir: <path>` (linked worktree). Handle both.
3. For a linked worktree, read `<gitdir>/commondir` to find the main
   repository's git directory, and take the main repo's name from that path's
   parent. This is what the worktree test pins.
4. Read `HEAD`. `ref: refs/heads/<branch>` is a branch; a bare 40-character
   hex string is a detached head, and the short sha is its first 8
   characters, matching what the shell produces.
5. If any read fails or yields an unexpected shape, fall back to
   `git rev-parse` rather than returning a wrong answer. Log nothing: this
   runs on every prompt draw and a stray line would corrupt the prompt.

Document WHY the direct reads exist, in one comment: they replace 17 spawn
sites, and the parent spec verified zero fallbacks were needed across 21 live
window directories.

- [ ] **Step 4: Run the tests to verify they pass**

```sh
cd ~/crates && cargo test --locked -p tmux-tools
cd ~/crates && cargo clippy --locked --all-targets -- -D warnings; echo "clippy=$?"
```

Expected: PASS, `clippy=0`.

- [ ] **Step 5: Verify the fixtures did not touch the real repository**

The papercut this guards against is real and was observed live.

```sh
cd ~ && config rev-parse HEAD > /tmp/head.before
cd ~/crates && env GIT_DIR="$HOME/.cfg" GIT_WORK_TREE="$HOME" cargo test --locked -p tmux-tools
cd ~ && ls ~/.cfg/index.lock 2>/dev/null && echo "LOCK LEAKED" || echo "no lock leaked"
config rev-parse HEAD > /tmp/head.after
diff /tmp/head.before /tmp/head.after && echo "HEAD unchanged"
test -z "$(config diff --cached --name-only)" && echo "index clean"
rm -f /tmp/head.before /tmp/head.after
```

Expected: tests pass, `no lock leaked`, `HEAD unchanged`, `index clean`.
**If `LOCK LEAKED` appears, the `env_remove` calls are missing from a git
invocation in the tests.** Find it and fix it before continuing; then clear
the lock with `rm -f ~/.cfg/index.lock` after confirming with
`ps aux | grep [g]it` that no real git process is running.

- [ ] **Step 6: Verify the tests bind**

```sh
cd ~/crates
cp tmux-tools/src/repo.rs /tmp/repo.rs.good
python3 - <<'PY'
import pathlib
path = pathlib.Path("tmux-tools/src/repo.rs")
text = path.read_text()
needle = "commondir"
assert needle in text, "adjust this sabotage to the real code"
path.write_text(text.replace(needle, "commondir-broken", 1))
PY
cargo test --locked -p tmux-tools repo 2>&1 | grep -cE 'FAILED|panicked'
cp /tmp/repo.rs.good tmux-tools/src/repo.rs && rm /tmp/repo.rs.good
cargo test --locked -p tmux-tools repo 2>&1 | tail -1
```

Expected: the sabotaged run fails
`a_linked_worktree_reports_the_main_repository_name`, proving that test is
about `commondir` rather than about compilation; the restored run passes.

- [ ] **Step 7: Rebuild and commit**

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
cat > /tmp/msg.txt <<'MSG'
Inspect repositories by reading .git directly

Replaces the naming script's git spawns with direct reads of .git,
commondir and HEAD, keeping git rev-parse as the fallback. The parent spec
re-verified this against 21 live window directories with zero fallbacks
needed, and the script has 17 spawn sites.

The non-repository case is a stat per ancestor rather than a spawn, which
matters because this runs on every prompt draw.

commondir is what makes a linked worktree report the MAIN repository's name
rather than its own directory name, and there is a test that fails when it
is misread rather than only when the code fails to compile.

The fixture helper clears GIT_DIR, GIT_WORK_TREE, GIT_INDEX_FILE and
GIT_PREFIX for every git call. Without that, an exported GIT_DIR from the
calling shell makes fixtures operate on the real dotfiles repo: observed one
taking its index lock and blocking every config command. See
.agents/PAPERCUTS.md.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/tmux-tools/
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 4: `name-windows`, the binary subcommand, and the shim

Wire Tasks 2 and 3 into a working subcommand, then replace the 241-line
script with a shim. This is the task that must not regress the per-prompt
path, so it measures before and after.

**Files:**
- Create: `crates/tmux-tools/src/tmux.rs`
- Modify: `crates/tmux-tools/src/main.rs` (dispatch `name-windows`)
- Modify: `.scripts/tmux-update-window-names.sh` (becomes a shim)

**Interfaces:**
- Consumes: `tmux_core::window_name` (Task 2), `repo::inspect` (Task 3).
- Produces: `tmux-tools name-windows [-a | -s <session>]`, matching the
  script's existing flags. Exit 0 when not inside tmux and no flag is given,
  which is the script's current early-exit behavior.

- [ ] **Step 1: Measure the current cost, and record it**

This is the baseline the port must beat. Run each three times and take the
best, since a loaded machine skews a single run.

```sh
cd ~
echo "--- outside tmux (early exit path)"
time (for i in 1 2 3 4 5; do .scripts/tmux-update-window-names.sh >/dev/null 2>&1; done)
echo "--- every window in every session (-a)"
tmux ls 2>/dev/null | wc -l
time (.scripts/tmux-update-window-names.sh -a >/dev/null 2>&1)
```

Record all three numbers and the session/window count. Measured while
planning, for comparison: ~18ms per call outside tmux, ~84ms on a
one-window session, 442ms for `-a` across 21 windows.

- [ ] **Step 2: Write the failing test**

The tmux calls are IO, so the test asserts on the binary's behavior against a
real throwaway tmux server rather than mocking. Create the test in
`crates/tmux-tools/tests/name_windows.rs` (an integration test, since it
drives the built binary):

```rust
//! Drives `tmux-tools name-windows` against a throwaway tmux server.
//!
//! A dedicated socket, not the developer's server: the suite already learned
//! that spawning sessions on the default server is what made the shell tmux
//! tests flaky on a developer machine.

use std::process::Command;

fn tmux(socket: &str, arguments: &[&str]) -> std::process::Output {
    Command::new("tmux")
        .args(["-L", socket])
        .args(arguments)
        .output()
        .expect("tmux runs")
}

/// A window in a repository directory gets named "repo/branch".
#[test]
fn it_names_a_repository_window() {
    let socket = format!("tmux-tools-test-{}", std::process::id());
    let repo = tempfile::tempdir().expect("tempdir");
    // Fixture git calls clear git's ambient environment; see
    // .agents/PAPERCUTS.md for what happens otherwise.
    for arguments in [
        vec!["init", "-q", "-b", "main"],
        vec!["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "first"],
    ] {
        let status = Command::new("git")
            .args(&arguments)
            .current_dir(repo.path())
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_PREFIX")
            .status()
            .expect("git runs");
        assert!(status.success());
    }

    tmux(&socket, &["new-session", "-d", "-s", "probe", "-c", repo.path().to_str().expect("utf-8")]);

    let binary = env!("CARGO_BIN_EXE_tmux-tools");
    let run = Command::new(binary)
        .args(["name-windows", "-s", "probe"])
        .env("TMUX_TOOLS_SOCKET", &socket)
        .output()
        .expect("the binary runs");
    assert!(run.status.success(), "stderr: {}", String::from_utf8_lossy(&run.stderr));

    let listed = tmux(&socket, &["list-windows", "-t", "probe", "-F", "#{window_name}"]);
    let names = String::from_utf8_lossy(&listed.stdout);

    // Positive control: the probe session must exist and report a window, or
    // the assertion below would hold for an empty listing.
    assert!(!names.trim().is_empty(), "the control must list a window");

    let expected = repo.path().file_name().expect("a name").to_string_lossy();
    assert!(
        names.contains(&format!("{expected}/main")),
        "expected a repo/branch name, got {names:?}"
    );

    tmux(&socket, &["kill-server"]);
}
```

`TMUX_TOOLS_SOCKET` is a test seam: the binary must pass `-L <socket>` to
tmux when that variable is set, and use the default server otherwise. Add
that to `tmux.rs` in Step 4 and document WHY (the test cannot use the
developer's server without the flake the shell suite already hit).

- [ ] **Step 3: Run the test to verify it fails**

```sh
cd ~/crates && cargo test --locked -p tmux-tools --test name_windows
```

Expected: FAIL, because `name-windows` is not a subcommand yet: the binary
exits 2 with `unknown subcommand name-windows`, so `run.status.success()` is
false and the assertion reports the stderr. Record it.

- [ ] **Step 4: Implement `tmux.rs` and the subcommand**

`tmux.rs` holds every `Command::new("tmux")` call:

- `list_windows(session: Option<&str>, all: bool) -> Vec<WindowTarget>` using
  `list-windows -F` with a format string carrying the window id, the active
  pane's path, and the automatic-rename state, in ONE call per session rather
  than one per window. That batching is where the 442ms goes.
- `window_option(target, name) -> Option<String>` for `@wname_label`.
- `server_option(name) -> Option<String>` for `@wname_bare_repos`.
- `rename(target, name)` using `rename-window`.
- Honour `TMUX_TOOLS_SOCKET` by prepending `-L <socket>` when set.

In `main.rs`, dispatch `name-windows`, parse `-a` and `-s <session>`, gather
facts per window, call `tmux_core::window_name`, and rename **only when the
computed name differs from the current one**. Renaming unconditionally fires
the `after-*` hooks the spec warns about and would make the tmux suites
flakier.

Split `@wname_bare_repos` on `|` here, in the binary, and pass the resulting
`Vec<String>` into the pure function, matching Task 2's interface note.

- [ ] **Step 5: Run the tests and clippy**

```sh
cd ~/crates && cargo test --locked -p tmux-tools
cd ~/crates && cargo clippy --locked --all-targets -- -D warnings; echo "clippy=$?"
```

Expected: PASS, `clippy=0`.

- [ ] **Step 6: Replace the script with a shim**

```sh
#!/bin/sh
#
# Name tmux windows after what they are actually pointed at.
#
# The computation moved to the tmux-tools binary: this file's 241 lines
# spawned 17 processes per invocation and ran on every prompt draw. The
# binary's naming precedence, label handling and bare-repo glob list are
# unchanged and documented in crates/tmux-core/src/naming.rs.
#
# Usage:
#   tmux-update-window-names.sh                 the current window, or
#                                               nothing when run outside tmux
#   tmux-update-window-names.sh -a              every window in every session
#   tmux-update-window-names.sh -s <session>    every window in one session

exec tmux-tools name-windows "$@"
```

Keep the usage text: `deps-docs.test.sh` resolves paths cited by the docs and
the README's own section may reference these flags. Verify with
`grep -rn "tmux-update-window-names" ~/README.md ~/tests/ | head` and update
any doc that describes the implementation rather than the interface.

- [ ] **Step 7: Measure the port and compare**

```sh
cd ~
echo "--- outside tmux (early exit path)"
time (for i in 1 2 3 4 5; do .scripts/tmux-update-window-names.sh >/dev/null 2>&1; done)
echo "--- every window in every session (-a)"
time (.scripts/tmux-update-window-names.sh -a >/dev/null 2>&1)
```

Expected: both faster than Step 1's numbers. **Report both sets side by
side.** If the `-a` path is slower, the per-window tmux calls were not
batched into one `list-windows`; fix that before committing, because this
path runs on every prompt draw.

- [ ] **Step 8: Run the existing shell suite for this script**

```sh
cd ~ && bash tests/tmux-update-window-names.test.sh 2>&1 | tail -3
```

Expected: PASS. This suite is the regression net for the port and it asserts
the `~` rendering the spec warns about. If it fails on the tilde, Task 2's
`is_home` precedence is not wired through the binary.

The spec notes these tmux suites are flaky on a developer machine because the
`after-*` hooks fire asynchronously. If you see a flake, re-run once and say
so; do not "fix" it by changing the suite.

- [ ] **Step 9: Rebuild, install, and commit**

The shim calls `tmux-tools` by name, so the binary must be on `PATH`.

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
command -v tmux-tools; echo "on_path=$?"
```

Expected: `doctor=0` and `tmux-tools` resolving under `~/.local/bin`. If it
does not resolve, `config build` does not yet install this binary: check
whether it installs every stamped member or an enumerated list, and extend
the list if so.

```sh
cd ~
cat > /tmp/msg.txt <<'MSG'
Port the window-naming script to tmux-tools

Replaces 241 lines of shell that spawned 17 processes per invocation and ran
on every prompt draw, not once at startup.

Measured before and after on the same 21-window session; both numbers are in
the task report. The win comes from one list-windows call per session
instead of per-window calls, plus direct .git reads instead of git spawns.

Renames only when the computed name differs from the current one. Renaming
unconditionally fires the six after-* tmux hooks, which is what makes these
suites flaky on a developer machine.

The shim keeps the flags and the usage text, because the interface is what
callers and docs depend on. The precedence rules now live in
crates/tmux-core/src/naming.rs, where they are unit-tested without a tmux
server.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/ .scripts/tmux-update-window-names.sh
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 5: `tmux-close.sh` and `tmux-worktree-config.sh`

The spec's order step 2: small, no blocker. 25 lines between them. Batched
into one task because they are the same shape of change and a reviewer can
judge them together.

**Files:**
- Modify: `crates/tmux-tools/src/main.rs` (two subcommands)
- Modify: `.scripts/tmux-close.sh`, `.scripts/tmux-worktree-config.sh`

**Interfaces:**
- Consumes: `tmux.rs` from Task 4.
- Produces: `tmux-tools close` and `tmux-tools worktree-config`.

- [ ] **Step 1: Read both scripts and record their contracts**

```sh
cd ~ && cat .scripts/tmux-close.sh .scripts/tmux-worktree-config.sh
```

`tmux-close.sh` is **sourced** (`alias c=`), so any `return` in it reaches
the interactive shell. Note whether it returns a value; if it does, the shim
must preserve that, the way Task 6 handles `tmux-split.sh`. Record what you
found before changing anything.

- [ ] **Step 2: Write the failing tests**

Add to `crates/tmux-tools/tests/name_windows.rs` or a sibling test file, one
test per subcommand, each asserting the observable effect against the
throwaway socket rather than the exit code alone. Use the same
`TMUX_TOOLS_SOCKET` seam and the same `env_remove` discipline for any git
calls.

For `close`: create a session with two windows, run the subcommand, assert
the window count dropped by one and the session still exists. Positive
control first: assert the session has two windows before the call.

For `worktree-config`: assert the effect the script actually has, which Step
1 told you. Do not invent an effect; if the script's only effect is setting a
tmux option, assert that option's value.

- [ ] **Step 3: Run the tests to verify they fail**

```sh
cd ~/crates && cargo test --locked -p tmux-tools
```

Expected: FAIL with `unknown subcommand`. Record it.

- [ ] **Step 4: Implement both subcommands and both shims**

Each shim keeps its invocation contract. `tmux-close.sh` stays sourceable:

```sh
#!/bin/sh
#
# Close the current tmux window or session.
#
# Still sourced (alias c=), so this file keeps its return contract: the
# binary's exit status becomes the interactive shell's $?.

tmux-tools close "$@"
return $?
```

If Step 1 found `tmux-close.sh` does NOT return a value, drop the `return`
and say so in your report. A bare `return` in a sourced script that never
returned before is a behavior change.

- [ ] **Step 5: Run the tests, clippy, and both shell suites**

```sh
cd ~/crates && cargo test --locked -p tmux-tools
cd ~/crates && cargo clippy --locked --all-targets -- -D warnings; echo "clippy=$?"
cd ~ && bash tests/run-all.sh 2>&1 | tail -3
```

Expected: PASS throughout.

- [ ] **Step 6: Verify the sourced contract by hand**

A sourced script's `$?` cannot be tested from a Rust integration test.

```sh
cd ~ && zsh -c 'source ~/.scripts/tmux-close.sh --help >/dev/null 2>&1; echo "sourced_status=$?"'
```

Expected: a status that matches what the binary returns for that input, and
critically, the shell must still be alive to print it. If sourcing kills the
shell, the shim is calling `exec` where it must not.

- [ ] **Step 7: Rebuild and commit**

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
cat > /tmp/msg.txt <<'MSG'
Port tmux-close and tmux-worktree-config

Two small scripts, 25 lines between them, batched because they are the same
shape of change.

tmux-close.sh stays sourced rather than becoming an exec shim: the alias
sources it, so exec would replace the interactive shell. Its return contract
is preserved deliberately, and there is a by-hand check that sourcing leaves
the shell alive, which no Rust integration test can cover.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/ .scripts/tmux-close.sh .scripts/tmux-worktree-config.sh
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 6: `zsh-git-widgets.sh`, per spec 3.2

The binary lists and formats the branches; the zsh widget keeps the
`LBUFFER=` assignment and the `fzf` picker. The spec's measurement: the
widget spawns six processes (`git`, `rg`, `sed`, `sed`, `fzf`, `cut`), and the
branch-listing half, excluding the interactive picker, takes 35ms.

**Files:**
- Create: `crates/tmux-core/src/branches.rs`
- Modify: `crates/tmux-core/src/lib.rs` (module, re-export, BOTH purity lists)
- Modify: `crates/tmux-tools/src/main.rs` (a `list-branches` subcommand)
- Modify: `.scripts/zsh-git-widgets.sh`

**Interfaces:**
- Consumes: `repo` inspection patterns from Task 3 (a new git call, not a
  reuse: this one lists refs).
- Produces: `tmux_core::format_branches(refs: &[BranchRef]) -> Vec<String>`
  and `tmux-tools list-branches`, which prints one formatted branch per line.

- [ ] **Step 1: Read the widget and record every transformation**

```sh
cd ~ && cat .scripts/zsh-git-widgets.sh
```

Write down, in your report, exactly what each of the six processes does: the
`git` `--format` string, both `sed` expressions, what `rg` filters, and what
`cut` takes. `format_branches` must reproduce that output byte for byte,
because the widget's `LBUFFER=` inserts it into a command line the user then
runs.

- [ ] **Step 2: Write the failing test**

In `branches.rs`, pure tests over injected ref data. Derive the expected
strings from Step 1's reading, not from this plan: the shell is the contract.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// The formatted output matches what the shell pipeline produced.
    ///
    /// Byte for byte, because the widget assigns the chosen line into
    /// LBUFFER and the user runs it: a stray space or a lost prefix becomes
    /// a broken command in the user's shell.
    #[test]
    fn it_formats_a_local_branch_the_way_the_pipeline_did() {
        let refs = vec![BranchRef {
            name: "feature/login".to_string(),
            is_remote: false,
        }];

        let formatted = format_branches(&refs);

        // Positive control: one ref in must produce one line out, or the
        // indexing below is reasoning about an empty vector.
        assert_eq!(formatted.len(), 1, "the control must format one branch");
        assert_eq!(formatted[0], "feature/login");
    }

    /// The filtering the `rg` stage performed is preserved.
    #[test]
    fn it_drops_the_refs_the_pipeline_dropped() {
        let refs = vec![
            BranchRef { name: "main".to_string(), is_remote: false },
            BranchRef { name: "origin/HEAD".to_string(), is_remote: true },
        ];

        let formatted = format_branches(&refs);

        assert!(
            !formatted.iter().any(|line| line.contains("HEAD")),
            "origin/HEAD was filtered by the pipeline and must stay filtered"
        );
        assert!(formatted.iter().any(|line| line == "main"), "main must survive");
    }
}
```

Correct both expected values against what Step 1 found. If the pipeline
keeps `origin/HEAD`, change the test to match the pipeline and say so.

- [ ] **Step 3: Run the test to verify it fails**

```sh
cd ~/crates && cargo test --locked -p tmux-core branches
```

Expected: FAIL to build on the missing module and function. Record it.

- [ ] **Step 4: Implement, and add the module to BOTH purity lists**

Implement `BranchRef` and `format_branches` in `branches.rs`, declare
`mod branches;` in `lib.rs`, re-export, and add `branches.rs` to **both**
purity source lists.

- [ ] **Step 5: Add the `list-branches` subcommand**

The subcommand performs one `git for-each-ref` call with an explicit
`--format`, parses it into `BranchRef` values, calls `format_branches`, and
prints one line each. One spawn, replacing five.

- [ ] **Step 6: Rewrite the widget, keeping what must stay shell**

```sh
# The LBUFFER assignment, the widget registration and the keybindings stay
# shell: assigning into the zsh line editor is impossible from another
# process. Only the branch listing moved, replacing five spawns (git, rg,
# sed, sed, cut) with one binary call.
#
# Not a keystroke path: this blocks on an interactive fzf picker, so a human
# is reading the screen while it runs. The spawn-cost argument that keeps
# .zshrc startup lean does not apply here.
```

Keep `zle -N` and the three `bindkey` calls exactly as they are. The widget
body becomes `tmux-tools list-branches | fzf ...` and then the existing
`LBUFFER=` assignment.

- [ ] **Step 7: Verify startup is unaffected and the widget still works**

```sh
cd ~ && bash tests/zshrc-startup-budget.test.sh 2>&1 | tail -2
cd ~ && bash tests/zsh-git-widgets.test.sh 2>&1 | tail -2
```

Expected: both PASS. The budget suite is the one that proves no spawn was
added to startup: the widget file is sourced at init, and only the widget's
body gained a spawn, which runs on Ctrl+G.

Then confirm the output is byte-identical to the old pipeline:

```sh
cd ~/crates && tmux-tools list-branches > /tmp/new-branches.txt
cd ~/crates && git branch -a --format='%(refname:short)' | head -20 > /tmp/raw-branches.txt
wc -l /tmp/new-branches.txt /tmp/raw-branches.txt
head -5 /tmp/new-branches.txt
```

Compare against what the pre-port pipeline produced. If you did not capture
that before editing the widget, recover it with
`config show HEAD~1:.scripts/zsh-git-widgets.sh` and run its pipeline by
hand. Report the comparison.

- [ ] **Step 8: Rebuild and commit**

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
cat > /tmp/msg.txt <<'MSG'
List branches in the binary, assign LBUFFER in the widget

The parent spec listed this file as permanently shell in two places, because
assigning LBUFFER is structurally impossible from another process. The
assignment is impossible. The computation is not, and the spec conflated
them.

Five of the widget's six spawns (git, rg, sed, sed, cut) become one binary
call. The measured branch-listing half was 35ms.

This is a latency improvement rather than a cost, and it is not a keystroke
path: the widget blocks on an interactive fzf picker, so a human is reading
the screen while it runs. The spawn-cost argument that keeps .zshrc startup
lean applies to startup and the prompt, not here.

The output is compared byte for byte against the old pipeline, because the
widget assigns the chosen line into LBUFFER and the user then runs it.

zle -N, the three bindkey calls and the LBUFFER assignment are unchanged.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/ .scripts/zsh-git-widgets.sh
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 7: `tmux-split.sh` per 3.1, then `tmux-start.sh` and `tmux-setup.sh`

Last, because 3.1's contract change is the one with behavioral risk.

**The defect being fixed.** `tmux-start.sh:34` sources `tmux-split.sh` with
**no arguments**, relying on positional-parameter inheritance so
`tmux-split.sh:78`'s `LAYOUT_TYPE=${1:-}` reads the *caller's* `$1`, which is
the session name. Verified in the spec: sourced, the inner script sees the
outer `$1`; exec'd, it sees empty.

That inheritance does two jobs at once: it passes an argument, and it uses
"unrecognized layout" as a silent no-op for the common case where the session
name is not a layout name.

**The decision.** `tmux-start.sh` calls `tmux-split "$1"` explicitly, and the
no-op becomes an explicit exit code the caller checks rather than a side
effect of printing usage.

**The test that matters, named by the spec:** a session name that is not a
layout must leave the session with **one** pane, and it must not print usage
to a user who did nothing wrong.

**Files:**
- Modify: `crates/tmux-core/src/layout.rs` (create), `lib.rs` (both purity
  lists), `crates/tmux-tools/src/main.rs`
- Modify: `.scripts/tmux-split.sh`, `.scripts/tmux-start.sh`,
  `.scripts/tmux-setup.sh`, and the `s`/`se` aliases in `.zshrc`

**Interfaces:**
- Consumes: `tmux.rs` from Task 4.
- Produces: `tmux_core::layout_for(name: &str) -> Option<Layout>` where
  `None` means "not a layout name", and `tmux-tools split <name>` exiting 0
  for a recognized layout, **3 for an unrecognized one** (distinct from 2,
  which is a usage error: an unrecognized layout is not the caller's
  mistake), and 2 for a genuine usage error such as no argument at all.

- [ ] **Step 1: Write the failing tests**

Pure tests for `layout_for` in `layout.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// A recognized layout name resolves to a layout.
    #[test]
    fn a_known_layout_name_resolves() {
        // Read .scripts/tmux-split.sh for the real layout names before
        // running this: they are the contract, not this plan's guess.
        let layout = layout_for("dev");

        assert!(layout.is_some(), "the control: a known name must resolve");
    }

    /// A session name that is not a layout resolves to None, which is the
    /// silent no-op the old inheritance produced by printing usage.
    #[test]
    fn a_session_name_that_is_not_a_layout_resolves_to_none() {
        assert!(
            layout_for("my-feature-branch").is_none(),
            "an unrecognized name is not a layout, and not an error either"
        );
    }
}
```

Plus an integration test for the exit-code contract, in
`crates/tmux-tools/tests/`:

```rust
/// An unrecognized layout exits 3 and prints no usage text.
///
/// The old shell version reached this case through positional-parameter
/// inheritance and signalled it by printing usage, so a user who typed a
/// perfectly good session name got told they had used the command wrong.
#[test]
fn an_unrecognized_layout_exits_three_and_stays_quiet() {
    let binary = env!("CARGO_BIN_EXE_tmux-tools");
    let run = std::process::Command::new(binary)
        .args(["split", "my-feature-branch"])
        .output()
        .expect("the binary runs");

    assert_eq!(run.status.code(), Some(3), "an unrecognized layout is exit 3");
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        !stderr.to_lowercase().contains("usage"),
        "a valid session name must not be told it is a usage error, got {stderr:?}"
    );
}

/// No argument at all IS a usage error, and exits 2.
#[test]
fn no_argument_is_a_usage_error() {
    let binary = env!("CARGO_BIN_EXE_tmux-tools");
    let run = std::process::Command::new(binary)
        .args(["split"])
        .output()
        .expect("the binary runs");

    assert_eq!(run.status.code(), Some(2), "a missing argument is exit 2");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```sh
cd ~/crates && cargo test --locked -p tmux-core layout
cd ~/crates && cargo test --locked -p tmux-tools split
```

Expected: both FAIL to build. Record the messages.

- [ ] **Step 3: Read the real layout names and implement**

```sh
cd ~ && cat .scripts/tmux-split.sh
```

Take the layout names and each one's pane arrangement from the script. Then
implement `Layout` and `layout_for` in `layout.rs`, declare the module, add
it to **both** purity lists, and add the `split` subcommand with the
three-way exit contract.

- [ ] **Step 4: Update `tmux-split.sh`, keeping it sourceable**

The alias sources it, and `tmux-split.sh:74`'s `return 1` currently reaches
the interactive shell. Preserve that:

```sh
#!/bin/sh
#
# Split the current tmux window into a named layout.
#
# Still sourced (alias sp=), so the binary's exit status becomes the
# interactive shell's $?, as this script's own `return 1` did before.
#
# Exit 3 means "that name is not a layout", which is not an error: the
# caller passed a session name that happens not to name a layout, and
# tmux-start.sh treats 3 as "plan no splits". Exit 2 is a real usage error.

tmux-tools split "$@"
return $?
```

- [ ] **Step 5: Update `tmux-start.sh` to pass the argument explicitly**

Replace the bare `source ~/.scripts/tmux-split.sh` with an explicit call, and
check the status rather than relying on usage output as control flow:

```sh
# Explicit argument, not positional-parameter inheritance. The old version
# sourced tmux-split.sh with no arguments so its ${1:-} read THIS script's
# $1, which is invisible at the call site and is why the parent spec called
# this conversion unsafe. Exit 3 means the session name is not a layout,
# which is the ordinary case and not a failure.
tmux-tools split "$1"
split_status=$?
if [ "$split_status" -ne 0 ] && [ "$split_status" -ne 3 ]; then
    printf 'tmux-start: splitting failed with status %s\n' "$split_status" >&2
    return "$split_status"
fi
```

- [ ] **Step 6: Move the attach calls into the aliases**

`tmux-setup.sh` has two `tmux attach` calls and `tmux-start.sh` has two
(`new-session -A` and `attach`). A subprocess that attaches attaches itself,
so these stay in the shell, and per the parent spec's remedy they move into
the alias so the binary computes and the alias attaches.

Update `.zshrc:60` and `.zshrc:67`. Read both scripts first to see which
attach runs under which condition; the alias must reproduce that condition,
not attach unconditionally.

- [ ] **Step 7: Verify the named test, by hand, on a throwaway socket**

This is the spec's own acceptance test and it cannot be automated: it needs
an interactive-shaped session.

```sh
cd ~
tmux -L split-probe new-session -d -s my-feature-branch
tmux -L split-probe list-panes -t my-feature-branch | wc -l
TMUX_TOOLS_SOCKET=split-probe tmux-tools split my-feature-branch
echo "unrecognized_status=$?"
tmux -L split-probe list-panes -t my-feature-branch | wc -l
tmux -L split-probe kill-server
```

Expected: one pane before, `unrecognized_status=3`, **one pane after**, and
no usage text printed. That is the spec's named test: a session name that is
not a layout leaves one pane and does not scold the user.

- [ ] **Step 8: Verify the sourced contract still holds for all three aliases**

```sh
cd ~ && zsh -ic 'alias s; alias se; alias sp' 2>/dev/null
cd ~ && zsh -c 'source ~/.scripts/tmux-split.sh not-a-layout; echo "sp_status=$?"'
```

Expected: the aliases print their definitions, and sourcing returns 3 with
the shell still alive. If sourcing kills the shell, a shim used `exec`.

- [ ] **Step 9: Run the full suite**

```sh
cd ~/crates && cargo test --locked
cd ~/crates && cargo clippy --locked --all-targets -- -D warnings; echo "clippy=$?"
cd ~ && bash tests/run-all.sh 2>&1 | tail -4
```

Expected: everything PASS, `clippy=0`. This is the last task, so the whole
workspace and the whole shell suite both matter.

- [ ] **Step 10: Rebuild and commit**

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
cat > /tmp/msg.txt <<'MSG'
Pass the layout explicitly, and move the attach into the aliases

tmux-start.sh sourced tmux-split.sh with no arguments, relying on
positional-parameter inheritance so the inner ${1:-} read the outer $1. That
inheritance did two jobs: it passed an argument, and it used "unrecognized
layout" as a silent no-op for the common case where the session name is not
a layout name. Both were invisible at the call site, which is why the parent
spec called this conversion unsafe.

The argument is now explicit and the no-op is an exit code the caller
checks. Exit 3 means "not a layout", distinct from 2, which is a real usage
error: a user who typed a good session name should not be told they used the
command wrong. The spec's named test is verified by hand, because it needs
an interactive-shaped session: a non-layout session name leaves one pane and
prints no usage.

The attach calls stay in shell and move into the aliases. A subprocess that
attaches attaches itself, so the binary computes and the alias attaches.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/ .scripts/tmux-split.sh .scripts/tmux-start.sh .scripts/tmux-setup.sh .zshrc
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Self-Review

**Spec coverage.** The spec's section 4 order maps to tasks: order step 1
(`tmux-update-window-names.sh` plus the `--all` fix) to Tasks 2, 3 and 4;
step 2 (`tmux-close.sh`, `tmux-worktree-config.sh`) to Task 5; step 3
(`zsh-git-widgets.sh` per 3.2) to Task 6; step 4 (`tmux-split.sh` per 3.1,
then `tmux-start.sh` and `tmux-setup.sh`) to Task 7. Task 1 is
infrastructure the spec implies but does not enumerate: two crates have to
exist before anything can move into them.

Section 5's three "must not break" items each have an explicit check: the
startup budget in Task 6 Step 7, the `~` assertion in Task 2 (its own test)
and Task 4 Step 8 (the existing suite), and the `after-*` hooks in Task 4
Step 4 (rename only on change) plus the Global Constraint forbidding new
hooks.

**Placeholder scan.** No TBD, no "similar to Task N". Three places
deliberately tell the implementer to read the shell for the exact values
rather than trusting this plan: Task 2 Step 1 (name formats), Task 6 Step 1
(the six-process pipeline), Task 7 Step 3 (layout names). That is correct
rather than a gap: the shell is the contract, this plan cannot see those
strings without reading 502 lines into it, and a wrong literal here would be
worse than an instruction to check.

**Type consistency.** `WindowFacts`/`RepositoryFacts`/`HeadState` are defined
in Task 2's Interfaces and used unchanged in Task 3's `inspect` return and
Task 4's gathering. `layout_for` returns `Option<Layout>` in Task 7's
Interfaces and its tests. `format_branches(&[BranchRef]) -> Vec<String>` is
consistent between Task 6's interface and its tests. `TMUX_TOOLS_SOCKET` is
introduced in Task 4 Step 2 and reused in Tasks 5 and 7.

**Two risks worth naming.**

1. **Task 4's measurement could come out worse.** The plan states the fix
   (batch into one `list-windows` per session) and forbids committing a
   regression on a path that runs every prompt draw.
2. **Task 7 changes an interactive contract**, and its acceptance test cannot
   be automated. Step 7 gives the exact by-hand procedure on a throwaway
   socket, and Step 8 checks all three sourced aliases still leave the shell
   alive.

**Ordering.** Task 1 is a hard prerequisite for everything. Tasks 2 and 3 are
independent of each other and both feed Task 4. Task 5 needs Task 4's
`tmux.rs`. Task 6 is independent of Tasks 4 and 5 and could run any time
after Task 1. Task 7 is last, as the spec requires.
