Take the first item from this list. Mark it as claimed in one commit, do the work, then remove it when done in another commit. This prevents any agent race conditions. If TODOS is empty, leave the heading in place l move onto QUESTIONS. For any of these, if the fix is clear/mechanistic, perform it autonomously as a single commit using /test-driven-development. If not, move onto the next clear item, and surface the remaining items for discussion via the /brainstorming skill at the end.

# TODOS:

- Readme cleanup:
  - "Welcome! Here is my rather unpolished .dotfile configuration. Below are some setup notes to my future self, for when I inevitably completely forget what I did or how I did it." remove this. This dotfiles repo is now quite polished.
    - Also cut down the README.md down to the most basic need-to-known eli5 details (install script, use `config` to init the repo & stage files, etc.)
  - Add tests for neovim setup after running setup.sh. When I tested just now, lots of deps failed to install--make sure setup is clean after runnign nvim for the first time on a fresh bootstrap session
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
- The branch-drift GitHub workflow races the two-branch push. `config sync`
  style pushes land the two branches seconds apart; the workflow run for the
  first push compares the new branch against the other branch's stale
  `origin/` ref and fails, while the second push's run passes.
  Mechanism confirmed 2026-09-05 by reading the workflow: it DOES fetch both
  branches (`git fetch origin mac:refs/remotes/origin/mac` and the linux
  equivalent), but it fetches at workflow start, which is before the second
  push has landed. The fetch is not missing, it is simply too early, and
  nothing re-checks afterwards. So "make the job re-fetch" is only a fix if
  the re-fetch happens after a wait or a retry, not at step one.
  Evidence as of 2026-09-05: five sightings, and every push in that session
  needed a manual rerun of the mac job. Three controlled retests -- rerunning
  the failed job with zero code changes -- passed each time, which isolates
  timing as the only variable. Gaps measured between the two runs' start
  times: 23s, 16s, 14s, 15s.
  This is now a standing tax on every push rather than an occasional flake,
  and it leaves the mac branch red by default until someone reruns it.
  Options, in the order the evidence now favors them:
  (1) gate the job so it only runs once per pair -- on the second branch's
  push, or on a workflow_run that follows both;
  (2) compare the pushed sha against the other branch's tip and skip when the
  other branch is behind what this run already knows about, which is the
  "skip rather than fail" shape;
  (3) re-fetch and retry after a short wait, which fixes the symptom but adds
  wall-clock to every run and still fails if the second push is slow.
  The pre-push hook already gates on local mac vs linux, so the CI job is a
  re-check rather than the only gate -- which is what makes a deliberate skip
  safe here.
- Our testing & repo infrastructure has grown quite complex. Let's consider porting some of these to Rust scripts -- both for ease of reading/writing/updating/managing/testing, but also for speed. Brainstorm options here
- Shell startup is currently verrryy slow, and this compounds for large setup tasks like the `se` alias. When I last ran it, it took minutes before Alacritty was responsive again. Let's consider/debug/profile what may be slowing things down here. Let's also take a bigger picture step back to see if there are other options to get the same results as the `se` alias that would run more quickly
- Renaming tmux windows seems to lag a bit on git branch change. Let's look to see if there are some "smarter" hooks we can hook into to update the window name on git branch change, new branch, checkout, etc.

# QUESTIONS (leave until queried)

- Are our git hooks currently configured to run the leak check on commit and then the test suite on push? If not, they should.
- Are we using the Docker container for the pre-push test suite? Should we be?

# DEFERRED TODOS

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
