Take the first item from this list. Mark it as claimed in one commit, do the work, then remove it when done in another commit. This prevents any agent race conditions. If TODOS is empty, leave the heading in place l move onto QUESTIONS. For any of these, if the fix is clear/mechanistic, perform it autonomously as a single commit using /test-driven-development. If not, move onto the next clear item, and surface the remaining items for discussion via the /brainstorming skill at the end.

# TODOS:

Swept 2026-09-08 against live code, checking the exact file:line each entry
cited rather than trusting the entry. Twelve items were already fixed by the
2026-09-06 and 2026-09-07 plan work, which resolved them without removing
them from this file: both PROMPT LATENCY items, all four leak-check and
lib.sh BLOCKERs, the bare-printf skips, the TRIGGER_PATHS gap, the
`config help --describe` contract, the assert_succeeds exit code, the
test-bootstrap empty-context build, and SKIP_LEAK_CHECK truthiness. They are
deleted below. What remains was confirmed still present.

Every BLOCKER from that survey pass is now closed. What is left is a mix of
small mechanical fixes, two latent path-handling gaps with no live trigger,
two config-manifest design decisions, and several items that need a
conversation rather than a commit.

- Capture stdout on a failed install, not only stderr.
  `ExecFailure::NonZeroExit` (`crates/deps-core/src/outcome.rs:32-37`) carries
  `code` and `stderr` and nothing else, so `describe_cause`
  (`crates/deps-core/src/render.rs:145-151`) can only report
  "exited 1 with no output on stderr" when a script explains itself on
  stdout.
  That cost real time on 2026-09-08. Four bootstrap legs failed with exactly
  that message, and the actual cause -- oh-my-zsh's installer printing
  "Zsh is not installed. Please install zsh first." and exiting 1 -- was
  visible only by running the script by hand in a container. Verified: that
  script writes zero bytes to stderr. The engine had the explanation and
  discarded it.
  Not a big change, but it is not a one-liner either: `NonZeroExit` gains a
  `stdout: BoundedText` field, which touches the type, every construction
  site, and every match arm. `BoundedText` already exists for exactly this
  bounding problem, so the shape is settled.
  Worth pairing with a decision about precedence in the rendered line: prefer
  stderr when both are non-empty, fall back to stdout, and say which stream
  a message came from so a reader knows where to look next.

- Remove the Nord theme from tmux. KEEP the status bar; make its background
  black.
  Confirmed cause: `.config/tmux/tmux.conf:27` declares
  `set -g @plugin "nordtheme/tmux"`, whose own README calls it "an arctic,
  north-bluish clean and elegant tmux color theme". It is one of three
  plugins, beside tpm itself and tmux-sensible.
  WHAT IS ACTUALLY BLUE, which is narrower than the whole theme and changes
  the shape of the fix. Nord's own `status-style` is ALREADY
  `bg=black,fg=white` (`plugins/tmux/src/nord.conf:24`), so the bar's
  background is not the problem. The blue is in the SEGMENTS, all set by
  `plugins/tmux/src/nord-status-content.conf`:
    - `status-left`: the session name on `bg=blue`.
    - `window-status-current-format`: the active window on `bg=cyan`.
    - `status-right`: the hostname on `bg=cyan`, with the date and time on
      `bg=brightblack`.
    - `window-status-format`: inactive windows on `bg=brightblack`.
  Two smaller blue settings outside the status line, worth deciding on at
  the same time: `pane-active-border-style fg=blue` and
  `message-style fg=cyan` (nord.conf:29 and :33).
  So the work is to keep the layout and re-colour the segments, not to
  delete the bar. The content is what this repo wants anyway: session name
  on the left, window list in the middle, date, time and hostname on the
  right.
  THE ONE REAL CONSTRAINT: this repo has NO status settings of its own. A
  grep for `status` across `tmux.conf`, `tmux-mac.conf` and
  `tmux-linux.conf` returns nothing, so Nord supplies the entire bar.
  Deleting the plugin line alone drops to tmux's DEFAULT green bar and loses
  the clock and the hostname. Keeping the bar therefore means writing our
  own `status-left`, `status-right`, `window-status-format` and
  `window-status-current-format` -- copy Nord's, which are the layout to
  keep, and change the colours.
  Suggested starting point, to be adjusted by eye rather than trusted from
  here: `bg=black` throughout for the bar, `bg=brightblack` for inactive
  segments (a visible but unobtrusive step up from the ground), and one
  non-blue accent for the active window and the hostname. Nord's structure
  uses a `#[fg=X,bg=black]` separator glyph between segments; that separator
  needs a patched font, and this config never sets
  `@nord_tmux_no_patched_font`, so it is currently getting the patched-font
  variant.
  Mechanics, verified:
    - no test asserts the plugin list or any status setting (grep for
      `@plugin`, `nordtheme` and `status` across `tests/*.test.sh` returns
      nothing on the first two). `tmux-conf-split.test.sh` reads the config
      but asserts on the platform-variant wiring.
    - the plugin tree at `.config/tmux/plugins/` is untracked, installed by
      tpm, so removal is `rm -rf .config/tmux/plugins/tmux` plus the line.
    - `.config/tmux/tmux.conf.bak.20260821121033` is an untracked backup
      from 2026-08-21 carrying the same plugin line. Delete it in the same
      pass so a future grep for `nordtheme` comes back clean.
  Worth considering while in there: whether the bar should show anything the
  window-naming binary already computes, since that is the one piece of
  status content this repo owns.

- Prettify console output for readability: colour, and surface warnings,
  errors and successes more clearly.
  Verified starting point: this repo emits NO colour at all. A grep for
  literal ANSI escapes across every tracked file outside `.claude/` returns
  nothing, and the only `IsTerminal` use is
  `crates/config-cli/src/deps/installer.rs:645`, which gates a prompt rather
  than styling. So this is new capability, not a cleanup.
  The vocabulary already exists and is worth keeping -- the work is styling a
  structure that is already correct, not inventing one:
    - `crates/deps-core/src/render.rs` emits `present`, `installed`,
      `FAILED` and `BLOCKED` per row, already splits stdout from stderr, and
      already indents a cause under the name it belongs to.
    - `tests/lib.sh:209-268` emits `ok:`, `FAIL:` and `skip:` per assertion.
    - `tests/run-all.sh:143-296` emits per-suite `PASS`/`FAIL`/`SKIP` plus
      the run summary.
  THE DESIGN TENSION, which is the reason this is not a one-hour change.
  `render()` is a pure function returning `Rendered { stdout, stderr,
  exit_code }`, and its doc comment says why: returning the streams rather
  than writing them keeps every reporting decision testable without
  capturing a process's output. A pure function cannot call `is_terminal()`.
  So the colour decision has to arrive as an argument, which means picking
  one of:
    - `render(report, verb, style)` where `Style` is a closed sum
      (`Style::Plain` / `Style::Ansi`) decided at the IO edge and passed in.
      Keeps the core pure and makes both branches testable by construction.
      Costs a parameter at every call site.
    - a `Rendered` that carries structured spans rather than a `String`, with
      the caller doing the painting. Most flexible, largest change, and it
      moves the "what is a failure" decision out of the one place that owns
      it today.
    - colour applied at the IO edge by pattern-matching the rendered text.
      Cheapest and wrong: it re-derives the status from the text after the
      type that knew it has been discarded.
  Prefer the first. The FP and interface rules in CLAUDE.md both point there,
  and it is the only option where "this row is a failure" stays a decision
  made once.
  NON-NEGOTIABLE, because they are how colour usually breaks things here:
    - honour `NO_COLOR` (any non-empty value, per no-color.org) and default
      to plain when stdout is not a tty. Every one of this repo's gates greps
      output -- `tests/run-all.sh:133` parses the summary line with sed, the
      deps-check workflow greps for `present   neovim`, and
      `tests/container.test.sh` matches assertion text. Escapes in a piped
      stream break all of them, and it is the CI-only breakage class this
      repo keeps hitting.
    - colour must never be the only channel. The words stay; colour is
      redundant emphasis on top. A reader with a mono terminal, a log file,
      or deuteranopia loses nothing.
    - one assertion per rule, and a test that pipes the output and asserts
      the bytes are unchanged. A colour feature with no piped-output test is
      the same silent-failure shape as a gate that greps for text it can no
      longer see.
  Worth a `/consult interaction-design` or `/consult expert-user-efficiency`
  pass on which states earn emphasis before writing code: the daily signal
  here is a wall of `present` rows where only the exceptions matter, and
  colouring all 17 green defeats the purpose.

- Switch the deps engine over to reading the TOML manifests, then delete the
  `.conf` files and `parse_manifest`.
  Both forms ship today. `deps/*.toml` is the format and `deps/*.conf` is
  what the engine actually reads; `crates/config-cli/src/deps/catalog.rs`
  embeds both and its
  `every_toml_manifest_is_equivalent_to_its_pipe_manifest` test compares all
  22 entries as PARSED values, so the two cannot drift silently.
  CORRECTION to the earlier version of this entry, which said the untracked
  hand-edited `deps-local.conf` was the hard part. It is not a factor:
  `deps-local.conf` is RETIRED. No Rust source constructs that name (grep
  confirms), no such file exists on this machine, `variant_file_name()` in
  `crates/config-cli/src/deps/selection.rs:45-46` returns the TRACKED
  `deps-mac.conf` / `deps-linux.conf`, and both `tests/deps-docs.test.sh:263`
  and `tests/deps-harness.test.sh:456` assert the name is gone from the
  README. `DEPS_LOCAL_CONF` survives as an env override taking a full path,
  so it is extension-agnostic. The only live traces are stale prose in
  `deps/deps.conf:5` and `:19`, which the deletion removes anyway, plus one
  line copied into `deps/deps.toml:4` that should say `.toml`.
  The functional edits, each verified present:
    - `catalog.rs:50-53` point `SHIPPED_CONF_FILES` at the `.toml` files and
      call `parse_manifest_toml`; drop the `#[cfg(test)]` from
      `SHIPPED_TOML_FILES` (a comment there says so).
    - `selection.rs:45-46` and `:154` are the three hardcoded stems
      (`deps.conf`, `deps-mac.conf`, `deps-linux.conf`).
    - `.github/workflows/test-suite.yml:142` sets `DEPS_CONF: deps-ci.conf`,
      which `tests/deps-harness.test.sh` asserts.
  THE REAL WORK is three test suites that parse the manifest format in shell,
  which TOML has no line-oriented equivalent for:
    - `tests/deps-docs.test.sh:194` and `tests/deps-manifest.test.sh:151` and
      `tests/nvim-mason-runtimes.test.sh:91` all get the name list with
      `cut -d'|' -f1`.
    - `tests/deps-manifest.test.sh:56` gets ONE entry's check field with
      `grep "^$1|" | cut -d'|' -f2`. This is the hard one: a check is now a
      named key, possibly a nested `any_of` list, so there is no field to cut.
  No production shell parses the manifests -- only these suites -- so the
  options are all test-side. Prefer adding a read-only reporting subcommand
  (`config deps list --names`, and a way to ask one entry's check) so the
  suites ask the engine that already owns the parser, rather than standing up
  a second parser in shell or python. The pipe format's rules lived in prose
  and the shell parsers had to be trusted to honour them; that is the shape
  the TOML move exists to end. Cost to weigh: a CLI subcommand is a contract
  to keep stable.
  Once nothing reads them: delete the four `.conf` files, `parse_manifest`,
  its `WrongFieldCount` and `DuplicateName` variants, and the equivalence
  test, which has no second side left to compare.

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

