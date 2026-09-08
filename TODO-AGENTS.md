Take the first item from this list. Mark it as claimed in one commit, do the work, then remove it when done in another commit. This prevents any agent race conditions. If TODOS is empty, leave the heading in place l move onto QUESTIONS. For any of these, if the fix is clear/mechanistic, perform it autonomously as a single commit using /test-driven-development. If not, move onto the next clear item, and surface the remaining items for discussion via the /brainstorming skill at the end.

# TODOS:

Items marked BLOCKER came from an /expert-review survey pass on 2026-09-05.
Each was reproduced by running the code, not by reading it; the reproduction
is recorded with the item so it can be turned into a regression test first.

The two PROMPT LATENCY items came from a research pass on 2026-09-06
(docs/research/rust-external-tool-boundaries.md). Both are shell fixes, and
between them they are worth more than the Rust migration they were found
while investigating. Every number below was measured twice, once by the
research agent and once independently.

- PROMPT LATENCY. `parse_git_dirty` runs a full `git status` on every prompt
  render, and costs 299ms in a large worktree.
  `.zshrc:208-213` defines it and `.zshrc:224` calls it from inside `PS1`, so
  it runs every time a prompt is drawn, in every pane.
  Measured in a 24,453-file worktree:
    git status (what it does now)            299.2 ms
    git -c core.untrackedCache=true status   133.4 ms
    git status --porcelain -uno --no-renames  44.6 ms
  In `$HOME` it is only 12ms, because `.cfg/config` sets
  `status.showUntrackedFiles=no`. The cost is paid in the real project
  worktrees, which is where most prompts are drawn.
  Two options, and they compose:
    - Set `core.untrackedCache=true` (and consider `core.fsmonitor`). Both
      are currently unset, verified. No code change, no behavior change.
    - Switch to `--porcelain -uno --no-renames` and match on the porcelain
      codes rather than three `[[ =~ ]]` tests against human-readable
      English. That is also a correctness improvement: the current form
      breaks if git ever rewords its output, and it depends on the user's
      locale. It drops untracked-file colouring, so that part is a
      deliberate behavior decision rather than a free win.
  For scale: the tmux naming script this repo has spent much more effort on
  costs 17.8ms on the same path.

- PROMPT LATENCY. `.scripts/tmux-update-window-names.sh --all` costs 1473ms,
  and roughly a third of that is spawns for directories that are not
  repositories.
  Measured: `--all` at 1473ms against 17.8ms for the default single-window
  path, so the `-a` path is about 80x the per-prompt cost. It is reached from
  `re` (the alias at .zshrc:85) and by anything that renames every window.
  Two causes, both fixable in shell:
    - The `--path-format` fallback at lines 105-113 fires whenever `$info` is
      empty, which is every non-repo directory, not only git older than 2.31.
      Git here is 2.50.0, so the fallback can never be needed for its stated
      reason. Measured 24.27ms of wasted spawns per non-repo directory,
      against 0.0033ms for a `test -e "$dir/.git"` guard.
    - The per-window `git rev-parse` can be a direct read of `.git`,
      `<gitdir>/commondir` and `<gitdir>/HEAD`. I verified agreement with
      `git branch --show-current` across all 21 live window directories:
      21 of 21 agree, including the linked worktrees whose `.git` is a file
      pointing into a `worktrees/` directory.
  Keep a `git rev-parse` fallback for the shapes a file read does not cover
  (`gitdir:` chains, `core.worktree`, unusual ref backends), which is the
  library-first-with-escape-hatch design starship uses. Build the equivalence
  harness first: 21 live directories is evidence, not proof.

- BLOCKER. `tests/leak-check.sh` reports a clean scan when the scan failed.
  `added_lines` detects a `git log` failure and calls `exit 2` (line 115),
  but it is only ever invoked inside a command substitution
  (`staged=$(echo "$scan_paths" | added_lines)`, line 132). The exit kills
  the subshell, the parent captures an empty string, and the next line
  (`[ -z "$staged" ] && exit 0`) reports success. `changed_paths` has the
  same shape at line 96.
  Reproduced with a standalone script: parent survives, capture empty,
  final exit 0.
  Consequence: `tests/pre-push:96` has a branch for exit status 2 ("could
  not scan, push blocked") that this path can never reach. The script
  prints the correct diagnostic and then does the opposite.
  Fix direction: capture the status explicitly
  (`scan_paths=$(changed_paths) || exit 2`) or write a sentinel file the
  parent checks. A subshell cannot propagate an exit to its parent.

- BLOCKER. `tests/leak-check.sh` layer 2 fails open when the pattern file
  is absent. When `~/.claude/local/leak-patterns.conf` is unreadable
  (line 167), the guard prints "term rules INACTIVE" to stderr and
  continues. The generic credential rules still run, so a credential-shaped
  string is still caught. What deactivates is the employer and project term
  layer, which is the layer that exists because this repo is public.
  Reproduced by differential test on identical content: pattern file
  present exits 1 (blocked), pattern file absent exits 0 (allowed).
  The file is untracked on purpose, so it is absent by default on every
  fresh machine. That is the same moment a new machine is committing its
  setup work.
  Fix direction: exit non-zero when the file is missing, with an explicit
  opt-out variable for a machine that genuinely has no terms to defend.
  Compare `tests/pre-push:86`, which hard-fails when leak-check.sh itself
  is missing.

- BLOCKER. `tests/leak-check.sh` can be blinded by one `.gitattributes`
  line. A path marked `-diff` produces "Binary files ... differ" with no
  `+` lines, so the content rules see nothing.
  Reproduced: a file holding a credential-shaped string is blocked
  normally (exit 1) and passes (exit 0) once `cred.txt -diff` is
  committed. This is distinct from the known binary-file gap below,
  because it lets an ordinary text file be marked unscannable and the
  marking is an innocuous-looking one-line commit.
  Fix direction: after computing `scan_paths`, assert every path produced
  at least one hunk, and block on any path that produced none. That one
  mechanism also closes the binary-file and newline-path gaps recorded
  below.

- BLOCKER. A test suite that runs zero assertions reports PASS.
  `tests/lib.sh:264` ends `finish` with `[ "$failed" -eq 0 ]`, which is
  true when nothing ran. Under `run-all.sh -q`, which is what the
  pre-push hook shows, that is byte-identical to a real pass.
  Reproduced: a suite whose body is only `finish` prints
  "0 passed, 0 failed" and exits 0.
  The sharpest live instance is `tests/githooks-installed.test.sh:26-30`,
  which uses a bare `printf` rather than `skip`, then `finish; exit 0`,
  when there is no `.cfg` directory. Reproduced under an isolated
  DOTFILES_ROOT: "0 passed, 0 failed", exit 0. `.github/workflows/test-suite.yml:139`
  sets DOTFILES_ROOT to the checkout workspace, which has `.git` and not
  `.cfg`, and the container has no `.cfg` either. So all 7 assertions in
  the suite that exists to catch "the hooks are not installed" run on one
  machine only: this one.
  Fix direction: make `finish` fail, or report a distinct verdict, when
  passed + failed + skipped is 0. `run-all.sh:117` already parses the
  summary line and can carry the marker up to the verdict.

- Six suites skip with a bare `printf` instead of `skip`, so the skipped
  assertions never reach the summary line or the runner's verdict.
  Confirmed by grep: `githooks-installed`, `alacritty-platform-split`,
  `workflow-labels`, `zshrc-node-startup`, `zshrc-python-startup`,
  `zshrc-platform-split`. (`skip-reporting.test.sh` also matches, but its
  hits are its own fixtures.)
  The measured cost, per an /expert-review agent that ran the suite on the
  host and in the container and diffed the counts:
  `config-manifest-lifecycle` drops 7 of 16 assertions in the container
  with no skip recorded, including the assertion that a binary built
  without the stamp variable reports `unstamped`. That assertion is the
  guard against the pre-push stamp gate comparing an empty string against
  a real tree id.
  `.claude/rules/dotfiles-tests.md` already states the rule this breaks:
  do not printf the skip yourself and do not silently return.
  `tests/setup.test.sh:422` is the model to copy.

- `tests/pre-push` TRIGGER_PATHS omits paths that suites read, so editing
  them pushes without running the suite that tests them. This is the same
  class as the `.config/nvim` gap already fixed once; the class was never
  swept.
  Verified against the regex at line 38:
    - every file in `.scripts/config/` except `usage.sh`. The alternative
      `^\.scripts/.*\.sh$` requires a `.sh` suffix, and the dispatcher and
      all 11 `config-*` subcommands are extensionless. Eight suites read
      that tree.
    - `.zshrc`, `.zshrc-mac`, `.zshrc-linux` (six zshrc-*.test.sh suites)
    - `setup.sh` (setup.test.sh, bootstrap-harness.test.sh)
    - `.profile` (profile-path.test.sh)
    - `.config/alacritty/` (alacritty-platform-split, platform)
    - `.scripts/deps/*.conf` and `.scripts/deps/docker/Dockerfile.*`
  Every one of these is already COPYed into the test image because the
  suites need it, so the Dockerfile and the trigger regex disagree about
  what counts as test input.
  Worth fixing structurally rather than by adding alternatives:
  `tests/container.test.sh:227-263` already derives referenced root files
  mechanically by grepping the suites. Extending that derivation to all
  referenced paths, then asserting each matches TRIGGER_PATHS, would have
  caught every entry above and would catch the next one.

- `config help` cannot describe a compiled subcommand, so the first
  `config-*` script ported to Rust will silently list as "(undocumented)".
  `.scripts/config/config-help:38` reads the description with
  `sed -n 's/^# help: //p'` over the file's source text. Verified against
  the installed `config-manifest` binary: the sed yields nothing.
  The dispatcher's execution contract (`.scripts/config/config:27-32`) is
  already binary-compatible. It is only the introspection contract that is
  source-text-only, so the two contracts share one name and the port
  breaks the second one.
  Fix direction: add a `--describe` execution contract with the existing
  `sed` read as the fallback, so shell subcommands need no change and the
  migration stays incremental. Do this BEFORE porting any subcommand.

- `tests/lib.sh:229` reports the wrong exit code on every `assert_succeeds`
  failure. `failed=$((failed + 1))` runs before the `printf` reads `$?`,
  so the arithmetic's status is what gets printed.
  Reproduced: a command exiting 42 reports "(exited 0)".
  `assert_succeeds` is used 170+ times, and the failures that matter most
  are the ones from a container run that cannot be reproduced
  interactively.
  Fix: capture the status into a local before the branch.

- `tests/zshrc-startup-budget.test.sh` has no floor asserting it measured
  anything, so the repo's only performance gate can pass while measuring
  nothing. The file's own comment at lines 76-79 documents the failure
  mode: without the `zmodload`, `$EPOCHREALTIME` is empty, every
  difference computes as zero, and the budget assertion passes. Line 118
  clamps negatives to zero, which hides it further.
  Fix: assert the harness measured a non-zero elapsed time before
  asserting the budget. One line.

- `.scripts/deps/test-bootstrap.sh` runs a guaranteed-failing `docker
  build` on every invocation. `$workdir/empty-context` is created only at
  line 121, inside the failure branch of the build at line 119, and the
  first attempt's stderr is discarded by `2>/dev/null`. Confirmed that
  `docker build` errors on a missing context path ("unable to prepare
  context: path not found"). Line 153 (the bare build) has no retry and
  works only because line 121 already ran, which is the tell that the
  try/retry shape was never intentional.
  Fix: `mkdir -p` beside the other mkdirs near line 55, then one
  unconditional build. About 15 lines become 6.

- `.scripts/alacritty-platform.sh:49` writes the pointer file
  non-atomically (`printf '%s' "$new" > "$pointer"`), and `.zshrc:178`
  backgrounds the script in every shell. The content-equality guard at
  line 44 suppresses the steady state, but on the first startup after a
  variant edit every pane races to truncate the same file, and Alacritty
  watches it. The script's own comment at line 42 names the 107-pane
  scenario.
  Fix: write to a temp path and `mv -f`, which is atomic within a
  filesystem.

- `.scripts/config/config-install-hooks:34` uses `find "$dir" -maxdepth 0`
  without `-L`, so it stats the symlink rather than its target. A
  world-writable directory reached through a symlink passes the
  trust-boundary check the file's header (lines 13-18) says these four
  directories are. `$HOME/.local/bin` and `$HOME/tests` are both plausible
  symlinks on a synced home.
  Fix: `find -L`, or resolve with `readlink -f` first (already done at
  line 21 for `$0`).

- `SKIP_LEAK_CHECK` skips on any non-empty value, so `SKIP_LEAK_CHECK=0`
  and `SKIP_LEAK_CHECK=false` both disable the guard.
  `tests/leak-check.sh:60` tests `[ -n "$SKIP_LEAK_CHECK" ]`. Reproduced
  for `1`, `0` and `false`. This is the sanctioned bypass of the repo's
  primary control, so its semantics should not surprise.
  Fix: match `1`, `true`, `yes` explicitly.

- `.scripts/tmux-start.sh:26` decides a session exists with
  `[ "$(tmux ls | rg $SESSION_NAME)" = "" ]`, which substring-matches and
  treats the name as a regex. `s dev` finds an existing `dev-tool`
  session, skips creation, then `tmux attach -t $SESSION_NAME` at line 38
  fails because `dev` does not exist. `$SESSION_NAME` is also unquoted, so
  a name starting with `-` makes ripgrep error.
  Fix: `tmux has-session -t`, which `.scripts/tmux-setup.sh:20` already
  uses correctly.

- `.zshrc:198` sets `plugin=(git)`, which does nothing. oh-my-zsh is never
  sourced anywhere in `.zshrc`, `.zshrc-mac`, `.zshrc-linux` or `.profile`
  (verified by grep; `.zshrc-linux:7` reaches into its custom plugins
  directory by path precisely because the framework is not loaded), and
  the variable oh-my-zsh reads is `plugins`, plural. Delete the assignment
  and its two comment lines.

- Latent, no live trigger today: several path-handling gaps share one
  cause, that git quotes unusual paths and the quoted form matches no
  pathspec when fed back. No tracked path currently contains a space or a
  non-ASCII byte (verified), so none of these fire now. They matter as
  evasion surface on a public-repo gate.
    - `tests/leak-check.sh:111` misses EVERY non-ASCII path, not only the
      newline case already recorded below. Reproduced: git emits
      `"caf\303\251/note.md"`, feeding it back matches nothing, and a
      credential there scans zero lines and exits 0. Fix with `-z` and
      NUL-delimited reads, or `-c core.quotePath=false` plus
      `--literal-pathspecs`.
    - `setup.sh:410` iterates `for path in $(cfg ls-tree -r --name-only
      "$branch")`, which word-splits on whitespace. A tracked path with a
      space is never moved aside, and `cfg checkout` at line 425 then
      fails under `set -e`, aborting the bootstrap. That is the exact
      failure the comment at lines 396-402 says the loop prevents.

- `crates/config-manifest`: an orphan `!` rule silently disables drift
  checking for the paths it names. `manifest.rs:186-204` returns
  `Classification::Excluded` for any path matching an `Excluded` pattern
  whether or not an enclosing `Shared` rule exists, and `check.rs:47`
  drops excluded paths from comparison. A typo (`!doc/private.md` against
  a `docs/` rule), or deleting a shared rule and leaving its exclusions,
  turns the guard off with no signal.
  Two fixes were proposed. The structural one makes exclusions children of
  the rule they modify (`SharedRule { pattern, exceptions }`), so an
  orphan is unrepresentable and precedence stops being ordering logic;
  cost is reassembling the flat file into a tree at parse time and
  re-flattening it on print. The cheap one keeps the flat `Vec<Rule>` and
  rejects, at parse time, any `Excluded` pattern not nested under some
  `Shared` directory pattern.
  Decide which before porting more scripts, because the next crate will
  copy this one's shape.

- `crates/config-manifest`: exit codes are bare `u8` literals returned
  from nine sites, and status 1 currently means drift, unmatched paths,
  malformed manifest, unreadable manifest, git subprocess failure,
  non-UTF-8 tree entry, refused sync, and post-sync coverage gap. The
  `Err` arm at `main.rs:86` flattens every structured error the crate
  built into the same status. The 2026-09-06 collapse deleted `check.rs`
  and branch-drift.yml, so the specific message-text grepping this entry
  cited is gone; re-read the surviving callers before acting on it.
  Fix direction: one `Outcome` sum with a single exhaustive
  `exit_code()` match. The numbers stay as they are today, so nothing
  downstream breaks, but they become derived from a named meaning in one
  place. Worth doing before the shape is copied.

- Fossil branches are reachable from the bootstrap path. `home`,
  `home-mac` and `work` last moved 2 to 3 years ago and each differs from
  `mac` in about 360 of 354 tracked files, while `mac` and `linux` differ
  in 8 (6 of which are unsynced planning docs). `setup.sh --branch work`
  will check out a 2023 tree onto a fresh machine, where `config init`
  then runs against a `.scripts/` layout predating every current
  convention. `setup.sh:317` asserts these are "real branches" and
  `setup.sh:26` advertises them.
  Either archive them under a name that reads as archived and correct
  those two comments, or delete them.

- `.scripts:q/` is a stray untracked directory from a mistyped vim `:q`.
  Three of its files are byte-identical to their `.scripts/` counterparts.
  Delete it.

- Migrate the rest of the `config ...` scripts to Rust
- `tests/leak-check.sh` does not scan paths containing a newline or binary
  files, in either staged or range mode. Git quotes a newline path, so
  `xargs -0` cannot address it; a binary diff has no `+` lines for the
  content rules to see. Confirmed identical at commit 76608b6, so this
  predates the range-mode work. The newline case is a deliberate-evasion
  shape worth closing on a public-repo gate; record only, no fix yet.
- `tests/tmux-update-window-names.test.sh` fails intermittently (1 of 33
  assertions) on a live tmux server and passes on rerun; find the timing
  dependency and make the assertion deterministic.
- Dropped after measurement, recorded so it is not retried: `compinit -C`.
  An isolated `zsh -f` test showed compinit at 1.17s, but that was an
  fpath artefact. In the real startup trace compinit is ~60ms and did not
  clear a 40ms bar. `-C` would save nothing meaningful and removes the
  compaudit security check.
- Our testing & repo infrastructure has grown quite complex. Let's consider porting some of these to Rust scripts -- both for ease of reading/writing/updating/managing/testing, but also for speed. Brainstorm options here
- `se` is slow, but shell startup is no longer the cause. Re-measured
  2026-09-05, after the lazy nvm and pyenv work landed.
  Interactive zsh startup is 170-176ms against a ~7ms bare shell, and
  tests/zshrc-startup-budget.test.sh now holds it under 400ms.
  The full layout is 21 windows x 5 panes = 105 panes. Measured three
  times on an isolated tmux server: build 6.1-7.8s, then 0.6-1.9s until
  every pane reaches its prompt, 6.6-9.7s total. Not minutes.
  The cost is tmux creating panes, NOT shell startup multiplied by 105:
  split-window returns once the pane exists, so the 105 shells start in
  parallel and cost 0.6s together rather than 105 x 176ms serially.
  So shaving shell startup further buys almost nothing here. What is
  left to consider, and why this is not closed:
    - whether 105 panes is the right layout at all, given 15 worktrees
    - whether windows can be created lazily, on first selection
    - whether the earlier "minutes" was a cold cache, a since-fixed
      eager init, or contention from something else on the machine.
      No measurement from that period survives, so this is unresolved.
- Renaming tmux windows lags on a git branch change. Investigated
  2026-09-05; cause found, not yet fixed.
  Nothing in tmux watches git. The tmux hooks in .config/tmux/tmux.conf
  (after-new-window, after-split-window, after-select-window,
  after-select-pane, after-kill-pane, client-session-changed) all fire
  on tmux events only. The git-aware trigger is precmd in .zshrc, which
  runs the script before drawing each prompt.
  So the name is not late, it is waiting for a prompt: `git checkout` in
  pane A does not rename until that pane draws its next prompt, and a
  checkout made anywhere else (another pane, an editor, a script) never
  triggers a rename in the window showing it.
  Also worth weighing before choosing a fix: the script costs 57ms
  (best of 5, measured) and precmd runs it on EVERY prompt, in every
  pane, whether or not the branch changed. With 105 panes that is a lot
  of repeated work to keep a name that usually did not change.
  Options to weigh, none implemented:
    - a git post-checkout / post-merge hook, which fires on the actual
      event rather than on the next prompt. Needs a per-repo hook or
      core.hooksPath, and does not cover a detached-HEAD move.
    - cache the last-seen branch per pane and skip the tmux calls when
      it has not changed, which cuts the 57ms on the common path
      without changing when the update happens.
    - keep precmd but make the no-change path cheap, which is the same
      idea one level down.

- Create an expert agent for agent sandboxing, local inference, GPU
  allocation, and orchestration. Nothing in `~/.claude/agents/` covers any of
  it: grep for gpu, sandbox, infer, orchestr or local returns no agent file.
  The lens is the machine an agent runs on rather than the code it writes,
  which is why none of the existing specialists fit. Roughly, what it should
  hold:
    - sandboxing and isolation. What a subagent can reach, filesystem and
      network scope, containers versus VMs versus per-process confinement,
      seccomp and namespaces on Linux, the sandbox-exec and TCC model on
      macOS, and the failure mode where a sandbox is asserted rather than
      enforced.
    - local inference. Serving runtimes (llama.cpp, vLLM, Ollama, MLX on
      Apple silicon), quantisation tradeoffs, context-window versus VRAM
      arithmetic, batching, and when local is genuinely cheaper than an API
      rather than assumed to be.
    - GPU allocation. Which process gets which device, VRAM budgeting across
      concurrent agents, unified memory on Apple silicon versus discrete
      VRAM, MIG and time-slicing, and detecting contention rather than
      discovering it as a slowdown.
    - orchestration. Concurrency limits, queueing and backpressure, what a
      stalled agent looks like from outside, resource-aware scheduling, and
      the cost model that decides how many agents are worth running at once.
  Use `/create-expert-agent`, which already knows how to research a domain
  and wire a new lens into `/expert-review`, `/expert-plan` and `/consult`.
  Worth splitting if the research shows it is really two agents: the
  sandboxing and orchestration half is a systems-and-safety lens, while local
  inference and GPU allocation is a hardware-and-serving lens, and that
  split is the sort of thing the skill is meant to decide on evidence.
  Evidence this is worth having: a session on 2026-09-07 ran up to four
  subagents concurrently and hit real resource questions with no expert to
  ask. It picked a concurrency cap of three to four by feel, discovered by
  accident that `cargo --locked` fails transiently while a concurrent agent
  rewrites `Cargo.lock`, and had one agent stall in a wait loop for over an
  hour before anyone noticed. Every one of those is in this agent's lens.

- Automate the sandboxing, inference and GPU configuration in this repo,
  once the agent above exists to say what the configuration should be.
  Deliberately a second item: the first decides what is correct, this one
  makes a fresh machine arrive at it without a human remembering the steps.
  The shape this repo already uses:
    - `deps.conf` and its platform variants declare what must be installed,
      and `config deps install` now installs them. Anything the setup needs
      (a serving runtime, a GPU toolchain, a container runtime) belongs
      there rather than in a README instruction.
    - `config-init` runs before a toolchain exists and is where bootstrap
      ordering lives.
    - `.claude/settings.json` and `~/.claude/local/` hold agent
      configuration, the latter untracked, which is where anything
      machine-specific or private goes.
  What that likely means concretely, to be confirmed by the agent's
  recommendations rather than assumed here: manifest entries for whatever
  runtime is chosen, a checked model cache location, per-machine GPU and
  concurrency limits expressed as configuration rather than as habits, and a
  `config` subcommand or a test that verifies the machine actually matches
  the declared setup. The last one matters most: this repo's dominant bug
  class is an environment quietly compensating for a gap in the engine, so
  a setup that is documented but unverified will drift the same way.

# QUESTIONS (leave until queried)

- Should the mac/linux two-branch model collapse to one branch?
  Raised by an /expert-review agent on 2026-09-05 and REJECTED then.
  RESOLVED 2026-09-06: collapsed, at the owner's direction, onto `main`.
  Recorded in full because the 2026-09-05 rejection was not wrong, and the
  reason it stopped applying is specific.
  What the rejection got right, and what still holds: `linux` was and is a
  LIVE branch, not a fossil. It received a real commit on 2026-09-06. So
  collapsing did delete the only automated consistency gate over two active
  public branches, exactly as argued.
  What changed:
    1. The rejection's decisive argument was "given four confirmed blockers
       whose common shape is a gate that passes silently, removing a gate
       that works is the wrong direction." All four of those blockers are
       now fixed and verified. The argument was about SEQUENCING, and the
       sequence completed.
    2. The cost is now measured rather than estimated. Of 163 linux-only
       commits, 35 are titled "Sync shared paths from mac" -- pure overhead
       the gate imposes to keep two branches byte-identical.
    3. Every platform-specific file already shipped on both branches
       (.zshrc-mac, .zshrc-linux, tmux-mac.conf, tmux-linux.conf,
       deps-mac.conf, deps-linux.conf), so runtime variant selection had
       already replaced per-branch content. Verified.
    4. `mac` was a strict superset: exactly ONE file existed only on
       `linux`, and it was a proptest seed deliberately untracked earlier
       the same day.
  What this cost, stated plainly rather than argued away:
    - The every-tracked-file-matches-a-rule invariant in `.sync-manifest` is
      gone, along with the file itself. Nothing now catches a file added
      without thought.
    - `origin/mac` and `origin/linux` are deliberately left in place, so
      this is reversible while they exist. Deleting them makes it not.

- Should GitHub secret scanning push protection be enabled?
  Free for public repos. It would be a second, higher-quality net for
  layer 1 of the leak guard (the credential-shape rules), covering every
  prefix hardcoded at `tests/leak-check.sh:152` plus many more, with fewer
  false positives. Verified that no secret scanner (gitleaks, trufflehog)
  is installed, configured, or listed in deps.conf, so there is no
  half-installed path to lean on.
  It does NOT replace layer 2: the project term rules read patterns from
  outside the repo precisely so the terms are not published, and no hosted
  scanner can do that. It also fires at the remote, not at pre-commit, so
  it does not serve the "a leak should never even land in a local commit"
  goal stated at `tests/pre-commit:11-12`.
  Check current state with:
    gh api repos/austintheriot/dotfiles --jq '.security_and_analysis'

- Are our git hooks currently configured to run the leak check on commit and then the test suite on push? If not, they should.
  ANSWERED 2026-09-05, read from tests/pre-commit and tests/pre-push. Yes,
  both, and both are installed: ~/.cfg/hooks/pre-commit and
  ~/.cfg/hooks/pre-push are symlinks to the tracked scripts in ~/tests/.
  pre-commit runs tests/leak-check.sh on the staged content and blocks the
  commit on a finding.
  pre-push does more than the question assumed, in this order:
    1. leak-check --range on every pushed range, blocking on a finding and
       also on exit 2, which is "could not scan" rather than "clean".
    2. on any branch push: the config-manifest binary's stamp must match
       the pushed crate. This used to fire only for mac or linux, so the
       2026-09-06 collapse to main disabled it silently until it was
       widened to every ref. The mac-vs-linux drift check it used to run
       after the stamp gate is deleted.
    3. the full suite, in Docker, but ONLY when a pushed path matches
       TRIGGER_PATHS (.scripts/, .claude/scripts/*.py, .claude/hooks/*.sh,
       tests/, .github/workflows/, crates/, .config/nvim/, .config/tmux/,
       setup.sh, .zshrc*). A push touching nothing else skips the suite and
       says so.
  Worth knowing about that last one: a change to a file outside those
  paths does not run the suite locally. .config/nvim/ is outside them, so
  the nvim work in this session pushed without the suite until a tests/
  file rode along with it.

- Are we using the Docker container for the pre-push test suite? Should we be?
  ANSWERED 2026-09-05. Yes. pre-push calls tests/run-in-docker.sh, with
  DOTFILES_TEST_REF pinned to the ref being pushed, so the container tests
  the committed content rather than the working tree.
  It should stay that way, and the DEFERRED entry on tmux isolation is the
  evidence: tmux-update-window-names.test.sh fails about 25% of the time on
  the host live tmux server and 0/12 in the container, because the container
  is a pristine server with no client, no windows, and no hooks.
  One thing to keep in mind rather than change: the container image COPYs
  per-path, so a suite that reads a path the image does not carry passes on
  the host and fails only at pre-push. That happened this session with
  .config/nvim/ and is why it now has its own COPY line.

# DEFERRED TODOS

- Bootstrapping from a prebuilt binary instead of a local compile.
  `config-build` stays shell (see
  `docs/superpowers/specs/2026-09-07-config-subcommand-ports-design.md`
  section 3.2) because it would otherwise be a subcommand of the binary it
  builds, so a machine with no binary could not build one. The more
  interesting answer is that a fresh machine should get its first binary from
  a release artifact, which is also what
  `2026-09-07-config-cli-adapter-design.md` section 8.2 does for the
  bootstrap Docker images: it injects a prebuilt binary through a seam at
  `bootstrap-curl-entrypoint.sh:45` rather than adding a Rust toolchain to
  an image whose bareness is the property under test. If that approach is
  adopted, `config-build` stops being load-bearing for a fresh machine and
  the stays-shell decision is worth revisiting. Also relevant: `rustup` is
  itself a manifest entry (`deps.conf:35`), so anything that installs a
  toolchain in order to bootstrap pre-satisfies a dependency the run exists
  to exercise.

- CI warning "aws/tap is not trusted" on macos-latest: investigated,
  no repo-side fix wanted. Recorded so it is not re-researched.
  Cosmetic, and not caused by anything in this repo. The runner image's
  own `images/macos/scripts/build/install-aws-tools.sh` runs
  `brew tap aws/tap`. Homebrew 6 warns about every untrusted tap on the
  machine during any `brew install`, so our own step in
  `.github/workflows/test-suite.yml` ("Install the suite's dependencies
  (brew)") trips it while installing tmux, fzf, ripgrep and
  zsh-autosuggestions.
  Nothing is skipped: verified in run 33826524451 that all four formulae
  install normally from homebrew/core. The untrusted tap is never read.
  Upstream already fixed it. runner-images #14271 adds `brew trust
aws/tap` and merged 2026-08-11, but the change has not reached
  `macos-latest` yet, so the warning still appeared 2026-09-04.
  Re-check after the next macOS image rollout, and expect the warning to
  disappear on its own. The two repo-side workarounds were both rejected
  as worse than the warning: `brew untap aws/tap` in our workflow papers
  over an upstream bug and becomes dead weight once the image ships the
  fix, and `HOMEBREW_NO_REQUIRE_TAP_TRUST=1` disables the trust check for
  every tap, which Homebrew's own message says is not recommended and
  will be removed in a later release.
- macOS Docker images (e.g. https://github.com/dockur/macos): investigated,
  not viable. Recorded so it is not re-researched.
  `dockur/macos` is not a macOS container; it is QEMU/KVM booting a macOS VM
  inside a container (base image `qemux/qemu`, OpenCore + OVMF). Three
  independent blockers, each disqualifying on its own:
  (1) It cannot run on a Mac. Its README: "Docker Desktop on Linux, macOS,
  and Windows 10 does not currently provide KVM access to containers and is
  therefore not supported." It needs `/dev/kvm` on a Linux or Windows 11
  host, so it is useless for the local pre-push gate.
  (2) Setup is a manual twelve-step click-through of Disk Utility and Setup
  Assistant in a browser VNC viewer. Nothing scriptable for a hook or CI.
  (3) The guest is x86_64 only (AVX2 requirement, amd64-pinned, no ARM64
  option), so it would test Intel macOS. `.claude/hooks/notify.sh` hardcodes
  `/opt/homebrew/bin/aerospace` and `.zshrc-mac` uses `$(brew --prefix)`,
  both Apple Silicon paths -- it would exercise the wrong architecture.
  Plus a licensing bind: Apple's EULA restricts macOS virtualization to
  Apple hardware, while the technical requirements exclude macOS hosts. The
  only compliant setup is Linux on Apple-branded hardware. `sickcodes/Docker-OSX`
  is more scriptable but carries the same host and EULA constraints.
  Already solved a better way: `.github/workflows/test-suite.yml` runs the
  full suite on `macos-latest`, a real macOS VM on Apple hardware, licensed,
  with Homebrew preinstalled. All 16 suites pass there, and no tmux suite
  flakes, because a fresh runner never loads `.config/tmux/tmux-common.conf`.
- Isolate the test suite onto its own tmux server. `tests/lib.sh` calls bare
  `tmux`, so every tmux suite runs on the live server -- currently 23 windows,
  an attached client, and 6 global `after-*` hooks that fire
  `.scripts/tmux-update-window-names.sh` via `run-shell -b` (asynchronous).
  Those background invocations race the tests' own synchronous runs against
  the tests' own windows. `tmux-update-window-names.test.sh` fails about 25%
  of the time on the host (measured 5/20 runs) and 0/12 in the container,
  which is a pristine server with no client, no windows, and no hooks. Three
  different assertions rotate through the failure, which is why it reads as
  random: `switching active pane updates the name`, `empty name restores the
labelled name`, `owned window follows branch changes`.
  Proposed fix: point `TMUX_TMPDIR` at the per-run fixture directory and use a
  dedicated socket (`tmux -L dotfiles-test-$$`) in `lib.sh`, giving the host
  the isolation the container already has. Deferred because it touches
  `lib.sh`, which every tmux suite depends on, so it wants a deliberate pass
  rather than a drive-by. Not urgent: the pre-push gate runs in the container,
  so this flake cannot block a push.
  Two failed fix attempts, recorded so they are not repeated:
  (1) retrying the state read (2 attempts, then 5 with a 20ms backoff) did not
  help -- 4/20 then 6/20;
  (2) an earlier "global hooks are not the cause" measurement was invalid,
  because it cleared the hooks but left the attached client and the live
  windows in place.

- Fix the silent stale name when `tmux display-message` returns empty.
  Separate latent bug in `.scripts/tmux-update-window-names.sh`, found
  while investigating the flake above and NOT its cause. `tmux display-message
-p -F '#{pane_current_path}'` intermittently prints nothing while exiting 0
  with an empty stderr, measured at 1 to 6 calls in 3000 depending on server
  load. `#{window_name}` alone measured 0/3000, so it is specific to reading
  the pane path. The script treats that empty read as "this window has no
  directory" and returns without renaming, so the window silently keeps a
  stale name until the next hook fires. Needs a test that stubs tmux rather
  than racing the real server, and the stub must never target a real window id
  (an earlier attempt passed `-w '@1'`, which is a live window on this
  machine).

- Add a nerd font as a tracked dependency.
  `.config/alacritty/alacritty.toml` names `Hack Nerd Font Mono` in all four
  font slots (normal, bold, italic, bold_italic), and
  `.config/nvim/lua/settings.lua:3` sets `vim.g.have_nerd_font = true`
  unconditionally, which turns on `nvim-web-devicons` for telescope, trouble
  and the mini statusline. No conf file in `.scripts/deps/` mentions a font at
  all, so a fresh machine gets a bootstrap that reports success and then
  renders tofu boxes in the terminal and in every nvim icon column.

  Open questions for whoever picks this up:
  - The install differs by platform. On macOS it is a cask
    (`font-hack-nerd-font`), which the `BrewPackage` availability with a
    `kind: Cask` already models. On Linux there is no single package name:
    Arch has `ttf-hack-nerd`, Debian and Ubuntu ship nothing current, so the
    apt path is probably a release-archive download into
    `~/.local/share/fonts` plus `fc-cache -f`. That is a new
    `PackageAvailability` shape, or an `ArchiveInstall` alongside `GitClone`.
  - The check command is not a `command -v`. It is
    `fc-list | grep -q 'Hack Nerd Font'` on Linux and a `ls` under
    `~/Library/Fonts` (or the same `fc-list` if fontconfig is installed) on
    macOS, so it may need the two-branch `-o` shape `zsh-autosuggestions`
    uses in `deps.conf:26`.
  - Decide whether `have_nerd_font` should stay unconditional in the nvim
    config or become a probe. Leaving it true is right if the font is a
    tracked dependency; it is wrong today, because nothing guarantees it.

- Make `config install-hooks` recoverable when `~/.local/bin` is group- or
  world-writable.
  Hit on a real Pop!_OS reinstall 2026-09-07. `setup.sh` cloned and checked
  out fine, then:

      config init: [2/4] link the git hooks and put config on PATH
      config install-hooks: refusing, /home/austin/.local/bin is group- or world-writable

  The guard itself is right and must stay: `.scripts/config/config-install-hooks:32-42`
  refuses to install into a directory anyone else can write, because the hooks
  and everything in `~/.local/bin` run as this user.

  What is wrong is that the refusal is a dead end. `install-hooks` is step 1
  of `config init` precisely because it is what puts `config` on PATH, so its
  `exit 1` leaves the machine with a cloned repo, no `config` command, and no
  way to run any of the remaining steps. The user's next four commands all
  failed (`config st`, `config build`, `s code`), and the only escape found
  was `rm -rf ~/.cfg` and starting over, which hit the same wall.

  The message also does not say what to do. The fix is one command
  (`chmod go-w ~/.local/bin`), and the tool knows it.

  It also fails on the FIRST offending directory, and it checks four
  (`config-install-hooks:49-52`: `~/.local/bin`, `~/.scripts/config`,
  `~/tests`, `~/.cfg/hooks`). On the machine above, fixing the first just
  revealed the second, so a user with a permissive umask pays one round trip
  per directory with no way to see how many are left.

  Root cause is probably the umask rather than any one directory: a `umask
  002` makes everything git checks out group-writable, so all four are
  offending at once and a per-directory fix is treating symptoms.

  What to change:
  - Check every directory, then report ALL offenders in one message with a
    single `chmod go-w <dir1> <dir2> ...` that clears them together.
  - Print the remedy in the error: name the offending mode bits and the exact
    chmod that clears them.
  - Consider naming the umask when all four are offending, since that is the
    actual cause and the chmod is a one-time patch over it.
  - Decide whether `config init` should offer to fix the permissions itself
    when the directory is owned by the user (owner-writable is the only safe
    auto-fix; a directory owned by someone else must still refuse).
  - Either way `config init` should not leave a machine with no `config` on
    PATH and no printed path forward. Consider continuing to the later steps
    and reporting the skipped one at the end, rather than exiting at step 1.

  A test belongs with this: create a fixture `bin` directory with mode 0775,
  run `install-hooks`, and assert the message names both the directory and
  the chmod. `tests/config-init.test.sh` and `tests/githooks-installed.test.sh`
  are the two suites that already drive these paths.

