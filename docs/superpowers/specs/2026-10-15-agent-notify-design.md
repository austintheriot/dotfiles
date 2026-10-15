# agent-notify: contextual, clickable notifications for coding agents

Date: 2026-10-15
Status: design approved in conversation, spec under review
Location: `~/tools/agent-notify/` (tracked in `config`)

## 1. Problem

Several Claude Code agents run at the same time, each one in a tmux pane inside Alacritty. Today one hook (`~/.claude/hooks/notify.sh`, wired to `Stop` only) sends `osascript display notification` with the title "Claude finished" and the body `basename $(pwd)`. That has four defects:

- The body names the directory only. In the `se` layout the directories are `1` to `7`, so the body tells the user nothing.
- Nothing is sent when an agent blocks on a permission prompt. That is the event the user most needs.
- A click opens Script Editor. It does not go to the agent.
- The hook works on macOS only.

## 2. Goals and non-goals

Goals:

1. Each notification says which agent sent it (worktree or repo, branch, session title) and, in one or two lines, what it finished or what it needs.
2. "Finished" (info) and "needs you" (attention) look different.
3. A click on a notification goes to the agent's tmux pane. Raising the Alacritty window is best effort.
4. Agents decide what to send. Loop-style skills such as `/monitor-ci` send one notification with the result, not one per iteration.
5. The system works with zero work-specific configuration. It works on a machine without the `se` layout.
6. Platform order: macOS arm64, then macOS Intel (best effort), then Linux (Pop!_OS 24.04 COSMIC, then Arch). Windows/WSL is out of scope for this spec.

Non-goals for v1:

- Tray icon, glance dropdown, monitor window. The design keeps room for them (section 9).
- Answering a permission prompt from the notification.
- Notifications across machines or to a phone.
- Agents other than Claude Code. The wire contract carries an `agent` field so Codex can be added later.

## 3. Research summary

Research done on 2026-10-15. The facts that shaped the design:

- No existing tool covers macOS, Linux and tmux pane targeting together. The closest, claude-notifications-go (GPL-3.0), does not support WSL, does not switch tmux panes on Linux, and does not mention COSMIC or AeroSpace.
- terminal-notifier `-execute` click actions work on this machine (macOS 26.6.2, terminal-notifier 2.0.0). Tested by hand on 2026-10-15. Version 3.x uses the current UserNotifications API.
- The Claude Code `Stop` hook carries `last_assistant_message`. `SessionStart` carries `session_title`. `Notification` carries `notification_type` and `message`. Hooks can run with `"async": true`.
- A Claude Code plugin can ship hooks but cannot ship permission allow rules. A local-path marketplace loads a plugin in place, with no cache copy.
- An existing Electron desktop app that the user works on draws its own toast window (frameless, transparent, non-focusable, non-activating panel on macOS) instead of using OS notifications. This spec uses the same pattern.
- Alacritty does not support OSC 9/777/99 notifications (alacritty/alacritty#7105).
- COSMIC's notification server supports actions on the popup only. COSMIC has no official CLI to focus another app's window. Wayland does not let an ordinary window place itself in a screen corner or stay on top. That needs the layer-shell protocol.
- tauri-plugin-notification 2.6.0 (2026-10-10) added desktop click events. This spec does not depend on it.

## 4. Architecture

```
~/tools/agent-notify/              own Cargo workspace and lockfile
  core/        agent-notify-core   pure: types, policy, label, focus plan. No IO.
  cli/         agent-notify        hook handler and explicit commands. No Tauri dependency.
  app/         agent-notifyd       Tauri v2 app: resident daemon and toast windows
    ui/                            Lit components, TypeScript, built with bun
  claude-plugin/                   Claude Code plugin: hooks and agent guidance
  marketplace.json                 local-path marketplace that lists the plugin
```

Placement: `tools/` and not `crates/`. The `crates/` workspace forbids `unsafe` and builds every member on every `config build`, including CI. The Tauri app needs `unsafe` for the macOS panel code, a frontend build, and several hundred dependencies. If a part of the core later proves useful to `crates/`, it moves then.

Processes:

- `agent-notifyd` runs resident with no Dock icon. A launchd agent (macOS) or a systemd user unit (Linux) keeps it running. It owns per-pane state, toast windows and click handling.
- `agent-notify` is a short-lived CLI. Hooks and agents run it. It sends one message over a Unix socket and exits.
- Both binaries depend on `agent-notify-core`. Every decision is a pure function in the core. The binaries do IO only.

### 4.1 Dependency injection

One composition root per process builds a `Services` record and passes it down explicitly. No code fetches a service from a global.

- Rust: `main` builds `Services { clock, tmux, raiser, presenter, log }`, each a trait object. The core never receives a service. It receives plain data and returns a plan.
- UI: `bootstrap()` builds the UI services (the Tauri bridge) and passes them to the root Lit component.

An interface exists only where more than one real implementation exists, or where a test seam is at a real boundary. Examples: `WindowRaiser` (AeroSpace, COSMIC, X11, none), `ToastPresenter` (custom window, D-Bus fallback).

### 4.2 UI architecture

The core is the Elm `update`: `(State, Event) -> (State, Vec<Effect>)`. The UI is the Elm `view`. The daemon pushes a typed `ToastViewModel` list to the UI, and the UI sends typed intents back (`Clicked { card_id }`, `Closed { card_id }`, `HoverChanged { card_id, hovered }`). Toasts need no client-side store. Redux Toolkit is added only if a later view needs real local state.

Type generation: `tauri-specta` generates TypeScript types for commands and events from the Rust types. `ts-rs` is the fallback. Spike 1 checks the status of `tauri-specta` for Tauri v2.

## 5. Event contract

### 5.1 CLI commands

| Command | Caller | Effect |
|---|---|---|
| `agent-notify hook` | Claude Code hooks: `SessionStart`, `UserPromptSubmit`, `Stop`, `Notification`, `SessionEnd` | Reads the hook JSON on stdin, sends the mapped event |
| `agent-notify send --kind info\|attention --body <text> [--title <text>]` | an agent, through Bash | Sends an explicit notification |
| `agent-notify quiet --until sent\|prompt` | an agent or a skill | Silences the `Stop` backup until the next explicit send or the next user prompt |
| `agent-notify focused --pane <id>` | tmux `pane-focus-in` hook | Clears pending cards for that pane |
| `agent-notify dnd on\|off` | the user | Turns do-not-disturb on or off |

Exit status: `hook`, `send`, `quiet` and `focused` always exit 0, so they never fail a turn. Errors go to the log file.

### 5.2 Identity

The join key is the tmux location: the tmux server socket path plus the pane id. Pane ids are unique only within one tmux server. A command that an agent runs through Bash has no documented way to know its Claude `session_id`, but it inherits `$TMUX` and `$TMUX_PANE`. Hook events carry both the pane and the session id, so the daemon learns the pane-to-session mapping from them. Spike 1 confirms that `$TMUX_PANE` reaches both hooks and Bash commands.

On `SessionStart`, the CLI sets a pane option `@agent_notify_id` to the session id (`tmux set -p -t $TMUX_PANE`). The focus plan checks this tag before it switches (section 7).

Outside tmux, `tmux` is `None`. The notification still shows. A click raises the terminal app only.

### 5.3 Wire format

One JSON object per line over a Unix socket with mode `0600`. Socket path: `$XDG_RUNTIME_DIR/agent-notify.sock` on Linux, `$TMPDIR/agent-notify.sock` on macOS. The types live in `core`. The CLI and the daemon share them.

```rust
struct Envelope { v: u32, sent_at: Timestamp, origin: Origin, event: Event }

struct Origin {
    agent: AgentKind,                // ClaudeCode
    session_id: Option<SessionId>,   // hook events only
    tmux: Option<TmuxLocation>,      // { socket_path, pane_id, window_name }
    cwd: PathBuf,
    repo: Option<RepoInfo>,          // { root_name, branch }
}

enum Event {
    SessionStarted { title: Option<String> },
    TurnStarted,
    TurnEnded { last_message: Option<String> },
    NeedsInput { reason: InputReason, message: String },   // Permission | Elicitation
    Explicit { kind: NotifyKind, title: Option<String>, body: String },
    Quiet { until: QuietUntil },                           // NextSend | NextPrompt
    PaneFocused,
    SessionEnded,
}
```

`v` starts at 1. The daemon rejects an unknown `v` and logs the raw line. The CLI caps every free-text field at 4 KB before it sends.

### 5.4 Label

The core builds the label from generic data:

- Line 1: the tmux window name if one exists, else `repo.root_name`, then ` · `, then the branch. Example: `4 · austin/fix-sync-retry`.
- Line 2: the session title, if known.
- Line 3: the body. For `TurnEnded` it is the first sentence of `last_message`. For `NeedsInput` it is the prompt message. For `Explicit` it is the agent's body.

Optional label rules (for example, nicer names for numbered worktrees) can come from a user config file. Work-specific rules live in `config-gl`, never in `config`.

### 5.5 Hook wiring

The plugin in `tools/agent-notify/claude-plugin/` declares the hooks. Each hook calls `$HOME/.local/bin/agent-notify hook` by absolute path, with `"async": true`. The plugin also ships a skill that tells agents when to call `send` and `quiet`.

User settings need three entries, added by hand until the home and work settings files are tracked:

- `extraKnownMarketplaces`: the local path `~/tools/agent-notify`.
- `enabledPlugins`: the plugin.
- `permissions.allow`: `Bash(agent-notify:*)`. A plugin cannot ship this rule.

`/monitor-ci` gets one instruction: call `agent-notify quiet --until sent` at the start, and `agent-notify send` with the result. The old `notify.sh` `Stop` hook is removed when the plugin goes live.

## 6. Notification policy

All rules are pure functions in the core.

Per-pane record: `state` (`Working`, `Finished`, `NeedsYou`), `card` (optional), `quiet` (optional `QuietUntil`), `sent_this_turn` (bool).

| Event | Rule |
|---|---|
| `TurnStarted` | Clear the pane's card. Set `sent_this_turn = false`. Clear `quiet` if it is `NextPrompt`. State `Working`. |
| `Explicit` | Show a card with the agent's kind. Set `sent_this_turn = true`. Clear `quiet` if it is `NextSend`. |
| `TurnEnded` | If `sent_this_turn` is false and `quiet` is not set, show an info card with the first sentence of `last_message`. Else show nothing. State `Finished`. |
| `NeedsInput` (permission, elicitation) | Show an attention card with the prompt message. State `NeedsYou`. |
| `Notification` with `idle_prompt` | Ignored. The CLI does not send it. It fires 60 s after every finished turn and would turn every info card into noise. |
| `PaneFocused`, card click, close | Clear the card. |
| `SessionEnded` | Clear the card. Drop the record. |

Coalescing: one card per pane at most. Attention outranks info. A new info event does not replace a pending attention card. A new attention event replaces an info card. Within one rank, the newer card replaces the older one.

Suppression: when Alacritty is the frontmost app and the pane is the active pane of the window that its client shows, no card is shown. The daemon reads the frontmost app and tmux state into a `FocusSnapshot`, and the core decides.

Do-not-disturb: custom windows ignore macOS Focus modes, so the system has its own switch. While it is on, no cards show and records still update. When it turns off, one summary card shows the count of panes in `NeedsYou` and `Finished`.

## 7. Focus plan

A click produces a `FocusPlan` from the core. The daemon runs it.

```rust
struct FocusPlan { verify: VerifyTag, tmux: TmuxStep, raise: RaiseStep }
```

1. Verify. On the card's tmux server, check that the target pane carries `@agent_notify_id` equal to the card's session id. If it does not, search all panes for the tag. If no pane has it, the card shows "agent no longer running" and nothing else happens.
2. Switch tmux. The daemon is not a tmux client, so it chooses one:
   1. A client attached to the target session: `select-window`, then `select-pane`.
   2. Else the most recently active client: `switch-client -c <tty> -t <target>`.
   3. Else no client exists: start Alacritty with `tmux attach -t <session>`, then select the pane.
3. Raise the window, best effort:
   - macOS with AeroSpace: find the window with `aerospace list-windows`, then `aerospace focus --window-id <id>`. Without AeroSpace: activate the Alacritty app.
   - COSMIC: activate the toplevel through `cosmic-protocols`.
   - X11: `xdotool windowactivate`.
   - Other: no raise.
4. Clear the card.

More than one Alacritty window: tmux `set-titles on` with a `set-titles-string` that contains the client tty. The raise step matches the window title. With one window, no matching is needed. The title setting goes in the tracked `tmux.conf`.

A raise failure goes to the log only. A tmux failure shows on the card.

## 8. Toast UI

Style B, approved from mockups:

- Info card: translucent neutral card, blue dot.
- Attention card: amber-brown fill, amber border, amber dot.
- Content: line 1 label with relative time on the right, line 2 session title, line 3 body cut to 2 lines with an ellipsis.
- No buttons. A click anywhere on the card, except the close button, runs the focus plan. The card brightens a little on hover.
- Close button: a round X over the card's top-left corner. It fades in on hover in 0.15 s, the same as the reference Electron toast.
- Light and dark appearance follow the system setting.
- Position: top-right of the display that shows Alacritty, 16 px from the edges. Newest card on top. At most 4 cards. A fifth card collapses the oldest info cards into a "+N more" row.
- Motion: slide in from the right in 150 ms. Info cards fade out after 6 s. Hover pauses the timer. Attention cards stay until clicked, closed, or cleared by the policy.
- Sound: one short system sound when an attention card appears. Info cards are silent.

Window properties, per platform:

- macOS: non-activating panel (`tauri-nspanel` or equivalent `objc2` code), never focused, always on top, visible on all Spaces and over fullscreen apps. AeroSpace gets an `on-window-detected` float rule for the app's bundle id in the tracked `aerospace.toml`.
- Linux X11: undecorated, always on top, skip taskbar, no focus.
- Linux Wayland: layer-shell surface if the Tauri window can use it (spike 2). Otherwise D-Bus notifications with the same text, and urgency `critical` for attention.

## 9. Failure handling

| Failure | Behavior |
|---|---|
| Daemon not running | The CLI sends a plain OS notification (terminal-notifier on macOS, notify-send on Linux) with the same label and body, then exits 0. |
| Socket connect or write slower than 300 ms | Same fallback. |
| Unknown `v` or bad message | The daemon rejects and logs it. The CLI falls back. |
| Daemon restart | State is in memory and rebuilds from new events. Pending cards are lost. Accepted for v1. |
| Focus plan step fails | Section 7. |

Logs: rotating files in `~/Library/Logs/agent-notify/` (macOS) or `$XDG_STATE_HOME/agent-notify/` (Linux).

Room for later views: the daemon state already holds every pane's record, last message and timestamps. A tray icon, glance list, or monitor window reads the same state through new view models.

## 10. Testing

- Core: table tests for every rule in section 6. Property tests: at most one card per pane for any event sequence, attention never replaced by info, `TurnStarted` always clears the pane's card, a focus plan never targets a pane whose tag does not match.
- Wire contract: round-trip tests for every `Envelope` variant, and golden JSON fixtures, so a renamed field fails a test.
- CLI: fixture tests from captured hook payloads (`SessionStart`, `UserPromptSubmit`, `Stop`, `Notification`, `SessionEnd`) to the expected `Envelope`.
- Adapters: integration tests against real systems. tmux on a throwaway server and socket. AeroSpace on macOS, run locally. D-Bus on a session bus.
- UI: view tests for Lit components from typed view-model fixtures. No end-to-end GUI tests in v1.
- TDD: red tests first for each core rule.

## 11. Build and install

- A build script in `tools/agent-notify/` builds the CLI and the app. A new opt-in `config build --tools` (or a separate `config` subcommand, decided in the plan) runs it. Plain `config build` does not build `tools/`.
- Install: the CLI to `~/.local/bin`, the app bundle to `~/Applications` (macOS), the launchd plist or the systemd user unit.
- Everything above is tracked in `config`. Only work-specific label rules or skill text go in `config-gl`.

## 12. Delivery order

Each spike has a kill condition. Work after a spike starts only when the spike passes.

1. Spike 1, macOS arm64. A Tauri non-activating panel that does not take focus from Alacritty, is not tiled by AeroSpace, shows over fullscreen apps, and runs `tmux select-pane` on click. Also confirm `$TMUX_PANE` in hooks and Bash commands, and the `tauri-specta` status. Kill condition: no non-activating panel is possible, so macOS uses OS notifications through terminal-notifier 3.x.
2. v1 macOS: core, CLI, daemon, plugin, `/monitor-ci` change, launchd. Remove `notify.sh`.
3. Intel Mac: build and run on the home machine. Best effort.
4. Spike 2, COSMIC: layer-shell toast and toplevel activation. Kill condition: COSMIC uses the D-Bus fallback and keeps the tmux switch.
5. v1 Linux: systemd unit and Linux adapters. Arch gets the D-Bus path and `xdotool`.
6. Later, separate specs: tray and glance, monitor window, Windows/WSL, Codex.

## 13. Open questions

- Does Claude Code fire `UserPromptSubmit` for turns that a scheduled wakeup or a background task starts? If it does, `quiet --until prompt` ends early in loops. `quiet --until sent` does not have this problem, so `/monitor-ci` uses it.
- Is a pane-id tag enough when one pane hosts several Claude sessions one after another? The tag is rewritten on each `SessionStart`, so the newest session owns the pane. Older cards then fail verification, by design.
