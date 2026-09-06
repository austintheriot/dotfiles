# Dot Files

| | mac | linux |
| --- | --- | --- |
| Test suite | [![mac test suite](https://github.com/austintheriot/dotfiles/actions/workflows/test-suite.yml/badge.svg?branch=mac)](https://github.com/austintheriot/dotfiles/actions/workflows/test-suite.yml?query=branch%3Amac) | [![linux test suite](https://github.com/austintheriot/dotfiles/actions/workflows/test-suite.yml/badge.svg?branch=linux)](https://github.com/austintheriot/dotfiles/actions/workflows/test-suite.yml?query=branch%3Alinux) |
| Branch drift | [![mac branch drift](https://github.com/austintheriot/dotfiles/actions/workflows/branch-drift.yml/badge.svg?branch=mac)](https://github.com/austintheriot/dotfiles/actions/workflows/branch-drift.yml?query=branch%3Amac) | [![linux branch drift](https://github.com/austintheriot/dotfiles/actions/workflows/branch-drift.yml/badge.svg?branch=linux)](https://github.com/austintheriot/dotfiles/actions/workflows/branch-drift.yml?query=branch%3Alinux) |

My shell, editor, terminal and tmux configuration, tracked in a bare git
repository with `$HOME` as the worktree.

## Install

One command, on macOS, Linux or WSL:

```sh
curl -fsSL https://raw.githubusercontent.com/austintheriot/dotfiles/mac/setup.sh | sh
```

Nothing needs to be installed first, not even git. The script clones the repo
to `~/.cfg`, picks the branch that matches this machine, checks the worktree
out into `$HOME`, and installs the dependencies. An existing `.zshrc` is moved
into a timestamped `~/.dotfiles-backup-*` directory rather than overwritten.

The `mac` in the URL is only the branch the file is fetched from, not the
branch you get. `setup.sh` reads `uname` and checks out the matching branch
itself.

Options are in [SETUP.md](./SETUP.md), along with what the script does and why
it stops where it does.

## Daily use

`config` is the front door. It is git against the bare repo, so the verbs are
the ones you already know:

```sh
config status            # what has changed
config add .zshrc        # stage a file, by its path under $HOME
config commit -m "..."   # commit
config push-all          # push mac and linux together
```

Paths are always home-relative: `config add .claude/skills/foo/SKILL.md`, never
an absolute path. `config status` hides untracked files by default, because
`$HOME` is full of them; use `config status -uall` to see them.

Two things are not git verbs:

- `config sync` copies the shared paths onto the other branch. Run it after a
  commit that touches a shared file, then `config push-all` so both branches
  land in one push.
- `config test` runs the test suite. The pre-push hook runs it too.

Run `config help` for the full list of subcommands, or see
[Repo utilities](#repo-utilities) below.

## Branches

`mac` and `linux` are both live. Most files are shared byte-for-byte between
them, and `.sync-manifest` lists which; `config check` reports drift and
`config sync` fixes it.

Anything true of only one machine lives in its own file, and both ship on both
branches so the drift check covers them:

- [README-MAC.md](./README-MAC.md) -- Homebrew, aerospace, macOS build
  performance, and the Claude Code notification hook.
- [README-LINUX.md](./README-LINUX.md) -- xclip, oh-my-zsh, and the WSL notes.

## Reference

- [SETUP.md](./SETUP.md) -- bootstrap options and how `setup.sh` and
  `config init` divide the work.
- [.scripts/deps/README.md](./.scripts/deps/README.md) -- the dependency
  manifest, how to add a dependency, and the startup check.
- [DOTFILES.md](./DOTFILES.md) -- the bare-repo technique this is built on.

### Repo utilities

`config help` prints the list. It is generated from each script's own
`# help:` line, so it cannot fall behind the scripts:

- `config build` builds the Rust crate and installs the stamped binary.
- `config check` reports drift between the mac and linux branches.
- `config init` finishes a fresh clone: git config, hooks, dependencies,
  binary. The post-clone half of the bootstrap; `setup.sh` is the other half.
- `config install` installs any missing tracked dependencies.
- `config install-hooks` links the git hooks and puts `config` on PATH.
- `config push-all` pushes mac and linux together in one atomic push.
- `config reload` reloads the tmux config.
- `config stamp` prints the tree id of the crate in the worktree.
- `config sync` copies the shared paths onto the other branch.
- `config test` runs the test suite.

`config <command> --help` prints that command's usage block.

To add one, drop a `config-<name>` script into `.scripts/config/` with a
`# help:` line and a `# usage:` block, source `.scripts/config/usage.sh` and
call `usage_if_requested "${1:-}"` before parsing anything, and add its name to
`EXPECTED_SUBCOMMANDS` in `tests/config.test.sh`.

`config help` shadows `git help`. Use `config -- <verb>` to send a verb
straight to git: `config -- help rebase` opens the git manual page.

### Dependency checking

Tracked dependencies live in `.scripts/deps/deps.conf`, plus
`.scripts/deps/deps-mac.conf` and `.scripts/deps/deps-linux.conf` for the ones
that belong to one platform.

- `~/.scripts/deps/check-deps.sh` checks them and reports what is missing.
- `depcheck` is a shell alias that checks them and offers to install anything
  missing.
- A shell-startup hook prints one line at most once every 24 hours when
  something has gone missing. It never installs and never blocks startup.

See [.scripts/deps/README.md](./.scripts/deps/README.md) for the manifest
format and how to add a dependency.
