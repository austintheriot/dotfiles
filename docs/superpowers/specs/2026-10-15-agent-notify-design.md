# agent-notify: contextual, clickable notifications for coding agents

Date: 2026-10-15
Status: revision 2, after an 11-lens expert review. Approved 2026-10-15.
Location: `~/crates/agent-notify-core`, `~/crates/agent-notify` (CLI), `~/tools/agent-notify/` (app). All tracked in `config`.

## 1. Problem

Several Claude Code agents run at the same time, each one in a tmux pane inside Alacritty. Today one hook (`~/.claude/hooks/notify.sh`, wired to `Stop` only) sends `osascript display notification` with the title "Claude finished" and the body `basename $(pwd)`. That has four defects:

- The body names the directory only. In the numbered-worktree layout the directories are `1` to `7`, so the body tells the user nothing.
- Nothing is sent when an agent blocks on a permission prompt or fails on an API error. Those are the events the user most needs.
- A click opens Script Editor. It does not go to the agent.
- The hook works on macOS only.

## 2. Goals and non-goals

Goals:

1. Each notification says which agent sent it (window, repo, branch, session title) and, in one or two lines, what it finished or what it needs.
2. "Finished" (info) and "needs you" (attention) look different.
3. A click on a notification goes to the agent's tmux pane. Raising the Alacritty window is best effort.
4. Agents decide what to send. Loop-style skills such as `/monitor-ci` send one notification with the result, not one per iteration.
5. The system works with zero work-specific configuration, and on a machine without the numbered-worktree layout.
6. Platform order: macOS arm64, then macOS Intel (best effort), then Linux (Pop!_OS 24.04 COSMIC, then Arch). Windows/WSL is out of scope.
7. When a notification does not appear, or a click goes to the wrong place, the user can find out why.

Non-goals for v1:

- A tray icon, glance dropdown, or monitor window. The tmux status line gives a text glance (section 8.6).
- Answering a permission prompt from the notification.
- Notifications across machines or to a phone.
- Agents other than Claude Code. The wire contract has an `agent` field so Codex can be added.
- Several Alacritty windows. v1 assumes one Alacritty window (section 7.3).

## 3. Research summary

Research done on 2026-10-15:

- No existing tool covers macOS, Linux and tmux pane targeting together. The closest, claude-notifications-go (GPL-3.0), does not switch tmux panes on Linux and does not mention COSMIC or AeroSpace.
- terminal-notifier `-execute` click actions work on this machine (macOS 26.6.2, terminal-notifier 2.0.0), tested by hand. Version 3.x uses the UserNotifications API, follows Focus modes, and removes `-sender` and `-ignoreDnD`.
- Claude Code hooks: every payload carries `session_id` and `prompt_id`. `Stop` carries `last_assistant_message` and does not fire on a user interrupt. `StopFailure` fires on API errors. `SessionStart` carries `session_title` and `source` (`startup`, `resume`, `clear`, `compact`, `fork`). `Notification` carries `notification_type` and `message`. Hooks can run with `"async": true`. A `Stop` hook that exits 2 blocks the stop.
- A Claude Code plugin can ship hooks but cannot ship permission allow rules. A local-path marketplace loads a plugin in place.
- An existing Electron desktop app draws its own toast window (frameless, transparent, non-focusable, non-activating panel on macOS). This spec uses the same pattern.
- Alacritty does not support OSC 9/777/99 notifications.
- COSMIC's notification server supports actions on the popup only. COSMIC has no official CLI to focus another app's window. Wayland does not let an ordinary window place itself or stay on top. That needs layer-shell.
- AeroSpace hides windows of inactive workspaces by moving them off screen. Sticky windows are not supported. A floating window belongs to one workspace.
- tmux (3.4, this machine): `#{pid}` and `#{start_time}` identify a server. `pane-focus-in`, `pane-focus-out` and `pane-exited` hooks exist. `focus-events` is `on`. Window names are already `repo/branch` (set by `tmux-tools name-windows`), with `automatic-rename` off.
- On this Mac, `launchctl getenv TMPDIR` equals the shell's `$TMPDIR`.
- tauri-specta for Tauri v2 is still `2.0.0-rc.21`.

## 4. Architecture

```
~/crates/agent-notify-core/    pure: types, policy, label, focus plan, view. No IO.
~/crates/agent-notify/         CLI binary. Hooks and agents call it. No Tauri.
~/tools/agent-notify/          own Cargo workspace and lockfile
  app/        agent-notifyd    Tauri v2 app: resident daemon and the toast panel
    ui/                        Lit components, TypeScript, built with bun
  claude-plugin/               Claude Code plugin: hooks and agent guidance skill
  marketplace.json             local-path marketplace that lists the plugin
```

Placement:

- The core and the CLI live in `crates/`. They need no `unsafe` and few dependencies. In `crates/` they get `config build`, binary stamping, `config doctor`, the pre-push test gates, and the shared lint policy.
- Only the Tauri app lives in `tools/`. It needs `unsafe` (macOS panel code), a frontend build, and several hundred dependencies. It depends on `crates/agent-notify-core` by path.
- The CLI reuses the git inspector in `crates/tmux-tools/src/repo.rs` (main repository name for linked worktrees, branch or detached sha). That module moves to a library target so both binaries can use it.
- The throwaway-tmux-server test harness moves from `crates/tmux-tools/tests/support/mod.rs` to `crates/dotfiles-test-support`. agent-notify is its third user.

Processes:

- `agent-notifyd` runs resident with no Dock icon. A launchd agent (macOS) or a systemd user unit (Linux) keeps it running. It owns all decision state, the toast panel and click handling.
- `agent-notify` is a short-lived CLI. It sends one request over a Unix socket, reads one reply, and exits. It also reads and writes two tmux pane options (section 5.2). That is the only state the CLI writes.

### 4.1 Dependency injection

One composition root per process builds a `Services` record and passes it down. No code fetches a service from a global.

- Daemon: `main` builds `Services { tmux, raiser, presenter, frontmost, idle, sound, log }`. CLI: `main` builds `Services { tmux, socket, git, fallback, log }`.
- A trait exists only where more than one real implementation exists: `WindowRaiser` (AeroSpace, macOS app activation, COSMIC, X11, none), `ToastPresenter` (custom panel, D-Bus), `FrontmostReader` (AeroSpace, X11, COSMIC, unknown), `FallbackNotifier` (terminal-notifier, notify-send). Single-implementation services are concrete types.
- The tmux adapter is stateless. Every call takes the server socket path, because tmux servers start and stop while the daemon runs.
- The core never receives a service. It receives plain data, including the current time, and returns data.

### 4.2 Daemon runtime

- One actor task on the Tauri async runtime owns `DaemonState`. It reads one queue of `DaemonInput` (socket requests, UI intents, ticks, snapshots, focus outcomes), calls the core, and runs the effects.
- Effects that touch the panel run on the main thread. Subprocess calls (tmux, aerospace) run on a blocking thread pool. Tauri commands are `async` and only enqueue. No lock is held across an `await`.
- A 250 ms tick drives deadlines (fade, dwell, quiet expiry). A 60 s tick triggers a pane-liveness snapshot.
- The daemon holds an App Nap activity assertion on macOS, so socket wakeups stay fast after hours idle.
- Single instance: at startup the daemon connects to the socket first. If a daemon answers, the new process exits. If the connect is refused, it removes the stale file and binds.

### 4.3 UI architecture

The core is the Elm `update`; a pure `view(&DaemonState, now) -> PanelViewModel` is the Elm `view`. The UI holds no store.

- The daemon pushes `PanelViewModel { revision, cards, finished_row, dnd }` after every update.
- The UI calls `get_view_model` in `bootstrap()` after it subscribes, and drops any push with a lower `revision`. Tauri events are not buffered, so this covers a slow first load and a WebContent process reload.
- UI intents: `Clicked { card_id }`, `Closed { card_id }`, `HoverChanged { card_id, hovered }`, `LayoutChanged { content_height_px }`, `UiError { message, stack }`. Intents carry only ids. The daemon resolves a card to a session from its own state and never accepts a tmux target from the UI.
- `card_id` is a string, fresh for every card instance. An intent for an unknown or replaced `card_id` does nothing.
- No 64-bit integers cross the bridge. Times are epoch milliseconds as JSON numbers.
- Types for the bridge: Rust types with `ts-rs`, or hand-written TypeScript types, plus a bun test that type-checks the golden JSON fixtures (section 10) against them. tauri-specta is not used while it has no stable release.

## 5. Contracts

### 5.1 CLI commands

| Command | Caller | Effect |
|---|---|---|
| `agent-notify hook claude-code` | Claude Code hooks | Reads the hook JSON on stdin, sends the mapped event |
| `agent-notify send --kind info\|attention --body <text>` | an agent, through Bash | Sends an explicit notification |
| `agent-notify quiet --until next-send\|next-prompt [--max-minutes <n>]` | an agent or a skill | Silences the `Stop` backup. Default `--max-minutes 120`. |
| `agent-notify quiet --off` | an agent or a skill | Ends quiet |
| `agent-notify focus-changed --socket <path> --pane <id> --in\|--out` | tmux `pane-focus-in` / `pane-focus-out` hooks | Reports pane focus |
| `agent-notify dnd on [--for-minutes <n>]\|off\|status` | the user | Do-not-disturb |
| `agent-notify jump` | the user, from a tmux key binding | Runs the focus plan for the newest attention card |
| `agent-notify status [--json\|--tmux]` | the user, the tmux status line | Prints daemon state (section 9.2) |
| `agent-notify doctor` | the user | Checks the installation (section 9.2) |

Rules:

- `send` and `quiet` take the pane only from `$TMUX` and `$TMUX_PANE`, never from a flag. One agent cannot quiet or impersonate another pane.
- `send` has no `--title` in v1. The label comes from the origin, never from agent text.
- Exit status of `hook claude-code`: always 0. A `Stop` hook that exits 2 blocks the stop and feeds stderr to the model, so a failure here must never become a non-zero exit. `main` catches panics, logs them, and exits 0.
- Exit status of `send` and `quiet`: 2 with one stderr line on a usage error, so the agent sees its mistake in Bash output. 0 on a delivery problem, with one stderr line when the fallback was used.
- `hook claude-code --dry-run` prints the parsed request and sends nothing.

Permission rule in user settings: `Bash(agent-notify send:*)` and `Bash(agent-notify quiet:*)`. No other subcommand is allowed without a prompt, so an agent cannot turn on do-not-disturb or clear another pane's card. Spike 1 confirms how Claude Code's prefix matcher treats `$(...)` inside an argument.

### 5.2 Identity

A record belongs to a Claude session. The tmux pane is where the session lives, not who it is.

- `SessionId`: the Claude `session_id`.
- `PromptId`: the Claude `prompt_id`. It identifies a turn.
- `PaneKey { server: ServerId, pane: PaneId }`, with `ServerId { socket_path, pid, start_time }`. Pane ids restart at `%0` on a new tmux server with the same socket path, so the pid and start time are part of the key.
- `PaneId` is a newtype that must match `^%[0-9]+$`.

Two pane options are the system of record for "which session and which turn live in this pane":

- `@agent_notify_session`: written by the CLI on `SessionStart`.
- `@agent_notify_prompt`: written by the CLI on `UserPromptSubmit`.

The `SessionStart` and `UserPromptSubmit` hooks run synchronously (no `async`). Each does one `tmux set -p` and one socket request, so it is cheap, and the tag is set before the agent runs any Bash command. Every other hook is `async`.

A Bash `send` or `quiet` reads both options from `$TMUX_PANE` and fills `session_id` and `prompt_id`. So every agent request carries a session id, also after a daemon restart.

On `SessionEnd`, the CLI clears `@agent_notify_session` only when its value equals the ending session id. A reordered `/clear` cannot erase the new session's tag.

Outside tmux: hook events still carry `session_id` and produce cards. A click raises the terminal app only. `send` from Bash has no session id, so it produces a one-off card with no policy state. `quiet` outside tmux is not supported and exits 2 with a message.

### 5.3 Wire protocol

Request and reply over a Unix socket. One JSON object per line in each direction.

Socket path, resolved by one core function:

1. `$AGENT_NOTIFY_SOCKET` if set.
2. macOS: `~/Library/Application Support/agent-notify/agent-notify.sock`.
3. Linux: `$XDG_RUNTIME_DIR/agent-notify/agent-notify.sock`. If `XDG_RUNTIME_DIR` is unset, there is no socket: the CLI uses the fallback, and `doctor` reports it.

The directory has mode 0700. The daemon sets umask 0077 before `bind`, checks the peer uid on accept, and refuses a line longer than 16 KB.

Request:

```rust
#[serde(tag = "type", rename_all = "snake_case")]
enum Request {
    Agent   { v: u16, event_id: EventId, sent_at_ms: i64, origin: Origin, event: AgentEvent },
    Control { v: u16, event_id: EventId, sent_at_ms: i64, control: Control },
    Query   { v: u16, event_id: EventId, query: Query },
}

struct Origin {
    agent: AgentKind,                     // claude_code
    session: Option<SessionId>,           // None only for `send` outside tmux
    prompt: Option<PromptId>,
    pane: Option<PaneKey>,
    window: Option<WindowLabel>,          // { index, name }
    repo: Option<RepoInfo>,               // { main_repo_name, head: Branch(name) | Detached(short_sha) }
    session_title: Option<DisplayText>,   // latest known title; latest wins
    cwd: PathBuf,
}

enum AgentEvent {
    SessionStarted { source: SessionSource },        // startup | resume | clear | compact | fork
    TurnStarted,
    TurnEnded { last_message: Option<DisplayText> },
    TurnFailed { error: DisplayText },               // StopFailure
    NeedsInput { reason: InputReason, message: DisplayText },  // permission | elicitation
    InputResolved,                                   // elicitation_complete
    Explicit { kind: Kind, body: DisplayText },
    Quiet { until: QuietUntil, max_minutes: u16 },
    QuietOff,
    SessionEnded,
    #[serde(other)] Unknown,
}

enum Control { FocusChanged { pane: PaneKey, focused: bool }, Dnd(DndCommand), #[serde(other)] Unknown }
enum Query { Status, ViewModel, #[serde(other)] Unknown }
```

Reply: `{"ok":true,"daemon":"<build stamp>"}` or `{"ok":false,"code":"<code>","accepts":[<min_v>,<max_v>]}`. A `Query` reply carries its data.

Evolution policy:

- Adding an optional field does not change `v`. The daemon ignores unknown fields.
- An unknown variant decodes to `Unknown`. The daemon replies `ok:false`, code `unknown_kind`, and logs it.
- Removing or renaming a field or variant, or changing a meaning, increments `v`. The daemon accepts a range of versions.
- The golden fixtures (section 10) pin the JSON shape. Fixtures are append-only.

`DisplayText` is a type, not a convention. It is built once at the CLI edge and checked again by the daemon on decode: capped at 4 KB on a character boundary, C0/C1 controls, bidi controls and zero-width characters removed, and never interpreted as markup anywhere.

`EventId` is a ULID made by the CLI. It appears in every log line, in the reply, and on the card it produces.

Delivery: the CLI has 300 ms for connect, write and reply together. It uses the fallback (section 9.1) when the connect fails, when no reply arrives in time, or when the reply is `ok:false`. A daemon that accepts after the deadline makes a duplicate. That is the accepted failure, not a silent drop.

### 5.4 Hook mapping

| Hook | Mode | Request |
|---|---|---|
| `SessionStart` | sync | write `@agent_notify_session`, then `SessionStarted { source }` |
| `UserPromptSubmit` | sync | write `@agent_notify_prompt`, then `TurnStarted` |
| `Stop` | async | `TurnEnded { last_message }` |
| `StopFailure` | async | `TurnFailed { error }` |
| `Notification` with `permission_prompt` | async | `NeedsInput { reason: permission }` |
| `Notification` with `elicitation_dialog` or `elicitation_url_dialog` | async | `NeedsInput { reason: elicitation }` |
| `Notification` with `elicitation_complete` | async | `InputResolved` |
| `Notification` with any other type, including `idle_prompt` | async | nothing. The raw type is logged. |
| `SessionEnd` | async | clear the tag if it matches, then `SessionEnded` |
| unknown hook event | async | nothing. The raw payload is logged. |

`Notification(permission_prompt)` is used, not `PermissionRequest`. `permission_prompt` fires about 6 s after the prompt shows, and only if the user has not answered. That delay is a useful debounce when the user is watching. The prompt-id check in section 6 drops a late `NeedsInput` for a turn that already ended.

### 5.5 Label

The core builds the label from origin data only:

- Line 1, inside tmux: `<window index> · <window name>`, for example `4 · code/at/url-import`. The window name already holds `repo/branch` in the numbered layout, and the optional `@wname_label` window option already names worktrees. Outside tmux: `<main repo name> · <branch>`, or `<main repo name>` alone when there is no branch, or the `cwd` basename when there is no repo.
- Line 2: the latest session title, if known.
- Line 3: the body. `TurnEnded`: the first sentence of `last_message`, or "Finished" when it is empty. `TurnFailed`: "Stopped: " and the error. `NeedsInput`: the prompt message. `Explicit`: the agent's body.

`first_sentence` is a pure function with defined behavior for: empty or whitespace text, no terminal punctuation, abbreviations ("e.g."), version strings ("v1.2"), URLs, a leading code fence or list, and CJK `。`. Its result is a prefix of the trimmed input and is not empty when the input has non-space characters.

### 5.6 Plugin and settings

`tools/agent-notify/claude-plugin/` declares the hooks in section 5.4. Each hook calls `$HOME/.local/bin/agent-notify hook claude-code` by absolute path. The plugin also ships a skill that tells agents when to call `send` and `quiet`.

User settings need these entries, added by hand until the home and work settings files are tracked:

- `extraKnownMarketplaces`: the local path `~/tools/agent-notify`.
- `enabledPlugins`: the plugin.
- `permissions.allow`: `Bash(agent-notify send:*)`, `Bash(agent-notify quiet:*)`.

`/monitor-ci` gets two instructions: call `agent-notify quiet --until next-send --max-minutes 180` at the start, and `agent-notify send` with the result.

The tracked `tmux.conf` gets the `pane-focus-in`, `pane-focus-out` hooks, the `M-n` binding for `agent-notify jump`, and the status segment (section 8.6).

## 6. Decision core

All rules in this section are pure functions in `agent-notify-core`.

### 6.1 State

```rust
struct DaemonState {
    sessions: HashMap<SessionId, SessionRecord>,
    oneoff_cards: Vec<Card>,                 // `send` outside tmux
    dnd: DndState,                           // Off | On { until: Option<Instant> }
    focus: FocusState,                       // focused pane, since when
    revision: u64,
}

struct SessionRecord {
    pane: Option<PaneKey>,                   // latest location
    window: Option<WindowLabel>,
    title: Option<DisplayText>,
    turn: Turn,
    quiet: Quiet,                            // Armed | Quiet { until: QuietUntil, deadline: Instant }
    card: Option<Card>,
    snoozed_until: Option<Instant>,
}

enum Turn {
    Idle,
    Working { prompt: PromptId, explicit_sent: bool },
    Finished { prompt: PromptId },
    NeedsYou { prompt: PromptId, reason: InputReason },
    Failed { prompt: PromptId },
}

enum Kind { Info, Attention }                // derives Ord: Attention > Info

struct Card { id: CardId, kind: Kind, body: DisplayText, event_id: EventId, created: Instant,
              phase: CardPhase }             // Showing { fade_at: Option<Instant> } | Folded
```

`update(&mut DaemonState, DaemonInput, now: Instant) -> Vec<Effect>`. `DaemonInput` is `Request`, a UI intent, `Tick`, `FocusSnapshot`, `PaneSnapshot`, or `FocusOutcome`.

`Effect` holds only non-view work: `RunFocusPlan`, `PlaySound`, `SetPaneState`, `Log`, and `Suppressed { event_id, reason }`. Cards are not effects. The shell calls `view` after every update and pushes the result.

### 6.2 Rules

Events for a session that has no record create one (upsert). No event requires an earlier one.

| Event | Rule |
|---|---|
| `SessionStarted` | New session id in a pane: the pane's old record is dropped, so no state carries over. `source` `compact` or `resume` with the same id: keep the record, update pane and title. |
| `TurnStarted` (prompt P) | `turn = Working { P, explicit_sent: false }`. Clear the card. End `quiet` if it is `next-prompt`. |
| `Explicit` | Show a card of the agent's kind. If `turn` is `Working`, set `explicit_sent`. End `quiet` if it is `next-send`. |
| `TurnEnded` (prompt P) | If P is not the current prompt, drop it (stale). Else `turn = Finished { P }`. Show an info card only if `explicit_sent` is false and `quiet` is `Armed`. |
| `TurnFailed` (prompt P) | `turn = Failed { P }`. Show an attention card. Quiet does not apply. |
| `NeedsInput` (prompt P) | If the turn for P already ended, drop it (stale). Else `turn = NeedsYou`. Show an attention card. |
| `InputResolved` | Clear an elicitation attention card. `turn = Working`. |
| `Quiet` | `quiet = Quiet { until, deadline: now + max }`. |
| `QuietOff`, quiet deadline passed | `quiet = Armed`. |
| `SessionEnded` | Drop the record only if the id matches. |
| `FocusChanged` in | Hide the pane's card at once. After 1 s of continuous focus (dwell), clear it. |
| `FocusChanged` out before the dwell | Show the card again. Moving through windows does not clear cards. |
| `Clicked` | Return `RunFocusPlan`. Clear the card when the plan reports success. |
| `Closed` on info | Clear the card. |
| `Closed` on attention | Snooze for 10 minutes. The card shows again if the session is still `NeedsYou` or `Failed` for the same prompt. |
| `PaneSnapshot` | Drop records whose pane no longer exists or whose pane tag names another session. |

Every input that produces no card returns `Suppressed { reason }`, with reason `ExplicitSent`, `Quiet`, `Outranked`, `Focused`, `Dnd`, `Stale`, or `Snoozed`. A property test checks that every event yields a card or a `Suppressed` effect.

### 6.3 Coalescing

One card per session. `merge(existing: Option<Card>, incoming: Card) -> Card` keeps the higher `Kind`. On a tie it keeps the newer card. An info event never replaces an attention card.

### 6.4 Suppression

Input: `FocusSnapshot { frontmost: Frontmost, active_pane: Option<PaneKey>, idle_seconds: Option<u32>, screen_locked: Option<bool>, screen_shared: Option<bool> }`.

A new card is suppressed only when all of these hold: Alacritty is frontmost, the card's pane is the active pane of the client's current window, the user gave input in the last 30 s, and the screen is not locked. Any `Unknown` or `None` value means "show". On Linux without a frontmost reader, cards always show.

### 6.5 Do-not-disturb

- `dnd on` hides all cards and plays no sounds. Records still update.
- `--for-minutes` sets an end time. Without it, DND stays on until `dnd off`.
- DND state is saved to a small file and survives a daemon restart.
- When DND ends, the real attention cards show and info cards go to the finished row (section 8.3). There is no count-only summary card.
- When `screen_shared` is true, cards show with line 3 hidden.

### 6.6 Focus plan

Two phases. On a click, the shell gathers a `FocusWorld` snapshot for the card's tmux server: pane tags, panes with window and session, and clients with tty, session and last activity. The core decides:

```rust
enum FocusPlan {
    SessionGone,                                          // card says "Session ended", removed after 3 s
    RaiseOnly(RaiseTarget),                               // origin outside tmux
    Focus { pane: PaneKey, switch: TmuxSwitch, raise: RaiseTarget },
}
enum TmuxSwitch { InAttachedClient, SwitchClient { tty: ClientTty }, AttachNewTerminal }
```

- The target is the pane whose `@agent_notify_session` tag equals the card's session id. If no pane has it, the plan is `SessionGone`.
- `InAttachedClient`: a client is attached to the pane's session. `SwitchClient`: otherwise, the most recently active client. `AttachNewTerminal`: no client exists.
- The shell runs the plan with argv only, never `sh -c`. Every tmux call passes `-S <socket_path>` and targets `%pane_id`. Before it runs, the shell checks the tag once more, which closes the gap between snapshot and switch.
- `AttachNewTerminal`: select the pane first, then start Alacritty with `tmux -S <socket> attach -t <session>`.
- The shell returns `FocusOutcome { Done, TmuxFailed(reason), RaiseFailed(reason) }` as an input. `TmuxFailed` shows on the card. `RaiseFailed` keeps the card for 2 s with "Switched to 4, window not raised".

## 7. Platforms

### 7.1 macOS

- App bundle in `~/Applications` with `LSUIElement`, a fixed bundle id, and a launchd agent with `LimitLoadToSessionType = Aqua`, `KeepAlive`, `StandardErrorPath`, `RUST_BACKTRACE=1`, `LANG`, and a `PATH` that includes `/opt/homebrew/bin` (or `/usr/local/bin` on Intel). The install step resolves absolute tool paths and `doctor` checks them.
- If the user turns off the agent in Login Items ("Allow in Background"), launchd does not start it. `doctor` reports this.
- Toast panel: one non-activating `NSPanel` (through `tauri-nspanel` or `objc2`), created hidden at startup, hosting the whole stack. It is never key, `acceptFirstMouse` is on, it is visible on all Spaces and over fullscreen apps, and `sharingType = none`.
- AeroSpace must not adopt the panel. Spike 1 checks this with a workspace switch while a card shows. If AeroSpace adopts it, an `on-window-detected` float rule is not enough, because AeroSpace moves the windows of inactive workspaces off screen.
- Placement: the display with keyboard focus (AeroSpace's focused monitor, else the main screen), inside its visible frame, 16 px from the top-right, with that display's scale factor. Cards move when displays change.
- Raise: `aerospace list-windows --app-bundle-id org.alacritty`, then `aerospace focus --window-id`. Without AeroSpace: activate the Alacritty app. Spike 1 tests whether macOS 14+ cooperative activation allows this from a click on a non-activating panel.
- Frontmost: `aerospace list-windows --focused`. Idle and lock: HID idle time and the screen-lock notification.
- Signing: ad hoc for v1. A new ad hoc signature can reset system permission grants. v1 needs none with AeroSpace present, so this matters only for the path without AeroSpace. Minimum macOS version: 26. Both machines run the latest macOS, and macOS 26 is the last release for Intel Macs.

### 7.2 Linux

- systemd user unit with `PartOf=graphical-session.target`, `After=graphical-session.target`, `WantedBy=graphical-session.target`, `Restart=on-failure`. Window managers without a session manager must run `dbus-update-activation-environment --systemd DISPLAY WAYLAND_DISPLAY XDG_SESSION_TYPE`.
- The adapter is chosen by `XDG_SESSION_TYPE`, not by whether X11 is reachable.
- COSMIC (Wayland): a layer-shell surface if Tauri's GTK window can use it (spike 2). gtk-layer-shell must initialize before the window is realized. Otherwise: D-Bus notifications with a `default` action, body text XML-escaped, urgency `critical` for attention, and no sound of our own. The D-Bus waiter is the daemon itself, which stays on the bus.
- Arch on X11: the custom window with type hint `Notification` and `WM_HINTS.input = False`, focus through `xdotool windowactivate`. Arch on Wayland: D-Bus.
- Sound: the freedesktop sound theme through libcanberra.
- Logs: journald (`journalctl --user -u agent-notifyd`).

### 7.3 Several Alacritty windows

Out of scope for v1. With one window, the raise step needs no matching. A later version sets `set-titles-string` to a token built from `#{client_tty}` only, filters windows by Alacritty's bundle id or `WM_CLASS` first, then matches the exact token. The title format and the matcher must live in one place.

## 8. Toast UI

Style B, approved from mockups.

### 8.1 Cards

- Info card: translucent neutral card, blue dot. Attention card: amber-brown fill, amber border, amber dot.
- Line 1 label with relative time on the right, line 2 session title, line 3 body cut to 2 lines with an ellipsis.
- No buttons. A click anywhere except the close button runs the focus plan. The card brightens a little on hover and shows a pressed state within 50 ms of a click.
- Close button: a round X over the top-left corner. It fades in on hover in 0.15 s.
- Text renders only through Lit text bindings. No `unsafeHTML`, no markdown, no `innerHTML`.
- Light and dark appearance follow the system setting.

### 8.2 Stack

- Newest card on top. Attention cards never collapse. At most 3 info cards show.
- While the pointer is over the stack, the layout freezes. New and replaced cards wait until the pointer leaves.
- The panel resizes to its content through `LayoutChanged`, so empty panel area does not catch clicks meant for windows under it.

### 8.3 Finished row

- By default an info card stays until the user clicks it, closes it, focuses its pane, or its session sends a newer card. At most three info cards show. Older ones go to the finished row. The core also supports a fold timer (`InfoLifetime::FoldAfter`). With that timer on, hover pauses it, and a 300 ms grace period covers the hover message in transit. Changed 2026-10-16 at the user's request, after the smoke test.
- A folded card goes into one persistent row at the bottom of the stack: "Finished: 2, 4, 6". A click on a window number runs that session's focus plan. An entry leaves the row when its pane gets focus or a new turn starts.

### 8.4 Motion and sound

- Slide in from the right, 150 ms ease-out. Fade out 200 ms. The stack reflows in 150 ms. With Reduce Motion on, all motion becomes an opacity fade.
- One short system alert sound when an attention card appears. At most one sound per 3 s. No sound when an attention card replaces an attention card for the same session.

### 8.5 Keyboard

`agent-notify jump`, bound to `M-n` in the tracked `tmux.conf`, runs the focus plan for the newest attention card.

### 8.6 tmux status line

The daemon writes `@agent_notify_state` (`working`, `finished`, `needs-you`, `failed`) to each session's pane. The window status format shows a marker per window, and `#(agent-notify status --tmux)` shows the DND state. This is the v1 glance view. It also works over SSH, where cards cannot show.

## 9. Failure handling and diagnostics

### 9.1 Fallback

When the daemon does not acknowledge a request:

| Request | Fallback |
|---|---|
| `NeedsInput`, `TurnFailed`, `Explicit` | plain OS notification with the label and body, subtitle "agent-notify offline" |
| `TurnEnded` | none. The CLI cannot evaluate `explicit_sent` or `quiet`, so it logs the loss. |
| every other event | none. Logged. |

- macOS: terminal-notifier 3.1 with `-title`, `-subtitle`, `-message` as separate arguments and `-activate org.alacritty`. No `-execute`. Model text never goes into a shell string. Banners follow Focus modes, unlike cards.
- Linux: `notify-send` with escaped text and no action. It does not wait.
- terminal-notifier and libnotify are listed in `deps/deps-mac.toml` and `deps/deps-linux.toml`.

### 9.2 Diagnostics

- `agent-notify status [--json]`: daemon build stamp, uptime, DND state, each session record (pane, turn, quiet, card), and the last 50 decisions with their `Suppressed` reasons.
- `agent-notify doctor`: the CLI is on PATH and matches the daemon's build stamp, the socket answers, the plugin is enabled, the permission rules are present, `$TMUX_PANE` is set, the current pane has a session tag, `focus-events` is on, the fallback notifier exists, tool paths resolve from the daemon's environment, and (macOS) the launchd agent is allowed in Login Items.
- Logging: `tracing`, level from `AGENT_NOTIFY_LOG`. Every line has `event_id`, `session_id`, `pane`, the event type, and the decision. The CLI and the daemon write separate files. The CLI appends only and never rotates. The daemon rotates its own file on macOS and logs to journald on Linux. macOS log directory `~/Library/Logs/agent-notify/` (mode 0700). Model text in logs is escaped. A rejected line is logged as its error kind and length.
- `AGENT_NOTIFY_TRACE=1` logs the raw hook stdin at trace level.
- The daemon logs its version and build stamp at startup, and a panic hook writes panics with a backtrace to the log.
- UI errors (`window.onerror`, unhandled rejections) go to the daemon as `UiError` and into the log.

### 9.3 Restart

- The session records rebuild from new events. The pane tags survive in tmux, so a Bash `send` still carries its session id.
- Lost on restart, by design: pending cards and quiet modes. A loop that was quiet sends one backup notification per turn until its next `quiet` call. DND survives (section 6.5).
- Install restarts the daemon (`launchctl kickstart -k`, `systemctl --user restart`), so the CLI and the daemon do not drift apart after a build.

## 10. Testing

TDD: red tests first for each core rule. Test fixtures are synthetic or scrubbed by hand. Real captured hook payloads contain work content, and `config` is public. `tools/agent-notify/**/fixtures` and `crates/agent-notify*/**/fixtures` are added to the leak check.

Core:

- Exhaustive transition tests over the product of event, turn, card, quiet and `explicit_sent`, not only one test per table row.
- A model-based test: a small reference model of section 6.2 compared with `update` over generated event sequences.
- Properties: an attention card leaves only through attention, `TurnStarted`, focus dwell, click, close, or `SessionEnded`. `TurnEnded` yields an info card exactly when no `Explicit` and no `quiet` happened since the last `TurnStarted` and no attention card is pending. Quiet always ends by its deadline. Permuting the events inside one turn never yields a card for a stale prompt. Every input yields a card or a `Suppressed` effect.
- Focus plan: for any `FocusWorld`, the plan targets pane P only if P's tag equals the card's session id. Cases: tag on two panes, no tag, server gone.
- Suppression table, label table, `first_sentence` properties, `DisplayText` properties (valid UTF-8, at most 4 KB, a prefix of the cleaned input).
- Time enters only as the `now` argument.

Wire:

- Round-trip tests for every variant. Append-only golden JSON fixtures, decoded by every later daemon. Cases for `v` too low, `v` too high, missing `v`, unknown fields, unknown variants.
- A bun test type-checks the same fixtures against the TypeScript bridge types.

CLI:

- A pure `run(args, stdin, env, now) -> Outcome` with table tests: send, fallback, drop, usage error.
- Process tests that spawn the real binary: no socket file, stale socket file, a listener that never reads, a listener that replies `ok:false`, empty or non-JSON stdin, unknown hook event, unwritable log directory. Each asserts the exit status and whether the fallback fired (stub notifier on PATH).
- A property: for any stdin bytes and argv, `run` does not panic.
- Fixture tests from synthetic hook payloads for every row of section 5.4, including `idle_prompt` and an unknown `notification_type`.

Daemon:

- A headless integration test: real socket in, recorded `PanelViewModel` stream out. Cases: several requests on one connection, a line without a newline, non-UTF-8 bytes, an oversized line, concurrent clients, stale socket file, second instance.
- tmux adapter tests against a throwaway tmux server (shared harness). They assert tmux state (`#{pane_active}`, `#{client_session}`), never command strings.
- The `notify.sh` focus-suppression cases in `crates/config-cli/tests/notify.rs` move into the core suppression table.

UI: view tests for Lit components from typed view-model fixtures. No end-to-end GUI tests in v1.

## 11. Build and install

- `config build` builds the core and the CLI like every other crate.
- A new opt-in `config build --tools` builds the app: `cargo build` in `tools/agent-notify` with `--locked`, `bun install --frozen-lockfile` and the UI build, `cargo deny check`, then the bundle into `~/Applications`, the launchd plist or systemd unit, and a daemon restart.
- `Cargo.lock` and `bun.lock` are committed. Git dependencies are pinned by `rev`. The (tauri, tao, tauri-nspanel) versions are recorded together in spike 1. Bun lifecycle scripts stay blocked, with no `trustedDependencies` added without review.
- Everything is tracked in `config`. Only work-specific label rules or skill text go in `config-gl`.
- Removing `notify.sh` also removes `crates/config-cli/tests/notify.rs`, updates `crates/config-cli/tests/workflow_labels.rs:231` (the reason for the macOS CI runner), and rewrites the comment at `deps/deps-mac.toml:6-7` (AeroSpace stays a dependency).

## 12. Delivery order

Each spike is time-boxed to two working days. Each criterion names what a failure changes.

1. **Spike 1, macOS arm64.**
   - The panel shows while Alacritty stays frontmost and keeps receiving keystrokes. Failure: macOS uses OS notifications through terminal-notifier 3.1 (approach B).
   - `aerospace list-windows --all` does not list the panel, and a card stays visible across a workspace switch. Failure: try the accessory activation policy and panel subrole. If AeroSpace still adopts it, use approach B.
   - The card shows over a fullscreen app (manual screenshot). Failure: accepted, documented.
   - Hover fades the close button and the first click fires while Alacritty is key. Failure: close button always visible, and the click handled natively.
   - A click from the launchd-started daemon leaves the client's `#{pane_id}` equal to the target. Failure: fix the service environment before any other work.
   - App activation without AeroSpace works from a click. Failure: without AeroSpace, the raise step does nothing.
   - `$TMUX_PANE` is present in sync hooks, async hooks, and Bash calls. Failure: redesign section 5.2.
   - `sharingType = none` hides the panel from a screen share. Failure: rely on `screen_shared` detection and manual DND.
   - The prefix permission rule rejects `agent-notify send` with a `$(...)` argument, or the risk is documented.
   - Record the (tauri, tao, tauri-nspanel) versions.
2. **v1 macOS:** core, CLI, daemon, plugin, `/monitor-ci` change, tmux.conf additions, launchd. Remove `notify.sh` and its dependents.
3. **Intel Mac:** build and run on the home machine. Best effort.
4. **Spike 2, COSMIC.** Layer-shell panel. Toplevel activation of Alacritty from an ordinary client. `ActionInvoked` from a popup click on the real COSMIC server. Failure on the panel: D-Bus fallback. Failure on activation: tmux switch only.
5. **v1 Linux:** systemd unit and Linux adapters.
6. **Later, separate specs:** tray and glance window, monitor window, several Alacritty windows, an envelope journal with `replay`, Windows/WSL, Codex.

## 13. Decisions taken against a reviewer's suggestion

- `PermissionRequest` was suggested for faster attention cards. This spec keeps `Notification(permission_prompt)`, because its 6 s delay suppresses cards for prompts the user answers at once, and the prompt-id check handles the late-arrival race.
- The UI was suggested as the owner of the fade timer. This spec keeps deadlines in the core, so the timer is testable with an injected clock, and adds a 300 ms hover grace period.
- An envelope journal with `replay` was suggested. It is deferred to a later spec. `status` and the per-decision log cover v1 diagnosis.

## 14. Open questions

- Does cosmic-session start `graphical-session.target`? Spike 2 answers it.
