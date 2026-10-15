# agent-notify spikes 1 and 2: results

Date: 2026-10-15. Spec: `docs/superpowers/specs/2026-10-15-agent-notify-design.md` (section 12). The spike code was throwaway and is not kept. This report keeps the results and the decisions that follow from them. Plans 2 and 3 cite the section ids below.

## Test setups

- **macOS:** macOS 26.6.2 on arm64, AeroSpace, Alacritty, tmux 3.4. A throwaway Tauri app showed one card in a non-activating `NSPanel` and took commands on a Unix socket. A small Swift tool posted synthetic mouse events. `screencapture` took the screenshots.
- **Claude Code:** version 2.1.301. Each run used a throwaway tmux server, a hook logger that recorded field names and `$TMUX*` only, and `--setting-sources project` so that the user's own hooks did not run.
- **COSMIC:** Pop!_OS 24.04 arm64 (generic ISO, build 3) in a Tart VM, upgraded to the current packages: cosmic-comp built 2026-10, cosmic-session 1.10.0, cosmic-notifications 1.10.0, libgtk-layer-shell0 0.8.2. A VNC client drove the screen. The same Tauri app ran with gtk-layer-shell.
- **X11:** Xvfb with Openbox in the same VM, the same app with `GDK_BACKEND=x11`, and xdotool.
- **Versions used:** tauri 2.12.4, tauri-build 2.7.3, tao 0.37.1, wry 0.57.0, tauri-runtime-wry 2.12.2, tauri-nspanel 2.1.1 (crates.io), objc2 0.6.5, objc2-app-kit 0.3.2, gtk 0.18, gtk-layer-shell 0.8.2 (feature `v0_6`), wayland-client 0.31.15, wayland-protocols 0.32.13, cosmic-protocols 0.2.0.

## S1: macOS panel and raise

| Id | Criterion (spec section 12) | Result | Evidence |
|---|---|---|---|
| S1-1 | The panel shows while Alacritty stays frontmost | Pass | After `show`, AeroSpace's focused window is still `org.alacritty`. The panel is not key, and the app is not active. |
| S1-2 | AeroSpace does not adopt the panel | Pass | `aerospace list-windows --all` lists no spike window. |
| S1-3 | The card stays visible across a workspace switch | Pass | A switch from workspace `a` to an empty workspace kept the card on screen. |
| S1-4 | The card shows over a fullscreen app | Not run | Left for a manual check. The spec accepts a failure here. |
| S1-5 | Hover fades the close button | Fail with CSS, pass with native tracking | WebKit does not update `:hover` on mouse moves in a non-key panel of an inactive app. It updates hover only on a button event. A tauri-nspanel tracking area (`active_always`, `mouse_entered_and_exited`) that emits a `hover` event to the web view faded the close button in within about 155 ms. |
| S1-6 | The first click fires while Alacritty is key | Pass | With `accept_first_mouse(true)`, `mousedown` and `click` reached the page. Alacritty stayed frontmost. |
| S1-7 | A click from the launchd-started app switches the client to the target pane | Pass, with PATH | With launchd's default `PATH`, spawning `tmux` failed with ENOENT. With `/opt/homebrew/bin` in `PATH`, the client moved from `%0` to `%2`. |
| S1-8 | App activation without AeroSpace works from a click | Pass | `NSRunningApplication.activateWithOptions`, the same call after `yieldActivationToApplication`, and `open -b org.alacritty` all moved the front app from Finder to Alacritty. |
| S1-9 | `sharingType = none` hides the panel from a screen share | Fail | `screencapture` (ScreenCaptureKit) shows the panel with `sharingType` 0. |
| S1-10 | The (tauri, tao, tauri-nspanel) versions | Recorded | 2.12.4, 0.37.1, 2.1.1. |

Two findings fall outside the criteria:

- **S1-11, AeroSpace raise.** `aerospace focus --window-id` exits 0, but the front app stays Finder when AeroSpace already counts Alacritty as its focused window. So `aerospace focus` alone is not a raise.
- **S1-12, AeroSpace query syntax.** `list-windows --all` conflicts with `--app-bundle-id`. The correct form is `aerospace list-windows --monitor all --app-bundle-id org.alacritty --format '%{window-id}'`.
- **S1-13, Intel.** The macOS spike cross-compiles for `x86_64-apple-darwin`. It was not run, because no Intel Mac was available.

## S1-H: Claude Code hooks and permissions

| Id | Check | Result |
|---|---|---|
| S1-H1 | `$TMUX_PANE` in sync hooks | Present in SessionStart, UserPromptSubmit, PreToolUse and SessionEnd. |
| S1-H2 | `$TMUX_PANE` in async hooks | Present in Stop and Notification. |
| S1-H3 | `$TMUX_PANE` in Bash tool calls | Present (`printenv TMUX_PANE` printed `%0`). |
| S1-H4 | Prefix rule `Bash(agent-notify send:*)` and `$(...)` | In `default` permission mode, the rule denies `$(id -un)` and backtick substitutions. A single-quoted `'$(...)'` is literal text, and the rule allows it. `agent-notify dnd on` is denied. A chained `; printenv` ran only because `printenv` had its own allow rule. |
| S1-H5 | Payload fields | SessionStart: `session_id`, `cwd`, `hook_event_name`, `source`, `transcript_path`, `model`. It has no `prompt_id` and no `session_title`. UserPromptSubmit: adds `prompt_id` and `prompt`. Notification (`permission_prompt`): `message`, `notification_type`, `prompt_id`. Stop: `last_assistant_message`, `stop_hook_active`, `background_tasks`, `prompt_id`. |
| S1-H6 | Escape at a permission prompt | Claude Code fires no Stop. Plan 1 already clears the card through pane focus (`FOCUS_DWELL_MS`) and through the next TurnStarted. |
| S1-H7 | Async hooks in `claude -p` | One of two runs dropped the async Stop hook, most likely because the process exited first. Interactive sessions are not affected. |

The child Claude session inherits the parent's permission mode unless `--permission-mode` is given. A permission test without that flag tests nothing.

## S2: COSMIC

| Id | Criterion | Result | Evidence |
|---|---|---|---|
| S2-1 | Layer-shell panel from a Tauri GTK window | Pass | Tauri does not realize the window when built with `visible(false)`. `init_layer_shell` then works. The surface stays 0x0 and invisible until `set_size_request` is called. With a size request, the card renders transparent at the top right on the Overlay layer. |
| S2-2 | The card does not take keyboard focus | Pass | With `KeyboardMode::None`, Alacritty stays activated after `show` and after a click. |
| S2-3 | Hover and first click | Pass | CSS `:hover` works, because Wayland pointer focus does not depend on keyboard focus. The first click fires. |
| S2-4 | Toplevel activation of Alacritty from an ordinary client | Pass | An ordinary client can bind `zcosmic_toplevel_manager_v1`. `activate(handle, seat)` raised Alacritty from behind COSMIC Terminal. |
| S2-5 | Frontmost reader | Pass, with a caveat | `zcosmic_toplevel_info_v1` reports `activated` (state value 2). The initial state arrives about 100 ms after `get_cosmic_toplevel`, not within the first round trip. A long-lived connection handles that. |
| S2-6 | `ActionInvoked` from a popup click | Pass | `notify-send --action=default=Open --wait` printed `default` after a click on the popup. |
| S2-7 | Body markup in D-Bus notifications | Spec rule fails | COSMIC parses tags (`<b>` renders bold), cuts the body at a bare `<`, and shows entities (`&amp;`, `&lt;`) literally. XML-escaping corrupts text on COSMIC. |
| S2-8 | Critical urgency persistence | Fail | A `critical` popup left the screen within 10 s. The notification stayed in the notification center, and `--wait` kept waiting. |
| S2-9 | `graphical-session.target` | Active | It is active under cosmic-session 1.10, which answers spec section 14. |
| S2-10 | systemd user unit | Pass | A unit with `PartOf=graphical-session.target` got `WAYLAND_DISPLAY`, `DISPLAY` and `XDG_SESSION_TYPE`. A click from the service switched tmux and raised Alacritty. |
| S2-11 | Screen lock detection | Unknown | `loginctl lock-session` did not lock COSMIC, and `LockedHint` stayed `no`. |
| S2-12 | WebKit rendering in the VM | No workaround needed | The card renders without `WEBKIT_DISABLE_DMABUF_RENDERER`. |

Other globals an ordinary client sees on COSMIC 1.10: `ext_foreign_toplevel_list_v1`, `ext_idle_notifier_v1`, `ext_workspace_manager_v1`, `zwlr_layer_shell_v1`, `xdg_activation_v1`, `wp_security_context_manager_v1`.

## S3: X11 (Arch path, low priority)

| Id | Check | Result |
|---|---|---|
| S3-1 | Type hint | `_NET_WM_WINDOW_TYPE_NOTIFICATION` is set. |
| S3-2 | `WM_HINTS.input = False` | Not achieved. `set_accept_focus(false)` left `input` True. A click gives the card focus for a moment, and the xdotool raise then returns focus to Alacritty. |
| S3-3 | Show does not take focus | Pass under Openbox. |
| S3-4 | Hover and click | Pass. |
| S3-5 | Raise | `xdotool search --class Alacritty windowactivate --sync` works. |

## S4: Linux follow-ups for plan 3

Run in the Tart VM `pop-cosmic-spike` (Pop!_OS 24.04 COSMIC, arm64) and in Docker on the work Mac.

| Id | Question | Result |
|---|---|---|
| S4-1 | Does `cargo deny` pass with the Linux dependency set? | No. cargo-deny 0.20.2 on aarch64 and x86_64 Linux targets reports exactly 5 failures. Licenses: `GPL-3.0-only` (`cosmic-protocols` 0.2.0) and `Apache-2.0 WITH LLVM-exception` (`target-lexicon`). Advisories: RUSTSEC-2024-0422 (`gtk-layer-shell`), RUSTSEC-2024-0423 (`gtk-layer-shell-sys`), RUSTSEC-2024-0370 (`proc-macro-error` through the GTK3 macros). No other GTK3 advisory fires. Plan 3 Task 1 adds narrow ignores and one license exception, and puts `cosmic-protocols` behind the default-on `cosmic` feature. |
| S4-2 | Does COSMIC send idle events? | Yes. `ext_idle_notifier_v1` version 2. A 3 s notification sent `idled` at 3006 ms, `resumed` on mouse move and on key press, and `idled` again 3 s later. `get_input_idle_notification` behaves the same. |
| S4-3 | Which X11 tool reads the frontmost class? | `xprop`. Pop's xdotool (`1:3.20160805.1-5build1`) has no `getwindowclassname`. `xprop -root _NET_ACTIVE_WINDOW` prints `window id # 0x400004`, or `not found.` with no window manager. `xprop -id <id> WM_CLASS` prints `"Alacritty", "Alacritty"`. `xprop -id 0x0` fails. `xdotool search --class Alacritty` works for the raise. |
| S4-4 | Which sound player is present? | `pw-play` (pipewire-bin) played `/usr/share/sounds/freedesktop/stereo/message-new-instant.oga`. `paplay` is present. `canberra-gtk-play` is missing. |
| S4-5 | Can the Linux build be checked in Docker on the Mac? | Yes. `rust:1.94-slim-bookworm` plus `libwebkit2gtk-4.1-dev` (2.50.6), `libgtk-layer-shell-dev` (0.8.0), `libssl-dev`, `pkg-config` and `build-essential` ran `cargo check --locked` of the full Linux set in 39.55 s. The slim image has no curl, so plan 3 adds `curl ca-certificates unzip`. |
| S4-6 | Does a layer-shell window resize, hide and show? | Yes. `set_size_request(380, h)` plus `resize` took the window 110 to 300 to 60 px, then hide, show, and 200 px. The anchors and the layer survive hide and show. The allocated size that GTK reports lags one step. The web view viewport follows. |
| S4-7 | Does the D-Bus notification path work with zbus? | Yes, zbus 5.19.0 (MIT, default features). `Notify` returned ids 1 and 2, `CloseNotification` closed a popup, and a click sent `ActionInvoked (1, "default")` then `NotificationClosed (1, 2)`. COSMIC sent two `NotificationClosed` signals for one close, (2, 3) then (2, 2), so the presenter must not rely on close signals. Look-alike characters rendered literally. The Overlay layer card covers notification popups at the top right. |

Decisions from S4, used in plan 3:

- D14. `cosmic-protocols` is GPL-3.0-only. It sits behind the default-on `cosmic` feature, with a narrow `[[licenses.exceptions]]` entry. The user decides whether to keep it on. Without it, cards and the tmux switch still work, and Alacritty is not raised on COSMIC.
- D15. Sound on Linux is `pw-play`, else `paplay`.
- D16. X11 frontmost uses `xprop`. X11 idle and lock read as unknown. COSMIC lock reads as unknown.
- D17. COSMIC idle is one 30 s idle notification: 0 while active, 30 while idle, unknown until the first 30 s pass.
- D18. The D-Bus presenter runs only on a Wayland desktop without layer shell. It ignores `NotificationClosed`.
- D19. The Linux daemon logs to stderr for journald.

## Decisions for plans 2 and 3

| Id | Decision | Source |
|---|---|---|
| D1 | macOS hover comes from a native tracking area. The core receives `HoverChanged` from the shell, not from the page. | S1-5 |
| D2 | The macOS raise is: `aerospace focus --window-id` when AeroSpace is present, then `NSRunningApplication.activateWithOptions` for `org.alacritty` in every case. | S1-8, S1-11 |
| D3 | The AeroSpace window query is `list-windows --monitor all --app-bundle-id org.alacritty`. | S1-12 |
| D4 | The launchd plist and the install step resolve absolute tool paths, as the spec says. The daemon spawns tools by absolute path. | S1-7 |
| D5 | `sharingType = none` stays (it costs nothing), but the spec's fallback applies: line 3 hides only through `screen_shared` detection and manual DND. v1 has no screen-share detector on macOS, so `screen_shared` is `None` there. | S1-9 |
| D6 | On COSMIC, the daemon shows a gtk-layer-shell card (Overlay layer, top and right anchors, `KeyboardMode::None`, explicit size request). The D-Bus path is the fallback, not the default. | S2-1 to S2-3 |
| D7 | On COSMIC, the raise, the frontmost reader and the idle reader each hold one Wayland connection: `zcosmic_toplevel_manager_v1`, `zcosmic_toplevel_info_v1` and `ext_idle_notifier_v1`. | S2-4, S2-5 |
| D8 | All D-Bus notification text and all notify-send text replaces `<`, `>` and `&` with U+FF1C, U+FF1E and U+FF06. Plan 1 Task 17 uses `neutralize_markup`. | S2-7 |
| D9 | The D-Bus fallback does not rely on popup persistence. An attention card that the user did not act on stays in the daemon's state, and the tmux status segment keeps counting it. | S2-8 |
| D10 | On COSMIC, screen lock is `Unknown`. The suppression rules already treat Unknown as "show". | S2-11 |
| D11 | The systemd unit uses `PartOf`, `After` and `WantedBy=graphical-session.target`, as the spec says. | S2-9, S2-10 |
| D12 | The X11 card accepts a focus flicker on click in v1. | S3-2 |
| D13 | Rust-to-TypeScript types come from `ts-rs` 12.0.1. `tauri-specta` has no stable 2.x release. | Planning check, not a spike |

## Manual checks left for the user

1. **Fullscreen (S1-4).** Put any app in native fullscreen, run the spike binary, and show a card. The spike binary lives only in a session scratch directory, so this check moves into plan 2's manual smoke test.
2. **Intel run (S1-13).** Build and run plan 2's daemon on the home Intel Mac.
