Take the first item from this list. Mark it as claimed in one commit, do the work, then remove it when done in another commit. This prevents any agent race conditions. If TODOS is empty, leave the heading in place l move onto QUESTIONS. For any of these, if the fix is clear/mechanistic, perform it autonomously as a single commit using /test-driven-development. If not, move onto the next clear item, and surface the remaining items for discussion via the /brainstorming skill at the end.

# TODOS:

- nvim: `ensure_installed` NEVER RUNS HEADLESS, which invalidates the
  obvious CI test for it. Verified verbatim at
  mason-lspconfig/lua/mason-lspconfig/init.lua:31:
      if not platform.is_headless and #settings.current.ensure_installed > 0
  So any CI or Rust test that runs `nvim --headless` and then asserts the
  servers installed is asserting THE GATE, not the install: nothing was
  attempted. That assertion passes on a machine where mason is entirely
  broken. A headless test has to drive the install directly (pkg:install or
  :MasonInstall) instead.
  Record this beside any mason test work so the trap is not rediscovered.

- nvim determinism: what is left after the 2026-09-08 hardening pass.
  DONE, so nobody re-does it: runtime tree installed whole (bd38c48f);
  lazy-lock.json tracked, it had been gitignored (7a308818); mason registry
  and 14 tool versions locked (261510bc); treesitter written for the `main`
  branch it is pinned to, which also locks all 19 parser SOURCE revisions
  transitively through nvim-treesitter's parsers.lua (a345469e);
  tree-sitter-cli moved from mason to the deps engine, because the editor
  cannot install what it needs during its own first run (0dd3deac); linters
  skipped when absent instead of erroring per buffer (b036a064); the Neovim
  version pinned on brew and pacman too, not only apt, since the treesitter
  ABI is keyed to it (2804f2e7); every release download checksum-verified
  (2a8055c7).
  WHAT REMAINS, and none of it is mechanical:
    - THE HONEST BOUNDARY, worth stating once rather than re-deriving: what
      is now guaranteed is "the same declared inputs on every machine" --
      same Neovim version and bytes, same 37 plugin commits, same 19 parser
      source revisions, same 14 tool versions. What is NOT guaranteed is
      byte-identity of anything compiled on the target (parsers, whose bytes
      depend on the host cc) or of an npm package's transitive tree under a
      pinned top-level version. Mason has no integrity-hash layer for
      package payloads. Closing that last gap means vendoring compiled
      artifacts, which all four expert lenses argued against: parser .so
      files are ABI-coupled to the Neovim build and platform-coupled to the
      machine, so vendoring creates a matrix to rebuild on every bump whose
      stale entries fail at buffer-open on the OTHER machine.
    - PARTLY CLOSED 2026-09-09: crates/config-cli/tests/nvim_config_load.rs
      now drives `MasonToolsInstallSync` in a scratch XDG home and formats
      real .rs/.lua/.ts/.json buffers through conform, failing when a tool is
      present and the file does not change. That covers mason installing and
      the formatter wiring locally and in the pre-push container. Still not
      exercised: LSP attach and treesitter parser compilation. The panel's tiering: an offline config-correctness tier
      on every push, a networked tier on a schedule plus workflow_dispatch,
      split by "touches the network" rather than by "is slow" so the fast
      signal is not hostage to npm uptime. Each layer gets its own CI STEP so
      a red run names the layer (install / runtime tree / plugin manager /
      mason / LSP) rather than "nvim exited 1".
      NOTE THE TRAP recorded separately below: ensure_installed does not run
      headlessly, so the obvious version of that test asserts the gate rather
      than the install.
    - Do NOT cache the unpacked tree or ~/.local/share/nvim between legs.
      All four lenses flagged it as this repo's pre-satisfied-path shape
      again; a cached parser built against another ABI passes invisibly.
      Cache the tarball BYTES if anything, which the digests now make safe.
    - Render snapshots: all four lenses said no as a blocking gate (terminal
      size, locale, font fallback, colorscheme, plugin version). Assert
      structured state instead -- nvim_eval_statusline, nvim_get_hl, buffer
      lines. Two measurement traps found while testing this by hand:
      extmark COUNT is not a highlighting signal (Neovim's treesitter
      highlighter paints during draw and persists no extmarks), and
      get_captures_at_pos returns nothing in a -S script that runs before
      the FileType autocmd; run the query against the tree instead.
    - RESOLVED 2026-09-09 by making the font a tracked dependency
      (`nerd-font` in deps.toml, installed by the engine on every platform),
      so `vim.g.have_nerd_font = true` is now a true statement on a fresh
      machine rather than an assumption. No probe needed.
    - data-flow's structural fix: `TarballLayout` as a closed sum
      (SingleBinary vs RelocatablePrefix) with `InstallAction::postconditions()`
      deriving checks from the artifact, so a check cannot disagree with what
      was installed. Partially anticipated by `tarball_is_archive`, which now
      names that same distinction as a predicate rather than a type. The same
      "type describes less than the artifact" mismatch likely lurks in
      Script{}, GitClone{} (DirExists passes on an empty dir left by an
      interrupted clone) and AptSource{}.

- Harden the nvim Lua config at the CODE level: lints, tests, refactors for
  clarity and purity. Requested by the owner 2026-09-09.
  Nothing lints the Lua today. `tests/shellcheck.test.sh` covers 25+ shell
  scripts and there is no equivalent for the ~20 files under
  `.config/nvim/lua/`, so the only feedback on a Lua mistake is nvim failing
  at runtime -- which is how the NvimTree and mason-version regressions both
  shipped.
  WHAT TO CONSIDER, roughly in order of value per effort:
    - DONE (b375c224): the `stylua --check` gate. 12 files had drifted from
      the repo's own .stylua.toml; they are formatted and
      tests/nvim-lua-format.test.sh now fails when they drift again.
    - DONE (3fbd91aa): the selene lint gate. The research answer was that
      selene ships no Neovim std and upstream issue #284 is still open, so
      every project supplies its own `vim.yml` with `vim: any: true` --
      mason.nvim, Allaman/nvim and plenary.nvim all converged on that. The
      `+vim` in `std = "lua51+vim"` resolves as a file beside selene.toml.
      Measured 0 errors and 54 warnings, all `mixed_table`, which is allowed
      because lazy.nvim plugin specs genuinely are mixed tables.
    - SUPERSEDED, kept for the reasoning:
      Both linters are in the mason registry: `selene`
      (pkg:github/Kampfkarren/selene@0.31.0, a Rust binary needing no lua
      runtime, which fits this repo) and `luacheck`
      (pkg:luarocks/luacheck@1.1.0, which needs luarocks).
      Verified selene works and catches the right class -- given a file with
      an unused local and an undefined call it reported 2 errors and 1
      warning. BUT it also reported "`vim` is not defined" on the very first
      real line, because its bundled standard libraries are plain Lua.
      So the work is not "add selene", it is "add selene plus a `vim` std
      definition". The ecosystem answer is a generated neovim.yml std file;
      without it the gate reports every line of every file and gets muted
      within a day. Budget for that, not for the binary.
    - DONE 2026-09-09, both halves. The three pieces of real logic now live
      as pure modules under `lua/dotfiles/`, each taking its IO as an
      injected predicate or table so no spec touches editor state:
      `treesitter_start.should_start(lang, has_parser)`,
      `lint_runnable.runnable(linters, is_executable)`,
      `mason_ensure.ensure_list(names, mason_names, lock_packages)`. Specs
      sit in `.config/nvim/tests/*_spec.lua` with a 20-line harness, run by
      `nvim --headless -l` (nvim as the Lua interpreter, so `vim.*` is real
      and no plenary or busted is needed) and driven from
      `crates/config-cli/tests/nvim_lua_units.rs`, which asserts each spec
      exits 0, keeping the gating assertion in Rust. The plugin specs call
      the modules; `nvim_config_load.rs` is the runtime net and passed
      unchanged after the rewiring.
      One text-anchor gate moved with the code: nvim-mason-runtimes.test.sh
      grepped lsp.lua for `version = version` and now greps the module, and
      asserts lsp.lua requires it. Two lints learned in passing: selene's
      lua51 std does not model `io.stderr` as a handle, so the harness
      prints; and stylua rewrapping a file between a read and an edit is how
      a match-by-memory edit silently no-ops.
  NOTE the interaction with the config-load test added today: that test
  catches "the config raises at runtime", which is the outer net. Lints and
  unit tests are the inner one, and they are what make a failure land at the
  edit rather than at the next launch.

- DONE 2026-09-09, and the intent was narrower than this entry said: keep the
  Nord status bar, make the TERMINAL ground full black. The blue was never
  the ground. Alacritty painted `0x2E3440` (Nord polar night) as
  `colors.primary.background`, and both nvim (`transparent = true`) and
  Nord's tmux panes (`bg=default`) show the terminal through, so one line
  in alacritty.toml was the whole change. Pinned by
  tests/alacritty-platform-split.test.sh so a future Nord palette paste does
  not drag it back.
  NOT changed, on purpose: the palette `black = 0x3B4252`, which is what
  makes the bar look like Nord. So the bar now sits as a grey-blue strip on
  a black ground with a visible edge. If that edge is unwanted, the next
  step is palette black to `0x000000`, and it recolours every `black` in
  nvim and zsh too, so it is a separate decision.
  Superseded reasoning kept: the earlier draft of this entry proposed
  removing the plugin and rewriting the four status segments by hand. That
  was solving the wrong layer.
- DONE 2026-09-09: the two path-handling gaps that shared one cause, that
  git C-quotes any path with a byte outside printable ASCII and the quoted
  form matches no pathspec when fed back.
    - `tests/leak-check.sh`: fixed with one line,
      `export GIT_CONFIG_PARAMETERS="'core.quotePath=false'"`, which reaches
      every git the script spawns including the ones under `xargs -0`, where
      a shell function would not. THE RECORDED CONSEQUENCE WAS WRONG and is
      corrected here: this entry predicted "scans zero lines and exits 0". 
      Measured before the fix, a credential under `café/` exited 2, blocked
      by unscannable_paths as "no readable diff" with a diagnosis naming
      .gitattributes and binaries. So the gate failed closed and nothing
      leaked; the real defect was that no non-ASCII path was committable at
      all, and the message pointed at the wrong cause. Both sides are now
      asserted: a clean file under such a path passes, a secret under it is
      blocked as a secret.
    - `setup.sh`: the move-aside loop was `for path in $(cfg ls-tree ...)`,
      which word-splits a path with a space and receives the quoted form for
      a non-ASCII one, so neither was moved and the retried checkout failed
      under set -e. Now a heredoc-fed `while IFS= read -r` over
      `-c core.quotePath=false` output, which is POSIX (`read -d ''` is not)
      and keeps the loop in the main shell so the moved counter survives.
      A path containing a newline is the one shape this still misses, and it
      stays recorded as such below.
- RESOLVED BY THE 2026-09-06 COLLAPSE, decided 2026-09-09. The `.sync-manifest`
  parser this entry describes, with its `Shared`/`Excluded` classification,
  went out with `config sync` and `config check`. A workspace-wide search for
  `Excluded`, `SharedRule`, `Classification::` and `sync-manifest` finds only
  an unrelated doc comment in deps-core. The decision taken stands as the
  rule for the next manifest-style parser: reject an orphan exclusion at
  parse time (a `!` pattern nested under no directory pattern is an error),
  keeping a flat rule list. An orphan that silently widens a gate is the
  fail-open shape this repo keeps paying for, so it must fail loudly at the
  edge.
- RESOLVED BY THE 2026-09-06 COLLAPSE, confirmed 2026-09-09 by reading the
  crate rather than this entry: `crates/config-manifest` no longer has a
  binary. `src/` is doctor.rs, git.rs, lib.rs, path.rs and stamp.rs, with no
  `[[bin]]` and no main.rs, so the "nine sites" and the `Err` arm at
  main.rs:86 this entry described are gone. What survives is one
  `Rendered.exit_code: u8` in stamp.rs with three assignment sites (0, 1, 2),
  each with a documented meaning, one consumer (config-cli's verify_stamps),
  and a doc comment on why `u8`. The pain this entry recorded, one status
  meaning eight things, does not exist in that shape. An `Outcome` sum over
  three sites would be a refactor for its own sake, so none is made. The
  design rule stands for the next crate: derive exit codes from a named
  meaning in one place rather than scattering literals.
- DONE 2026-09-09: the fossil branches `home`, `home-mac` and `work` (tips
  eb6ca73d, 558f1fb9, bdd98825; last moved 2023) are deleted, with no
  archive tags. The first plan was tags pushed to origin, and the leak guard
  refused that push: those branches had NEVER been on origin (it holds only
  linux, mac and main), so the push would have published three years of
  never-published history, and one compiled Vim spell file in it
  (.config/nvim/spell/en.utf-8.add.spl) is a binary the guard cannot scan.
  Every text file scanned clean. The decision was to publish nothing and keep
  nothing: the tips survive only in this machine's reflog for its expiry
  window. `--branch` stays as the way to reach a real feature branch;
  setup.sh's two comments no longer steer a reader at a 2023 tree, and
  tests/setup.test.sh asserts that. The fixture branch that suite names
  `work` is unrelated and unchanged.
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
- CLOSED AS MEASURED 2026-09-09. `se` builds 21 windows x 5 panes = 105
  panes in 6.6-9.7s, of which shell startup is 0.6s total because the 105
  shells start in parallel; the rest is tmux creating panes. That is tmux's
  pane-creation cost, not a defect, and the decision is to keep the eager
  layout. Interactive zsh startup is 170-176ms, held under 400ms by
  tests/zshrc-startup-budget.test.sh. Lazy window creation and a smaller
  layout were weighed and declined. The earlier "minutes" has no surviving
  measurement and is treated as a cold cache or a since-fixed eager init.
- DONE 2026-09-09, both halves, decided as two commits.
  COST (3aab00c2): the TODO's 57ms had no surviving method. Measured with
  zsh's EPOCHREALTIME: the binary alone is 15-22ms per prompt (6-9ms process
  start plus one 8-13ms `tmux list-windows`), the common path spawns no git
  (repo.rs reads .git directly; `fallback_to_git_rev_parse` records zero
  fallbacks across 21 live windows), and an unchanged rename is already
  skipped (main.rs:414). Through the sh wrapper the same call was 25-36ms:
  the exec hop was a third of every prompt. precmd now calls
  `tmux-tools name-windows` directly; the wrapper stays for the tmux hooks
  and the `re` alias. There was nothing to cache, so no cache was built.
  LATENCY: a tracked .scripts/git-hooks/post-checkout runs
  `tmux-tools name-windows -a &` on a branch checkout (flag 1; a file
  checkout renames nothing), installed per repository by the new
  `config install-repo-hooks [dir]` into the common git directory so one
  install covers every worktree. Not a global core.hooksPath, which would
  replace ~/.cfg's own hooks. tests/git-post-checkout-hook.test.sh stubs
  tmux-tools on PATH and asserts a checkout invokes it with -a, from a
  worktree too, that install is idempotent, and that a non-repo is refused
  with exit 2. Installing into your existing repos is one
  `config install-repo-hooks` each; not done here, since that is machine
  state and the loop touches only the repo.
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

