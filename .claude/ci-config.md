# Tracked CI configuration

Read by the `monitor-ci` skill, after `~/.claude/local/ci-config.md`. This file
is tracked in the public dotfiles repo, so it holds only public projects. A
project that names a private repo, or its internal workflows and jobs, goes in
the local file instead. When both files have a section for the same repo, the
local section wins.

Each project below maps the generic monitoring shape onto a concrete CI setup.
The skill picks the project whose `Repo` matches the current `gh` remote.

---

## Project: dotfiles

- **Repo**: `austintheriot/dotfiles` (public). Pushed with `config push` from the
  bare repo at `~/.cfg`.
- **CI provider**: GitHub Actions
- **CI CLI**: `gh run`, always with `-R austintheriot/dotfiles`. `~` is not a normal
  git checkout, so `gh` cannot infer the repo from the working directory.
- **Install / token**: `gh auth status` must show a logged-in account.

### No pull requests

Commits go straight to `main`, so Step 1 of the skill finds no PR. Monitor by commit
instead: take the SHA from `config log -1 --format=%H` and match it against the
`headSha` of each run (`gh run list ... --json databaseId,headSha,workflowName,status`).
There is no AI review bot on this repo, so skip Step 2b.

The pre-push hook gates the push itself before CI ever runs. It needs a running Docker
daemon whenever the push touches tested code. If it prints "the Docker daemon is not
responding", start Docker (`open -a Docker`) and push again. Never pass `--no-verify`.

### Command map

| Generic step | Command |
|---|---|
| List workflows for a branch | `gh run list -R austintheriot/dotfiles -b main -L 5` |
| List jobs in a workflow | `gh run view <run-id> -R austintheriot/dotfiles --json jobs --jq '.jobs[] \| "\(.conclusion)\t\(.name)"'` |
| Failure logs (prefer over artifacts) | `gh run view <run-id> -R austintheriot/dotfiles --log-failed` |
| List / download artifacts | `gh run download <run-id> -R austintheriot/dotfiles` (no workflow uploads any today) |
| Retry failed jobs | `gh run rerun <run-id> -R austintheriot/dotfiles --failed` |
| Blocking wait (run backgrounded) | `gh run watch <run-id> -R austintheriot/dotfiles --exit-status --interval 30` |

`gh run watch` takes one run. When a push starts more than one workflow, watch them one after
the other in a single backgrounded command.

`--log-failed` prints every step as `UNKNOWN STEP`. Filter with
`grep -E 'panicked|left:|right:|FAILED|error'` to find the failure.

### Status vocabulary

- Terminal: `completed`, with the conclusion `success` / `failure` / `cancelled` / `skipped`
- Running: `in_progress`
- Not yet started: `queued` / `waiting` / `pending`
- Gated: none. Neither workflow has an `environment:` block.

### Workflows

| Workflow | File | Fires on | Jobs |
|---|---|---|---|
| Test suite (linux + macOS) | `test-suite.yml` | every push to `main`, `pull_request`, `workflow_dispatch` | `Linux / ubuntu-latest`, `macOS / macos-latest` |
| agent-notify (macOS) | `agent-notify.yml` | push to `main` or `pull_request` touching `tools/agent-notify/**`, `crates/agent-notify-core/**`, `crates/dotfiles-test-support/**`, `crates/Cargo.toml`, `crates/Cargo.lock`, `tests/tools-checks.sh`, or the workflow file; `workflow_dispatch` | `macOS / macos-latest` |
| Dependency bootstrap | `deps-check.yml` | push to `main` touching `deps/**`, `setup.sh`, `.scripts/config/**`, `crates/**`, or the workflow file; cron `0 13 1,15 * *`; `workflow_dispatch` | Ubuntu, Pop!_OS, Arch, macOS installs, plus three full-bootstrap jobs |

A push that touches only skills or rules starts the test suite alone. Both workflows
finish in about 3.5 to 5.5 minutes.

The test suite sets `concurrency: test-suite-<ref>`, so a newer push to `main` cancels
the run already in flight. A `cancelled` conclusion right after another push is not a
failure.

### Known manual gates

None.

### Known flakes

- **macOS tmux pane race, fixed 2026-10-09 in `8e25fd7f`.** `new-window` returns before
  the pane execs its command, so macOS reports `tmux` for the pane. The fixture in
  `crates/tmux-tools/tests/support/mod.rs` now waits for the exec. If a tmux-tools test
  on macOS fails with `left: "tmux"` again, the wait has regressed. Do not treat it as a
  rerun-and-forget flake.
