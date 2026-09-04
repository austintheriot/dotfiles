# Shell-to-Rust prior art (2026-09-04)

Research pass done before designing a port of this repo's test and script
infrastructure to Rust. Question: has anyone done this, and what did they
learn? Three parallel research passes plus local measurement on this
machine (macOS arm64, Darwin 25.5.0).

Verdict up front: the evidence supports a narrow port, not a full one.
Two of the premises behind the original TODO turned out to be wrong, and
the highest-value findings are unrelated to Rust.

## What this repo actually looks like

| Measure | Value |
|---|---|
| Shell in `tests/` | 3,687 lines, 19 suites |
| Shell in `.scripts/` | 873 lines |
| Python in `.claude/scripts/` | 709 lines, 42 unit tests |
| Shebang census | 37 bash, 6 sh, 5 zsh |
| Suite runtime | ~72s serial |

## Premise 1: speed. Mostly wrong, and wrong in both directions

The TODO asked for Rust partly "for speed." Measured here:

| Operation | Per call |
|---|---|
| zsh builtin, no fork | 0.14 ms |
| **Rust binary** | **12.3 ms** |
| `bash -c true` | 15.8 ms |
| `/bin/sh` script | 22.3 ms |
| `tmux display-message` round trip | 22.4 ms |
| `git rev-parse` | 10-20 ms |

A Rust binary starts *faster than a shell script* on this machine. The
~13ms is generic macOS fork+exec cost, not a Rust tax: Apple's own signed
`/usr/bin/true` costs 10.7ms. Corroborated by
[bdrung/startup-time](https://github.com/bdrung/startup-time) (1000 runs
per language): Rust 0.51ms vs bash 0.71ms vs dash 0.33ms.

So the speed argument is wrong for the test suite and wrong *against*
Rust on the hook path:

- **Test suite: no win.** ~72s is spent waiting on external processes.
  One `zsh -i -c true` alone is 8.3s. Parallelism was already tried and
  was slower (26s vs 22s serial, recorded in `run-all.sh`) because every
  suite serializes on the single-threaded tmux server. Rust cannot make
  zsh or tmux or git faster.
- **Hook path: a real win, but not from the language.**
  `.scripts/tmux-update-window-names.sh` fires on 6 hooks including
  `after-select-pane`, costs 110-200ms, and spawns 13 subprocesses. The
  cost is the process count, not the interpreter.

### The batching finding

One call returns every window, pane, path and name at once, measured at
20ms for the whole server:

    tmux list-panes -a -F '#{window_id} #{pane_id} #{pane_current_path} #{window_name}'

The script currently loops over windows calling `display-message -p -t`
once each. Collapsing that loop is available *in shell* and captures most
of the win. Do not credit Rust for what batching delivers on its own.

The perceptibility budget matters here.
[zsh-bench](https://github.com/romkatv/zsh-bench) puts command lag at
**10ms**, input lag 20ms, first prompt 50ms. Its diagnosis of starship is
the transferable lesson: 354% of the command-lag threshold, because
"starship clones 158 times" per prompt. Starship is slow from
fan-out, not from being a binary. A self-contained binary that shells out
to nothing lands near the floor.

## Premise 2: the sourcing boundary. Wrong, and this is the useful one

I expected "a binary cannot change the caller's shell" to block the four
sourced scripts. Checked each:

| Script | Alias | Sourced because | Mutates caller state? |
|---|---|---|---|
| `tmux-start.sh` | `s` | early `return` | No (but attaches, see below) |
| `tmux-setup.sh` | `se` | early `return` | No |
| `tmux-split.sh` | `sp` | early `return 1` | No |
| `tmux-close.sh` | `c` | early `return` | No |
| `zsh-git-widgets.sh` | n/a | ZLE widget | **Yes: `LBUFFER`, `zle`, `bindkey`** |

None of the four sets a variable, calls `cd`, or changes a shell option.
Each is sourced only because it uses `return` instead of `exit`, which
its own header comment states. tmux state lives in the tmux server and is
reached over a socket, so a child process can change it freely.

**The irreducible boundary is `zsh-git-widgets.sh`: 27 lines, not 870.**
ZLE widgets run inside the line editor and cannot be a binary at any
price. Everything else is portable with `return` to `exit`.

This also means the `eval "$(tool init zsh)"` dance that zoxide, starship
and direnv need does not apply here. Worth knowing, because that pattern
has a documented cost:
[direnv #650](https://github.com/direnv/direnv/issues/650) is silent data
corruption where `$` and `#` in a value survived quoting;
[zoxide #953](https://github.com/ajeetdsouza/zoxide/issues/953) is a
syntax error at a line number in generated code that exists only in
memory. Note also that anything defined by `eval` is invisible to tmux's
`run-shell`, which always uses `/bin/sh` -- so a hook must call a binary
directly, never a shell function.

## What the ecosystem actually does

Checked manifests directly rather than trusting READMEs.

| Project | CLI test harness | Snapshots |
|---|---|---|
| jj | `assert_cmd` + custom `TestEnvironment` | `insta` |
| uv | dedicated `uv-test` crate | `insta` |
| ruff | `insta-cmd` + Markdown fixtures | `insta`, 3,723 snaps |
| cargo | `cargo-test-support` + snapbox | inline `str![]` |
| bat | `assert_cmd` + real PTY via `nix::pty` | `expect-test` |
| ripgrep, fd | hand-rolled | none |
| **starship, atuin** | **none at all** | none |

Two results stand out. **Starship does not test its shell integration**:
~400 lines of `starship.zsh`/`.bash`/`.fish` have only path-quoting unit
tests, and CI never installs zsh or fish. **Atuin's answer is shellcheck
in CI.** These are the two projects closest to this repo's problem, and
neither built a foreign-language harness for shell hooks.

The irony worth recording: **Astral, whose entire thesis is rewriting
Python tooling in Rust, keeps ~50 dev scripts in `scripts/` (16 shell
plus Python), runs shellcheck on them, and has no xtask.** Rust for the
product, shell for the glue.

`cargo-xtask` adoption is narrower than its reputation: rust-analyzer,
zellij, helix, Biome, cargo. Not uv, ruff, ripgrep, fd, bat, starship,
atuin, nushell, rustup, or Deno. matklad's own README says it "is not an
officially recommended workflow" and "simple bash scripts remain viable
for Unix-only projects." rust-analyzer's xtask manifest carries
"# Avoid adding more dependencies to this crate" -- compile cost is
actively policed.

## The flake: fixable this week, without Rust

The recorded 25% flake in `tmux-update-window-names.test.sh` has a
documented cure, and it comes from tmux's own test suite.

**tmux tests itself with ~200 POSIX `sh` scripts** in
[`regress/`](https://github.com/tmux/tmux/tree/master/regress). No
expect, no PTY library. Every test does:

```sh
TMP=$(mktemp -d); TMUX_TMPDIR="$TMP"; export TMUX_TMPDIR
TMUX="$TEST_TMUX -LtestA$$ -f/dev/null"   # PID-unique socket = private server
trap cleanup EXIT                          # kill-server + rm -rf
env -i LC_CTYPE=C.UTF-8
```

Three things to steal:

1. **Server per test**, not a shared one. `-L` plus a private
   `TMUX_TMPDIR` gives a genuinely independent server.
2. **Bounded polling on a queryable predicate**, never sleeps.
   `while [ $i -lt 50 ]` against `#{pane_in_mode}`, 200ms apart.
3. **Correlation markers.** Each call passes a distinct marker so a
   capture provably belongs to the call under test. This kills the
   "captured the previous screen" race class.

`regress/Makefile` uses `.NOTPARALLEL`. tmux's own maintainers chose
serial execution over debugging parallel flake.

This matches the deferred TODO's own proposed fix. The lesson from the
research is the ordering: **if the isolation bug is not fixed first, a
Rust rewrite produces flaky Rust tests.** Zellij, which has the most
mature Rust PTY suite, isolates with a Docker container and its test file
header says "These tests are very heavy... Avoid adding new ones."

## Testing interactive zsh: do not use a Rust expect library

This was the weakest part of the original plan.

- **`rexpect`** is maintained but **Unix-only, CI tests Linux only, macOS
  never tested**. It does no terminal emulation.
  [#143](https://github.com/rust-cli/rexpect/issues/143) is an open bug
  where it fails its *own* test on macOS because bracketed paste leaks
  `\u{1b}[?2004l` into matches.
  [#104](https://github.com/rust-cli/rexpect/issues/104) is a ~1-in-270
  flake from doubled echo.
- **`expectrl`**'s Windows support is unverified; the maintainer said "on
  CI it fails everything 😞 I don't know why."
- **`vt100` upstream is stale** and atuin forked it (`atuin-vt100` 0.19.0
  vs upstream 0.16.2) for exactly the fidelity issues that matter.
- **`trycmd`'s own docs** route interactive testing away: "For
  interactive programs, use rexpect."

**zsh's own suite is the design lesson.** It uses `zsh/zpty` and does not
screen-scrape. It binds instrumented ZLE widgets that print delimited
markers (`<LBUFFER>`, `CURSOR:`) and asserts on parsed sentinels. Make
the shell emit structured assertions rather than parsing its rendering.
`completest-pty` (epage) is the other exemplar: `ptyprocess` + `vt100`,
waiting on quiescence rather than sleeps, with `--noglobalrcs`, a temp
`ZDOTDIR`, `PS1='%% '`, and `set_echo(false)`.

## Build lifecycle: the disqualifying measurement

Measured on this machine, release profile, nothing to rebuild:

| Invocation | Wall clock |
|---|---|
| `cargo run --release -q` (no-op) | **240-340 ms** |
| `cargo build --release -q` via rustup shim | 60-80 ms |
| `cargo build --release -q`, shim bypassed | 30 ms |
| **Direct binary exec** | **0-10 ms** |

**Never put `cargo run` in a shell rc or a tmux hook.** It exceeds the
10ms command-lag budget by ~25x. Hooks must exec a built binary by
absolute path.

`cargo install --path` is safer than feared: per the Cargo Book it
"will always build and install," so it cannot silently run stale code.
Pay its ~30ms once from a deliberate `rebuild` step, not per invocation.

**If a staleness shim is built anyway, hash the source; never use mtime.**
Git checkout sets mtime to checkout time, so across per-machine branches
mtime lies in both directions. Cargo's own fingerprint docs concede
mtime's edge cases, and the
[self-compiling Rust](https://neosmart.net/blog/self-compiling-rust-code/)
prior art chose content hashing for this reason.

### macOS Gatekeeper: the one real latency trap

`syspolicyd` phones Apple before the **first execution of any new
executable**, including locally compiled ones. Measured externally at
**314ms first run, 3ms after**; with the Developer Tools exemption, 1ms.
This machine currently has `assessments enabled` and "Developer mode is
currently disabled," and I reproduced a ~180ms first-run cost per fresh
binary. Ad-hoc `codesign` does not avoid it; `spctl --global-disable`
does not either. Only the Developer Tools exemption does.

Sources genuinely disagree here: HackTricks says Gatekeeper fires only on
quarantined items, which locally-built binaries never get. The measured
first-run penalty contradicts that. Empirical reports win, but this
deserves re-measuring rather than trusting either claim.

### Distribution

**Every Rust tool in shell startup -- starship, zoxide, atuin, sheldon,
eza, direnv -- ships prebuilt per-platform binaries and recommends
download-not-compile.** `cargo install` is the fallback, never the front
door; `cargo-binstall` exists to route around it. Documented failures
cluster on the compile path (atuin
[#2645](https://github.com/atuinsh/atuin/issues/2645),
[#2011](https://github.com/atuinsh/atuin/issues/2011)).

**No precedent exists for a compile step in the critical path of getting
a shell.** The argument is structural, not latency: a fresh machine
either hangs for minutes or fails outright, leaving you needing a working
shell to repair your shell.

Committed binaries are especially bad for this repo: per-machine branches
(`mac`, `linux`, `work`, `home`) mean per-platform artifacts, git stores
every version in full and never GCs, and removal requires force-pushing
all four branches.

### CI and container

Premise correction: **Rust ships preinstalled on GitHub runners** --
ubuntu-24.04 has 1.98.0, macos-15-arm64 has 1.97.1. `dtolnay/rust-toolchain`
is unnecessary unless pinning. Expect little from `Swatinem/rust-cache`
on a small crate; it caches dependencies and there would be few.

The Docker gate is the real cost. The test image is **259MB** on a 97MB
base; a rustup toolchain is ~1.5GB locally. And `debian:bookworm-slim`'s
apt only offers **rustc 1.63** (2022) against 1.94 local -- a 31-version
gap, so the image needs rustup, not apt. Every source agrees slow hooks
get bypassed, and a bypassed hook is "worse than useless" (Thoughtworks).
If it must compile in the container, mount the cargo registry and
`target/`.

## Decision frameworks

Google's shell style guide, both halves:

> If you are writing a script that is more than 100 lines long, or that
> uses non-straightforward control flow logic, you should rewrite it in a
> more structured language *now*.

> If you're mostly calling other utilities and are doing relatively
> little data manipulation, shell is an acceptable choice for the task.

This repo blows the line count and lands squarely in the acceptable case.
Raymond (TAOUP) gives the best published *stay* condition: "it's all
simple command dispatching, with no internal data structures or complex
logic, so shell is good enough."

Nick Gerace,
[Do Not Rewrite Your Bash Script In Another Programming Language](https://nickgerace.dev/posts/do-not-rewrite-your-bash-script/):
"do not write software that could be written in Bash unless you need
multi-platform support, or you are not executing any external commands."
Neither exception applies.

The genuine pro-port mechanism, from
[Daniel Orner](https://stackoverflow.blog/2022/03/09/rewriting-bash-scripts-in-go-using-black-box-testing/),
is the one that maps onto the interface-boundaries priority in
`CLAUDE.md`: "no way of telling whether a particular environment
variable is an input to your script... or if it's a local variable."
Sourced bash has no export surface; every function is public.

## Costs, with citations

- **[Cargo #15691](https://github.com/rust-lang/cargo/issues/15691)** is
  the best citation available because it documents behavior change: "These
  tests have such a high overhead, that **I feel bad for adding them**."
  Acknowledged by epage, unresolved. Expensive harnesses mean fewer tests.
- **The linking tax is measured.** Cargo's test consolidation went
  125.64s to 73.40s compile, 5.8GB to 1.1GB, post-edit recompile 56.33s
  to 11.62s ([PR #5022](https://github.com/rust-lang/cargo/pull/5022)).
- **Snapshot rubber-stamping is the under-discussed hazard.**
  `cargo insta review` offers accept-all, and uv's documented default is
  `cargo insta test --accept`. uv also needs `apply-ci-snapshots.sh`
  because "different platforms may produce different snapshots." With
  3,687 lines of behavior, a rubber-stamped snapshot suite is worse than
  fewer explicit assertions.
- **Local loop cost, measured:** `assert_cmd` + `insta` + `tempfile` is
  35 packages, ~6.2s cold build, but **~0.29s incremental** after editing
  a test file. The edit-test loop is better than feared; cold and CI
  builds are where the cost lands.

## Deliberate shell-keepers

**runc** states the doctrine: "Integration tests do **not** replace unit
tests... written in bash using the bats framework." **Buck2** (Meta,
Rust) chose cram for CLI tests and Rust for units. **Mercurial** gives
the hard number, ~100x faster translated, **but gates his own proposal**:
migrate only where "user CLI experience or full end-to-end is not the
point of the test."

`bats-core` is healthy (v1.14.0, July 2026) with real isolation:
`BATS_TEST_TMPDIR` per test, three-tier temp dirs, `--jobs`, and
`--no-parallelize-within-files` (the direct analogue of nextest test
groups). Gotcha: FD 3 blocks bats when a test spawns a background process
like a tmux server; close it with `cmd 3>&-`. Scale is not the objection
-- **git maintains 305,866 lines of shell tests across 1,058 files** on a
~4,100-line harness. This repo's 3,687 lines is ~1.2% of that.

The one genuine harness reversal found runs **opposite** to the
hypothesis ([HN](https://news.ycombinator.com/item?id=19222578)): "my
experience doing integration testing with BATS was complete misery...
rewriting our BATS tests in Go was a huge step forward."

**No reversion post-mortem exists** for shell-to-Rust in either
direction. Likely publication bias; it cannot be claimed as support.

## This repo already built the alternatives

`tests/lib.sh` already implements git's `test-lib.sh` pattern: 19 suites
sourcing one harness, `assert_equals`/`assert_contains`/`assert_succeeds`,
`isolate_hooks`, `make_repo`, `make_worktree`, `mktemp -d` fixtures, plus
`run-all.sh` and `run-in-docker.sh`.

And the harness already contains the rebuttal to the dependency-injection
goal, in its own comment:

> the scripts under test are almost entirely orchestration of tmux and
> git, so stubbing those out would only test the stubs.

That is correct. DI here buys tests of the mocks. The Flipp team hit
exactly this: after replacing external commands with native Go, they lost
"our armor-plated certainty that nothing had changed."

## Recommendation

Ranked by value per unit of work. The top two are unrelated to Rust and
should happen regardless.

1. **Write tests for `tests/leak-check.sh`.** It gates every commit
   against leaking secrets to a public repo, is 128 lines of stacked
   `grep -inE` regexes, and has **no dedicated test file**. A false
   negative leaks a secret. This is the largest real risk in the repo and
   it has nothing to do with the port question.
2. **Install shellcheck, add it to `deps.conf`, run it in `run-all.sh`.**
   The code already carries `# shellcheck disable=SC2086` directives
   while shellcheck is **not installed and not a tracked dependency**, so
   those directives are decorative. 43 of 51 files are bash or sh and
   fully checkable; only 5 are zsh. This is atuin's entire answer to the
   same problem. Hours, not weeks.
3. **Fix the tmux flake in shell**, using tmux's own recipe:
   `-Ldotfiles-test-$$`, private `TMUX_TMPDIR`, `trap cleanup EXIT`,
   bounded polling with correlation markers. Cheapest test of whether the
   flake is a language problem. It is not.
4. **Batch the tmux queries in `tmux-update-window-names.sh`.** One
   `list-panes -a -F` call replaces the per-window loop and captures most
   of the 110-200ms, in shell. Establishes the baseline any port must beat.
5. **Drop the unnecessary `source`.** Four scripts are sourced for
   `return` semantics they do not need. `return` to `exit` makes them
   plain executables, shrinks the eval boundary to 27 lines, and improves
   the export surface -- the one goal a rewrite genuinely serves.
6. **If porting anything, port `tests/check-branch-drift.sh` first.** 184
   lines, **24 `IFS` references** -- the signature of missing data
   structures. Pure computation, no shell boundary, no tmux, no
   interactive shell. `.sync-manifest` parsing (rules, `!` excludes, `~`
   per-branch, trailing-slash globs, precedence) is a genuine ADT with
   exhaustive matching, and it is security-adjacent via the drift gate.
   Build an equivalence harness against the current bash *first*; that is
   the one move every successful report shares.
7. **Do not port** the tmux orchestration helpers or the interactive-zsh
   tests. Orchestration with no data structures is exactly where Google
   and Raymond both say stay, and testing zsh from Rust means fighting
   the open PTY bugs above on the platform rexpect's CI never covers.

If a Rust harness is still wanted after 1-5, copy jj's `TestEnvironment`:
`env_clear()`, tempdir `HOME`, `GIT_CONFIG_SYSTEM=/dev/null`, injected
timestamps, a seeded randomness counter, and path normalization for
stable snapshots, plus nextest test groups keyed on
`NEXTEST_TEST_GROUP_SLOT` for per-test socket names. Never build during
shell startup; enable the macOS Developer Tools exemption first.

## Revisit if

- The flake survives socket-per-test isolation. That would mean the
  problem is not contention and the analysis here is wrong.
- `.sync-manifest` grows more rule kinds. Precedence logic in shell gets
  bad fast, and item 6 becomes clearly worth doing.
- A hook lands on the interactive path that genuinely needs to fan out to
  many processes. That is the one shape where a single binary wins big.
