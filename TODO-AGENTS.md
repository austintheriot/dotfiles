Take the first item from this list. Mark it as claimed in one commit, do the work, then remove it when done in another commit. This prevents any agent race conditions. If TODOS is empty, leave the heading in place l move onto QUESTIONS. For any of these, if the fix is clear/mechanistic, perform it autonomously as a single commit using /test-driven-development. If not, move onto the next clear item, and surface the remaining items for discussion via the /brainstorming skill at the end.

# TODOS:

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

- Migrate the rest of the `config ...` scripts to Rust
- `tests/leak-check.sh` does not scan paths containing a newline or binary
  files, in either staged or range mode. Git quotes a newline path, so
  `xargs -0` cannot address it; a binary diff has no `+` lines for the
  content rules to see. Confirmed identical at commit 76608b6, so this
  predates the range-mode work. The newline case is a deliberate-evasion
  shape worth closing on a public-repo gate; record only, no fix yet.
- Dropped after measurement, recorded so it is not retried: `compinit -C`.
  An isolated `zsh -f` test showed compinit at 1.17s, but that was an
  fpath artefact. In the real startup trace compinit is ~60ms and did not
  clear a 40ms bar. `-C` would save nothing meaningful and removes the
  compaudit security check.
- Our testing & repo infrastructure has grown quite complex. Let's consider porting some of these to Rust scripts -- both for ease of reading/writing/updating/managing/testing, but also for speed. Brainstorm options here
- Track the Claude Code settings that can be public, and keep the local
  overrides untracked. `~/.claude/settings.json` is untracked in full today
  because some of its blocks name private detail, so nothing about the
  sandbox, telemetry or timeouts is under version control. Split it: private
  blocks into `~/.claude/settings.local.json`, the rest tracked and pinned
  by a test. A panel brief with the measurements and the open decisions is
  in `~/.claude/local/research-notes/agent-config-panel-2026-09-10.md`.
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

- DECIDED 2026-09-10, enable both. Not yet done: it needs an account this
  machine is not authenticated as. GitHub secret scanning and push
  protection are both free on this public repo, and both were verified OFF
  (the `security_and_analysis` field is absent from the API response, and
  `/secret-scanning/alerts` returns 404 rather than 403).
  What to do, two toggles at
  https://github.com/austintheriot/dotfiles/settings/security_analysis:
  Secret scanning to Enabled, Push protection to Enabled.
  Why it could not be done from the CLI: `gh` here is authenticated as a
  work account with `admin: false, push: true` on this repo, and changing
  `security_and_analysis` needs admin. The PATCH returns 404, which is
  GitHub masking a 403. Pushes are unaffected. Anything needing admin, or
  any `gh` command that creates content here, will hit the same wall or
  attribute to the wrong account.
  What each one buys, kept distinct because the original entry conflated
  them:
    - Secret scanning is retroactive detection. It scans the EXISTING
      history, which the local guard has never done: `tests/leak-check.sh`
      has only ever seen content that passed through it since it was
      installed. This is the more valuable half.
    - Push protection is a pre-receive block on new pushes. It fires at the
      remote, so it does NOT serve the "a leak should never even land in a
      local commit" goal at `tests/pre-commit:11-12`. That goal stays the
      local guard's job.
  What it does NOT replace, so nothing gets removed when it is on:
    - Layer 1, the credential-shape rules at `tests/leak-check.sh:269`
      (eight patterns: `ghp_`, `gho_`, `github_pat_`, `xox[baprs]-`, `AKIA`,
      `sk-`, `BEGIN PRIVATE KEY`, `_authToken=`). GitHub's ruleset is
      broader and has fewer false positives, but it is a third net at the
      remote, not a substitute for the pre-commit one.
    - Layer 2, the project-term rules, which read patterns from an untracked
      file OUTSIDE the repo precisely so the terms are never published. No
      hosted scanner can do this, by construction.
  Deliberately untested. No suite assertion can check a remote setting: the
  suite runs in Docker with no credentials and no network expectations. The
  verification is a `gh api` read-back once it is on, and the entry stays
  here until that read-back is done.
  One operational note: a push protection block is bypassable through a web
  prompt. Bypassing it in this repo is never the right move, because layer 2
  means a real finding here can be a published project term rather than a
  credential.

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
