# The tmux and zsh scripts: what a process cannot do, decided

**Date:** 2026-09-07
**Status:** design, not yet planned
**Parent spec:** `docs/superpowers/specs/2026-09-06-pure-core-architecture.md`
(step 5 of section 7.4). This document corrects the parent's 3.7 and 7.2.
**Depends on:** nothing. Both blockers are resolved in section 3, so this
step can run at any point.
**Resolves:** the corrections raised in
`2026-09-07-config-cli-adapter-design.md` sections 7.1 and 7.2, which defer
`zsh-git-widgets.sh` and `tmux-split.sh` to this document.

## 1. Why this was blocked

Step 5 was the only remaining step that was *stuck* rather than merely
unstarted. Two sourcing dependencies had no decided contract, and the parent
spec contradicts itself about one of them.

Both are now decided (section 3), so this step is plannable.

## 2. What is actually here

Measured, because the parent spec's "four sourced tmux scripts" undercounts
the surface and misattributes the difficulty.

| Script | Lines | Invoked as | Converts? |
|---|---|---|---|
| `tmux-update-window-names.sh` | 241 | **executed** (`alias re=`) | Freely. No sourcing dependency at all. |
| `tmux-split.sh` | 89 | sourced (`alias sp=`) | Only with a decided argument contract; see 3.1. |
| `tmux-setup.sh` | 82 | sourced (`alias se=`) | All but its two `tmux attach` calls. |
| `tmux-start.sh` | 38 | sourced (`alias s=`) | All but its `tmux attach`; also calls `tmux-split.sh`. |
| `tmux-close.sh` | 17 | sourced (`alias c=`) | Freely. |
| `tmux-worktree-config.sh` | 8 | executed | Freely. |
| `zsh-git-widgets.sh` | 27 | sourced at shell init | Computation only; see 3.2. |

**`tmux-update-window-names.sh` is 241 of 502 lines across the seven files
above, and is already executed rather than sourced.** It is the largest and easiest piece,
and the parent spec buries it behind the two blocked scripts. It should go
first.

**Two scripts have `return`-with-value, not one.** The parent spec's 3.7 says
the sourced scripts' `return` statements "are early-exit guards passing no
value back". `tmux-split.sh:74` is `return 1` and
`tmux-update-window-names.sh` has one too. Section 7.4 contradicts 3.7 on
`tmux-split.sh` specifically; 3.7 is the wrong half.

**Why sourcing at all.** Not the `return` statements: `tmux attach` must run
in the caller's terminal to take over the TTY, and a subprocess that attaches
attaches itself. `tmux-setup.sh` has two such calls and `tmux-start.sh` has
two (`new-session -A` and `attach`). The parent spec's remedy stands: the
attach moves into the alias, so the binary computes and the alias attaches.

## 3. The two decisions

### 3.1 `tmux-split.sh`: pass the argument explicitly

`tmux-start.sh:34` sources `tmux-split.sh` with **no arguments**, relying on
positional-parameter inheritance so `tmux-split.sh:78`'s `LAYOUT_TYPE=${1:-}`
reads the *caller's* `$1`, which is the session name. Verified: sourced, the
inner script sees the outer `$1`; exec'd, it sees empty.

`tmux-start.sh:29-33` documents a consequence that the parent spec does not
mention, and it is load-bearing rather than incidental:

> "A session name that is not a layout name prints usage and skips the
> splits."

So the inheritance is doing two jobs: passing an argument, and using
"unrecognized layout" as a silent no-op for the common case where the session
name is not a layout.

**Decision: `tmux-start.sh` calls `tmux-split "$1"` explicitly, and the
no-op becomes an explicit exit code the caller checks rather than a side
effect of printing usage.**

Rationale: inheritance is invisible at the call site, which is why converting
it "silently changes behavior" per 7.4. An explicit argument plus an explicit
status says the same thing in a form a reader can see and a test can assert.
The usage text stops being a control-flow mechanism, which it should never
have been.

The test that matters: a session name that is not a layout must leave the
session with **one** pane, and it must not print usage to a user who did
nothing wrong.

### 3.2 `zsh-git-widgets.sh`: the binary computes, the widget assigns

The parent spec lists this file as permanently shell in **two** places, 3.7
and 7.2's table, both on the grounds that assigning `LBUFFER` is
"structurally impossible from another process".

**The assignment is impossible from another process. The computation is
not**, and the spec conflated them.

Measured: the widget spawns **six** processes (`git`, `rg`, `sed`, `sed`,
`fzf`, `cut`), and the branch-listing half of that pipeline, excluding the
interactive picker, takes **35 ms**.

**Decision: a binary lists and formats the branches. The zsh widget keeps the
`LBUFFER=` assignment and the `fzf` picker.**

Three reasons, in order:

1. It is a **latency improvement**, not a cost. Five spawned processes plus
   four pipe stages become one spawn, against the ~7 ms spawn floor the parent spec measures in its 7.2.
2. It is **not a keystroke path**. The widget blocks on an interactive `fzf`
   picker, so a human is reading the screen while it runs. The parent spec's
   spawn-cost objection applies to `.zshrc` startup and the prompt, not here.
3. The branch-listing logic is a `--format` string, two `sed` expressions and
   a `cut`, all of which are exactly the "grep and sed over structured text"
   shape that section 7.5 identifies as this repo's verified bug class.

The `LBUFFER=` line, `zle -N`, and the three `bindkey` calls stay shell.

**The correction to parent 7.2 is its second sentence, not its first.**
An earlier draft of this section asked for wording that parent
`2026-09-06-pure-core-architecture.md:1003` already contains verbatim
("Registers a ZLE widget and assigns `LBUFFER`."), which would have been a
no-op edit, or worse, would have read as already applied. The sentence that
needs to change is the one after it: "Structurally impossible from another
process." That is true of the assignment and false of the computation, and it
is the overreach this section corrects.

## 4. Order

1. `tmux-update-window-names.sh`. Largest, already executed, no blocker. Fold
   in the `--all` fix the parent spec's 7.4 describes (guard on `.git`
   existing, then read `.git`, `commondir` and `HEAD` directly, keeping
   `git rev-parse` as the fallback). Parent spec re-verified this against 21
   live window directories with zero fallbacks needed.
2. `tmux-close.sh` and `tmux-worktree-config.sh`. Small, no blocker.
3. `zsh-git-widgets.sh` per 3.2.
4. `tmux-split.sh` per 3.1, then `tmux-start.sh` and `tmux-setup.sh`, whose
   attach calls move into their aliases. Last, because 3.1's contract change
   is the one with behavioral risk.

## 5. What this step must not break

- `zshrc-startup-budget.test.sh` times interactive startup. None of these
  scripts may add a spawn to `.zshrc` startup: the widget file is *sourced*
  at init, and only the widget's *body* gains a spawn, which runs on Ctrl+G.
- `tmux-update-window-names.test.sh` asserts `$HOME` renders as `~`. The
  parent spec notes the naming script only reaches that branch for a
  directory that is not a repository, and that `/Users/austin` is itself a
  bare-repo worktree with no `.git`, so both implementations skip it
  identically. That is agreement, not evidence the file read handles
  bare-repo shapes: a converted implementation needs its own case for it.
- The six `after-*` tmux hooks in `.config/tmux/tmux.conf` fire the naming
  script asynchronously. The parent spec's test-suite workflow comment
  records that this is why the tmux suites are flaky on a developer machine
  and stable on a runner.
