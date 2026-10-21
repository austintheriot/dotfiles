# thumper

Desktop notifications for coding agents that run in tmux. When a Claude Code session stops, needs a permission, or reports a result, a card shows on screen. Click the card, and thumper takes you to the tmux pane that sent it.

macOS only for now. The CLI has a Linux fallback (`notify-send`), but the daemon and the installer do not run on Linux yet.

## How it works

```
Claude Code hooks ──► thumper (CLI) ──unix socket──► thumperd (daemon + card panel)
tmux focus hooks  ──►                                        │
                                                             └─► tmux: select pane, switch client, raise terminal
```

- **`thumper`**: the CLI. Hooks and agents call it. It reads the tmux pane and the git repository, then sends one request to the daemon. When the daemon does not answer, it shows a plain OS notification through `terminal-notifier` instead.
- **`thumperd`**: a Tauri background app (no Dock icon) that launchd keeps running. It owns all card state and draws the card stack.
- **Claude Code plugin**: hooks that forward session events, and a skill that tells the agent when to call `send` and `quiet`.
- **tmux config**: pane focus hooks, the `M-n` jump key, and per-window markers in the status line.

### What makes a card

| Event | Card |
|---|---|
| Turn ends | Info card with the last message |
| Turn fails | Attention card |
| Permission prompt or elicitation | Attention card |
| `thumper send --kind info` | Info card |
| `thumper send --kind attention` | Attention card, amber, with a sound |

Attention cards stay until you handle them. Info cards also stay until handled by default: a click, the close button, focus on the pane, or a newer card from the same session clears them.

### When no card shows

- You are looking at the pane: the terminal is frontmost, the pane has tmux focus, you were active recently, and the screen is unlocked. If any of those facts is unknown, the card shows.
- The agent already called `send` this turn, so the end-of-turn card is redundant.
- The session is quiet (`thumper quiet`).
- Do-not-disturb is on. The card is stored and shows when do-not-disturb ends.
- A visible attention card from the same session outranks a new info card.

While the pointer is over the stack, new cards wait, so nothing moves under the pointer. While the screen is shared, cards hide their bodies.

## Commands

```bash
thumper send --kind attention --body "CI failed on lint: 3 errors in src/api.ts"
thumper send --kind info --body "Benchmarks done: p99 down 18%"

thumper quiet --until next-send --max-minutes 180   # mute end-of-turn cards for a poll loop
thumper quiet --until next-prompt                   # ...or until you type
thumper quiet --off

thumper dnd on [--for-minutes 30]
thumper dnd off
thumper dnd status

thumper jump            # go to the newest card that needs you (bound to M-n in tmux)
thumper status [--json | --tmux]
thumper doctor          # check the installation and print what to fix
```

Hooks and tmux call these. You do not run them by hand:

```bash
thumper hook claude-code [--dry-run]    # reads a hook payload on stdin, always exits 0
thumper focus-changed --socket ... --pid ... --start-time ... --pane %3 --in|--out
```

`quiet` needs a tmux pane that the SessionStart hook tagged. Outside tmux, `quiet` exits 2.

### tmux window markers

Each window name gets one marker: `!` needs you, `x` failed, `+` finished, `~` working.

## Install

1. Build the CLI with the rest of the crates workspace. This installs `~/.local/bin/thumper`:

   ```bash
   config build
   ```

2. Build and install the daemon. This runs `tools/*/scripts/install.ts`:

   ```bash
   config build --tools
   ```

   The script builds the UI, runs `cargo deny check`, builds `thumperd`, installs `~/Applications/thumperd.app`, signs it ad hoc, and loads the launch agent `dev.dotfiles.thumperd`. To install without touching launchd, run `bun scripts/install.ts --no-launchd` from this directory.

3. Install the Claude Code plugin from this directory's marketplace (`dotfiles-tools`), and enable `thumper@dotfiles-tools`. Allow `Bash(thumper send:*)` and `Bash(thumper quiet:*)` so the agent can call them without a prompt.

4. tmux sources `~/.config/tmux/thumper.conf` when `~/.local/bin/thumper` exists. Reload tmux after the first install.

5. Run `thumper doctor`. It checks the CLI, the socket, the daemon version against the source stamp, the plugin and permission rules, the tmux pane tag and `focus-events`, the fallback notifier, the login item, and the log size.

## Files

| Path | What |
|---|---|
| `~/Applications/thumperd.app` | The daemon app |
| `~/Library/LaunchAgents/dev.dotfiles.thumperd.plist` | The launch agent |
| `~/Library/Application Support/thumper/` | State, including `installed-stamp` |
| `~/Library/Logs/thumper/` | Daemon and CLI logs |
| `~/.cache/thumper/target` | Cargo target directory for the daemon build |

Card bodies go to the OS notification center (fallback only) and to the log files. Do not put secrets in `--body`.

## Layout

| Path | What |
|---|---|
| `crates/thumper-core` | Pure decisions: wire types, hook mapping, labels, the daemon state machine, the focus plan. No IO and no clock. |
| `crates/thumper` | The CLI. |
| `tools/thumper/daemon` | The daemon shell: socket server, actor, tmux and AeroSpace adapters, presence. Tauri-free. |
| `tools/thumper/app` | The Tauri app: the macOS panel and the platform services. |
| `tools/thumper/ui` | The card stack UI (TypeScript on Bun). |
| `tools/thumper/claude-plugin` | The Claude Code plugin: hooks and skill. |
| `tools/thumper/scripts` | The installer. `install-plan.ts` decides, `install.ts` performs the IO. |

The core crate decides and the shells perform IO. Time arrives as data, so every rule in `update.rs` is testable with plain values.

## Test

```bash
cargo test -p thumper -p thumper-core          # from ~/crates
cargo test                                               # from tools/thumper (daemon and app)
bun test                                                 # from tools/thumper (installer)
bun run test && bun run typecheck                        # from tools/thumper/ui
```

The wire format has fixtures in `crates/thumper-core/tests/fixtures/`. A change to the wire must keep the `v1` fixtures decoding.
