# agent-notify Plan 2: macOS Daemon Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `agent-notifyd`, the resident Tauri daemon that shows contextual toast cards on macOS and switches tmux to the agent's pane on a click, plus the remaining CLI commands, the tmux integration, the install path, and the removal of `notify.sh`.

**Architecture:** Functional core, imperative shell, three layers. `agent-notify-core` (plan 1) keeps every decision. The new `agent-notify-daemon` library is the Tauri-free shell: socket server, intake queue, one actor that owns `DaemonState`, one effects worker, and the stateless tmux adapter. It takes every platform service through one `Services` record. The `agent-notifyd` Tauri app is the composition root: it builds the macOS services (non-activating `NSPanel`, native hover, NSWorkspace, AeroSpace and app activation), starts the daemon library, and hosts the Lit UI. The UI is an Elm loop (`update(model, msg) -> [model, cmds]`) over a view model the daemon pushes, with every IO call behind a `UiServices` record built in `main.ts`.

**Tech Stack:** Rust 2024 (rustc 1.94), tauri 2.12.4, tauri-nspanel 2.1.1, objc2 0.6.5, objc2-app-kit 0.3.2, objc2-foundation 0.3.2, tokio 1.53, ts-rs 12.0.1, tracing-appender 0.2.5, cargo-deny 0.20.2. TypeScript 7.0.2 on Bun 1.3, lit 3.3.3, @tauri-apps/api 2.12.4, happy-dom 21.0.1 for view tests. tmux 3.4.

**Spec:** `~/docs/superpowers/specs/2026-10-15-agent-notify-design.md` (approved, commit `534dcfad`). **Spike report:** `~/docs/superpowers/spikes/2026-10-15-agent-notify-spikes.md` (commit `fbe18b10`). Read both before starting. Where the spike report's decisions D1 to D13 differ from the spec, the spike report wins. Plan 1 (`2026-10-15-agent-notify-core-cli.md`) must be complete before Task 1.

## Global Constraints

- Rust edition 2024. Crates in `~/crates` keep that workspace's lints (`unsafe_code = "forbid"`). The new `~/tools/agent-notify` workspace sets `unsafe_code = "deny"`, `missing_docs = "warn"`, `unused_qualifications = "warn"`, `clippy::all = "warn"`. Gates run with `-D warnings`. Only `app/src/macos/panel.rs` may carry `#![allow(unsafe_code)]`, because the `tauri_panel!` macro expands to Objective-C class definitions.
- Every binary starts with `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]` and answers `--stamp` with `option_env!("CONFIG_MANIFEST_STAMP").unwrap_or("unstamped")`.
- Pinned versions, exact (`=`) in `Cargo.toml` and exact in `package.json`: tauri `=2.12.4` (feature `macos-private-api`), tauri-build `=2.7.3`, tauri-nspanel `=2.1.1`, objc2 `=0.6.5`, objc2-app-kit `=0.3.2`, objc2-foundation `=0.3.2`, ts-rs `=12.0.1`, tracing-appender `=0.2.5`, tokio `=1.53.3`; lit `3.3.3`, @tauri-apps/api `2.12.4`, typescript `7.0.2`, @happy-dom/global-registrator `21.0.1`, @types/bun `1.4.3`. `Cargo.lock` and `bun.lock` are committed. No `trustedDependencies`.
- The daemon spawns every external tool by absolute path (spike D4). tmux and AeroSpace come from `AGENT_NOTIFY_TMUX` and `AGENT_NOTIFY_AEROSPACE`, which the launchd plist sets, then from `PATH`. Fixed paths: `/usr/sbin/ioreg`, `/usr/bin/afplay`, `/usr/bin/open`.
- Identifiers: bundle id and launchd label `dev.dotfiles.agent-notifyd`; app bundle `~/Applications/agent-notifyd.app`; Alacritty bundle id `org.alacritty`; panel label `toast`.
- tmux options: `@agent_notify_session`, `@agent_notify_prompt` (plan 1), `@agent_notify_state` (pane) and `@agent_notify_window_state` (window), with values `working`, `finished`, `needs-you`, `failed`. Idle unsets the option. Every tmux call passes `-S <socket path>` and targets a `%pane_id`. Argv only, never `sh -c`.
- Timing: actor tick 250 ms, pane-liveness snapshot 60 s, socket line read timeout 5 s, query answer timeout 1 s, raise answer timeout 1 s. Core constants from plan 1 are unchanged.
- The daemon never accepts a tmux target from the UI. UI messages carry card ids only.
- UI text renders only through Lit text bindings. No `unsafeHTML`, no `innerHTML`, no markdown. No TypeScript decorators (Lit `static properties` instead). No non-null `!` in production TypeScript.
- No single-letter names (except `i`, `j`, `k` and math values such as `x`, `y`). No em dashes anywhere. No comments except a non-obvious why.
- The `config` repo is public. No employer, product or ticket names in code, fixtures or commit messages. Fixtures are synthetic.
- Commit only your own paths: `config add <paths>` then `config commit -m "<subject>" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- <paths>`. Never `--no-verify`.
- Do not edit `~/.claude/settings.json`, and do not run `launchctl` against the live agent, without the user's approval in the session. Task 17 and Task 19 name the exact points where you stop and ask.
- Minimum macOS version: 26.

## Review Focus

These inputs are implied by the spec, but no rule table names them. Each has a test in the task named.

1. **A tmux server restarts on the same socket path while the daemon runs.** Pane `%3` on the new server is a different pane. The daemon must not write state to it or switch to it. Test: Task 9, `a_restarted_server_reads_as_gone`.
2. **A client sends a line over the limit, or connects and never sends a newline.** The daemon must answer `line_too_long` or time out that one connection, and keep serving other clients. Tests: Task 11, `an_oversized_line_is_refused_and_other_clients_are_served` and `a_silent_client_is_dropped_after_the_timeout`.
3. **A stale socket file exists at startup, or a second daemon starts.** The first case must bind; the second must exit without touching the running daemon's socket. Test: Task 11, `a_stale_socket_file_is_replaced` and `a_second_instance_sees_the_first`.
4. **The do-not-disturb file is corrupt or half-written.** The daemon must start with DND off and log the error, not crash-loop under launchd. Test: Task 12, `a_corrupt_dnd_file_reads_as_off`.
5. **The web view reloads and an old push arrives after a newer one.** The UI must keep the newer view model. Test: Task 13, `an_older_revision_is_dropped`.

## Deviations from the spec

Tell the user about these at handoff. Each one follows a spike result or closes a gap the spec did not cover.

- **Hover (D1).** The native tracking area reports the pointer for the whole panel, not per card. On macOS the close buttons on all cards show while the pointer is over the stack, and the per-card brighten on hover is dropped. Linux (plan 3) keeps per-card CSS hover.
- **Frontmost.** `NSWorkspace.frontmostApplication`, not `aerospace list-windows --focused`. It reports the real front app even when AeroSpace's focus disagrees (S1-11), and it needs no AeroSpace.
- **Idle and screen lock.** Read from `ioreg` (`HIDIdleTime`, `CGSSessionScreenIsLocked`), not through HID APIs, so the shell needs no `unsafe`. Any parse failure is `None`, which the core treats as "show".
- **Socket permissions.** Directory mode 0700, socket mode 0600 after `bind`, and peer uid compared with the directory owner. Setting a umask needs `unsafe` libc calls. The 0700 directory closes the gap between `bind` and `chmod`.
- **Early reply.** The daemon replies `ok` as soon as a request decodes. Focus gathering and the update run after the reply, on a sequential intake queue that keeps arrival order. This keeps the CLI inside its 300 ms budget.
- **`focus-changed` arguments.** The tmux hook also passes `--pid #{pid} --start-time #{start_time}`, so the CLI needs no tmux call on each focus change.
- **`Control::Jump`.** A new control variant for `agent-notify jump`. `v` stays 1: an older daemon answers `unknown_kind`, as the evolution policy says.
- **Window marker.** The daemon also writes the window option `@agent_notify_window_state`, the most urgent state of the window's panes, because a window status format can only read the active pane's options. The status formats load from a file sourced after tpm, because the nord theme overwrites earlier format settings.
- **No Redux.** The UI state is the pushed view model plus four transient fields. A pure `update(model, msg)` Elm loop covers it. The user asked for Redux only if the UI needed something fancy.
- **Bridge fixtures.** Generated `.ts` files with `satisfies PanelViewModel`, so `tsc` checks them. A JSON import widens `"attention"` to `string`, so a JSON fixture cannot be type-checked against the union.
- **Bundle.** The install script assembles and ad hoc signs the `.app` itself. No tauri-cli.
- **Daemon build stamp.** The install script generates the stamp, embeds it through `CONFIG_MANIFEST_STAMP`, and writes it to `installed-stamp`. `doctor` compares the running daemon's stamp with that file, which catches a daemon that launchd did not restart.
- **`revision`** crosses the bridge as a TypeScript `number` (ts-rs `large_int = number`). It grows by one per input, so it stays far below 2^53.
- **`sharingType = none`** is set but does not hide the panel from capture (S1-9). `screen_shared` is `None` on macOS (D5).
- **Sound.** `afplay /System/Library/Sounds/Glass.aiff`, one short system sound.
- **Launch agent restart policy.** `KeepAlive` is `{SuccessfulExit: false}`, not `true`. A second daemon exits 0 when another one holds the lock, and `KeepAlive true` would relaunch it in a loop. launchd still restarts the daemon after a crash.
- **tmux file.** The hooks, binding and formats live in `.config/tmux/agent-notify.conf`, sourced from the end of `tmux.conf` only when the CLI is installed. The spec puts them in `tmux.conf` itself.

## File map

Create:

- `crates/agent-notify-core/src/status.rs`: `StatusReport`, `Decision`, `DaemonFacts`, pane and window state values
- `crates/agent-notify/src/render.rs`: status text, JSON and tmux segment
- `crates/agent-notify/src/doctor.rs`: pure checks over gathered facts, plus the fact gathering
- `tools/agent-notify/Cargo.toml`, `Cargo.lock`, `deny.toml`, `.gitignore`, `.cargo/config.toml`, `bunfig.toml`
- `tools/agent-notify/daemon/Cargo.toml`
- `tools/agent-notify/daemon/src/lib.rs`: module list
- `tools/agent-notify/daemon/src/environment.rs`: daemon env snapshot and path inputs
- `tools/agent-notify/daemon/src/tools.rs`: absolute tool paths
- `tools/agent-notify/daemon/src/services.rs`: service traits, `Services`, platform-neutral implementations
- `tools/agent-notify/daemon/src/presence.rs`: `ioreg` parsers, frontmost classification
- `tools/agent-notify/daemon/src/aerospace.rs`: AeroSpace window query and focus
- `tools/agent-notify/daemon/src/tmux.rs`: stateless tmux adapter and its parsers
- `tools/agent-notify/daemon/src/focus_run.rs`: runs a focus plan
- `tools/agent-notify/daemon/src/dnd_store.rs`: do-not-disturb file
- `tools/agent-notify/daemon/src/socket.rs`: bind, single instance, connection handling
- `tools/agent-notify/daemon/src/actor.rs`: actor, intake, worker, ticks, `Daemon` handle
- `tools/agent-notify/daemon/src/bridge.rs`: UI message types (ts-rs) and routing
- `tools/agent-notify/daemon/tests/support/mod.rs`, `tests/tmux_adapter.rs`, `tests/focus_run.rs`, `tests/socket.rs`, `tests/daemon.rs`, `tests/bindings.rs`
- `tools/agent-notify/app/Cargo.toml`, `build.rs`, `tauri.conf.json`, `capabilities/default.json`, `icons/icon.png`
- `tools/agent-notify/app/src/main.rs`, `src/geometry.rs`, `src/logging.rs`, `src/shell.rs`
- `tools/agent-notify/app/src/macos/mod.rs`, `panel.rs`, `services.rs`
- `tools/agent-notify/ui/package.json`, `bun.lock`, `tsconfig.json`, `bunfig.toml`, `index.html`, `build.ts`
- `tools/agent-notify/ui/src/main.ts`, `services.ts`, `tauri-services.ts`, `model.ts`, `runtime.ts`, `time.ts`, `styles.ts`
- `tools/agent-notify/ui/src/components/toast-stack.ts`, `toast-card.ts`, `finished-row.ts`
- `tools/agent-notify/ui/src/bindings/*.ts` (generated, committed)
- `tools/agent-notify/ui/test/setup.ts`, `model.test.ts`, `runtime.test.ts`, `time.test.ts`, `components.test.ts`, `recording-services.ts`, `fixtures/view-models.ts` (generated, committed)
- `tools/agent-notify/scripts/install.ts`, `scripts/install-plan.ts`, `scripts/make-icon.ts`
- `tools/agent-notify/tests/install-plan.test.ts`
- `.config/tmux/agent-notify.conf`
- `crates/agent-notify/tests/tmux_conf.rs`

Modify:

- `crates/Cargo.toml` (workspace dependency `ts-rs`)
- `crates/agent-notify-core/Cargo.toml`, `src/lib.rs`, `src/ids.rs`, `src/wire.rs`, `src/update.rs`, `src/view.rs`, `src/paths.rs`, `tests/fixtures/wire-v1.jsonl`, `tests/wire_fixtures.rs`
- `crates/agent-notify/src/cli.rs`, `src/invocation.rs`, `src/context.rs`, `src/client.rs`, `src/main.rs`, `tests/process.rs`
- `.scripts/config/config-build`, `README.md` (the `config build` line), `deps/deps.toml`, `crates/config-cli/tests/config_dispatcher.rs`
- `.config/tmux/tmux.conf`
- `README-MAC.md`, `README-LINUX.md`, `deps/deps-mac.toml`, `crates/config-cli/tests/workflow_labels.rs`, `.github/workflows/test-suite.yml`, `tests/run-in-docker.sh`

Delete: `.claude/hooks/notify.sh`, `crates/config-cli/tests/notify.rs`.

---

### Task 0: Preconditions

**Files:** none.

**Interfaces:**
- Consumes: plan 1, complete and committed.
- Produces: nothing.

- [ ] **Step 1: Confirm plan 1 is in place**

Run: `cd ~/crates && cargo test -p agent-notify-core -p agent-notify && ~/.local/bin/agent-notify --stamp`
Expected: all tests pass, and the stamp is not `unstamped`. If either fails, stop: plan 1 is not done.

- [ ] **Step 2: Confirm the tools**

Run: `bun --version && cargo deny --version && df -h ~ | tail -1`
Expected: bun 1.3.x, cargo-deny 0.20.x, and at least 8 GB free. A Tauri release build uses about 3 GB of target directory.

If `cargo deny` is missing, run `cargo install cargo-deny --locked --version 0.20.2`. If less than 8 GB is free, stop and tell the user.

---

### Task 1: `Control::Jump` in the core

**Files:**
- Modify: `crates/agent-notify-core/src/wire.rs` (`Control`)
- Modify: `crates/agent-notify-core/src/update.rs` (`control_event`, new `jump_target`, new test module)
- Modify: `crates/agent-notify-core/tests/fixtures/wire-v1.jsonl`, `tests/wire_fixtures.rs`

**Interfaces:**
- Consumes: plan 1 `Control`, `update`, `click_target`, `Effect::RunFocusPlan`.
- Produces: `Control::Jump` (JSON `{"type":"jump"}`). `update` answers it with one `Effect::RunFocusPlan` for the newest attention card that is not `Expiring`, or with no effect.

- [ ] **Step 1: Write the failing tests**

Append to `crates/agent-notify-core/src/update.rs`:

```rust
#[cfg(test)]
mod jump_tests {
    use super::*;
    use crate::DisplayText;
    use crate::test_support::{card, event_id, send, session};
    use crate::wire::{AgentEvent, Control, InputReason, Request, WIRE_VERSION};

    fn jump(state: &mut DaemonState, number: i64) -> Vec<Effect> {
        let request = Request::Control { v: WIRE_VERSION, event_id: event_id(number), sent_at_ms: EpochMs(number), control: Control::Jump };
        update(state, DaemonInput::Received { request, focus: FocusSnapshot::unknown() }, EpochMs(number))
    }

    fn needs(state: &mut DaemonState, number: i64, name: &str) {
        send(state, number, name, Some("p1"), AgentEvent::NeedsInput { reason: InputReason::Permission, message: DisplayText::new("Allow?") });
    }

    fn card_id(state: &DaemonState, name: &str) -> CardId {
        card(state, name).map(|shown| shown.id.clone()).expect("the session has a card")
    }

    #[test]
    fn jump_targets_the_newest_attention_card() {
        let mut state = DaemonState::default();
        needs(&mut state, 1, "a");
        needs(&mut state, 2, "b");
        send(&mut state, 3, "c", None, AgentEvent::Explicit { kind: Kind::Info, body: DisplayText::new("done") });
        let expected = card_id(&state, "b");
        let effects = jump(&mut state, 4);
        assert!(
            matches!(
                effects.as_slice(),
                [Effect::RunFocusPlan { card, target: FocusTarget::Session { session: target, .. } }]
                    if *card == expected && *target == session("b")
            ),
            "{effects:?}"
        );
    }

    #[test]
    fn jump_reaches_a_snoozed_attention_card() {
        let mut state = DaemonState::default();
        needs(&mut state, 1, "a");
        let expected = card_id(&state, "a");
        update(&mut state, DaemonInput::Ui(UiIntent::Closed { card_id: expected.clone() }), EpochMs(2));
        let effects = jump(&mut state, 3);
        assert!(matches!(effects.as_slice(), [Effect::RunFocusPlan { card, .. }] if *card == expected), "{effects:?}");
    }

    #[test]
    fn jump_without_an_attention_card_does_nothing() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", None, AgentEvent::Explicit { kind: Kind::Info, body: DisplayText::new("done") });
        assert!(jump(&mut state, 2).is_empty());
    }
}
```

Append this line to `crates/agent-notify-core/tests/fixtures/wire-v1.jsonl` (the file is append-only):

```
{"type":"control","v":1,"event_id":"0000000000000000000000000M","sent_at_ms":1760500000017,"control":{"type":"jump"}}
```

In `tests/wire_fixtures.rs`, change `assert_eq!(count, 19, "a fixture line went missing");` to `assert_eq!(count, 20, "a fixture line went missing");`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cd ~/crates && cargo test -p agent-notify-core jump wire_fixtures`
Expected: compile error `no variant named Jump` (run `cargo test -p agent-notify-core` if the two-filter form errors).

- [ ] **Step 3: Implement**

In `wire.rs`, add to `enum Control`, before `Unknown`:

```rust
    /// Run the focus plan for the newest attention card.
    Jump,
```

In `update.rs`, add an arm to `control_event`, before `Control::Unknown => {}`:

```rust
        Control::Jump => {
            if let Some((card, target)) = jump_target(state) {
                effects.push(Effect::RunFocusPlan { card, target });
            }
        }
```

Add after `click_target`:

```rust
/// The newest attention card that is not on its way out, and where it goes.
/// A snoozed card counts: the user closed it, but the agent still waits.
fn jump_target(state: &DaemonState) -> Option<(CardId, FocusTarget)> {
    let sessions = state.sessions.values().filter_map(|record| record.card.as_ref());
    let oneoffs = state.oneoff_cards.iter().map(|oneoff| &oneoff.card);
    let newest = sessions
        .chain(oneoffs)
        .filter(|card| card.kind == Kind::Attention && !matches!(card.phase, CardPhase::Expiring { .. }))
        .max_by_key(|card| card.created)?;
    let card = newest.id.clone();
    click_target(state, &card).map(|target| (card, target))
}
```

Add `CardId` to the `crate::` import line of `update.rs` if it is not there yet.

- [ ] **Step 4: Run the tests and clippy**

Run: `cd ~/crates && cargo test -p agent-notify-core && cargo clippy -p agent-notify-core --all-targets -- -D warnings`
Expected: all pass, including the 20-line fixture round trip.

- [ ] **Step 5: Commit**

```bash
cd ~ && config add crates/agent-notify-core
config commit -m "Add the jump control to the agent-notify core" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/agent-notify-core
```

---

### Task 2: Status report, tmux state values and the state directory

**Files:**
- Create: `crates/agent-notify-core/src/status.rs`
- Modify: `crates/agent-notify-core/src/lib.rs` (add `pub mod status;`), `src/paths.rs` (`state_directory`)

**Interfaces:**
- Consumes: plan 1 `DaemonState`, `SessionRecord`, `Turn`, `QuietMode`, `Card`, `CardPhase`, `Dnd`, `Effect`, `PaneState`, `SuppressReason`, `Request`, `AgentEvent`, `Control`, `location_line`, `PathInputs`, `Platform`.
- Produces (`agent_notify_core::status`):
  - `MAX_DECISIONS: usize = 50`, `PANE_STATE_OPTION = "@agent_notify_state"`, `WINDOW_STATE_OPTION = "@agent_notify_window_state"`.
  - `DaemonFacts { stamp: String, started_ms: EpochMs, tools: Vec<ToolStatus> }`, `ToolStatus { name: String, path: Option<PathBuf> }`.
  - `Decision { at_ms: EpochMs, event_id: EventId, kind: String, outcome: String }`, `decision(&Request, &[Effect], EpochMs) -> Option<Decision>`.
  - `StatusReport { daemon: DaemonFacts, now_ms: EpochMs, dnd: DndStatus, sessions: Vec<SessionStatus>, decisions: Vec<Decision> }`, `DndStatus { on: bool, until_ms: Option<EpochMs> }`, `SessionStatus { session, pane: Option<String>, location, turn, quiet, card: Option<String> }`. All `Serialize + Deserialize`.
  - `status_report(&DaemonState, EpochMs, &DaemonFacts, &[Decision]) -> StatusReport`.
  - `pane_state_value(PaneState) -> Option<&'static str>`, `parse_pane_state(&str) -> PaneState`, `window_state(impl IntoIterator<Item = PaneState>) -> PaneState`.
- Produces (`agent_notify_core::paths`): `state_directory(&PathInputs<'_>) -> Option<PathBuf>`.

- [ ] **Step 1: Write the failing tests**

Create `crates/agent-notify-core/src/status.rs` with only the tests for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::DisplayText;
    use crate::state::DaemonState;
    use crate::test_support::{event_id, send};
    use crate::update::{Effect, PaneState, SuppressReason};
    use crate::wire::{AgentEvent, InputReason, Kind, QuietUntil};

    fn facts() -> DaemonFacts {
        DaemonFacts { stamp: "stamp-1".to_string(), started_ms: EpochMs(0), tools: Vec::new() }
    }

    #[test]
    fn the_report_lists_sessions_with_turn_quiet_and_card() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", Some("p1"), AgentEvent::Quiet { until: QuietUntil::NextSend, max_minutes: 1 });
        send(&mut state, 3, "b", Some("p1"), AgentEvent::NeedsInput { reason: InputReason::Permission, message: DisplayText::new("Allow?") });
        let report = status_report(&state, EpochMs(4), &facts(), &[]);
        let rows: Vec<(&str, &str, Option<&str>)> =
            report.sessions.iter().map(|row| (row.turn.as_str(), row.quiet.as_str(), row.card.as_deref())).collect();
        assert_eq!(
            rows,
            vec![("working", "until next-send, at most 59 s more", None), ("needs_you", "armed", Some("attention showing"))]
        );
        assert_eq!(report.sessions[0].pane.as_deref(), Some("%1"));
        assert_eq!(report.sessions[0].location, "4 · code/feature-x");
        assert!(!report.dnd.on);
        assert_eq!(report.daemon, facts());
    }

    #[test]
    fn the_report_keeps_the_newest_decisions() {
        let decisions: Vec<Decision> = (0..60)
            .map(|number| Decision { at_ms: EpochMs(number), event_id: event_id(number), kind: "turn_ended".to_string(), outcome: "card".to_string() })
            .collect();
        let report = status_report(&DaemonState::default(), EpochMs(100), &facts(), &decisions);
        assert_eq!(report.decisions.len(), MAX_DECISIONS);
        assert_eq!(report.decisions.first().map(|decision| decision.at_ms), Some(EpochMs(10)));
    }

    #[test]
    fn a_decision_names_the_suppression_reason() {
        let mut state = DaemonState::default();
        let input = crate::test_support::agent_input(
            7,
            crate::test_support::origin(Some("a"), Some("p1")),
            AgentEvent::TurnEnded { last_message: None },
        );
        let crate::update::DaemonInput::Received { request, .. } = &input else { panic!("an agent input") };
        let request = request.clone();
        let effects = crate::update::update(&mut state, input, EpochMs(7));
        let made = decision(&request, &effects, EpochMs(7)).expect("agent requests make decisions");
        assert_eq!(made.kind, "turn_ended");
        assert!(made.outcome == "card" || made.outcome.starts_with("suppressed: "), "{made:?}");
        let suppressed = [Effect::Suppressed { event_id: event_id(7), reason: SuppressReason::Quiet }];
        assert_eq!(decision(&request, &suppressed, EpochMs(7)).map(|made| made.outcome), Some("suppressed: quiet".to_string()));
    }

    #[test]
    fn pane_state_values_round_trip_and_idle_unsets() {
        for state in [PaneState::Working, PaneState::Finished, PaneState::NeedsYou, PaneState::Failed] {
            let value = pane_state_value(state).expect("a non-idle state has a value");
            assert_eq!(parse_pane_state(value), state);
        }
        assert_eq!(pane_state_value(PaneState::Idle), None);
        assert_eq!(parse_pane_state(""), PaneState::Idle);
        assert_eq!(parse_pane_state("from-the-future"), PaneState::Idle);
    }

    #[test]
    fn a_window_shows_its_most_urgent_pane() {
        assert_eq!(window_state([PaneState::Finished, PaneState::NeedsYou, PaneState::Working]), PaneState::NeedsYou);
        assert_eq!(window_state([PaneState::Finished, PaneState::Failed]), PaneState::Failed);
        assert_eq!(window_state([PaneState::Idle, PaneState::Working]), PaneState::Working);
        assert_eq!(window_state([]), PaneState::Idle);
    }

    #[test]
    fn the_report_survives_a_json_round_trip() {
        let report = status_report(&DaemonState::default(), EpochMs(1), &facts(), &[]);
        let text = serde_json::to_string(&report).expect("encodes");
        assert_eq!(serde_json::from_str::<StatusReport>(&text).expect("decodes"), report);
    }

    #[test]
    fn info_and_attention_render_their_names() {
        assert_eq!(kind_name(Kind::Info), "info");
        assert_eq!(kind_name(Kind::Attention), "attention");
    }
}
```

If `test_support::agent_input` and `test_support::origin` are private to their module, make them `pub(crate)`. They are plain builders.

In `src/paths.rs`, append to the existing tests module:

```rust
    #[test]
    fn the_state_directory_follows_the_platform() {
        let home = Path::new("/home/dev");
        let inputs = |platform, xdg_state_home| PathInputs {
            platform,
            home: Some(home),
            xdg_runtime_dir: None,
            xdg_state_home,
            socket_override: None,
            log_dir_override: None,
        };
        assert_eq!(
            state_directory(&inputs(Platform::MacOs, None)),
            Some(PathBuf::from("/home/dev/Library/Application Support/agent-notify"))
        );
        assert_eq!(state_directory(&inputs(Platform::Linux, None)), Some(PathBuf::from("/home/dev/.local/state/agent-notify")));
        assert_eq!(
            state_directory(&inputs(Platform::Linux, Some(Path::new("/state")))),
            Some(PathBuf::from("/state/agent-notify"))
        );
        assert_eq!(state_directory(&inputs(Platform::Linux, Some(Path::new("relative")))), Some(PathBuf::from("/home/dev/.local/state/agent-notify")));
    }
```

Add `pub mod status;` to `src/lib.rs`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cd ~/crates && cargo test -p agent-notify-core status state_directory`
Expected: compile errors, `cannot find function status_report` and `state_directory`.

- [ ] **Step 3: Implement `status.rs`**

Put this above the tests module:

```rust
//! The `status` report and the tmux state values. Pure: the daemon passes its
//! own facts in, and the CLI renders the result.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::label::location_line;
use crate::state::{Card, CardPhase, DaemonState, Dnd, QuietMode, Turn};
use crate::update::{Effect, PaneState, SuppressReason};
use crate::wire::{AgentEvent, Control, Kind, QuietUntil, Request};
use crate::{EpochMs, EventId, SessionId};

/// How many decisions `status` shows.
pub const MAX_DECISIONS: usize = 50;
/// The pane option that holds a session's turn state.
pub const PANE_STATE_OPTION: &str = "@agent_notify_state";
/// The window option the tmux status line reads.
pub const WINDOW_STATE_OPTION: &str = "@agent_notify_window_state";

/// Facts only the daemon process knows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonFacts {
    /// The daemon's build stamp.
    pub stamp: String,
    /// When the daemon started.
    pub started_ms: EpochMs,
    /// The external tools the daemon resolved at startup.
    pub tools: Vec<ToolStatus>,
}

/// One external tool and the absolute path the daemon runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolStatus {
    /// The tool, for example `tmux`.
    pub name: String,
    /// The absolute path, or `None` when the daemon did not find the tool.
    pub path: Option<PathBuf>,
}

/// One request the daemon handled, and what came of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    /// When.
    pub at_ms: EpochMs,
    /// The request's event id.
    pub event_id: EventId,
    /// The event or control kind, for example `turn_ended`.
    pub kind: String,
    /// `card`, `handled`, or `suppressed: <reason>`.
    pub outcome: String,
}

/// Do-not-disturb as `status` shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DndStatus {
    /// True while cards are held back.
    pub on: bool,
    /// The end time, when one was set.
    pub until_ms: Option<EpochMs>,
}

/// One session record as `status` shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionStatus {
    /// The session.
    pub session: SessionId,
    /// The pane id, for example `%3`.
    pub pane: Option<String>,
    /// The card's line 1.
    pub location: String,
    /// `idle`, `working`, `finished`, `needs_you` or `failed`.
    pub turn: String,
    /// `armed`, or the quiet mode and the time left.
    pub quiet: String,
    /// The card's kind and phase, for example `attention showing`.
    pub card: Option<String>,
}

/// The `agent-notify status` report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusReport {
    /// The daemon's own facts.
    pub daemon: DaemonFacts,
    /// When the report was made.
    pub now_ms: EpochMs,
    /// Do-not-disturb.
    pub dnd: DndStatus,
    /// Every session record, ordered by session id.
    pub sessions: Vec<SessionStatus>,
    /// The newest decisions, oldest first.
    pub decisions: Vec<Decision>,
}

/// Builds the report. `decisions` is the daemon's history, oldest first.
#[must_use]
pub fn status_report(state: &DaemonState, now: EpochMs, daemon: &DaemonFacts, decisions: &[Decision]) -> StatusReport {
    let until_ms = match state.dnd {
        Dnd::On { until } => until,
        Dnd::Off => None,
    };
    let sessions = state
        .sessions
        .iter()
        .map(|(session, record)| SessionStatus {
            session: session.clone(),
            pane: record.pane.as_ref().map(|pane| pane.pane.as_str().to_string()),
            location: location_line(record.window.as_ref(), record.repo.as_ref(), &record.cwd),
            turn: turn_name(&record.turn).to_string(),
            quiet: quiet_text(&record.quiet, now),
            card: record.card.as_ref().map(card_text),
        })
        .collect();
    let skip = decisions.len().saturating_sub(MAX_DECISIONS);
    StatusReport {
        daemon: daemon.clone(),
        now_ms: now,
        dnd: DndStatus { on: state.dnd_active(now), until_ms },
        sessions,
        decisions: decisions.iter().skip(skip).cloned().collect(),
    }
}

/// The history entry for one request and the effects it caused. Queries make
/// none: they change nothing.
#[must_use]
pub fn decision(request: &Request, effects: &[Effect], now: EpochMs) -> Option<Decision> {
    let (event_id, kind, default_outcome) = match request {
        Request::Agent { event_id, event, .. } => (event_id, event_kind(event), "card"),
        Request::Control { event_id, control, .. } => (event_id, control_kind(control), "handled"),
        Request::Query { .. } | Request::Unknown => return None,
    };
    let suppressed = effects.iter().find_map(|effect| match effect {
        Effect::Suppressed { event_id: id, reason } if id == event_id => Some(*reason),
        _ => None,
    });
    let outcome = suppressed.map_or_else(|| default_outcome.to_string(), |reason| format!("suppressed: {}", reason_name(reason)));
    Some(Decision { at_ms: now, event_id: event_id.clone(), kind: kind.to_string(), outcome })
}

/// The tmux option value for `state`. `None` means "unset the option".
#[must_use]
pub fn pane_state_value(state: PaneState) -> Option<&'static str> {
    match state {
        PaneState::Idle => None,
        PaneState::Working => Some("working"),
        PaneState::Finished => Some("finished"),
        PaneState::NeedsYou => Some("needs-you"),
        PaneState::Failed => Some("failed"),
    }
}

/// Reads a value [`pane_state_value`] wrote. Empty or unknown text is idle.
#[must_use]
pub fn parse_pane_state(value: &str) -> PaneState {
    match value {
        "working" => PaneState::Working,
        "finished" => PaneState::Finished,
        "needs-you" => PaneState::NeedsYou,
        "failed" => PaneState::Failed,
        _ => PaneState::Idle,
    }
}

/// The state a window shows: the most urgent state of its panes.
#[must_use]
pub fn window_state(states: impl IntoIterator<Item = PaneState>) -> PaneState {
    states.into_iter().max_by_key(|state| urgency(*state)).unwrap_or(PaneState::Idle)
}

fn urgency(state: PaneState) -> u8 {
    match state {
        PaneState::Idle => 0,
        PaneState::Working => 1,
        PaneState::Finished => 2,
        PaneState::Failed => 3,
        PaneState::NeedsYou => 4,
    }
}

fn turn_name(turn: &Turn) -> &'static str {
    match turn {
        Turn::Idle => "idle",
        Turn::Working { .. } => "working",
        Turn::Finished { .. } => "finished",
        Turn::NeedsYou { .. } => "needs_you",
        Turn::Failed { .. } => "failed",
    }
}

fn quiet_text(quiet: &QuietMode, now: EpochMs) -> String {
    match quiet {
        QuietMode::Armed => "armed".to_string(),
        QuietMode::Until { until, deadline } => {
            let mode = match until {
                QuietUntil::NextSend => "next-send",
                QuietUntil::NextPrompt => "next-prompt",
            };
            format!("until {mode}, at most {} s more", deadline.since(now).max(0) / 1_000)
        }
    }
}

fn card_text(card: &Card) -> String {
    let phase = match card.phase {
        CardPhase::Showing { .. } => "showing",
        CardPhase::Folded => "folded",
        CardPhase::Snoozed { .. } => "snoozed",
        CardPhase::Expiring { .. } => "expiring",
    };
    format!("{} {phase}", kind_name(card.kind))
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Info => "info",
        Kind::Attention => "attention",
    }
}

fn event_kind(event: &AgentEvent) -> &'static str {
    match event {
        AgentEvent::SessionStarted { .. } => "session_started",
        AgentEvent::TurnStarted => "turn_started",
        AgentEvent::TurnEnded { .. } => "turn_ended",
        AgentEvent::TurnFailed { .. } => "turn_failed",
        AgentEvent::NeedsInput { .. } => "needs_input",
        AgentEvent::InputResolved => "input_resolved",
        AgentEvent::Explicit { .. } => "explicit",
        AgentEvent::Quiet { .. } => "quiet",
        AgentEvent::QuietOff => "quiet_off",
        AgentEvent::SessionEnded => "session_ended",
        AgentEvent::Unknown => "unknown",
    }
}

fn control_kind(control: &Control) -> &'static str {
    match control {
        Control::FocusChanged { .. } => "focus_changed",
        Control::Dnd { .. } => "dnd",
        Control::Jump => "jump",
        Control::Unknown => "unknown",
    }
}

fn reason_name(reason: SuppressReason) -> &'static str {
    match reason {
        SuppressReason::ExplicitSent => "explicit_sent",
        SuppressReason::Quiet => "quiet",
        SuppressReason::Outranked => "outranked",
        SuppressReason::Focused => "focused",
        SuppressReason::Dnd => "dnd",
        SuppressReason::Stale => "stale",
        SuppressReason::NoCardForEvent => "no_card_for_event",
    }
}
```

If plan 1's `SessionRecord.cwd` is a `PathBuf`, `&record.cwd` coerces to `&Path`. If `PaneState` does not derive `Copy`, add `Copy` to its derive list: it is a field-less enum.

- [ ] **Step 4: Implement `state_directory`**

Add to `src/paths.rs`, after `log_directory`:

```rust
/// Where the daemon keeps state that must survive a restart: do-not-disturb
/// and the install stamp. Not the socket directory: on Linux that is a tmpfs.
#[must_use]
pub fn state_directory(inputs: &PathInputs<'_>) -> Option<PathBuf> {
    let home = absolute(inputs.home);
    match inputs.platform {
        Platform::MacOs => home.map(|home| home.join("Library/Application Support/agent-notify")),
        Platform::Linux | Platform::Other => absolute(inputs.xdg_state_home)
            .map(Path::to_path_buf)
            .or_else(|| home.map(|home| home.join(".local/state")))
            .map(|state| state.join("agent-notify")),
    }
}
```

- [ ] **Step 5: Run the tests and clippy**

Run: `cd ~/crates && cargo test -p agent-notify-core && cargo clippy -p agent-notify-core --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
cd ~ && config add crates/agent-notify-core
config commit -m "Add the agent-notify status report and tmux state values" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/agent-notify-core
```

---

### Task 3: TypeScript bindings for the view model (feature `ts`)

**Files:**
- Modify: `crates/Cargo.toml` (`[workspace.dependencies]`: `ts-rs`)
- Modify: `crates/agent-notify-core/Cargo.toml` (optional dependency, feature `ts`)
- Modify: `crates/agent-notify-core/src/ids.rs` (`EpochMs`, `checked_id!`), `src/wire.rs` (`Kind`), `src/view.rs` (three view types, one test module)

**Interfaces:**
- Consumes: plan 1 view types.
- Produces: with feature `ts`, `PanelViewModel`, `CardView`, `FinishedEntry`, `Kind`, `EpochMs` and every checked id implement `ts_rs::TS`. `EpochMs` is `number` and ids are `string` in TypeScript. Without the feature nothing changes.

- [ ] **Step 1: Add the dependency and feature**

In `crates/Cargo.toml` under `[workspace.dependencies]`, keeping alphabetical order:

```toml
ts-rs = "=12.0.1"
```

In `crates/agent-notify-core/Cargo.toml`:

```toml
[features]
ts = ["dep:ts-rs"]
```

and under `[dependencies]`:

```toml
ts-rs = { workspace = true, optional = true }
```

- [ ] **Step 2: Write the failing test**

Append to `src/view.rs`:

```rust
#[cfg(all(test, feature = "ts"))]
mod ts_tests {
    use ts_rs::{Config, TS};

    use super::{CardView, PanelViewModel};

    #[test]
    fn the_view_model_binding_has_no_bigint_and_typed_kinds() {
        let config = Config::new().with_large_int("number");
        let model = PanelViewModel::export_to_string(&config).expect("exports");
        assert!(model.contains("revision: number"), "{model}");
        assert!(!model.contains("bigint"), "{model}");
        let card = CardView::export_to_string(&config).expect("exports");
        assert!(card.contains("body: string | null"), "{card}");
        assert!(card.contains("kind: Kind"), "{card}");
        assert!(card.contains("created_ms: EpochMs"), "{card}");
    }
}
```

- [ ] **Step 3: Run it to see it fail**

Run: `cd ~/crates && cargo test -p agent-notify-core --features ts ts_tests`
Expected: compile error, `PanelViewModel` does not implement `TS`.

- [ ] **Step 4: Add the derives**

In `ids.rs`, on `EpochMs`, after its `#[derive(...)]` and `#[serde(transparent)]` lines:

```rust
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(type = "number"))]
```

In the `checked_id!` macro, after `#[serde(try_from = "String", into = "String")]`:

```rust
        #[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(type = "string"))]
```

In `wire.rs`, on `enum Kind`, after its serde attribute:

```rust
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
```

In `view.rs`, on each of `PanelViewModel`, `CardView` and `FinishedEntry`, after the `#[derive(...)]` line:

```rust
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
```

- [ ] **Step 5: Run the tests and clippy, with and without the feature**

Run: `cd ~/crates && cargo test -p agent-notify-core && cargo test -p agent-notify-core --features ts && cargo clippy -p agent-notify-core --all-targets --features ts -- -D warnings && cargo clippy -p agent-notify-core --all-targets -- -D warnings`
Expected: all pass. If ts-rs warns that a serde attribute is unsupported (for example on `Kind`'s `rename_all`), the test output shows the generated text: confirm that the union is `"info" | "attention"` before you continue.

- [ ] **Step 6: Commit**

```bash
cd ~ && config add crates/Cargo.toml crates/Cargo.lock crates/agent-notify-core
config commit -m "Generate TypeScript types for the agent-notify view model" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/Cargo.toml crates/Cargo.lock crates/agent-notify-core
```

---

### Task 4: Reply data in the CLI client

**Files:**
- Modify: `crates/agent-notify/src/client.rs`

**Interfaces:**
- Consumes: plan 1 `client::deliver`, `Delivery`.
- Produces: `Delivery::Acked { daemon: Option<String>, data: Option<serde_json::Value> }`. Every other variant is unchanged.

- [ ] **Step 1: Write the failing test**

In the `client.rs` tests module, change the pattern in `an_ok_reply_is_an_acknowledgement` to `Delivery::Acked { daemon: Some(daemon), .. }`, and add:

```rust
    #[test]
    fn a_query_reply_carries_its_data() {
        let directory = socket_dir();
        let path = directory.path().join("s.sock");
        serve(&path, Some(r#"{"ok":true,"daemon":"stamp","data":{"answer":42}}"#));
        let delivery = deliver(&path, "{}\n", BUDGET);
        assert!(
            matches!(&delivery, Delivery::Acked { data: Some(data), .. } if data["answer"] == 42),
            "{delivery:?}"
        );
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `cd ~/crates && cargo test -p agent-notify client`
Expected: compile error, `Delivery::Acked` has no field `data`.

- [ ] **Step 3: Implement**

In `client.rs`, change the variant to:

```rust
    /// The daemon accepted it.
    Acked {
        /// The daemon's build stamp.
        daemon: Option<String>,
        /// The answer to a query.
        data: Option<serde_json::Value>,
    },
```

and the matching arm in `deliver` to:

```rust
            Ok(reply) if reply.ok => Delivery::Acked { daemon: reply.daemon, data: reply.data },
```

- [ ] **Step 4: Run the tests and clippy**

Run: `cd ~/crates && cargo test -p agent-notify && cargo clippy -p agent-notify --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
cd ~ && config add crates/agent-notify
config commit -m "Keep query data in agent-notify socket replies" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/agent-notify
```

---

### Task 5: `focus-changed`, `dnd` and `jump`

**Files:**
- Modify: `crates/agent-notify/src/cli.rs`, `src/invocation.rs`, `src/context.rs`, `src/main.rs`
- Create: `crates/agent-notify/src/render.rs` (only `delivery_problem` in this task)
- Test: `crates/agent-notify/tests/process.rs`

**Interfaces:**
- Consumes: Task 1 `Control::Jump`; Task 4 `Delivery`; plan 1 `plan_invocation`, `Invocation`, `context::gather`.
- Produces:
  - `cli::Command::{FocusChanged { socket: PathBuf, pid: u32, start_time: i64, pane: String, focus_in: bool, out: bool }, Dnd { action: DndAction }, Jump, Status { json: bool, tmux: bool }, Doctor}`; `cli::DndAction::{On { for_minutes: Option<u16> }, Off, Status}`; `Command::reads_origin(&self) -> bool`.
  - `invocation::Output::{FallbackOnFailure, Silent, Confirm(Option<&'static str>), Status(StatusFormat), DndStatus}`; `invocation::StatusFormat::{Text, Json, Tmux}`; `Invocation.output: Output`.
  - `render::delivery_problem(&Delivery) -> String`.
  - Task 6 fills `Output::Status` and `Output::DndStatus`; Task 7 fills `Doctor`. Until then both print "not built yet" and exit 1.

- [ ] **Step 1: Write the failing planner tests**

Append to the tests module of `invocation.rs`:

```rust
    fn bare_inputs() -> Inputs {
        Inputs {
            now: EpochMs(5),
            event_id: EventId::from_parts(EpochMs(5), [0; 10]),
            cwd: PathBuf::from("/"),
            pane: PaneLookup::NotInTmux,
            repo: None,
            stdin: None,
        }
    }

    fn focus_changed(pane: &str, socket: &str, focus_in: bool) -> Command {
        Command::FocusChanged { socket: PathBuf::from(socket), pid: 77, start_time: 1_700_000_000, pane: pane.to_string(), focus_in, out: !focus_in }
    }

    #[test]
    fn focus_changed_builds_a_silent_control_with_the_server_identity() {
        let invocation = plan_invocation(&focus_changed("%3", "/tmp/tmux-501/default", true), &bare_inputs());
        assert_eq!(invocation.output, Output::Silent);
        let Some(Request::Control { control: Control::FocusChanged { pane, focused }, .. }) = &invocation.request else {
            panic!("a focus control: {invocation:?}");
        };
        assert!(*focused);
        assert_eq!(pane.pane.as_str(), "%3");
        assert_eq!((pane.server.pid, pane.server.start_time), (77, 1_700_000_000));
        assert!(invocation.fallback.is_none());
    }

    #[test]
    fn focus_changed_rejects_a_bad_pane_or_relative_socket() {
        for command in [focus_changed("3", "/tmp/s", true), focus_changed("%3", "relative/s", false)] {
            let invocation = plan_invocation(&command, &bare_inputs());
            assert_eq!(invocation.exit, Exit::Usage, "{command:?}");
            assert!(invocation.request.is_none());
        }
    }

    #[test]
    fn dnd_and_jump_build_confirmed_controls() {
        let on = plan_invocation(&Command::Dnd { action: DndAction::On { for_minutes: Some(30) } }, &bare_inputs());
        assert!(matches!(
            on.request,
            Some(Request::Control { control: Control::Dnd { command: DndCommand::On { for_minutes: Some(30) } }, .. })
        ));
        assert_eq!(on.output, Output::Confirm(Some("do-not-disturb is on")));
        let jump = plan_invocation(&Command::Jump, &bare_inputs());
        assert!(matches!(jump.request, Some(Request::Control { control: Control::Jump, .. })));
        assert_eq!(jump.output, Output::Confirm(None));
    }

    #[test]
    fn status_and_dnd_status_are_queries() {
        let status = plan_invocation(&Command::Status { json: false, tmux: true }, &bare_inputs());
        assert!(matches!(status.request, Some(Request::Query { query: Query::Status, .. })));
        assert_eq!(status.output, Output::Status(StatusFormat::Tmux));
        let dnd = plan_invocation(&Command::Dnd { action: DndAction::Status }, &bare_inputs());
        assert_eq!(dnd.output, Output::DndStatus);
    }

    #[test]
    fn only_agent_commands_read_the_origin() {
        assert!(!Command::Jump.reads_origin());
        assert!(!focus_changed("%1", "/tmp/s", true).reads_origin());
        assert!(Command::Send { kind: KindArg::Info, body: "x".to_string() }.reads_origin());
    }
```

Extend the tests module's `use` lines with `crate::cli::DndAction`, `agent_notify_core::wire::{Control, DndCommand, Query}` and `agent_notify_core::{EpochMs, EventId}` where they are not imported yet.

- [ ] **Step 2: Run them to see them fail**

Run: `cd ~/crates && cargo test -p agent-notify invocation`
Expected: compile errors for the new variants.

- [ ] **Step 3: Add the arguments**

Add to `enum Command` in `cli.rs`:

```rust
    /// Report a tmux pane focus change. tmux's pane-focus hooks call this.
    FocusChanged {
        /// The tmux server socket, `#{socket_path}`.
        #[arg(long)]
        socket: PathBuf,
        /// The tmux server pid, `#{pid}`.
        #[arg(long)]
        pid: u32,
        /// The tmux server start time, `#{start_time}`.
        #[arg(long)]
        start_time: i64,
        /// The pane, `#{pane_id}`.
        #[arg(long)]
        pane: String,
        /// The pane gained focus.
        #[arg(long = "in", conflicts_with = "out", required_unless_present = "out")]
        focus_in: bool,
        /// The pane lost focus.
        #[arg(long)]
        out: bool,
    },
    /// Hold every card back, or let them through again.
    Dnd {
        /// on, off or status.
        #[command(subcommand)]
        action: DndAction,
    },
    /// Go to the newest card that needs you.
    Jump,
    /// Print the daemon's state.
    Status {
        /// The full report as JSON.
        #[arg(long, conflicts_with = "tmux")]
        json: bool,
        /// A short segment for the tmux status line. Prints nothing when no daemon is installed.
        #[arg(long)]
        tmux: bool,
    },
    /// Check the installation and print what to fix.
    Doctor,
```

Add after `enum HookAgent`:

```rust
/// A do-not-disturb action.
#[derive(Debug, Clone, Copy, Subcommand)]
pub enum DndAction {
    /// Hold every card back.
    On {
        /// End on its own after this many minutes.
        #[arg(long, value_parser = clap::value_parser!(u16).range(1..=1440))]
        for_minutes: Option<u16>,
    },
    /// Let cards through again.
    Off,
    /// Print whether do-not-disturb is on.
    Status,
}
```

Add to `impl Command`:

```rust
    /// True for the commands whose request carries an origin, so the CLI
    /// reads the tmux pane and the git repository first.
    pub fn reads_origin(&self) -> bool {
        matches!(self, Command::Hook { .. } | Command::Send { .. } | Command::Quiet { .. })
    }
```

Add `use std::path::PathBuf;` to `cli.rs`.

- [ ] **Step 4: Plan the new commands**

In `invocation.rs`, add:

```rust
/// What the shell does with the daemon's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// Show the fallback notification when the daemon does not acknowledge.
    FallbackOnFailure,
    /// Print nothing, whatever happens. tmux shows any output of a hook command.
    Silent,
    /// Print the line on success. Exit 1 with a message when the daemon does not answer.
    Confirm(Option<&'static str>),
    /// Print the status report.
    Status(StatusFormat),
    /// Print whether do-not-disturb is on.
    DndStatus,
}

/// How `status` prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFormat {
    /// For a person.
    Text,
    /// The report as JSON.
    Json,
    /// A tmux status-line segment.
    Tmux,
}
```

Add the field to `struct Invocation`:

```rust
    /// What to do with the answer.
    pub output: Output,
```

Add `output: Output::FallbackOnFailure` to the three literals in `Invocation::nothing`, `Invocation::usage` and `Invocation::agent`. Add two constructors to `impl Invocation`:

```rust
    fn control(inputs: &Inputs, control: Control, output: Output) -> Self {
        let request = Request::Control { v: WIRE_VERSION, event_id: inputs.event_id.clone(), sent_at_ms: inputs.now, control };
        Invocation { tags: Vec::new(), request: Some(request), fallback: None, exit: Exit::Success, stderr: None, log: None, output }
    }

    fn query(inputs: &Inputs, query: Query, output: Output) -> Self {
        let request = Request::Query { v: WIRE_VERSION, event_id: inputs.event_id.clone(), query };
        Invocation { tags: Vec::new(), request: Some(request), fallback: None, exit: Exit::Success, stderr: None, log: None, output }
    }
```

Replace the body of `plan_invocation` with:

```rust
    match command {
        Command::Hook { agent: HookAgent::ClaudeCode { .. } } => plan_hook(inputs),
        Command::Send { kind, body } => plan_send(Kind::from(*kind), body, inputs),
        Command::Quiet { until, max_minutes, off } => plan_quiet(until.map(Into::into), *max_minutes, *off, inputs),
        Command::FocusChanged { socket, pid, start_time, pane, focus_in, .. } => {
            plan_focus_changed(socket, *pid, *start_time, pane, *focus_in, inputs)
        }
        Command::Dnd { action } => plan_dnd(*action, inputs),
        Command::Jump => Invocation::control(inputs, Control::Jump, Output::Confirm(None)),
        Command::Status { json, tmux } => {
            let format = match (json, tmux) {
                (true, _) => StatusFormat::Json,
                (false, true) => StatusFormat::Tmux,
                (false, false) => StatusFormat::Text,
            };
            Invocation::query(inputs, Query::Status, Output::Status(format))
        }
        Command::Doctor => Invocation::nothing("doctor gathers its own facts in main".to_string()),
    }
```

Add:

```rust
fn plan_focus_changed(socket: &Path, pid: u32, start_time: i64, pane: &str, focused: bool, inputs: &Inputs) -> Invocation {
    let (Ok(socket_path), Ok(pane)) = (SocketPath::parse(socket), PaneId::parse(pane)) else {
        return Invocation { output: Output::Silent, ..Invocation::usage("focus-changed needs an absolute --socket and a %pane id") };
    };
    let pane = PaneKey { server: ServerId { socket_path, pid, start_time }, pane };
    Invocation::control(inputs, Control::FocusChanged { pane, focused }, Output::Silent)
}

fn plan_dnd(action: DndAction, inputs: &Inputs) -> Invocation {
    match action {
        DndAction::On { for_minutes } => Invocation::control(
            inputs,
            Control::Dnd { command: DndCommand::On { for_minutes } },
            Output::Confirm(Some("do-not-disturb is on")),
        ),
        DndAction::Off => {
            Invocation::control(inputs, Control::Dnd { command: DndCommand::Off }, Output::Confirm(Some("do-not-disturb is off")))
        }
        DndAction::Status => Invocation::query(inputs, Query::Status, Output::DndStatus),
    }
}
```

Extend the imports of `invocation.rs`: `std::path::Path`, `crate::cli::DndAction`, `agent_notify_core::wire::{Control, DndCommand, Query}`, `agent_notify_core::{PaneId, PaneKey, ServerId, SocketPath}`.

- [ ] **Step 5: Skip the origin reads for the new commands**

At the top of `context::gather`, before `let stdin = ...`:

```rust
    if !command.reads_origin() {
        let cwd = std::env::current_dir().unwrap_or_default();
        return Inputs { now, event_id: EventId::from_parts(now, random), cwd, pane: PaneLookup::NotInTmux, repo: None, stdin: None };
    }
```

- [ ] **Step 6: Run the planner tests**

Run: `cd ~/crates && cargo test -p agent-notify invocation`
Expected: pass. `main.rs` does not compile yet if it matches on `Command` exhaustively; Step 7 fixes that.

- [ ] **Step 7: Write the failing process tests**

Append to `crates/agent-notify/tests/process.rs`:

```rust
use std::io::{BufRead, BufReader, Write as _};
use std::os::unix::net::UnixListener;

fn answer_once(path: &std::path::Path, reply: String) -> std::thread::JoinHandle<Option<String>> {
    let listener = UnixListener::bind(path).expect("bind");
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().ok()?;
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line).ok()?;
        (&stream).write_all(reply.as_bytes()).ok()?;
        Some(line)
    })
}

const ACK: &str = "{\"ok\":true,\"daemon\":\"stamp\"}\n";

fn focus_arguments() -> [&'static str; 11] {
    ["focus-changed", "--socket", "/tmp/tmux-501/default", "--pid", "77", "--start-time", "1700000000", "--pane", "%3", "--in", "--"]
}

#[test]
fn focus_changed_sends_a_control_and_prints_nothing() {
    let sandbox = Sandbox::new();
    let received = answer_once(&sandbox.socket, ACK.to_string());
    let arguments = focus_arguments();
    let output = sandbox.run(&mut sandbox.command(&arguments[..10]), "");
    assert!(output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty() && output.stderr.is_empty(), "{output:?}");
    let line = received.join().expect("listener thread").expect("a request line");
    let request: serde_json::Value = serde_json::from_str(&line).expect("JSON");
    assert_eq!(request["control"]["type"], "focus_changed");
    assert_eq!(request["control"]["pane"]["pane"], "%3");
    assert_eq!(request["control"]["focused"], true);
}

#[test]
fn focus_changed_without_a_daemon_is_silent_and_succeeds() {
    let sandbox = Sandbox::new();
    let arguments = focus_arguments();
    let output = sandbox.run(&mut sandbox.command(&arguments[..10]), "");
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert!(sandbox.fallback_calls().is_empty());
}

#[test]
fn dnd_on_without_a_daemon_fails_and_names_doctor() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&mut sandbox.command(&["dnd", "on"]), "");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("agent-notify doctor"), "{output:?}");
    assert!(sandbox.fallback_calls().is_empty());
}

#[test]
fn dnd_on_with_a_daemon_confirms() {
    let sandbox = Sandbox::new();
    let received = answer_once(&sandbox.socket, ACK.to_string());
    let output = sandbox.run(&mut sandbox.command(&["dnd", "on", "--for-minutes", "30"]), "");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "do-not-disturb is on");
    let line = received.join().expect("listener thread").expect("a request line");
    assert!(line.contains("\"for_minutes\":30"), "{line}");
}
```

The `"--"` in `focus_arguments` only pads the array; the tests pass the first ten items.

- [ ] **Step 8: Run them to see them fail**

Run: `cd ~/crates && cargo test -p agent-notify --test process focus_changed dnd_on`
Expected: compile error in `main.rs` (non-exhaustive `Output` handling), or failures.

- [ ] **Step 9: Execute the new outputs in `main.rs`**

Create `crates/agent-notify/src/render.rs`:

```rust
//! Turns daemon answers into text. Pure: `main` prints the result.

use crate::client::Delivery;

/// One line that says why the daemon gave no usable answer.
pub fn delivery_problem(delivery: &Delivery) -> String {
    match delivery {
        Delivery::Acked { .. } => "the daemon answered".to_string(),
        Delivery::Rejected { code } => format!("the daemon refused the request ({code:?})"),
        Delivery::NoSocket(reason) => format!("no socket: {reason}"),
        Delivery::ConnectFailed(kind) => format!("the daemon is not running ({kind})"),
        Delivery::TimedOut => "the daemon did not answer in time".to_string(),
        Delivery::BadReply(message) => format!("the daemon sent a bad reply: {message}"),
    }
}
```

Add `mod render;` to `main.rs`. In `execute`, replace everything from `let delivery = match socket_location(...` to the end of the function with:

```rust
    let socket = socket_location(&env.path_inputs());
    let delivery = match &socket {
        SocketLocation::At(path) => client::deliver(path, &line, client::BUDGET),
        SocketLocation::Unavailable(reason) => client::Delivery::NoSocket(reason),
    };
    tracing::info!(event_id, request = ?request, delivery = ?delivery, "sent");
    match invocation.output {
        Output::FallbackOnFailure => {
            if !delivery.acknowledged() {
                fall_back(command, invocation, event_id);
            }
            exit_code(invocation.exit)
        }
        Output::Silent => exit_code(invocation.exit),
        Output::Confirm(message) => confirm(&delivery, message),
        Output::Status(_) | Output::DndStatus => {
            eprintln!("agent-notify: status is not built yet");
            ExitCode::from(1)
        }
    }
}

fn confirm(delivery: &client::Delivery, message: Option<&str>) -> ExitCode {
    if delivery.acknowledged() {
        if let Some(message) = message {
            println!("{message}");
        }
        return ExitCode::SUCCESS;
    }
    eprintln!("agent-notify: {}; run `agent-notify doctor`", render::delivery_problem(delivery));
    ExitCode::from(1)
}
```

Change the import `use crate::invocation::{Exit, Inputs, Invocation, PaneLookup};` to also bring in `Output`. In `run`, before `let inputs = ...`, add a temporary arm for `doctor` (Task 7 replaces it):

```rust
    if matches!(cli.command, Command::Doctor) {
        eprintln!("agent-notify: doctor is not built yet");
        return ExitCode::from(1);
    }
```

- [ ] **Step 10: Run all CLI tests and clippy**

Run: `cd ~/crates && cargo test -p agent-notify && cargo clippy -p agent-notify --all-targets -- -D warnings`
Expected: all pass, the plan 1 process tests included.

- [ ] **Step 11: Commit**

```bash
cd ~ && config add crates/agent-notify
config commit -m "Add focus-changed, dnd and jump to the agent-notify CLI" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/agent-notify
```

---

### Task 6: `status`, `status --json`, `status --tmux` and `dnd status`

**Files:**
- Modify: `crates/agent-notify/src/render.rs`, `src/main.rs`
- Test: `crates/agent-notify/tests/process.rs`

**Interfaces:**
- Consumes: Task 2 `StatusReport`; Task 4 `Delivery::Acked { data }`; Task 5 `Output`, `StatusFormat`.
- Produces:
  - `render::Printed { stdout: String, stderr: Option<String>, success: bool }`.
  - `render::status_output(StatusFormat, &Delivery, socket_file_exists: bool) -> Printed`, `render::dnd_output(&Delivery) -> Printed`.
  - `render::status_text(&StatusReport) -> String`, `render::tmux_segment(Option<&StatusReport>, socket_file_exists: bool) -> String`, `render::duration_text(i64) -> String`.

- [ ] **Step 1: Write the failing unit tests**

Append to `render.rs`:

```rust
#[cfg(test)]
mod tests {
    use agent_notify_core::status::{DaemonFacts, Decision, DndStatus, SessionStatus, StatusReport, ToolStatus};
    use agent_notify_core::{EpochMs, EventId, SessionId};

    use super::*;
    use crate::invocation::StatusFormat;

    fn report(dnd_on: bool) -> StatusReport {
        StatusReport {
            daemon: DaemonFacts {
                stamp: "stamp-1".to_string(),
                started_ms: EpochMs(0),
                tools: vec![ToolStatus { name: "tmux".to_string(), path: Some("/opt/homebrew/bin/tmux".into()) }],
            },
            now_ms: EpochMs(3 * 3_600_000 + 12 * 60_000),
            dnd: DndStatus { on: dnd_on, until_ms: None },
            sessions: vec![SessionStatus {
                session: SessionId::parse("session-a").expect("an id"),
                pane: Some("%3".to_string()),
                location: "4 · code/feature-x".to_string(),
                turn: "needs_you".to_string(),
                quiet: "armed".to_string(),
                card: Some("attention showing".to_string()),
            }],
            decisions: vec![Decision {
                at_ms: EpochMs(3 * 3_600_000 + 11 * 60_000),
                event_id: EventId::from_parts(EpochMs(1), [0; 10]),
                kind: "needs_input".to_string(),
                outcome: "card".to_string(),
            }],
        }
    }

    fn acked(report: &StatusReport) -> Delivery {
        Delivery::Acked { daemon: Some("stamp-1".to_string()), data: Some(serde_json::to_value(report).expect("encodes")) }
    }

    #[test]
    fn durations_read_naturally() {
        assert_eq!(duration_text(45_000), "45s");
        assert_eq!(duration_text(5 * 60_000 + 3_000), "5m");
        assert_eq!(duration_text(3 * 3_600_000 + 12 * 60_000), "3h 12m");
        assert_eq!(duration_text(-5), "0s");
    }

    #[test]
    fn status_text_names_the_daemon_dnd_sessions_and_decisions() {
        let text = status_text(&report(false));
        assert!(text.contains("agent-notifyd stamp-1, up 3h 12m"), "{text}");
        assert!(text.contains("do-not-disturb: off"), "{text}");
        assert!(text.contains("%3  4 · code/feature-x  needs_you  quiet armed  attention showing"), "{text}");
        assert!(text.contains("1m ago  needs_input  card"), "{text}");
        assert!(text.contains("tmux: /opt/homebrew/bin/tmux"), "{text}");
    }

    #[test]
    fn the_tmux_segment_shows_dnd_and_offline_only() {
        assert_eq!(tmux_segment(Some(&report(true)), true), "#[fg=yellow]DND#[default] ");
        assert_eq!(tmux_segment(Some(&report(false)), true), "");
        assert_eq!(tmux_segment(None, true), "#[fg=red]notify offline#[default] ");
        assert_eq!(tmux_segment(None, false), "");
    }

    #[test]
    fn status_output_by_format() {
        let delivery = acked(&report(false));
        let json = status_output(StatusFormat::Json, &delivery, true);
        assert!(json.success);
        assert_eq!(serde_json::from_str::<StatusReport>(&json.stdout).expect("JSON"), report(false));
        let down = status_output(StatusFormat::Text, &Delivery::TimedOut, true);
        assert!(!down.success);
        assert!(down.stderr.as_deref().is_some_and(|line| line.contains("did not answer in time")));
        let tmux = status_output(StatusFormat::Tmux, &Delivery::TimedOut, true);
        assert!(tmux.success, "the tmux segment never fails the status line");
        assert_eq!(tmux.stdout, "#[fg=red]notify offline#[default] ");
    }

    #[test]
    fn dnd_output_prints_one_word() {
        assert_eq!(dnd_output(&acked(&report(true))).stdout, "on\n");
        assert_eq!(dnd_output(&acked(&report(false))).stdout, "off\n");
        assert!(!dnd_output(&Delivery::TimedOut).success);
    }

    #[test]
    fn a_reply_without_a_report_is_a_failure() {
        let delivery = Delivery::Acked { daemon: None, data: None };
        assert!(!status_output(StatusFormat::Text, &delivery, true).success);
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd ~/crates && cargo test -p agent-notify render`
Expected: compile errors, `cannot find function status_text`.

- [ ] **Step 3: Implement**

Add to `render.rs`, above the tests (extend the `use` lines):

```rust
use agent_notify_core::EpochMs;
use agent_notify_core::status::StatusReport;

use crate::invocation::StatusFormat;

/// What `main` prints and how it exits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Printed {
    /// For stdout, with its own newlines.
    pub stdout: String,
    /// One line for stderr.
    pub stderr: Option<String>,
    /// Exit 0 when true, 1 when false.
    pub success: bool,
}

impl Printed {
    fn ok(stdout: String) -> Self {
        Printed { stdout, stderr: None, success: true }
    }

    fn failed(problem: String) -> Self {
        Printed { stdout: String::new(), stderr: Some(format!("{problem}; run `agent-notify doctor`")), success: false }
    }
}

/// The `status` output in `format`. `socket_file_exists` tells "no daemon
/// installed" (print nothing in tmux) from "daemon installed but down".
pub fn status_output(format: StatusFormat, delivery: &Delivery, socket_file_exists: bool) -> Printed {
    let report = report_from(delivery);
    match (format, report) {
        (StatusFormat::Tmux, report) => Printed::ok(tmux_segment(report.as_ref().ok(), socket_file_exists)),
        (StatusFormat::Json, Ok(report)) => match serde_json::to_string_pretty(&report) {
            Ok(text) => Printed::ok(format!("{text}\n")),
            Err(error) => Printed::failed(format!("the report did not encode: {error}")),
        },
        (StatusFormat::Text, Ok(report)) => Printed::ok(status_text(&report)),
        (StatusFormat::Json | StatusFormat::Text, Err(problem)) => Printed::failed(problem),
    }
}

/// `dnd status`: `on` or `off`.
pub fn dnd_output(delivery: &Delivery) -> Printed {
    match report_from(delivery) {
        Ok(report) => Printed::ok(if report.dnd.on { "on\n" } else { "off\n" }.to_string()),
        Err(problem) => Printed::failed(problem),
    }
}

fn report_from(delivery: &Delivery) -> Result<StatusReport, String> {
    match delivery {
        Delivery::Acked { data: Some(data), .. } => {
            serde_json::from_value(data.clone()).map_err(|error| format!("the daemon sent a report this CLI cannot read: {error}"))
        }
        Delivery::Acked { data: None, .. } => Err("the daemon answered without a report".to_string()),
        other => Err(delivery_problem(other)),
    }
}

/// The report for a person.
pub fn status_text(report: &StatusReport) -> String {
    let now = report.now_ms;
    let mut lines = vec![
        format!("agent-notifyd {}, up {}", report.daemon.stamp, duration_text(now.since(report.daemon.started_ms))),
        format!("do-not-disturb: {}", dnd_text(report, now)),
    ];
    lines.push("tools:".to_string());
    for tool in &report.daemon.tools {
        let path = tool.path.as_ref().map_or_else(|| "not found".to_string(), |path| path.display().to_string());
        lines.push(format!("  {}: {path}", tool.name));
    }
    lines.push(format!("sessions: {}", report.sessions.len()));
    for session in &report.sessions {
        let pane = session.pane.as_deref().unwrap_or("-");
        let card = session.card.as_deref().unwrap_or("no card");
        lines.push(format!("  {pane}  {}  {}  quiet {}  {card}", session.location, session.turn, session.quiet));
    }
    lines.push("decisions, newest last:".to_string());
    for decision in &report.decisions {
        lines.push(format!(
            "  {} ago  {}  {}  {}",
            duration_text(now.since(decision.at_ms)),
            decision.kind,
            decision.outcome,
            decision.event_id
        ));
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

fn dnd_text(report: &StatusReport, now: EpochMs) -> String {
    match (report.dnd.on, report.dnd.until_ms) {
        (false, _) => "off".to_string(),
        (true, None) => "on".to_string(),
        (true, Some(until)) => format!("on, {} left", duration_text(until.since(now))),
    }
}

/// The tmux status-line segment. Empty unless do-not-disturb is on or an
/// installed daemon does not answer.
pub fn tmux_segment(report: Option<&StatusReport>, socket_file_exists: bool) -> String {
    match report {
        Some(report) if report.dnd.on => "#[fg=yellow]DND#[default] ".to_string(),
        Some(_) => String::new(),
        None if socket_file_exists => "#[fg=red]notify offline#[default] ".to_string(),
        None => String::new(),
    }
}

/// `45s`, `5m`, `3h 12m`. Negative spans read as `0s`.
pub fn duration_text(milliseconds: i64) -> String {
    let seconds = milliseconds.max(0) / 1_000;
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3_600 => format!("{}m", seconds / 60),
        _ => format!("{}h {}m", seconds / 3_600, seconds % 3_600 / 60),
    }
}
```

In `main.rs`, replace the temporary `Output::Status(_) | Output::DndStatus` arm with:

```rust
        Output::Status(format) => {
            let socket_file_exists = matches!(&socket, SocketLocation::At(path) if path.exists());
            emit(render::status_output(format, &delivery, socket_file_exists))
        }
        Output::DndStatus => emit(render::dnd_output(&delivery)),
```

and add:

```rust
fn emit(printed: render::Printed) -> ExitCode {
    print!("{}", printed.stdout);
    if let Some(line) = printed.stderr {
        eprintln!("agent-notify: {line}");
    }
    if printed.success { ExitCode::SUCCESS } else { ExitCode::from(1) }
}
```

- [ ] **Step 4: Write the failing process tests**

Append to `tests/process.rs`:

```rust
#[test]
fn status_tmux_prints_nothing_without_a_socket_and_offline_with_a_stale_one() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&mut sandbox.command(&["status", "--tmux"]), "");
    assert!(output.status.success());
    assert!(output.stdout.is_empty(), "{output:?}");
    std::fs::write(&sandbox.socket, "").expect("a stale regular file");
    let output = sandbox.run(&mut sandbox.command(&["status", "--tmux"]), "");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "#[fg=red]notify offline#[default] ");
}

#[test]
fn status_prints_the_daemon_report() {
    let sandbox = Sandbox::new();
    let report = serde_json::json!({
        "daemon": { "stamp": "stamp-9", "started_ms": 0, "tools": [] },
        "now_ms": 60_000,
        "dnd": { "on": true, "until_ms": null },
        "sessions": [],
        "decisions": []
    });
    let reply = format!("{}\n", serde_json::json!({ "ok": true, "daemon": "stamp-9", "data": report }));
    let received = answer_once(&sandbox.socket, reply);
    let output = sandbox.run(&mut sandbox.command(&["status"]), "");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("agent-notifyd stamp-9, up 1m"), "{stdout}");
    assert!(stdout.contains("do-not-disturb: on"), "{stdout}");
    let line = received.join().expect("listener thread").expect("a request line");
    assert!(line.contains("\"query\":\"status\""), "{line}");
}
```

- [ ] **Step 5: Run the tests and clippy**

Run: `cd ~/crates && cargo test -p agent-notify && cargo clippy -p agent-notify --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
cd ~ && config add crates/agent-notify
config commit -m "Add status output to the agent-notify CLI" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/agent-notify
```

---

### Task 7: `doctor`

**Files:**
- Create: `crates/agent-notify/src/doctor.rs`
- Modify: `crates/agent-notify/src/main.rs`, `src/tmux.rs` (`show_global_option`)
- Test: `crates/agent-notify/tests/process.rs`

**Interfaces:**
- Consumes: Task 2 `StatusReport`, `state_directory`; Task 6 `render::delivery_problem`; plan 1 `tmux::{socket_from_tmux_env, read_pane, parse_pane_line}`, `context::EnvSnapshot`, `client::deliver`.
- Produces:
  - `doctor::DoctorFacts { platform, cli_on_path: Option<PathBuf>, cli_self: Option<PathBuf>, socket: SocketLocation, status: Result<StatusReport, String>, installed_stamp: Option<String>, settings: Option<String>, in_tmux_pane: bool, pane_session_tag: Option<bool>, focus_events: Option<bool>, fallback_notifier: Option<PathBuf>, launchd: Option<LaunchdState> }`.
  - `doctor::LaunchdState::{Loaded, NotLoaded, Disabled}`; `doctor::CheckStatus::{Pass, Warn, Fail}`; `doctor::Check { name: &'static str, status: CheckStatus, detail: String }`.
  - `doctor::evaluate(&DoctorFacts) -> Vec<Check>`, `doctor::render(&[Check]) -> String`, `doctor::any_failed(&[Check]) -> bool`, `doctor::parse_disabled(&str, label: &str) -> bool`, `doctor::gather(&EnvSnapshot) -> DoctorFacts`.
  - `doctor::PLUGIN_KEY = "agent-notify@dotfiles-tools"`, `doctor::LAUNCHD_LABEL = "dev.dotfiles.agent-notifyd"`.
  - `tmux::show_global_option(&SocketPath, &str) -> io::Result<String>`.

- [ ] **Step 1: Write the failing unit tests**

Create `crates/agent-notify/src/doctor.rs` with the tests first:

```rust
#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use agent_notify_core::EpochMs;
    use agent_notify_core::paths::{Platform, SocketLocation};
    use agent_notify_core::status::{DaemonFacts, DndStatus, StatusReport, ToolStatus};

    use super::*;

    const GOOD_SETTINGS: &str = r#"{
        "enabledPlugins": { "agent-notify@dotfiles-tools": true },
        "permissions": { "allow": ["Bash(agent-notify send:*)", "Bash(agent-notify quiet:*)"] }
    }"#;

    fn report(stamp: &str, aerospace: Option<&str>) -> StatusReport {
        StatusReport {
            daemon: DaemonFacts {
                stamp: stamp.to_string(),
                started_ms: EpochMs(0),
                tools: vec![
                    ToolStatus { name: "tmux".to_string(), path: Some("/opt/homebrew/bin/tmux".into()) },
                    ToolStatus { name: "aerospace".to_string(), path: aerospace.map(PathBuf::from) },
                ],
            },
            now_ms: EpochMs(1),
            dnd: DndStatus { on: false, until_ms: None },
            sessions: Vec::new(),
            decisions: Vec::new(),
        }
    }

    fn healthy() -> DoctorFacts {
        DoctorFacts {
            platform: Platform::MacOs,
            cli_on_path: Some(PathBuf::from("/home/dev/.local/bin/agent-notify")),
            cli_self: Some(PathBuf::from("/home/dev/.local/bin/agent-notify")),
            socket: SocketLocation::At(PathBuf::from("/home/dev/Library/Application Support/agent-notify/agent-notify.sock")),
            status: Ok(report("stamp-1", Some("/opt/homebrew/bin/aerospace"))),
            installed_stamp: Some("stamp-1".to_string()),
            settings: Some(GOOD_SETTINGS.to_string()),
            in_tmux_pane: true,
            pane_session_tag: Some(true),
            focus_events: Some(true),
            fallback_notifier: Some(PathBuf::from("/opt/homebrew/bin/terminal-notifier")),
            launchd: Some(LaunchdState::Loaded),
        }
    }

    fn status_of(checks: &[Check], name: &str) -> CheckStatus {
        checks.iter().find(|check| check.name == name).map(|check| check.status).unwrap_or_else(|| panic!("no check {name}"))
    }

    #[test]
    fn a_healthy_install_passes_everything() {
        let checks = evaluate(&healthy());
        assert!(checks.iter().all(|check| check.status == CheckStatus::Pass), "{checks:?}");
        assert!(!any_failed(&checks));
    }

    #[test]
    fn a_dead_daemon_fails() {
        let facts = DoctorFacts { status: Err("the daemon is not running".to_string()), ..healthy() };
        let checks = evaluate(&facts);
        assert_eq!(status_of(&checks, "daemon answers"), CheckStatus::Fail);
        assert!(any_failed(&checks));
    }

    #[test]
    fn a_stale_daemon_warns_with_the_restart_command() {
        let facts = DoctorFacts { installed_stamp: Some("stamp-2".to_string()), ..healthy() };
        let checks = evaluate(&facts);
        assert_eq!(status_of(&checks, "daemon is current"), CheckStatus::Warn);
        let detail = &checks.iter().find(|check| check.name == "daemon is current").expect("the check").detail;
        assert!(detail.contains("launchctl kickstart -k"), "{detail}");
    }

    #[test]
    fn missing_settings_entries_are_named() {
        let facts = DoctorFacts { settings: Some("{}".to_string()), ..healthy() };
        let checks = evaluate(&facts);
        assert_eq!(status_of(&checks, "plugin enabled"), CheckStatus::Fail);
        assert_eq!(status_of(&checks, "permission rules"), CheckStatus::Warn);
    }

    #[test]
    fn tmux_problems_are_reported() {
        let facts = DoctorFacts { pane_session_tag: Some(false), focus_events: Some(false), ..healthy() };
        let checks = evaluate(&facts);
        assert_eq!(status_of(&checks, "session tag"), CheckStatus::Warn);
        assert_eq!(status_of(&checks, "focus events"), CheckStatus::Fail);
    }

    #[test]
    fn a_disabled_login_item_fails_with_where_to_fix_it() {
        let facts = DoctorFacts { launchd: Some(LaunchdState::Disabled), ..healthy() };
        let checks = evaluate(&facts);
        assert_eq!(status_of(&checks, "login item"), CheckStatus::Fail);
    }

    #[test]
    fn missing_tools_in_the_daemon_environment() {
        let facts = DoctorFacts { status: Ok(report("stamp-1", None)), ..healthy() };
        assert_eq!(status_of(&evaluate(&facts), "daemon tools"), CheckStatus::Warn);
    }

    #[test]
    fn launchctl_disabled_output_parses_both_spellings() {
        let label = LAUNCHD_LABEL;
        assert!(parse_disabled("\t\"dev.dotfiles.agent-notifyd\" => disabled\n", label));
        assert!(parse_disabled("\t\"dev.dotfiles.agent-notifyd\" => true\n", label));
        assert!(!parse_disabled("\t\"dev.dotfiles.agent-notifyd\" => enabled\n", label));
        assert!(!parse_disabled("\t\"other\" => disabled\n", label));
    }

    #[test]
    fn render_puts_one_check_per_line() {
        let text = render(&evaluate(&healthy()));
        assert!(text.lines().all(|line| line.starts_with("ok    ") || line.starts_with("warn  ") || line.starts_with("FAIL  ")), "{text}");
    }
}
```

Add `mod doctor;` to `main.rs`.

- [ ] **Step 2: Run them to see them fail**

Run: `cd ~/crates && cargo test -p agent-notify doctor`
Expected: compile errors, `cannot find function evaluate`.

- [ ] **Step 3: Implement the pure checks**

Put this above the tests in `doctor.rs`:

```rust
//! `agent-notify doctor`: gathers facts about the installation, then checks
//! them with pure rules.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use agent_notify_core::paths::{Platform, SocketLocation, socket_location, state_directory};
use agent_notify_core::status::StatusReport;
use agent_notify_core::wire::{Query, Request, WIRE_VERSION, encode_line};
use agent_notify_core::{EventId, PaneId};

use crate::client::{self, Delivery};
use crate::context::{self, EnvSnapshot};
use crate::{render, tmux};

/// The plugin's key in `enabledPlugins`.
pub const PLUGIN_KEY: &str = "agent-notify@dotfiles-tools";
/// The launchd label and bundle id of the daemon.
pub const LAUNCHD_LABEL: &str = "dev.dotfiles.agent-notifyd";
const PERMISSION_RULES: [&str; 2] = ["Bash(agent-notify send:*)", "Bash(agent-notify quiet:*)"];

/// Whether launchd runs the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchdState {
    /// Loaded in the user's GUI domain.
    Loaded,
    /// Not loaded.
    NotLoaded,
    /// Turned off in Login Items.
    Disabled,
}

/// Everything the checks read.
#[derive(Debug, Clone)]
pub struct DoctorFacts {
    /// This machine's platform.
    pub platform: Platform,
    /// The first `agent-notify` on PATH.
    pub cli_on_path: Option<PathBuf>,
    /// This binary.
    pub cli_self: Option<PathBuf>,
    /// The socket location.
    pub socket: SocketLocation,
    /// The daemon's status report, or why there is none.
    pub status: Result<StatusReport, String>,
    /// The stamp the last install wrote.
    pub installed_stamp: Option<String>,
    /// The text of `~/.claude/settings.json`.
    pub settings: Option<String>,
    /// `$TMUX_PANE` is set.
    pub in_tmux_pane: bool,
    /// Whether this pane has a session tag. `None` outside tmux or when unreadable.
    pub pane_session_tag: Option<bool>,
    /// tmux's `focus-events`. `None` outside tmux.
    pub focus_events: Option<bool>,
    /// terminal-notifier or notify-send.
    pub fallback_notifier: Option<PathBuf>,
    /// The launchd agent. `None` off macOS.
    pub launchd: Option<LaunchdState>,
}

/// A check's result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    /// Fine.
    Pass,
    /// Works, with a gap.
    Warn,
    /// Broken.
    Fail,
}

/// One check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// Short name.
    pub name: &'static str,
    /// Result.
    pub status: CheckStatus,
    /// What was found, and what to do.
    pub detail: String,
}

fn check(name: &'static str, status: CheckStatus, detail: impl Into<String>) -> Check {
    Check { name, status, detail: detail.into() }
}

/// Runs every rule over `facts`.
pub fn evaluate(facts: &DoctorFacts) -> Vec<Check> {
    let mut checks = vec![cli_check(facts), socket_check(&facts.socket), daemon_check(&facts.status)];
    if let Ok(report) = &facts.status {
        checks.push(current_check(report, facts.installed_stamp.as_deref()));
        checks.push(tools_check(report, facts.platform));
    }
    checks.extend(settings_checks(facts.settings.as_deref()));
    checks.extend(tmux_checks(facts));
    checks.push(match &facts.fallback_notifier {
        Some(path) => check("fallback notifier", CheckStatus::Pass, path.display().to_string()),
        None => check("fallback notifier", CheckStatus::Warn, "terminal-notifier or notify-send not found: no notification when the daemon is down"),
    });
    if let Some(state) = facts.launchd {
        checks.push(login_item_check(state));
    }
    checks
}

fn cli_check(facts: &DoctorFacts) -> Check {
    match (&facts.cli_on_path, &facts.cli_self) {
        (None, _) => check("cli on PATH", CheckStatus::Fail, "agent-notify is not on PATH: hooks call ~/.local/bin/agent-notify, run `config build`"),
        (Some(found), Some(this)) if found != this => {
            check("cli on PATH", CheckStatus::Warn, format!("PATH resolves {}, but this is {}", found.display(), this.display()))
        }
        (Some(found), _) => check("cli on PATH", CheckStatus::Pass, found.display().to_string()),
    }
}

fn socket_check(socket: &SocketLocation) -> Check {
    match socket {
        SocketLocation::At(path) => check("socket", CheckStatus::Pass, path.display().to_string()),
        SocketLocation::Unavailable(reason) => check("socket", CheckStatus::Fail, format!("no socket path: {reason}")),
    }
}

fn daemon_check(status: &Result<StatusReport, String>) -> Check {
    match status {
        Ok(report) => check(
            "daemon answers",
            CheckStatus::Pass,
            format!("stamp {}, up {}", report.daemon.stamp, render::duration_text(report.now_ms.since(report.daemon.started_ms))),
        ),
        Err(problem) => check("daemon answers", CheckStatus::Fail, format!("{problem}: run `config build --tools`")),
    }
}

fn current_check(report: &StatusReport, installed: Option<&str>) -> Check {
    match installed {
        Some(stamp) if stamp == report.daemon.stamp => check("daemon is current", CheckStatus::Pass, stamp.to_string()),
        Some(stamp) => check(
            "daemon is current",
            CheckStatus::Warn,
            format!(
                "running {}, installed {stamp}: run `launchctl kickstart -k gui/$(id -u)/{LAUNCHD_LABEL}`",
                report.daemon.stamp
            ),
        ),
        None => check("daemon is current", CheckStatus::Warn, "no install stamp: run `config build --tools`"),
    }
}

fn tools_check(report: &StatusReport, platform: Platform) -> Check {
    let missing: Vec<&str> = report.daemon.tools.iter().filter(|tool| tool.path.is_none()).map(|tool| tool.name.as_str()).collect();
    if missing.contains(&"tmux") {
        return check("daemon tools", CheckStatus::Fail, "the daemon cannot find tmux: clicks cannot switch panes; set AGENT_NOTIFY_TMUX in the launchd plist");
    }
    match (missing.is_empty(), platform) {
        (true, _) => check("daemon tools", CheckStatus::Pass, "every tool resolves"),
        (false, Platform::MacOs) => {
            check("daemon tools", CheckStatus::Warn, format!("not found: {}; the raise uses app activation only", missing.join(", ")))
        }
        (false, _) => check("daemon tools", CheckStatus::Warn, format!("not found: {}", missing.join(", "))),
    }
}

fn settings_checks(settings: Option<&str>) -> Vec<Check> {
    let Some(value) = settings.and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok()) else {
        return vec![check("plugin enabled", CheckStatus::Fail, "~/.claude/settings.json is missing or not JSON")];
    };
    let enabled = value["enabledPlugins"][PLUGIN_KEY] == serde_json::Value::Bool(true);
    let allowed: Vec<&str> = value["permissions"]["allow"].as_array().map(|rules| rules.iter().filter_map(|rule| rule.as_str()).collect()).unwrap_or_default();
    let missing: Vec<&str> = PERMISSION_RULES.iter().copied().filter(|rule| !allowed.contains(rule)).collect();
    vec![
        if enabled {
            check("plugin enabled", CheckStatus::Pass, PLUGIN_KEY)
        } else {
            check("plugin enabled", CheckStatus::Fail, format!("enabledPlugins has no \"{PLUGIN_KEY}\": true"))
        },
        if missing.is_empty() {
            check("permission rules", CheckStatus::Pass, "send and quiet are allowed")
        } else {
            check("permission rules", CheckStatus::Warn, format!("missing from permissions.allow: {}; agents get a prompt", missing.join(", ")))
        },
    ]
}

fn tmux_checks(facts: &DoctorFacts) -> Vec<Check> {
    if !facts.in_tmux_pane {
        return vec![check("tmux pane", CheckStatus::Warn, "$TMUX_PANE is unset: run doctor inside tmux; outside tmux a click only raises the terminal")];
    }
    let mut checks = vec![check("tmux pane", CheckStatus::Pass, "inside tmux")];
    match facts.pane_session_tag {
        Some(true) => checks.push(check("session tag", CheckStatus::Pass, "this pane has @agent_notify_session")),
        Some(false) => checks.push(check(
            "session tag",
            CheckStatus::Warn,
            "this pane has no @agent_notify_session: start Claude Code here, or check that the SessionStart hook runs",
        )),
        None => {}
    }
    match facts.focus_events {
        Some(true) => checks.push(check("focus events", CheckStatus::Pass, "focus-events is on")),
        Some(false) => checks.push(check("focus events", CheckStatus::Fail, "focus-events is off: add `set -g focus-events on` to tmux.conf")),
        None => {}
    }
    checks
}

fn login_item_check(state: LaunchdState) -> Check {
    match state {
        LaunchdState::Loaded => check("login item", CheckStatus::Pass, format!("{LAUNCHD_LABEL} is loaded")),
        LaunchdState::NotLoaded => check("login item", CheckStatus::Fail, format!("{LAUNCHD_LABEL} is not loaded: run `config build --tools`")),
        LaunchdState::Disabled => check(
            "login item",
            CheckStatus::Fail,
            "turned off: System Settings > General > Login Items & Extensions > Allow in the Background",
        ),
    }
}

/// True when `launchctl print-disabled` lists `label` as disabled. macOS has
/// printed both `=> disabled` and `=> true` for that.
pub fn parse_disabled(output: &str, label: &str) -> bool {
    let quoted = format!("\"{label}\"");
    output.lines().any(|line| {
        let mut parts = line.trim().splitn(2, "=>");
        parts.next().is_some_and(|key| key.trim() == quoted)
            && parts.next().is_some_and(|value| matches!(value.trim(), "disabled" | "true"))
    })
}

/// One line per check.
pub fn render(checks: &[Check]) -> String {
    checks
        .iter()
        .map(|check| {
            let tag = match check.status {
                CheckStatus::Pass => "ok    ",
                CheckStatus::Warn => "warn  ",
                CheckStatus::Fail => "FAIL  ",
            };
            format!("{tag}{}: {}\n", check.name, check.detail)
        })
        .collect()
}

/// True when any check failed.
pub fn any_failed(checks: &[Check]) -> bool {
    checks.iter().any(|check| check.status == CheckStatus::Fail)
}
```

- [ ] **Step 4: Implement the fact gathering**

Append to `doctor.rs`, above the tests:

```rust
/// Reads every fact. Each read that fails becomes `None` or an `Err`, never a panic.
pub fn gather(env: &EnvSnapshot) -> DoctorFacts {
    let platform = Platform::current();
    let socket = socket_location(&env.path_inputs());
    let tmux_socket = env.tmux.as_deref().and_then(tmux::socket_from_tmux_env);
    let pane = env.tmux_pane.as_deref().and_then(|pane| PaneId::parse(pane).ok());
    let pane_session_tag = match (&tmux_socket, &pane) {
        (Some(socket), Some(pane)) => tmux::read_pane(socket, pane)
            .ok()
            .and_then(|line| tmux::parse_pane_line(&line, socket.clone(), pane.clone()).ok())
            .map(|facts| facts.session_tag.is_some()),
        _ => None,
    };
    let focus_events = tmux_socket.as_ref().and_then(|socket| tmux::show_global_option(socket, "focus-events").ok()).map(|value| value.trim() == "on");
    let notifier = match platform {
        Platform::MacOs => "terminal-notifier",
        Platform::Linux | Platform::Other => "notify-send",
    };
    DoctorFacts {
        platform,
        cli_on_path: find_on_path("agent-notify"),
        cli_self: std::env::current_exe().ok(),
        status: query_status(&socket),
        socket,
        installed_stamp: state_directory(&env.path_inputs())
            .and_then(|directory| std::fs::read_to_string(directory.join("installed-stamp")).ok())
            .map(|text| text.trim().to_string()),
        settings: env.home.as_ref().and_then(|home| std::fs::read_to_string(home.join(".claude/settings.json")).ok()),
        in_tmux_pane: env.tmux_pane.is_some(),
        pane_session_tag,
        focus_events,
        fallback_notifier: find_on_path(notifier),
        launchd: (platform == Platform::MacOs).then(launchd_state),
    }
}

fn query_status(socket: &SocketLocation) -> Result<StatusReport, String> {
    let SocketLocation::At(path) = socket else { return Err("no socket path".to_string()) };
    let now = context::clock();
    let request = Request::Query { v: WIRE_VERSION, event_id: EventId::from_parts(now, context::random_bytes()), query: Query::Status };
    let line = encode_line(&request).map_err(|error| error.to_string())?;
    match client::deliver(path, &line, client::BUDGET) {
        Delivery::Acked { data: Some(data), .. } => serde_json::from_value(data).map_err(|error| format!("unreadable report: {error}")),
        other => Err(render::delivery_problem(&other)),
    }
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|directory| directory.join(name)).find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

fn launchd_state() -> LaunchdState {
    let Some(uid) = run_text("/usr/bin/id", &["-u"]).map(|text| text.trim().to_string()) else { return LaunchdState::NotLoaded };
    let domain = format!("gui/{uid}");
    if run_text("/bin/launchctl", &["print-disabled", &domain]).is_some_and(|output| parse_disabled(&output, LAUNCHD_LABEL)) {
        return LaunchdState::Disabled;
    }
    let loaded = Command::new("/bin/launchctl")
        .args(["print", &format!("{domain}/{LAUNCHD_LABEL}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if loaded { LaunchdState::Loaded } else { LaunchdState::NotLoaded }
}

fn run_text(program: &str, arguments: &[&str]) -> Option<String> {
    let output = Command::new(program).args(arguments).stdin(Stdio::null()).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}
```

Add to `tmux.rs`:

```rust
/// Reads one global option's value, for example `focus-events`.
///
/// # Errors
///
/// Returns the spawn error, or tmux's stderr when tmux fails.
pub fn show_global_option(socket: &SocketPath, name: &str) -> io::Result<String> {
    run_tmux(socket, &["show-options", "-g", "-v", name])
}
```

In `main.rs`, replace the temporary doctor arm in `run` with:

```rust
    if matches!(cli.command, Command::Doctor) {
        let checks = doctor::evaluate(&doctor::gather(env));
        print!("{}", doctor::render(&checks));
        return if doctor::any_failed(&checks) { ExitCode::from(1) } else { ExitCode::SUCCESS };
    }
```

- [ ] **Step 5: Write the process test**

Append to `tests/process.rs`:

```rust
#[test]
fn doctor_without_a_daemon_fails_and_names_the_check() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&mut sandbox.command(&["doctor"]), "");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("FAIL  daemon answers"), "{stdout}");
}
```

- [ ] **Step 6: Run the tests and clippy**

Run: `cd ~/crates && cargo test -p agent-notify && cargo clippy -p agent-notify --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 7: Run doctor by hand**

Run: `cd ~/crates && cargo run -q -p agent-notify -- doctor; echo "exit $?"`
Expected: exit 1, `FAIL  daemon answers` (no daemon exists yet), and real values for the PATH, settings and tmux checks. Read each line. A check that reports something you know is false is a bug in `gather`: fix it before you commit.

- [ ] **Step 8: Commit**

```bash
cd ~ && config add crates/agent-notify
config commit -m "Add agent-notify doctor" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/agent-notify
```

---

### Task 8: The tools workspace and the daemon's leaf modules

This task creates `~/tools/agent-notify` and the `agent-notify-daemon` library with the modules that have no daemon logic yet: tool paths, the environment snapshot, the service traits, the `ioreg` parsers and the AeroSpace adapter.

**Files:**
- Create: `tools/agent-notify/Cargo.toml`, `deny.toml`, `.gitignore`, `.cargo/config.toml`, `bunfig.toml`
- Create: `tools/agent-notify/daemon/Cargo.toml`, `daemon/src/lib.rs`, `src/environment.rs`, `src/tools.rs`, `src/services.rs`, `src/presence.rs`, `src/aerospace.rs`
- Generated: `tools/agent-notify/Cargo.lock`

**Interfaces:**
- Consumes: plan 1 `PathInputs`, `Platform`, `Frontmost`, `PanelViewModel`, `EpochMs`, `Dnd`; Task 2 `DaemonFacts`, `ToolStatus`.
- Produces (`agent_notify_daemon`):
  - `environment::DaemonEnvironment { home, xdg_runtime_dir, xdg_state_home, socket_override, log_dir_override, path: Option<OsString>, tmux_override, aerospace_override }` (paths are `Option<PathBuf>`), `DaemonEnvironment::read() -> Self`, `path_inputs(&self) -> PathInputs<'_>`.
  - `tools::{ToolPaths { tmux: Option<PathBuf>, aerospace: Option<PathBuf> }, resolve(name: &str, override_path: Option<&Path>, path: Option<&OsStr>, is_executable: impl Fn(&Path) -> bool) -> Option<PathBuf>, is_executable(&Path) -> bool}`; `ToolPaths::discover(&DaemonEnvironment) -> Self`, `ToolPaths::statuses(&self) -> Vec<ToolStatus>`.
  - `services::{ToastPresenter, WindowRaiser, FrontmostReader, PresenceReader, SoundPlayer}` traits, `AttachRequest { tmux: PathBuf, socket: PathBuf, session: String }`, `Presence { idle_seconds: Option<u32>, screen_locked: Option<bool> }`, `Clock`, `system_clock() -> Clock`, and the neutral implementations `NoRaiser`, `UnknownFrontmost`, `UnknownPresence`, `Silent`, `NoPresenter`. Task 12 adds the `Services` record.
  - `presence::{parse_hid_idle_seconds(&str) -> Option<u32>, parse_screen_locked(&str) -> Option<bool>, frontmost_from_bundle(Option<&str>) -> Frontmost, TERMINAL_BUNDLE_ID: &str = "org.alacritty", IoregPresence}`.
  - `aerospace::{Aerospace, first_window_id(&str) -> Option<u64>}`; `Aerospace::new(PathBuf)`, `terminal_window(&self) -> Option<u64>`, `focus(&self, window_id: u64) -> bool`.

- [ ] **Step 1: Create the workspace files**

`tools/agent-notify/Cargo.toml`:

```toml
[workspace]
resolver = "3"
members = ["daemon"]

[workspace.package]
edition = "2024"
rust-version = "1.94"
publish = false

[workspace.lints.rust]
unsafe_code = "deny"
missing_docs = "warn"
unused_qualifications = "warn"
rust_2018_idioms = { level = "warn", priority = -1 }

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }

[profile.release]
lto = "thin"
codegen-units = 4
strip = "debuginfo"
```

`tools/agent-notify/.cargo/config.toml` (keeps the 3 GB Tauri target directory out of the dotfiles tree; the path is relative to `tools/agent-notify`):

```toml
[build]
target-dir = "../../.cache/agent-notify/target"
```

`tools/agent-notify/.gitignore`:

```
ui/node_modules/
ui/dist/
```

`tools/agent-notify/bunfig.toml` (so `bun test` at the workspace root runs only `tests/`, not the UI suite, which has its own `bunfig.toml`):

```toml
[test]
root = "tests"
```

`tools/agent-notify/deny.toml`. The license list was checked against the spike's Tauri, tauri-nspanel and objc2 lockfile on 2026-10-15 with cargo-deny 0.20.2: `advisories ok, bans ok, licenses ok, sources ok`, no warnings. Plan 3 adds the Linux targets and their licenses.

```toml
[graph]
targets = ["aarch64-apple-darwin", "x86_64-apple-darwin"]

[advisories]
version = 2
yanked = "deny"
unmaintained = "all"

[licenses]
version = 2
allow = ["MIT", "Apache-2.0", "BSD-3-Clause", "Unicode-3.0", "Zlib", "MPL-2.0", "CC0-1.0", "0BSD", "Unlicense"]
confidence-threshold = 0.9

[licenses.private]
ignore = true

[bans]
multiple-versions = "allow"
wildcards = "deny"
allow-wildcard-paths = true

[sources]
unknown-registry = "deny"
unknown-git = "deny"
```

If a later task's dependency brings a license that is not in the list, add it only when it is OSI-approved and permissive or weak copyleft (MPL-2.0 style), and name the crate and license in that task's commit message. For anything else, stop and ask the user. `cargo deny` warns `license-not-encountered` for an allowed license that no crate uses: delete that entry rather than ignore the warning.

`tools/agent-notify/daemon/Cargo.toml`:

```toml
[package]
name = "agent-notify-daemon"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
publish.workspace = true
description = "The platform-neutral shell of agent-notifyd: socket, actor, tmux adapter."

[lints]
workspace = true

[dependencies]
agent-notify-core = { path = "../../../crates/agent-notify-core", features = ["ts"] }
serde = { version = "=1.0.229", features = ["derive"] }
serde_json = "=1.0.154"
tokio = { version = "=1.53.3", features = ["rt-multi-thread", "net", "io-util", "sync", "time", "macros"] }
tracing = "=0.1.44"
ts-rs = "=12.0.1"

[dev-dependencies]
dotfiles-test-support = { path = "../../../crates/dotfiles-test-support" }
tempfile = "3.27"
```

`tools/agent-notify/daemon/src/lib.rs`:

```rust
//! The Tauri-free shell of `agent-notifyd`. The core crate decides; this
//! crate reads the world, applies effects and serves the socket. Every
//! platform service arrives through [`services`].

pub mod aerospace;
pub mod environment;
pub mod presence;
pub mod services;
pub mod tools;
```

- [ ] **Step 2: Write the failing tests**

`daemon/src/tools.rs`, tests first:

```rust
#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    use super::*;

    fn only(paths: &'static [&'static str]) -> impl Fn(&Path) -> bool {
        move |candidate| paths.iter().any(|path| Path::new(path) == candidate)
    }

    fn search_path(entries: &[&str]) -> OsString {
        std::env::join_paths(entries).expect("joinable")
    }

    #[test]
    fn the_override_wins_when_it_is_an_absolute_executable() {
        let path = search_path(&["/usr/bin"]);
        let found = resolve("tmux", Some(Path::new("/opt/tmux")), Some(&path), only(&["/opt/tmux", "/usr/bin/tmux"]));
        assert_eq!(found, Some(PathBuf::from("/opt/tmux")));
    }

    #[test]
    fn a_bad_override_finds_nothing_rather_than_another_binary() {
        let path = search_path(&["/usr/bin"]);
        assert_eq!(resolve("tmux", Some(Path::new("tmux")), Some(&path), only(&["/usr/bin/tmux"])), None);
        assert_eq!(resolve("tmux", Some(Path::new("/missing/tmux")), Some(&path), only(&["/usr/bin/tmux"])), None);
    }

    #[test]
    fn path_is_searched_in_order_and_relative_entries_are_skipped() {
        let path = search_path(&["bin", "/opt/homebrew/bin", "/usr/local/bin"]);
        let found = resolve("tmux", None, Some(&path), only(&["bin/tmux", "/opt/homebrew/bin/tmux", "/usr/local/bin/tmux"]));
        assert_eq!(found, Some(PathBuf::from("/opt/homebrew/bin/tmux")));
    }

    #[test]
    fn nothing_found_is_none() {
        assert_eq!(resolve("tmux", None, None, only(&[])), None);
        let path = search_path(&["/usr/bin"]);
        assert_eq!(resolve("tmux", None, Some(&path), only(&[])), None);
    }

    #[test]
    fn statuses_name_every_tool() {
        let tools = ToolPaths { tmux: Some(PathBuf::from("/opt/homebrew/bin/tmux")), aerospace: None };
        let names: Vec<(String, bool)> = tools.statuses().into_iter().map(|status| (status.name, status.path.is_some())).collect();
        assert_eq!(names, vec![("tmux".to_string(), true), ("aerospace".to_string(), false)]);
    }
}
```

`daemon/src/presence.rs`, tests first. The sample text is the shape `ioreg` printed on macOS 26.6 on 2026-10-15:

```rust
#[cfg(test)]
mod tests {
    use agent_notify_core::update::Frontmost;

    use super::*;

    const IDLE: &str = "    | |   \"HIDIdleTime\" = 345156583\n    | |   \"HIDParameters\" = {}\n";
    const UNLOCKED: &str = "+-o Root  <class IORegistryEntry>\n    \"IOConsoleUsers\" = ({\"kCGSSessionOnConsoleKey\"=Yes,\"kCGSSessionUserIDKey\"=501})\n";
    const LOCKED: &str =
        "+-o Root  <class IORegistryEntry>\n    \"IOConsoleUsers\" = ({\"CGSSessionScreenIsLocked\"=Yes,\"kCGSSessionOnConsoleKey\"=Yes})\n";

    #[test]
    fn idle_time_is_nanoseconds_rounded_down_to_seconds() {
        assert_eq!(parse_hid_idle_seconds(IDLE), Some(0));
        assert_eq!(parse_hid_idle_seconds("\"HIDIdleTime\" = 125000000000"), Some(125));
    }

    #[test]
    fn unreadable_idle_time_is_none() {
        assert_eq!(parse_hid_idle_seconds(""), None);
        assert_eq!(parse_hid_idle_seconds("\"HIDIdleTime\" = <data>"), None);
        assert_eq!(parse_hid_idle_seconds("\"HIDIdleTime\" = 99999999999999999999999"), None);
    }

    #[test]
    fn screen_lock_reads_the_console_user_record() {
        assert_eq!(parse_screen_locked(LOCKED), Some(true));
        assert_eq!(parse_screen_locked(UNLOCKED), Some(false));
        assert_eq!(parse_screen_locked("+-o Root\n"), None);
    }

    #[test]
    fn the_terminal_is_alacritty_by_bundle_id() {
        assert_eq!(frontmost_from_bundle(Some("org.alacritty")), Frontmost::Terminal);
        assert_eq!(frontmost_from_bundle(Some("com.apple.finder")), Frontmost::Other);
        assert_eq!(frontmost_from_bundle(None), Frontmost::Unknown);
    }
}
```

`daemon/src/aerospace.rs`, tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_window_id_is_used() {
        assert_eq!(first_window_id("4021\n4022\n"), Some(4021));
        assert_eq!(first_window_id("  77  \n"), Some(77));
    }

    #[test]
    fn no_window_is_none() {
        assert_eq!(first_window_id(""), None);
        assert_eq!(first_window_id("\n"), None);
        assert_eq!(first_window_id("error\n"), None);
    }
}
```

Add `mod`s for all three to `lib.rs` (already listed).

- [ ] **Step 3: Run them to see them fail**

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon`
Expected: compile errors, `cannot find function resolve` and the others. The first run also downloads the dependencies and writes `Cargo.lock`.

If `Frontmost` lives in a module other than `agent_notify_core::state` in plan 1's code, fix the `use` line to where it is (`grep -rn "pub enum Frontmost" ~/crates/agent-notify-core/src`). Do the same for every plan 1 path this plan names.

- [ ] **Step 4: Implement `tools.rs`**

Put above the tests:

```rust
//! Absolute paths for the external tools the daemon runs. launchd starts
//! the daemon with a minimal `PATH`, so a bare `tmux` fails with ENOENT
//! (spike S1-7). The daemon resolves each tool once, at startup.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use agent_notify_core::status::ToolStatus;

use crate::environment::DaemonEnvironment;

/// The tools found at startup. `None` turns off the feature that needs it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolPaths {
    /// tmux. Without it a click only raises the terminal.
    pub tmux: Option<PathBuf>,
    /// AeroSpace. Without it the raise uses app activation only.
    pub aerospace: Option<PathBuf>,
}

impl ToolPaths {
    /// Resolves every tool from the overrides and `PATH`.
    #[must_use]
    pub fn discover(environment: &DaemonEnvironment) -> Self {
        let path = environment.path.as_deref();
        ToolPaths {
            tmux: resolve("tmux", environment.tmux_override.as_deref(), path, is_executable),
            aerospace: resolve("aerospace", environment.aerospace_override.as_deref(), path, is_executable),
        }
    }

    /// One row per tool, for `status` and `doctor`.
    #[must_use]
    pub fn statuses(&self) -> Vec<ToolStatus> {
        vec![
            ToolStatus { name: "tmux".to_string(), path: self.tmux.clone() },
            ToolStatus { name: "aerospace".to_string(), path: self.aerospace.clone() },
        ]
    }
}

/// Finds `name`. An override must be an absolute executable; when it is not,
/// the answer is `None`, never a different binary from `PATH`.
pub fn resolve(name: &str, override_path: Option<&Path>, path: Option<&OsStr>, is_executable: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    if let Some(candidate) = override_path {
        return (candidate.is_absolute() && is_executable(candidate)).then(|| candidate.to_path_buf());
    }
    std::env::split_paths(path?)
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(name))
        .find(|candidate| is_executable(candidate))
}

/// True for a regular file with an execute bit.
#[must_use]
pub fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}
```

- [ ] **Step 5: Implement `environment.rs`**

```rust
//! The environment variables the daemon reads, read once at startup.

use std::ffi::OsString;
use std::path::PathBuf;

use agent_notify_core::paths::{PathInputs, Platform};

/// A snapshot of the daemon's environment. Empty values count as unset.
#[derive(Debug, Clone, Default)]
pub struct DaemonEnvironment {
    /// `$HOME`.
    pub home: Option<PathBuf>,
    /// `$XDG_RUNTIME_DIR`.
    pub xdg_runtime_dir: Option<PathBuf>,
    /// `$XDG_STATE_HOME`.
    pub xdg_state_home: Option<PathBuf>,
    /// `$AGENT_NOTIFY_SOCKET`.
    pub socket_override: Option<PathBuf>,
    /// `$AGENT_NOTIFY_LOG_DIR`.
    pub log_dir_override: Option<PathBuf>,
    /// `$PATH`.
    pub path: Option<OsString>,
    /// `$AGENT_NOTIFY_TMUX`, which the launchd plist sets.
    pub tmux_override: Option<PathBuf>,
    /// `$AGENT_NOTIFY_AEROSPACE`, which the launchd plist sets.
    pub aerospace_override: Option<PathBuf>,
}

impl DaemonEnvironment {
    /// Reads the variables.
    #[must_use]
    pub fn read() -> Self {
        let path = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty()).map(PathBuf::from);
        DaemonEnvironment {
            home: path("HOME"),
            xdg_runtime_dir: path("XDG_RUNTIME_DIR"),
            xdg_state_home: path("XDG_STATE_HOME"),
            socket_override: path("AGENT_NOTIFY_SOCKET"),
            log_dir_override: path("AGENT_NOTIFY_LOG_DIR"),
            path: std::env::var_os("PATH").filter(|value| !value.is_empty()),
            tmux_override: path("AGENT_NOTIFY_TMUX"),
            aerospace_override: path("AGENT_NOTIFY_AEROSPACE"),
        }
    }

    /// The facts the core's path rules read.
    #[must_use]
    pub fn path_inputs(&self) -> PathInputs<'_> {
        PathInputs {
            platform: Platform::current(),
            home: self.home.as_deref(),
            xdg_runtime_dir: self.xdg_runtime_dir.as_deref(),
            xdg_state_home: self.xdg_state_home.as_deref(),
            socket_override: self.socket_override.as_deref(),
            log_dir_override: self.log_dir_override.as_deref(),
        }
    }
}
```

`environment.rs` has no unit tests: `read` is pure IO and `path_inputs` is a field copy. Task 12's harness and Task 16's start-up exercise both.

- [ ] **Step 6: Implement `services.rs`**

```rust
//! The platform services the daemon calls. Each trait has one
//! implementation per platform (macOS here, Wayland and X11 in plan 3) plus
//! a neutral one. `main` builds the record once; nothing reaches for a
//! global.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_notify_core::EpochMs;
use agent_notify_core::update::Frontmost;
use agent_notify_core::view::PanelViewModel;

/// Draws the toast stack. Must return at once: it is called from the actor.
pub trait ToastPresenter: Send + Sync {
    /// Shows `view`, or hides the panel when it has no cards and no finished row.
    fn present(&self, view: &PanelViewModel);
}

/// Brings the terminal forward.
pub trait WindowRaiser: Send + Sync {
    /// Raises the terminal window. False when it did not come forward.
    fn raise(&self) -> bool;
    /// Starts a new terminal window attached to a tmux session. False when
    /// the terminal did not start.
    fn open_terminal(&self, request: &AttachRequest) -> bool;
}

/// A terminal to start, attached to a tmux session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachRequest {
    /// The tmux binary.
    pub tmux: PathBuf,
    /// The tmux server socket.
    pub socket: PathBuf,
    /// The tmux session to attach.
    pub session: String,
}

/// Reads which app is frontmost.
pub trait FrontmostReader: Send + Sync {
    /// The frontmost app, or `Unknown` when it cannot be read.
    fn frontmost(&self) -> Frontmost;
}

/// Facts about the user's presence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Presence {
    /// Seconds since the last input event.
    pub idle_seconds: Option<u32>,
    /// True while the screen is locked.
    pub screen_locked: Option<bool>,
}

/// Reads idle time and screen lock.
pub trait PresenceReader: Send + Sync {
    /// The current facts. Unreadable facts are `None`, which the core treats as "show".
    fn presence(&self) -> Presence;
}

/// Plays the attention sound.
pub trait SoundPlayer: Send + Sync {
    /// Starts the sound and returns at once.
    fn play(&self);
}

/// The wall clock, injected so tests can fix the time.
pub type Clock = Arc<dyn Fn() -> EpochMs + Send + Sync>;

/// The system clock.
#[must_use]
pub fn system_clock() -> Clock {
    Arc::new(|| {
        let millis = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis());
        EpochMs(i64::try_from(millis).unwrap_or(i64::MAX))
    })
}

/// A raiser for platforms without one. Every raise reports failure.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoRaiser;

impl WindowRaiser for NoRaiser {
    fn raise(&self) -> bool {
        false
    }

    fn open_terminal(&self, _request: &AttachRequest) -> bool {
        false
    }
}

/// A frontmost reader that knows nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnknownFrontmost;

impl FrontmostReader for UnknownFrontmost {
    fn frontmost(&self) -> Frontmost {
        Frontmost::Unknown
    }
}

/// A presence reader that knows nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnknownPresence;

impl PresenceReader for UnknownPresence {
    fn presence(&self) -> Presence {
        Presence::default()
    }
}

/// No sound.
#[derive(Debug, Clone, Copy, Default)]
pub struct Silent;

impl SoundPlayer for Silent {
    fn play(&self) {}
}

/// No panel. For a headless run and for tests that do not look at the view.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoPresenter;

impl ToastPresenter for NoPresenter {
    fn present(&self, _view: &PanelViewModel) {}
}
```

- [ ] **Step 7: Implement `presence.rs` and `aerospace.rs`**

Above the tests in `presence.rs`:

```rust
//! Idle time, screen lock and the frontmost app, read without `unsafe`.
//! The parsers are pure; [`IoregPresence`] runs `/usr/sbin/ioreg`.

use std::process::{Command, Stdio};

use agent_notify_core::update::Frontmost;

use crate::services::{Presence, PresenceReader};

/// Alacritty's bundle id.
pub const TERMINAL_BUNDLE_ID: &str = "org.alacritty";
const IOREG: &str = "/usr/sbin/ioreg";

/// Seconds since the last input, from `ioreg -c IOHIDSystem -d 4`.
#[must_use]
pub fn parse_hid_idle_seconds(ioreg: &str) -> Option<u32> {
    let line = ioreg.lines().find(|line| line.contains("\"HIDIdleTime\""))?;
    let nanoseconds: u64 = line.split('=').nth(1)?.trim().parse().ok()?;
    u32::try_from(nanoseconds / 1_000_000_000).ok()
}

/// Screen lock, from `ioreg -n Root -d 1`. The console user record carries
/// `CGSSessionScreenIsLocked` only while the screen is locked.
#[must_use]
pub fn parse_screen_locked(ioreg: &str) -> Option<bool> {
    if ioreg.contains("\"CGSSessionScreenIsLocked\"=Yes") {
        Some(true)
    } else if ioreg.contains("\"IOConsoleUsers\"") {
        Some(false)
    } else {
        None
    }
}

/// Classifies the frontmost app by bundle id.
#[must_use]
pub fn frontmost_from_bundle(bundle: Option<&str>) -> Frontmost {
    match bundle {
        Some(TERMINAL_BUNDLE_ID) => Frontmost::Terminal,
        Some(_) => Frontmost::Other,
        None => Frontmost::Unknown,
    }
}

/// Reads presence with two `ioreg` runs, about 20 ms together.
#[derive(Debug, Clone, Copy, Default)]
pub struct IoregPresence;

impl PresenceReader for IoregPresence {
    fn presence(&self) -> Presence {
        Presence {
            idle_seconds: ioreg(&["-c", "IOHIDSystem", "-d", "4"]).as_deref().and_then(parse_hid_idle_seconds),
            screen_locked: ioreg(&["-n", "Root", "-d", "1"]).as_deref().and_then(parse_screen_locked),
        }
    }
}

fn ioreg(arguments: &[&str]) -> Option<String> {
    let output = Command::new(IOREG).args(arguments).stdin(Stdio::null()).stderr(Stdio::null()).output();
    match output {
        Ok(output) if output.status.success() => Some(String::from_utf8_lossy(&output.stdout).into_owned()),
        Ok(output) => {
            tracing::debug!(status = ?output.status, "ioreg failed");
            None
        }
        Err(error) => {
            tracing::debug!(%error, "ioreg did not start");
            None
        }
    }
}
```

`ioreg` builds on Linux too, where `IoregPresence` is never constructed. Plan 3 moves it behind `cfg(target_os = "macos")` if clippy flags dead code there.

Above the tests in `aerospace.rs`:

```rust
//! The AeroSpace window query and focus (spike D2, D3).

use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::presence::TERMINAL_BUNDLE_ID;

/// AeroSpace, by absolute path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aerospace {
    binary: PathBuf,
}

impl Aerospace {
    /// Wraps the binary at `binary`.
    #[must_use]
    pub fn new(binary: PathBuf) -> Self {
        Aerospace { binary }
    }

    /// The first Alacritty window on any monitor.
    #[must_use]
    pub fn terminal_window(&self) -> Option<u64> {
        let output = Command::new(&self.binary)
            .args(["list-windows", "--monitor", "all", "--app-bundle-id", TERMINAL_BUNDLE_ID, "--format", "%{window-id}"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        output.status.success().then(|| first_window_id(&String::from_utf8_lossy(&output.stdout))).flatten()
    }

    /// Focuses `window_id`. True when AeroSpace exits 0. That is not a
    /// raise on its own (spike S1-11): the caller activates the app after.
    #[must_use]
    pub fn focus(&self, window_id: u64) -> bool {
        Command::new(&self.binary)
            .args(["focus", "--window-id", &window_id.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
}

/// The first window id in `list-windows` output.
#[must_use]
pub fn first_window_id(output: &str) -> Option<u64> {
    output.lines().find_map(|line| line.trim().parse().ok())
}
```

- [ ] **Step 8: Run the tests and clippy**

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon && cargo clippy --workspace --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 9: Run cargo deny**

Run: `cd ~/tools/agent-notify && cargo deny check`
Expected: `advisories ok, bans ok, licenses ok, sources ok`. `license-not-encountered` warnings are expected now, because Tauri is not a dependency until Task 14; leave the list as written. Task 14 Step 7 runs `cargo deny` again with no warnings allowed.

- [ ] **Step 10: Commit**

```bash
cd ~ && config add tools/agent-notify
config diff --cached --name-only
config commit -m "Add the agent-notify tools workspace and daemon leaf modules" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- tools/agent-notify
```

Read the `--name-only` list before you commit. It must not contain a `target/` path.

---

### Task 9: The tmux adapter and the focus runner

**Files:**
- Create: `tools/agent-notify/daemon/src/tmux.rs`, `src/focus_run.rs`
- Create: `tools/agent-notify/daemon/tests/support/mod.rs`, `tests/tmux_adapter.rs`, `tests/focus_run.rs`
- Modify: `tools/agent-notify/daemon/src/lib.rs`

**Interfaces:**
- Consumes: plan 1 `ServerId`, `PaneKey`, `PaneId`, `SessionId`, `SocketPath`, `FocusWorld`, `WorldPane`, `WorldClient`, `PaneSnapshot`, `PaneTag`, `PaneState`, `FocusTarget`, `FocusPlan`, `TmuxSwitch`, `plan_focus`, `FocusOutcome`, `DisplayText`; Task 2 `PANE_STATE_OPTION`, `WINDOW_STATE_OPTION`, `pane_state_value`, `parse_pane_state`, `window_state`; Task 8 `WindowRaiser`, `AttachRequest`.
- Produces:
  - `tmux::Tmux { binary }` with `new(PathBuf)`, `binary(&self) -> &Path`, `server_id(&self, &SocketPath) -> Result<ServerId, TmuxError>`, `focus_world(&self, &ServerId) -> Result<FocusWorld, TmuxError>`, `pane_snapshot(&self, &ServerId) -> Result<PaneSnapshot, TmuxError>`, `active_pane(&self, &ServerId) -> Result<Option<PaneKey>, TmuxError>`, `session_tag(&self, &PaneKey) -> Result<Option<SessionId>, TmuxError>`, `switch(&self, &ServerId, &PaneId, &TmuxSwitch) -> Result<(), TmuxError>`, `set_pane_state(&self, &PaneKey, PaneState) -> Result<(), TmuxError>`.
  - `tmux::TmuxError::{Spawn(String), ServerGone, Failed(String)}` with `Display`.
  - `tmux::{parse_world, parse_pane_tags, parse_active_pane, parse_window_panes}` (pure, described with the code).
  - `focus_run::run_focus(tmux: Option<&Tmux>, raiser: &dyn WindowRaiser, target: &FocusTarget) -> FocusOutcome`.

**Identity rule.** A tmux server that restarts on the same socket path reuses pane ids from `%0`. Every read that the daemon acts on prints `#{pid}` and `#{start_time}` on each line, and a line from a different server makes the whole read `ServerGone`. Writes check identity first; the gap between the check and the write is one tmux round trip.

- [ ] **Step 1: Write the test harness**

`daemon/tests/support/mod.rs`:

```rust
//! Shared test helpers: a throwaway tmux server and its identity.

#![allow(dead_code)] // Each integration target uses a different subset.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use agent_notify_core::{PaneId, PaneKey, ServerId, SocketPath};
use agent_notify_daemon::services::{AttachRequest, WindowRaiser};
use agent_notify_daemon::tmux::Tmux;
pub use dotfiles_test_support::tmux_server::Server;

/// A tmux server with one session, `main`, in a scratch directory.
pub fn tmux_server(label: &str) -> Server {
    let server = Server::new(label);
    server.new_session("main", Path::new("/tmp"));
    server
}

/// The server's identity, read the way the daemon reads it.
pub fn server_id(server: &Server) -> ServerId {
    ServerId {
        socket_path: SocketPath::parse(&server.socket_path()).expect("an absolute socket path"),
        pid: server.pid(),
        start_time: server.stdout(&["display-message", "-p", "#{start_time}"]).parse().expect("a start time"),
    }
}

/// The adapter, pointed at the `tmux` on `PATH`.
pub fn adapter() -> Tmux {
    let found = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|directory| directory.join("tmux"))
        .find(|candidate| candidate.is_file())
        .expect("tmux on PATH");
    Tmux::new(found)
}

/// The first pane of `target`, for example `main:0`.
pub fn first_pane(server: &Server, target: &str) -> PaneId {
    let id = server.stdout(&["list-panes", "-t", target, "-F", "#{pane_id}"]);
    PaneId::parse(id.lines().next().expect("a pane")).expect("a pane id")
}

/// A second pane in the window of `target`, running `cat`.
pub fn split(server: &Server, target: &str) -> PaneId {
    let id = server.stdout(&["split-window", "-d", "-t", target, "-P", "-F", "#{pane_id}", "cat"]);
    PaneId::parse(&id).expect("a pane id")
}

/// A new window in `main`, returning its first pane.
pub fn new_window(server: &Server) -> PaneId {
    let window = server.new_window("main", Path::new("/tmp"), &[]);
    first_pane(server, &window)
}

/// Tags `pane` with a session id, as the SessionStart hook does.
pub fn tag(server: &Server, pane: &PaneId, session: &str) {
    let status = server.tmux(&["set-option", "-p", "-t", pane.as_str(), "@agent_notify_session", session]).status;
    assert!(status.success(), "tagging {}", pane.as_str());
}

/// One pane option, or `""` when unset.
pub fn pane_option(server: &Server, pane: &PaneId, option: &str) -> String {
    server.stdout(&["show-options", "-p", "-v", "-t", pane.as_str(), option])
}

/// One window option of the window holding `pane`, or `""` when unset.
pub fn window_option(server: &Server, pane: &PaneId, option: &str) -> String {
    server.stdout(&["show-options", "-w", "-v", "-t", pane.as_str(), option])
}

/// A key for `pane` on `server`.
pub fn key(server: &Server, pane: &PaneId) -> PaneKey {
    PaneKey { server: server_id(server), pane: pane.clone() }
}

/// A raiser that records what it was asked to do and answers `answer`.
#[derive(Debug, Default)]
pub struct RecordingRaiser {
    pub raises: Mutex<u32>,
    pub attaches: Mutex<Vec<AttachRequest>>,
    pub fail: bool,
}

impl WindowRaiser for RecordingRaiser {
    fn raise(&self) -> bool {
        *self.raises.lock().expect("unpoisoned") += 1;
        !self.fail
    }

    fn open_terminal(&self, request: &AttachRequest) -> bool {
        self.attaches.lock().expect("unpoisoned").push(request.clone());
        !self.fail
    }
}

/// The socket path of `server` as a `PathBuf`, for `AttachRequest` checks.
pub fn socket_of(server: &Server) -> PathBuf {
    server.socket_path()
}
```

`Server::socket_path` and `Server::pid` come from plan 1 Task 13. The test sessions run an inert command, not a shell, so no zsh hook renames a window under a test.

- [ ] **Step 2: Write the failing parser tests**

`daemon/src/tmux.rs`, tests first. `\u{1f}` is the unit separator the adapter puts between fields:

```rust
#[cfg(test)]
mod tests {
    use agent_notify_core::{PaneId, ServerId, SessionId, SocketPath};

    use super::*;

    fn server() -> ServerId {
        ServerId { socket_path: SocketPath::parse(std::path::Path::new("/tmp/tmux-501/default")).expect("absolute"), pid: 77, start_time: 1_700_000_000 }
    }

    fn line(fields: &[&str]) -> String {
        format!("{}\n", fields.join("\u{1f}"))
    }

    #[test]
    fn the_world_lists_tagged_panes_and_clients() {
        let panes = [line(&["77", "1700000000", "%1", "session-a", "main"]), line(&["77", "1700000000", "%2", "", "other session"])].concat();
        let clients = line(&["77", "1700000000", "/dev/ttys004", "1700000500", "main"]);
        let world = parse_world(&panes, &clients, &server()).expect("same server");
        assert_eq!(world.panes.len(), 2);
        assert_eq!(world.panes[0].session_tag, Some(SessionId::parse("session-a").expect("id")));
        assert_eq!(world.panes[1].session_tag, None);
        assert_eq!(world.panes[1].tmux_session, "other session");
        assert_eq!(world.clients[0].tty, "/dev/ttys004");
        assert_eq!(world.clients[0].activity, 1_700_000_500);
    }

    #[test]
    fn a_line_from_another_server_is_server_gone() {
        let panes = line(&["78", "1700000000", "%1", "session-a", "main"]);
        assert!(matches!(parse_world(&panes, "", &server()), Err(TmuxError::ServerGone)));
        let restarted = line(&["77", "1700000999", "%1", "session-a", "main"]);
        assert!(matches!(parse_pane_tags(&restarted, &server()), Err(TmuxError::ServerGone)));
    }

    #[test]
    fn a_session_name_may_hold_the_separator() {
        let panes = line(&["77", "1700000000", "%1", "", "odd\u{1f}name"]);
        let world = parse_world(&panes, "", &server()).expect("parses");
        assert_eq!(world.panes[0].tmux_session, "odd\u{1f}name");
    }

    #[test]
    fn pane_tags_skip_bad_ids() {
        let tags = parse_pane_tags(&[line(&["77", "1700000000", "%1", "session-a"]), line(&["77", "1700000000", "%2", "not an id!"])].concat(), &server())
            .expect("same server");
        assert_eq!(tags, vec![(PaneId::parse("%1").expect("id"), Some(SessionId::parse("session-a").expect("id"))), (PaneId::parse("%2").expect("id"), None)]);
    }

    #[test]
    fn the_active_pane_is_the_most_recent_clients() {
        let clients = [line(&["77", "1700000000", "100", "%1"]), line(&["77", "1700000000", "300", "%4"])].concat();
        let active = parse_active_pane(&clients, &server()).expect("same server");
        assert_eq!(active.map(|key| key.pane), Some(PaneId::parse("%4").expect("id")));
        assert_eq!(parse_active_pane("", &server()).expect("no clients"), None);
    }

    #[test]
    fn window_panes_carry_their_state() {
        let panes = [line(&["77", "1700000000", "@3", "%1", "working"]), line(&["77", "1700000000", "@3", "%2", ""])].concat();
        let window = parse_window_panes(&panes, &server()).expect("same server");
        assert_eq!(window.window, "@3");
        assert_eq!(window.states, vec![(PaneId::parse("%1").expect("id"), PaneState::Working), (PaneId::parse("%2").expect("id"), PaneState::Idle)]);
    }
}
```

- [ ] **Step 3: Write the failing integration tests**

`daemon/tests/tmux_adapter.rs`:

```rust
//! The tmux adapter against a real, throwaway tmux server.

mod support;

use agent_notify_core::update::PaneState;
use agent_notify_core::status::{PANE_STATE_OPTION, WINDOW_STATE_OPTION};
use agent_notify_core::{SessionId, SocketPath};
use agent_notify_daemon::tmux::TmuxError;
use support::{adapter, first_pane, key, new_window, pane_option, server_id, split, tag, tmux_server, window_option};

#[test]
fn the_server_id_matches_what_tmux_reports() {
    let server = tmux_server("an-id");
    let socket = SocketPath::parse(&server.socket_path()).expect("absolute");
    assert_eq!(adapter().server_id(&socket).expect("reads"), server_id(&server));
}

#[test]
fn the_world_finds_a_tagged_pane() {
    let server = tmux_server("an-world");
    let pane = new_window(&server);
    tag(&server, &pane, "session-a");
    let world = adapter().focus_world(&server_id(&server)).expect("reads");
    let tagged: Vec<_> = world.panes.iter().filter(|entry| entry.session_tag.is_some()).collect();
    assert_eq!(tagged.len(), 1);
    assert_eq!(tagged[0].pane, pane);
    assert_eq!(tagged[0].tmux_session, "main");
    assert!(world.clients.is_empty(), "no client is attached in a test");
}

#[test]
fn pane_state_writes_the_pane_and_the_windows_most_urgent_state() {
    let server = tmux_server("an-state");
    let first = first_pane(&server, "main:0");
    let second = split(&server, "main:0");
    let tmux = adapter();
    tmux.set_pane_state(&key(&server, &first), PaneState::Working).expect("writes");
    tmux.set_pane_state(&key(&server, &second), PaneState::NeedsYou).expect("writes");
    assert_eq!(pane_option(&server, &first, PANE_STATE_OPTION), "working");
    assert_eq!(pane_option(&server, &second, PANE_STATE_OPTION), "needs-you");
    assert_eq!(window_option(&server, &first, WINDOW_STATE_OPTION), "needs-you");
    tmux.set_pane_state(&key(&server, &second), PaneState::Idle).expect("writes");
    assert_eq!(pane_option(&server, &second, PANE_STATE_OPTION), "");
    assert_eq!(window_option(&server, &first, WINDOW_STATE_OPTION), "working");
    tmux.set_pane_state(&key(&server, &first), PaneState::Idle).expect("writes");
    assert_eq!(window_option(&server, &first, WINDOW_STATE_OPTION), "");
}

#[test]
fn a_restarted_server_reads_as_gone() {
    let server = tmux_server("an-restart");
    let old = server_id(&server);
    let old_pane = first_pane(&server, "main:0");
    assert!(server.tmux(&["kill-server"]).status.success());
    server.new_session("main", std::path::Path::new("/tmp"));
    let new_pane = first_pane(&server, "main:0");
    assert_eq!(new_pane, old_pane, "the new server reuses the pane id, which is the hazard");
    let tmux = adapter();
    let stale = agent_notify_core::PaneKey { server: old.clone(), pane: old_pane };
    assert!(matches!(tmux.set_pane_state(&stale, PaneState::Working), Err(TmuxError::ServerGone)));
    assert_eq!(pane_option(&server, &new_pane, PANE_STATE_OPTION), "", "nothing was written to the new pane");
    assert!(matches!(tmux.focus_world(&old), Err(TmuxError::ServerGone)));
    assert!(tmux.pane_snapshot(&old).expect("a gone server is an empty snapshot").panes.is_empty());
    assert!(matches!(tmux.session_tag(&stale), Err(TmuxError::ServerGone)));
}

#[test]
fn a_missing_server_is_gone_not_failed() {
    let server = tmux_server("an-missing");
    let id = server_id(&server);
    assert!(server.tmux(&["kill-server"]).status.success());
    assert!(matches!(adapter().focus_world(&id), Err(TmuxError::ServerGone)));
}

#[test]
fn the_session_tag_reads_back() {
    let server = tmux_server("an-tag");
    let pane = first_pane(&server, "main:0");
    let tmux = adapter();
    assert_eq!(tmux.session_tag(&key(&server, &pane)).expect("reads"), None);
    tag(&server, &pane, "session-b");
    assert_eq!(tmux.session_tag(&key(&server, &pane)).expect("reads"), Some(SessionId::parse("session-b").expect("id")));
}
```

`daemon/tests/focus_run.rs`:

```rust
//! Running focus plans against a real tmux server. A test cannot attach a
//! tmux client without a terminal, so these cover the no-client path
//! (`AttachNewTerminal`), `SessionGone` and `RaiseOnly`. The final smoke
//! test covers `SwitchClient` by hand.

mod support;

use agent_notify_core::update::FocusTarget;
use agent_notify_core::update::FocusOutcome;
use agent_notify_core::{PaneKey, SessionId};
use support::{RecordingRaiser, adapter, first_pane, key, new_window, server_id, socket_of, tag, tmux_server};

fn target(session: &str, pane: PaneKey) -> FocusTarget {
    FocusTarget::Session { session: SessionId::parse(session).expect("id"), pane }
}

#[test]
fn a_tagged_pane_with_no_client_selects_it_and_opens_a_terminal() {
    let server = tmux_server("an-run");
    let pane = new_window(&server);
    tag(&server, &pane, "session-a");
    let raiser = RecordingRaiser::default();
    let tmux = adapter();
    let last_seen = PaneKey { server: server_id(&server), pane: first_pane(&server, "main:0") };
    let outcome = agent_notify_daemon::focus_run::run_focus(Some(&tmux), &raiser, &target("session-a", last_seen));
    assert_eq!(outcome, FocusOutcome::Done);
    assert_eq!(server.stdout(&["display-message", "-p", "-t", "main", "#{pane_id}"]), pane.as_str(), "the tagged pane is now current");
    let attaches = raiser.attaches.lock().expect("unpoisoned");
    assert_eq!(attaches.len(), 1);
    assert_eq!(attaches[0].session, "main");
    assert_eq!(attaches[0].socket, socket_of(&server));
}

#[test]
fn an_untagged_world_is_session_gone() {
    let server = tmux_server("an-gone");
    let pane = first_pane(&server, "main:0");
    let raiser = RecordingRaiser::default();
    let outcome = agent_notify_daemon::focus_run::run_focus(Some(&adapter()), &raiser, &target("session-a", key(&server, &pane)));
    assert_eq!(outcome, FocusOutcome::SessionGone);
    assert_eq!(*raiser.raises.lock().expect("unpoisoned"), 0);
}

#[test]
fn a_dead_server_is_session_gone() {
    let server = tmux_server("an-dead");
    let pane_key = key(&server, &first_pane(&server, "main:0"));
    assert!(server.tmux(&["kill-server"]).status.success());
    let outcome = agent_notify_daemon::focus_run::run_focus(Some(&adapter()), &RecordingRaiser::default(), &target("session-a", pane_key));
    assert_eq!(outcome, FocusOutcome::SessionGone);
}

#[test]
fn raise_only_raises_and_reports_a_failed_raise() {
    let raiser = RecordingRaiser::default();
    assert_eq!(agent_notify_daemon::focus_run::run_focus(None, &raiser, &FocusTarget::RaiseOnly), FocusOutcome::Done);
    assert_eq!(*raiser.raises.lock().expect("unpoisoned"), 1);
    let failing = RecordingRaiser { fail: true, ..RecordingRaiser::default() };
    assert_eq!(agent_notify_daemon::focus_run::run_focus(None, &failing, &FocusTarget::RaiseOnly), FocusOutcome::RaiseFailed);
}
```

Plan 1 puts `FocusTarget`, `FocusOutcome`, `PaneState`, `UiIntent`, `DaemonInput`, `Effect`, `FocusSnapshot`, `Frontmost` and `PaneSnapshot` in `update`; `DaemonState` and `Dnd` in `state`; `FocusWorld`, `FocusPlan` and `TmuxSwitch` in `focus`. If a `use` line fails, find the type with `grep -rn "pub enum <Name>\|pub struct <Name>" ~/crates/agent-notify-core/src`.

- [ ] **Step 4: Run them to see them fail**

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon`
Expected: compile errors, `could not find tmux in agent_notify_daemon`.

- [ ] **Step 5: Implement `tmux.rs`**

Add `pub mod focus_run;` and `pub mod tmux;` to `lib.rs`. Above the tests in `tmux.rs`:

```rust
//! The stateless tmux adapter. Every call passes `-S <socket>` and an
//! argv, never a shell string. Reads print the server's pid and start time
//! on each line; see the identity rule in the plan, Task 9.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use agent_notify_core::focus::{FocusWorld, TmuxSwitch, WorldClient, WorldPane};
use agent_notify_core::update::{PaneSnapshot, PaneState, PaneTag};
use agent_notify_core::status::{PANE_STATE_OPTION, WINDOW_STATE_OPTION, pane_state_value, parse_pane_state, window_state};
use agent_notify_core::{PaneId, PaneKey, ServerId, SessionId, SocketPath};

const SEPARATOR: char = '\u{1f}';
const IDENTITY: &str = "#{pid}\u{1f}#{start_time}";

/// Why a tmux call did not do its job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TmuxError {
    /// The binary did not start.
    Spawn(String),
    /// No server on the socket, or a different server than the one named.
    ServerGone,
    /// tmux ran and refused. The text is its stderr.
    Failed(String),
}

impl fmt::Display for TmuxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TmuxError::Spawn(reason) => write!(formatter, "tmux did not start: {reason}"),
            TmuxError::ServerGone => write!(formatter, "the tmux server is gone"),
            TmuxError::Failed(stderr) => write!(formatter, "tmux failed: {stderr}"),
        }
    }
}

impl std::error::Error for TmuxError {}

/// The panes of one window and their states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowPanes {
    /// `#{window_id}`, for example `@3`.
    pub window: String,
    /// Each pane's current `@agent_notify_state`.
    pub states: Vec<(PaneId, PaneState)>,
}

/// tmux, by absolute path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tmux {
    binary: PathBuf,
}

impl Tmux {
    /// Wraps the binary at `binary`.
    #[must_use]
    pub fn new(binary: PathBuf) -> Self {
        Tmux { binary }
    }

    /// The binary, for a terminal that attaches to a session.
    #[must_use]
    pub fn binary(&self) -> &Path {
        &self.binary
    }

    /// Reads the identity of the server on `socket`.
    ///
    /// # Errors
    ///
    /// `ServerGone` when nothing answers on the socket.
    pub fn server_id(&self, socket: &SocketPath) -> Result<ServerId, TmuxError> {
        let text = self.run(socket, &["list-sessions", "-F", IDENTITY])?;
        let first = text.lines().next().ok_or(TmuxError::ServerGone)?;
        let mut fields = first.split(SEPARATOR);
        let pid = fields.next().and_then(|value| value.parse().ok());
        let start_time = fields.next().and_then(|value| value.parse().ok());
        match (pid, start_time) {
            (Some(pid), Some(start_time)) => Ok(ServerId { socket_path: socket.clone(), pid, start_time }),
            _ => Err(TmuxError::Failed(format!("unreadable identity {first:?}"))),
        }
    }

    /// Every pane with its tag and tmux session, and every client.
    ///
    /// # Errors
    ///
    /// `ServerGone` when the server is gone or restarted.
    pub fn focus_world(&self, server: &ServerId) -> Result<FocusWorld, TmuxError> {
        let socket = &server.socket_path;
        let panes = self.run(socket, &["list-panes", "-a", "-F", &format!("{IDENTITY}\u{1f}#{{pane_id}}\u{1f}#{{@agent_notify_session}}\u{1f}#{{session_name}}")])?;
        let clients =
            self.run(socket, &["list-clients", "-F", &format!("{IDENTITY}\u{1f}#{{client_tty}}\u{1f}#{{client_activity}}\u{1f}#{{client_session}}")])?;
        parse_world(&panes, &clients, server)
    }

    /// Every pane's session tag. A gone or restarted server gives an empty
    /// snapshot, which the core reads as "every session on it ended".
    ///
    /// # Errors
    ///
    /// `Spawn` or `Failed`. Those must not end any session.
    pub fn pane_snapshot(&self, server: &ServerId) -> Result<PaneSnapshot, TmuxError> {
        let text = self.run(&server.socket_path, &["list-panes", "-a", "-F", &format!("{IDENTITY}\u{1f}#{{pane_id}}\u{1f}#{{@agent_notify_session}}")]);
        let panes = match text.and_then(|text| parse_pane_tags(&text, server)) {
            Ok(tags) => tags.into_iter().map(|(pane, session)| PaneTag { pane, session }).collect(),
            Err(TmuxError::ServerGone) => Vec::new(),
            Err(other) => return Err(other),
        };
        Ok(PaneSnapshot { server: server.clone(), panes })
    }

    /// The current pane of the most recently active client.
    ///
    /// # Errors
    ///
    /// `ServerGone` when the server is gone or restarted.
    pub fn active_pane(&self, server: &ServerId) -> Result<Option<PaneKey>, TmuxError> {
        let text = self.run(&server.socket_path, &["list-clients", "-F", &format!("{IDENTITY}\u{1f}#{{client_activity}}\u{1f}#{{pane_id}}")])?;
        parse_active_pane(&text, server)
    }

    /// The session tag on `pane`.
    ///
    /// # Errors
    ///
    /// `ServerGone` when the server is gone or restarted, or the pane is gone.
    pub fn session_tag(&self, pane: &PaneKey) -> Result<Option<SessionId>, TmuxError> {
        let text = self.run(
            &pane.server.socket_path,
            &["display-message", "-p", "-t", pane.pane.as_str(), &format!("{IDENTITY}\u{1f}#{{pane_id}}\u{1f}#{{@agent_notify_session}}")],
        )?;
        let tags = parse_pane_tags(&text, &pane.server)?;
        match tags.into_iter().next() {
            Some((found, tag)) if found == pane.pane => Ok(tag),
            _ => Err(TmuxError::ServerGone),
        }
    }

    /// Makes `pane` current in its window and session, and moves a client
    /// to it as `switch` says. `AttachNewTerminal` only selects: the caller
    /// starts the terminal.
    ///
    /// # Errors
    ///
    /// The first tmux error.
    pub fn switch(&self, server: &ServerId, pane: &PaneId, switch: &TmuxSwitch) -> Result<(), TmuxError> {
        let target = pane.as_str();
        let socket = &server.socket_path;
        self.run(socket, &["select-window", "-t", target, ";", "select-pane", "-t", target])?;
        if let TmuxSwitch::SwitchClient { tty } = switch {
            self.run(socket, &["switch-client", "-c", tty, "-t", target])?;
        }
        Ok(())
    }

    /// Writes `state` on the pane and the most urgent state of its window
    /// on the window. Idle unsets the option.
    ///
    /// # Errors
    ///
    /// `ServerGone` when the server is gone or restarted; nothing is written then.
    pub fn set_pane_state(&self, pane: &PaneKey, state: PaneState) -> Result<(), TmuxError> {
        let socket = &pane.server.socket_path;
        let target = pane.pane.as_str();
        let text = self.run(
            socket,
            &["list-panes", "-t", target, "-F", &format!("{IDENTITY}\u{1f}#{{window_id}}\u{1f}#{{pane_id}}\u{1f}#{{{PANE_STATE_OPTION}}}")],
        )?;
        let window = parse_window_panes(&text, &pane.server)?;
        let merged = window_state(window.states.iter().map(|(id, current)| if *id == pane.pane { state } else { *current }));
        let mut arguments: Vec<String> = option_arguments("-p", target, PANE_STATE_OPTION, state);
        arguments.push(";".to_string());
        arguments.extend(option_arguments("-w", &window.window, WINDOW_STATE_OPTION, merged));
        let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
        self.run(socket, &borrowed).map(|_| ())
    }

    fn run(&self, socket: &SocketPath, arguments: &[&str]) -> Result<String, TmuxError> {
        let output = Command::new(&self.binary)
            .arg("-S")
            .arg(socket.as_path())
            .args(arguments)
            .stdin(Stdio::null())
            .output()
            .map_err(|error| TmuxError::Spawn(error.to_string()))?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if is_server_gone(&stderr) { Err(TmuxError::ServerGone) } else { Err(TmuxError::Failed(stderr)) }
    }
}

fn option_arguments(scope: &str, target: &str, option: &str, state: PaneState) -> Vec<String> {
    match pane_state_value(state) {
        Some(value) => vec!["set-option".into(), scope.into(), "-t".into(), target.into(), option.into(), value.into()],
        None => vec!["set-option".into(), scope.into(), "-u".into(), "-t".into(), target.into(), option.into()],
    }
}

fn is_server_gone(stderr: &str) -> bool {
    ["no server running", "error connecting", "No such file or directory", "can't find pane", "can't find window"].iter().any(|needle| stderr.contains(needle))
}

fn checked_fields<'line>(line: &'line str, server: &ServerId, count: usize) -> Result<Vec<&'line str>, TmuxError> {
    let fields: Vec<&str> = line.splitn(count, SEPARATOR).collect();
    let same = fields.first().and_then(|pid| pid.parse::<u32>().ok()) == Some(server.pid)
        && fields.get(1).and_then(|start| start.parse::<i64>().ok()) == Some(server.start_time);
    if !same {
        return Err(TmuxError::ServerGone);
    }
    if fields.len() < count {
        return Err(TmuxError::Failed(format!("short tmux line {line:?}")));
    }
    Ok(fields)
}

fn tag_of(text: &str) -> Option<SessionId> {
    (!text.is_empty()).then(|| SessionId::parse(text).ok()).flatten()
}

/// Parses the `focus_world` reads. The session name is last, so it may
/// hold any character.
///
/// # Errors
///
/// `ServerGone` when any line comes from a different server.
pub fn parse_world(panes: &str, clients: &str, server: &ServerId) -> Result<FocusWorld, TmuxError> {
    let panes = panes
        .lines()
        .map(|line| {
            let fields = checked_fields(line, server, 5)?;
            Ok(PaneId::parse(fields[2]).ok().map(|pane| WorldPane { pane, session_tag: tag_of(fields[3]), tmux_session: fields[4].to_string() }))
        })
        .collect::<Result<Vec<_>, TmuxError>>()?
        .into_iter()
        .flatten()
        .collect();
    let clients = clients
        .lines()
        .map(|line| {
            let fields = checked_fields(line, server, 5)?;
            Ok(WorldClient { tty: fields[2].to_string(), activity: fields[3].parse().unwrap_or(0), tmux_session: fields[4].to_string() })
        })
        .collect::<Result<Vec<_>, TmuxError>>()?;
    Ok(FocusWorld { panes, clients })
}

/// Parses pane id and session tag lines. Lines with a bad pane id are skipped.
///
/// # Errors
///
/// `ServerGone` when any line comes from a different server.
pub fn parse_pane_tags(text: &str, server: &ServerId) -> Result<Vec<(PaneId, Option<SessionId>)>, TmuxError> {
    let rows = text
        .lines()
        .map(|line| {
            let fields = checked_fields(line, server, 4)?;
            Ok(PaneId::parse(fields[2]).ok().map(|pane| (pane, tag_of(fields[3]))))
        })
        .collect::<Result<Vec<_>, TmuxError>>()?;
    Ok(rows.into_iter().flatten().collect())
}

/// Parses `list-clients` activity and pane lines into the most recent client's pane.
///
/// # Errors
///
/// `ServerGone` when any line comes from a different server.
pub fn parse_active_pane(text: &str, server: &ServerId) -> Result<Option<PaneKey>, TmuxError> {
    let rows = text
        .lines()
        .map(|line| {
            let fields = checked_fields(line, server, 4)?;
            Ok((fields[2].parse::<i64>().unwrap_or(0), PaneId::parse(fields[3]).ok()))
        })
        .collect::<Result<Vec<_>, TmuxError>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(activity, pane)| pane.map(|pane| (activity, pane)))
        .max_by_key(|(activity, _)| *activity)
        .map(|(_, pane)| PaneKey { server: server.clone(), pane }))
}

/// Parses the panes of one window with their states.
///
/// # Errors
///
/// `ServerGone` when any line comes from a different server or the window is empty.
pub fn parse_window_panes(text: &str, server: &ServerId) -> Result<WindowPanes, TmuxError> {
    let rows = text
        .lines()
        .map(|line| {
            let fields = checked_fields(line, server, 5)?;
            Ok((fields[2].to_string(), PaneId::parse(fields[3]).ok().map(|pane| (pane, parse_pane_state(fields[4])))))
        })
        .collect::<Result<Vec<_>, TmuxError>>()?;
    let window = rows.first().map(|(window, _)| window.clone()).ok_or(TmuxError::ServerGone)?;
    Ok(WindowPanes { window, states: rows.into_iter().filter_map(|(_, state)| state).collect() })
}
```

`window_state` takes `impl IntoIterator<Item = PaneState>` (Task 2), so the closure passes values, not references.

- [ ] **Step 6: Implement `focus_run.rs`**

```rust
//! Runs one focus plan: read the world, plan, recheck, switch, raise.

use agent_notify_core::DisplayText;
use agent_notify_core::PaneKey;
use agent_notify_core::focus::{FocusPlan, TmuxSwitch, plan_focus};
use agent_notify_core::update::{FocusOutcome, FocusTarget};

use crate::services::{AttachRequest, WindowRaiser};
use crate::tmux::{Tmux, TmuxError};

/// Takes the user to `target`. Without tmux, a session target can only
/// raise the terminal.
pub fn run_focus(tmux: Option<&Tmux>, raiser: &dyn WindowRaiser, target: &FocusTarget) -> FocusOutcome {
    let (FocusTarget::Session { session, pane: last_seen }, Some(tmux)) = (target, tmux) else {
        return raised(raiser.raise());
    };
    let world = match tmux.focus_world(&last_seen.server) {
        Ok(world) => Some(world),
        Err(TmuxError::ServerGone) => None,
        Err(error) => return failed(&error),
    };
    let (pane, switch) = match plan_focus(target, world.as_ref()) {
        FocusPlan::Focus { pane, switch } => (pane, switch),
        FocusPlan::RaiseOnly => return raised(raiser.raise()),
        FocusPlan::SessionGone => return FocusOutcome::SessionGone,
    };
    let chosen = PaneKey { server: last_seen.server.clone(), pane: pane.clone() };
    match tmux.session_tag(&chosen) {
        Ok(Some(tag)) if &tag == session => {}
        Ok(_) | Err(TmuxError::ServerGone) => return FocusOutcome::SessionGone,
        Err(error) => return failed(&error),
    }
    if let Err(error) = tmux.switch(&chosen.server, &pane, &switch) {
        return if error == TmuxError::ServerGone { FocusOutcome::SessionGone } else { failed(&error) };
    }
    match switch {
        TmuxSwitch::AttachNewTerminal { tmux_session } => raised(raiser.open_terminal(&AttachRequest {
            tmux: tmux.binary().to_path_buf(),
            socket: chosen.server.socket_path.as_path().to_path_buf(),
            session: tmux_session,
        })),
        TmuxSwitch::InAttachedClient | TmuxSwitch::SwitchClient { .. } => raised(raiser.raise()),
    }
}

fn raised(success: bool) -> FocusOutcome {
    if success { FocusOutcome::Done } else { FocusOutcome::RaiseFailed }
}

fn failed(error: &TmuxError) -> FocusOutcome {
    tracing::warn!(%error, "focus plan failed");
    FocusOutcome::TmuxFailed(DisplayText::new(&error.to_string()))
}
```

Check how plan 1 spells `DisplayText::new` (it takes `&str` or `impl Into<String>`) and match it.

- [ ] **Step 7: Run the tests and clippy**

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon && cargo clippy --workspace --all-targets -- -D warnings`
Expected: all pass. The tmux tests start real tmux servers in `/tmp`; they need `tmux` on `PATH`.

- [ ] **Step 8: Prove the identity check is load-bearing**

Make `checked_fields` return `Ok(fields)` without comparing the pid and start time (back up the file first). Run `cargo test -p agent-notify-daemon --test tmux_adapter a_restarted_server_reads_as_gone; echo "exit $?"`. Expected: a non-zero exit. Restore the file, `touch` it, confirm with `config diff --stat -- tools/agent-notify/daemon/src/tmux.rs` that only your intended changes remain (the file is new, so compare with your backup: `diff` prints nothing), and rerun the test green.

- [ ] **Step 9: Commit**

```bash
cd ~ && config add tools/agent-notify
config diff --cached --name-only
config commit -m "Add the agent-notify tmux adapter and focus runner" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- tools/agent-notify
```

---

### Task 10: The UI bridge types and generated bindings

**Files:**
- Create: `tools/agent-notify/daemon/src/bridge.rs`, `daemon/tests/bindings.rs`
- Generated and committed: `tools/agent-notify/ui/src/bindings/*.ts`, `tools/agent-notify/ui/test/fixtures/view-models.ts`
- Modify: `tools/agent-notify/daemon/src/lib.rs`

**Interfaces:**
- Consumes: Task 3 `TS` derives on the view types; plan 1 `UiIntent`, `CardId`.
- Produces (`agent_notify_daemon::bridge`):
  - `UiMessage::{Clicked { card_id: CardId }, Closed { card_id: CardId }, Hover { hovered: bool }, Layout { content_height_px: u32 }, Error { message: String, stack: Option<String> }}`, serde `tag = "type"`, `rename_all = "snake_case"`, `Deserialize` + `TS`.
  - `UiBootstrap { view_model: PanelViewModel, native_hover: bool }`, `HoverPayload { hovered: bool }`, both `Serialize` + `TS`.
  - `VIEW_MODEL_EVENT = "view-model"`, `HOVER_EVENT = "hover"`.
  - `Routed::{Intent(UiIntent), Layout { content_height_px: u32 }, Error { message: String, stack: Option<String> }}`, `route(UiMessage) -> Routed`.
  - TypeScript files in `ui/src/bindings/`: `PanelViewModel.ts`, `CardView.ts`, `FinishedEntry.ts`, `Kind.ts`, `CardId.ts`, `EpochMs.ts`, `UiMessage.ts`, `UiBootstrap.ts`, `HoverPayload.ts`, and `events.ts` with the two event-name constants, so the page never spells them by hand. Fixtures in `ui/test/fixtures/view-models.ts`: `empty`, `mixed`, `dndOn`, `revisionTen`, `revisionNine`, each `satisfies PanelViewModel`.

- [ ] **Step 1: Write the failing unit tests**

`daemon/src/bridge.rs`, tests first:

```rust
#[cfg(test)]
mod tests {
    use agent_notify_core::{CardId, EpochMs, EventId};

    use super::*;

    fn card() -> CardId {
        CardId::for_event(&EventId::from_parts(EpochMs(1), [0; 10]))
    }

    #[test]
    fn ui_messages_decode_from_tagged_json() {
        let text = format!(r#"{{"type":"clicked","card_id":"{}"}}"#, card().as_str());
        assert_eq!(serde_json::from_str::<UiMessage>(&text).expect("decodes"), UiMessage::Clicked { card_id: card() });
        let layout: UiMessage = serde_json::from_str(r#"{"type":"layout","content_height_px":212}"#).expect("decodes");
        assert_eq!(layout, UiMessage::Layout { content_height_px: 212 });
        let error: UiMessage = serde_json::from_str(r#"{"type":"error","message":"boom","stack":null}"#).expect("decodes");
        assert_eq!(error, UiMessage::Error { message: "boom".to_string(), stack: None });
    }

    #[test]
    fn a_ui_message_cannot_name_a_tmux_target() {
        let text = r#"{"type":"clicked","card_id":"x","pane":"%1"}"#;
        assert!(serde_json::from_str::<UiMessage>(text).is_err(), "unknown fields and bad ids are refused");
    }

    #[test]
    fn routing_turns_card_messages_into_intents() {
        assert_eq!(route(UiMessage::Clicked { card_id: card() }), Routed::Intent(UiIntent::Clicked { card_id: card() }));
        assert_eq!(route(UiMessage::Closed { card_id: card() }), Routed::Intent(UiIntent::Closed { card_id: card() }));
        assert_eq!(route(UiMessage::Hover { hovered: true }), Routed::Intent(UiIntent::HoverChanged { hovered: true }));
        assert_eq!(route(UiMessage::Layout { content_height_px: 9 }), Routed::Layout { content_height_px: 9 });
    }
}
```

`CardId::as_str` comes from plan 1's `checked_id!` macro. If `CardId` parsing accepts `"x"`, the second test still fails on `deny_unknown_fields`, which Step 3 adds.

- [ ] **Step 2: Write the failing bindings test**

`daemon/tests/bindings.rs`:

```rust
//! The TypeScript bindings and view-model fixtures are generated from the
//! Rust types and committed. This test fails when they are stale. To
//! regenerate: `AGENT_NOTIFY_WRITE_BINDINGS=1 cargo test -p agent-notify-daemon --test bindings`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use agent_notify_core::view::{CardView, FinishedEntry, PanelViewModel};
use agent_notify_core::wire::Kind;
use agent_notify_core::{CardId, EpochMs, EventId};
use agent_notify_daemon::bridge::{HOVER_EVENT, HoverPayload, UiBootstrap, UiMessage, VIEW_MODEL_EVENT};
use ts_rs::{Config, TS};

fn ui_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui")
}

fn writing() -> bool {
    std::env::var("AGENT_NOTIFY_WRITE_BINDINGS").as_deref() == Ok("1")
}

fn read_directory(directory: &Path) -> BTreeMap<String, String> {
    std::fs::read_dir(directory)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.path().extension().is_some_and(|extension| extension == "ts"))
                .map(|entry| (entry.file_name().to_string_lossy().into_owned(), std::fs::read_to_string(entry.path()).expect("readable")))
                .collect()
        })
        .unwrap_or_default()
}

fn export_into(directory: &Path) {
    let config = Config::new().with_large_int("number").with_out_dir(directory);
    UiBootstrap::export_all(&config).expect("exports UiBootstrap");
    UiMessage::export_all(&config).expect("exports UiMessage");
    HoverPayload::export_all(&config).expect("exports HoverPayload");
    let events = format!(
        "// Generated by daemon/tests/bindings.rs. Do not edit.\nexport const VIEW_MODEL_EVENT = {VIEW_MODEL_EVENT:?};\nexport const HOVER_EVENT = {HOVER_EVENT:?};\n"
    );
    std::fs::write(directory.join("events.ts"), events).expect("writes events.ts");
}

#[test]
fn the_typescript_bindings_are_current() {
    let committed = ui_directory().join("src/bindings");
    if writing() {
        let _ = std::fs::remove_dir_all(&committed);
        export_into(&committed);
    }
    let scratch = tempfile::tempdir().expect("a scratch directory");
    export_into(scratch.path());
    let generated = read_directory(scratch.path());
    assert!(generated.contains_key("PanelViewModel.ts") && generated.contains_key("UiMessage.ts"), "{:?}", generated.keys());
    assert_eq!(read_directory(&committed), generated, "stale bindings; rerun with AGENT_NOTIFY_WRITE_BINDINGS=1");
}

fn card(number: u8, kind: Kind, location: &str, title: Option<&str>, body: Option<&str>, note: Option<&str>) -> CardView {
    CardView {
        card_id: CardId::for_event(&EventId::from_parts(EpochMs(1_760_500_000_000 + i64::from(number)), [number; 10])),
        kind,
        location: location.to_string(),
        title: title.map(str::to_string),
        body: body.map(str::to_string),
        created_ms: EpochMs(1_760_500_000_000 + i64::from(number) * 60_000),
        note: note.map(str::to_string),
    }
}

fn fixtures() -> Vec<(&'static str, PanelViewModel)> {
    let attention = card(1, Kind::Attention, "4 · code/feature-x", Some("Fix sync retry"), Some("Allow Bash: cargo test?"), None);
    let info = card(2, Kind::Info, "2 · notes/main", Some("Draft release notes"), Some("Done. All tests pass. <b>not bold</b>"), None);
    let noted = card(3, Kind::Attention, "7 · api/auth", None, None, Some("Session ended"));
    let folded = FinishedEntry { card_id: card(4, Kind::Info, "", None, None, None).card_id, label: "6".to_string() };
    vec![
        ("empty", PanelViewModel { revision: 1, cards: Vec::new(), finished_row: Vec::new(), dnd: false }),
        ("mixed", PanelViewModel { revision: 5, cards: vec![attention.clone(), info.clone(), noted], finished_row: vec![folded], dnd: false }),
        ("dndOn", PanelViewModel { revision: 6, cards: Vec::new(), finished_row: Vec::new(), dnd: true }),
        ("revisionTen", PanelViewModel { revision: 10, cards: vec![info.clone()], finished_row: Vec::new(), dnd: false }),
        ("revisionNine", PanelViewModel { revision: 9, cards: vec![attention, info], finished_row: Vec::new(), dnd: false }),
    ]
}

fn fixture_module() -> String {
    let mut text = String::from(
        "// Generated by daemon/tests/bindings.rs. Do not edit.\nimport type { PanelViewModel } from \"../../src/bindings/PanelViewModel\";\n",
    );
    for (name, model) in fixtures() {
        let json = serde_json::to_string_pretty(&model).expect("encodes");
        text.push_str(&format!("\nexport const {name} = {json} satisfies PanelViewModel;\n"));
    }
    text
}

#[test]
fn the_view_model_fixtures_are_current() {
    let path = ui_directory().join("test/fixtures/view-models.ts");
    if writing() {
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("creates");
        std::fs::write(&path, fixture_module()).expect("writes");
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(committed, fixture_module(), "stale fixtures; rerun with AGENT_NOTIFY_WRITE_BINDINGS=1");
}
```

The info body carries `<b>not bold</b>` on purpose: Task 13's component test asserts it renders as text.

- [ ] **Step 3: Run them to see them fail**

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon bridge && cargo test -p agent-notify-daemon --test bindings`
Expected: compile errors, `could not find bridge`.

- [ ] **Step 4: Implement `bridge.rs`**

Add `pub mod bridge;` to `lib.rs`. Above the tests:

```rust
//! The messages between the Rust shell and the web view. UI messages
//! carry card ids only; the daemon never takes a tmux target from the page.

use agent_notify_core::CardId;
use agent_notify_core::update::UiIntent;
use agent_notify_core::view::PanelViewModel;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The event that carries a new [`PanelViewModel`] to the page.
pub const VIEW_MODEL_EVENT: &str = "view-model";
/// The event that carries native hover to the page (macOS).
pub const HOVER_EVENT: &str = "hover";

/// A message from the page.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiMessage {
    /// A click on a card or a finished-row entry.
    Clicked {
        /// The card.
        card_id: CardId,
    },
    /// The close button.
    Closed {
        /// The card.
        card_id: CardId,
    },
    /// The pointer entered or left the stack. Sent only where the platform
    /// has no native hover (plan 3).
    Hover {
        /// True on enter.
        hovered: bool,
    },
    /// The stack's rendered height, so the shell can size the panel.
    Layout {
        /// CSS pixels.
        content_height_px: u32,
    },
    /// An uncaught error in the page.
    Error {
        /// The message.
        message: String,
        /// The stack, when the page has one.
        stack: Option<String>,
    },
}

/// What the page reads once at start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct UiBootstrap {
    /// The current view model.
    pub view_model: PanelViewModel,
    /// True when the shell reports hover (macOS). The page then shows close
    /// buttons from [`HOVER_EVENT`] instead of CSS `:hover`.
    pub native_hover: bool,
}

/// The payload of [`HOVER_EVENT`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
pub struct HoverPayload {
    /// True while the pointer is over the panel.
    pub hovered: bool,
}

/// Where a UI message goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Routed {
    /// To the core.
    Intent(UiIntent),
    /// To the panel geometry.
    Layout {
        /// CSS pixels.
        content_height_px: u32,
    },
    /// To the log.
    Error {
        /// The message.
        message: String,
        /// The stack.
        stack: Option<String>,
    },
}

/// Routes one message.
#[must_use]
pub fn route(message: UiMessage) -> Routed {
    match message {
        UiMessage::Clicked { card_id } => Routed::Intent(UiIntent::Clicked { card_id }),
        UiMessage::Closed { card_id } => Routed::Intent(UiIntent::Closed { card_id }),
        UiMessage::Hover { hovered } => Routed::Intent(UiIntent::HoverChanged { hovered }),
        UiMessage::Layout { content_height_px } => Routed::Layout { content_height_px },
        UiMessage::Error { message, stack } => Routed::Error { message, stack },
    }
}
```

`deny_unknown_fields` with an internally tagged enum applies to each variant's fields; serde accepts the combination.

- [ ] **Step 5: Generate the bindings and fixtures, then read them**

Run: `cd ~/tools/agent-notify && AGENT_NOTIFY_WRITE_BINDINGS=1 cargo test -p agent-notify-daemon --test bindings && ls ui/src/bindings && cat ui/src/bindings/UiMessage.ts ui/src/bindings/CardView.ts`
Expected: ten `.ts` files. `UiMessage.ts` is a union of `{ "type": "clicked", card_id: CardId }` and the others. `CardView.ts` has `kind: Kind`, `created_ms: EpochMs`, `body: string | null`. No `bigint` anywhere: `grep -r bigint ui/src/bindings` prints nothing.

- [ ] **Step 6: Run the tests and clippy**

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon && cargo clippy --workspace --all-targets -- -D warnings`
Expected: all pass, with the environment variable unset.

- [ ] **Step 7: Commit**

```bash
cd ~ && config add tools/agent-notify
config diff --cached --name-only
config commit -m "Add the agent-notify UI bridge types and generated bindings" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- tools/agent-notify
```

---

### Task 11: The socket server

**Files:**
- Create: `tools/agent-notify/daemon/src/socket.rs`, `daemon/tests/socket.rs`
- Modify: `tools/agent-notify/daemon/src/lib.rs`

**Interfaces:**
- Consumes: plan 1 `decode_request`, `DecodeError`, `encode_line`, `Reply`, `ReplyCode`, `Request`, `Query`, `MAX_LINE_BYTES`.
- Produces (`agent_notify_daemon::socket`):
  - `bind(&Path) -> io::Result<BindOutcome>`, `BindOutcome::{Bound(Listener), AlreadyRunning}`, `Listener` (removes its socket file on drop).
  - `Inbound::{Request(Request), Query { query: Query, answer: oneshot::Sender<serde_json::Value> }}`.
  - `ServeOptions { line_timeout: Duration, query_timeout: Duration, max_connections: usize }` with `Default` (5 s, 1 s, 64).
  - `serve(listener: Listener, stamp: String, inbound: mpsc::Sender<Inbound>, options: ServeOptions) -> io::Result<()>` (async; runs until its task is dropped).
  - `classify(&[u8], bool) -> Result<Request, ReplyCode>` (pure: the bytes read and whether the line overflowed).

**Contract.** One request per connection. The daemon reads one line, answers one line, and closes. Agent and control requests get `ok` as soon as they decode and enter the intake queue (deviation "early reply"). A query gets `ok` with `data` once the actor answers. A full intake queue, a query with no answer in time, a silent client and a peer of another user each get the connection closed with no reply: the CLI then reports a timeout or uses its fallback. A peer of another user is logged at `warn`.

- [ ] **Step 1: Write the failing unit tests**

`daemon/src/socket.rs`, tests first:

```rust
#[cfg(test)]
mod tests {
    use agent_notify_core::wire::{MAX_LINE_BYTES, ReplyCode};

    use super::*;

    const JUMP: &str = r#"{"type":"control","v":1,"event_id":"0000000000000000000000000M","sent_at_ms":1760500000017,"control":{"type":"jump"}}"#;

    #[test]
    fn a_valid_line_classifies_as_its_request() {
        assert!(matches!(classify(JUMP.as_bytes(), false), Ok(Request::Control { .. })));
    }

    #[test]
    fn bad_lines_map_to_reply_codes() {
        assert_eq!(classify(b"not json", false).err(), Some(ReplyCode::BadRequest));
        assert_eq!(classify(&[0xff, 0xfe], false).err(), Some(ReplyCode::BadRequest));
        assert_eq!(classify(&vec![b'x'; MAX_LINE_BYTES + 1], true).err(), Some(ReplyCode::LineTooLong));
        let future = JUMP.replace(r#""v":1"#, r#""v":9"#);
        assert_eq!(classify(future.as_bytes(), false).err(), Some(ReplyCode::UnsupportedVersion));
        let unknown = JUMP.replace(r#""type":"jump""#, r#""type":"teleport""#);
        assert_eq!(classify(unknown.as_bytes(), false).err(), Some(ReplyCode::UnknownKind));
    }
}
```

`0000000000000000000000000M` is the fixture event id from Task 1. If plan 1's `ReplyCode` lacks `PartialEq`, add `PartialEq, Eq` to its derive in the core as part of this task.

- [ ] **Step 2: Write the failing integration tests**

`daemon/tests/socket.rs`:

```rust
//! The socket server, end to end over a real Unix socket.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_notify_core::wire::MAX_LINE_BYTES;
use agent_notify_daemon::socket::{BindOutcome, Inbound, ServeOptions, bind, serve};
use tokio::sync::mpsc;

const JUMP: &str = r#"{"type":"control","v":1,"event_id":"0000000000000000000000000M","sent_at_ms":1760500000017,"control":{"type":"jump"}}"#;
const STATUS: &str = r#"{"type":"query","v":1,"event_id":"0000000000000000000000000N","query":"status"}"#;

struct World {
    directory: tempfile::TempDir,
    socket: PathBuf,
    inbound: mpsc::Receiver<Inbound>,
    server: tokio::task::JoinHandle<std::io::Result<()>>,
}

fn options() -> ServeOptions {
    ServeOptions { line_timeout: Duration::from_millis(300), query_timeout: Duration::from_millis(300), max_connections: 8 }
}

fn scratch() -> tempfile::TempDir {
    tempfile::Builder::new().prefix("an-").tempdir_in("/tmp").expect("a directory under /tmp")
}

fn start() -> World {
    let directory = scratch();
    let socket = directory.path().join("run/agent-notify.sock");
    let BindOutcome::Bound(listener) = bind(&socket).expect("binds") else { panic!("nothing else runs here") };
    let (sender, inbound) = mpsc::channel(16);
    let server = tokio::spawn(serve(listener, "stamp-1".to_string(), sender, options()));
    World { directory, socket, inbound, server }
}

fn exchange(socket: &Path, line: &[u8]) -> String {
    let mut stream = UnixStream::connect(socket).expect("connects");
    stream.set_read_timeout(Some(Duration::from_secs(2))).expect("sets a timeout");
    stream.write_all(line).expect("writes");
    let mut reply = String::new();
    let _ = BufReader::new(&stream).read_line(&mut reply);
    reply
}

async fn exchange_async(socket: PathBuf, line: Vec<u8>) -> String {
    tokio::task::spawn_blocking(move || exchange(&socket, &line)).await.expect("joins")
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agent_line_is_acknowledged_and_forwarded() {
    let mut world = start();
    let reply = exchange_async(world.socket.clone(), format!("{JUMP}\n").into_bytes()).await;
    assert_eq!(serde_json::from_str::<serde_json::Value>(&reply).expect("JSON")["ok"], true, "{reply}");
    assert!(reply.contains("stamp-1"), "{reply}");
    assert!(matches!(world.inbound.recv().await, Some(Inbound::Request(_))));
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_query_is_answered_with_data() {
    let mut world = start();
    let asking = tokio::spawn(exchange_async(world.socket.clone(), format!("{STATUS}\n").into_bytes()));
    let Some(Inbound::Query { answer, .. }) = world.inbound.recv().await else { panic!("a query") };
    answer.send(serde_json::json!({ "answer": 42 })).expect("the server waits");
    let reply = asking.await.expect("joins");
    assert_eq!(serde_json::from_str::<serde_json::Value>(&reply).expect("JSON")["data"]["answer"], 42, "{reply}");
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unanswered_query_closes_without_a_reply() {
    let mut world = start();
    let asking = tokio::spawn(exchange_async(world.socket.clone(), format!("{STATUS}\n").into_bytes()));
    let held = world.inbound.recv().await;
    assert_eq!(asking.await.expect("joins"), "");
    drop(held);
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_malformed_line_is_refused_with_bad_request() {
    let world = start();
    let reply = exchange_async(world.socket.clone(), b"{nope\n".to_vec()).await;
    assert!(reply.contains("\"code\":\"bad_request\""), "{reply}");
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn an_oversized_line_is_refused_and_other_clients_are_served() {
    let mut world = start();
    let mut huge = vec![b'x'; MAX_LINE_BYTES + 10];
    huge.push(b'\n');
    let refused = exchange_async(world.socket.clone(), huge).await;
    assert!(refused.contains("\"code\":\"line_too_long\""), "{refused}");
    let served = exchange_async(world.socket.clone(), format!("{JUMP}\n").into_bytes()).await;
    assert!(served.contains("\"ok\":true"), "{served}");
    assert!(world.inbound.recv().await.is_some());
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_silent_client_is_dropped_after_the_timeout() {
    let world = start();
    let socket = world.socket.clone();
    let silent = tokio::task::spawn_blocking(move || {
        let stream = UnixStream::connect(&socket).expect("connects");
        stream.set_read_timeout(Some(Duration::from_secs(3))).expect("sets a timeout");
        let mut reply = String::new();
        let read = BufReader::new(&stream).read_line(&mut reply);
        (read.map_err(|error| error.kind()), reply)
    });
    let served = exchange_async(world.socket.clone(), format!("{JUMP}\n").into_bytes()).await;
    assert!(served.contains("\"ok\":true"), "a silent client must not block others: {served}");
    let (read, reply) = silent.await.expect("joins");
    assert_eq!(read, Ok(0), "the daemon closed the connection, it did not wait for the client's 3 s timeout");
    assert_eq!(reply, "");
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stale_socket_file_is_replaced() {
    let directory = scratch();
    let socket = directory.path().join("run/agent-notify.sock");
    std::fs::create_dir_all(socket.parent().expect("a parent")).expect("creates");
    drop(std::os::unix::net::UnixListener::bind(&socket).expect("a stale socket"));
    assert!(socket.exists(), "the stale file stays after the listener closes");
    assert!(matches!(bind(&socket).expect("binds"), BindOutcome::Bound(_)));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_instance_sees_the_first() {
    let world = start();
    assert!(matches!(bind(&world.socket).expect("checks"), BindOutcome::AlreadyRunning));
    let reply = exchange_async(world.socket.clone(), format!("{JUMP}\n").into_bytes()).await;
    assert!(reply.contains("\"ok\":true"), "the first daemon still answers: {reply}");
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_socket_and_its_directory_are_private() {
    let world = start();
    let mode = |path: &Path| std::fs::metadata(path).expect("exists").permissions().mode() & 0o777;
    assert_eq!(mode(&world.socket), 0o600);
    assert_eq!(mode(world.socket.parent().expect("a parent")), 0o700);
    drop(world.directory);
}

#[tokio::test(flavor = "multi_thread")]
async fn stopping_the_server_removes_the_socket_file() {
    let world = start();
    world.server.abort();
    let _ = world.server.await;
    assert!(!world.socket.exists());
}
```

Add to `daemon/Cargo.toml` `[dev-dependencies]`: nothing new; `tokio` with `macros` and `rt-multi-thread` is already a normal dependency.

- [ ] **Step 3: Run them to see them fail**

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon --test socket`
Expected: compile errors, `could not find socket`.

- [ ] **Step 4: Implement `socket.rs`**

Add `pub mod socket;` to `lib.rs`. Above the unit tests:

```rust
//! The daemon's Unix socket: single instance, private permissions, one
//! request per connection, and a bounded line. See the contract in the
//! plan, Task 11.

use std::fs::{self, File, Permissions, TryLockError};
use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use agent_notify_core::wire::{DecodeError, MAX_LINE_BYTES, Query, Reply, ReplyCode, Request, decode_request, encode_line};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::{Semaphore, mpsc, oneshot};
use tokio::time::timeout;

const LOCK_FILE: &str = "agent-notifyd.lock";

/// The result of [`bind`].
#[derive(Debug)]
pub enum BindOutcome {
    /// This process owns the socket.
    Bound(Listener),
    /// Another daemon holds the lock. Exit without touching its socket.
    AlreadyRunning,
}

/// A bound socket and the instance lock. Dropping it removes the socket
/// file, then releases the lock.
#[derive(Debug)]
pub struct Listener {
    listener: std::os::unix::net::UnixListener,
    path: PathBuf,
    owner: u32,
    _lock: File,
}

impl Drop for Listener {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.path) {
            tracing::debug!(%error, path = %self.path.display(), "the socket file was already gone");
        }
    }
}

/// Takes the instance lock, replaces a stale socket file and binds.
///
/// # Errors
///
/// Any filesystem or bind error.
pub fn bind(path: &Path) -> io::Result<BindOutcome> {
    let directory = path.parent().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the socket path has no directory"))?;
    fs::create_dir_all(directory)?;
    fs::set_permissions(directory, Permissions::from_mode(0o700))?;
    let lock = File::options().create(true).truncate(false).write(true).open(directory.join(LOCK_FILE))?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => return Ok(BindOutcome::AlreadyRunning),
        Err(TryLockError::Error(error)) => return Err(error),
    }
    match fs::remove_file(path) {
        Ok(()) => tracing::info!(path = %path.display(), "replaced a stale socket file"),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let listener = std::os::unix::net::UnixListener::bind(path)?;
    fs::set_permissions(path, Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    let owner = fs::metadata(directory)?.uid();
    Ok(BindOutcome::Bound(Listener { listener, path: path.to_path_buf(), owner, _lock: lock }))
}

/// A decoded request on its way to the daemon.
#[derive(Debug)]
pub enum Inbound {
    /// An agent or control request. Already acknowledged.
    Request(Request),
    /// A query. The connection waits for `answer`.
    Query {
        /// What is asked.
        query: Query,
        /// Where the answer goes.
        answer: oneshot::Sender<serde_json::Value>,
    },
}

/// Limits for [`serve`].
#[derive(Debug, Clone, Copy)]
pub struct ServeOptions {
    /// How long a client has to send its line.
    pub line_timeout: Duration,
    /// How long a query waits for the actor.
    pub query_timeout: Duration,
    /// Connections handled at once. More are closed at accept.
    pub max_connections: usize,
}

impl Default for ServeOptions {
    fn default() -> Self {
        ServeOptions { line_timeout: Duration::from_secs(5), query_timeout: Duration::from_secs(1), max_connections: 64 }
    }
}

struct Context {
    stamp: String,
    inbound: mpsc::Sender<Inbound>,
    options: ServeOptions,
    owner: u32,
}

/// Accepts connections until the task running it is dropped. Must run
/// inside a Tokio runtime.
///
/// # Errors
///
/// When the listener cannot join the runtime.
pub async fn serve(listener: Listener, stamp: String, inbound: mpsc::Sender<Inbound>, options: ServeOptions) -> io::Result<()> {
    let accepting = tokio::net::UnixListener::from_std(listener.listener.try_clone()?)?;
    let limit = Arc::new(Semaphore::new(options.max_connections));
    let context = Arc::new(Context { stamp, inbound, options, owner: listener.owner });
    let _owned = listener;
    loop {
        let stream = match accepting.accept().await {
            Ok((stream, _)) => stream,
            Err(error) => {
                tracing::warn!(%error, "accept failed");
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
        };
        let Ok(permit) = Arc::clone(&limit).try_acquire_owned() else {
            tracing::warn!("too many connections; closed one at accept");
            continue;
        };
        let context = Arc::clone(&context);
        tokio::spawn(async move {
            handle(stream, &context).await;
            drop(permit);
        });
    }
}

async fn handle(stream: UnixStream, context: &Context) {
    match stream.peer_cred() {
        Ok(credentials) if credentials.uid() == context.owner => {}
        other => {
            tracing::warn!(peer = ?other, "refused a peer that is not the socket's owner");
            return;
        }
    }
    let (read_half, mut write_half) = stream.into_split();
    let (bytes, overflowed) = match timeout(context.options.line_timeout, read_line(read_half)).await {
        Ok(Ok(read)) => read,
        Ok(Err(error)) => {
            tracing::debug!(%error, "a client read failed");
            return;
        }
        Err(_) => {
            tracing::info!("a client sent no line in time");
            return;
        }
    };
    let Some(reply) = answer(&bytes, overflowed, context).await else { return };
    match encode_line(&reply) {
        Ok(line) => {
            if let Ok(Err(error)) = timeout(context.options.line_timeout, write_half.write_all(line.as_bytes())).await {
                tracing::debug!(%error, "a reply write failed");
            }
        }
        Err(error) => tracing::error!(%error, "a reply did not encode"),
    }
}

async fn read_line(read_half: tokio::net::unix::OwnedReadHalf) -> io::Result<(Vec<u8>, bool)> {
    let limit = u64::try_from(MAX_LINE_BYTES).unwrap_or(u64::MAX).saturating_add(1);
    let mut bytes = Vec::new();
    BufReader::new(read_half.take(limit)).read_until(b'\n', &mut bytes).await?;
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
        return Ok((bytes, false));
    }
    let overflowed = bytes.len() > MAX_LINE_BYTES;
    Ok((bytes, overflowed))
}

async fn answer(bytes: &[u8], overflowed: bool, context: &Context) -> Option<Reply> {
    match classify(bytes, overflowed) {
        Err(code) => Some(Reply::rejected(code)),
        Ok(Request::Query { query, .. }) => {
            let (sender, receiver) = oneshot::channel();
            context.inbound.try_send(Inbound::Query { query, answer: sender }).ok()?;
            match timeout(context.options.query_timeout, receiver).await {
                Ok(Ok(data)) => Some(Reply { data: Some(data), ..Reply::accepted(&context.stamp) }),
                _ => {
                    tracing::warn!("a query got no answer in time");
                    None
                }
            }
        }
        Ok(request) => match context.inbound.try_send(Inbound::Request(request)) {
            Ok(()) => Some(Reply::accepted(&context.stamp)),
            Err(error) => {
                tracing::error!(%error, "the intake queue refused a request; the client falls back");
                None
            }
        },
    }
}

/// Decodes one line, or names the reply code that refuses it.
///
/// # Errors
///
/// The [`ReplyCode`] for an overlong, non-UTF-8 or invalid line.
pub fn classify(bytes: &[u8], overflowed: bool) -> Result<Request, ReplyCode> {
    if overflowed {
        return Err(ReplyCode::LineTooLong);
    }
    let line = std::str::from_utf8(bytes).map_err(|_| ReplyCode::BadRequest)?;
    decode_request(line).map_err(|error| match error {
        DecodeError::LineTooLong { .. } => ReplyCode::LineTooLong,
        DecodeError::UnsupportedVersion { .. } => ReplyCode::UnsupportedVersion,
        DecodeError::UnknownKind => ReplyCode::UnknownKind,
        DecodeError::Malformed { .. } => ReplyCode::BadRequest,
    })
}
```

If plan 1's `DecodeError` has more variants than these four, map each one explicitly; do not add a `_` arm.

- [ ] **Step 5: Run the tests and clippy**

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon && cargo clippy --workspace --all-targets -- -D warnings`
Expected: all pass. `a_silent_client_is_dropped_after_the_timeout` takes about 300 ms.

- [ ] **Step 6: Prove the length limit and the lock are load-bearing**

One at a time, with a backup and a verified restore each time: (a) change `limit` in `read_line` to `u64::MAX`; run `cargo test -p agent-notify-daemon --test socket an_oversized_line; echo "exit $?"`. (b) Replace the `try_lock` match with `Ok::<(), TryLockError>(())` handling; run `cargo test -p agent-notify-daemon --test socket a_second_instance; echo "exit $?"`. Expected: a non-zero exit both times. Restore, `touch`, `diff` against the backup, and rerun green.

- [ ] **Step 7: Commit**

```bash
cd ~ && config add tools/agent-notify
config diff --cached --name-only
config commit -m "Add the agent-notify daemon socket server" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- tools/agent-notify
```

---

### Task 12: The actor, the effects worker and the do-not-disturb store

**Files:**
- Create: `tools/agent-notify/daemon/src/dnd_store.rs`, `src/actor.rs`, `daemon/tests/daemon.rs`
- Modify: `tools/agent-notify/daemon/src/lib.rs`, `src/services.rs` (the `Services` record), `daemon/tests/support/mod.rs` (the daemon harness)

**Interfaces:**
- Consumes: plan 1 `DaemonState`, `DaemonInput`, `UiIntent`, `FocusSnapshot`, `update`, `view`, `Effect`, `Dnd`; Task 2 `status_report`, `decision`, `Decision`, `MAX_DECISIONS`, `DaemonFacts`; Tasks 8 to 11.
- Produces:
  - `dnd_store::DndStore` with `new(PathBuf)`, `load(&self) -> Dnd`, `save(&self, &Dnd) -> io::Result<()>`; `dnd_store::{dnd_from_text(&str) -> Result<Dnd, serde_json::Error>, dnd_to_text(&Dnd) -> String}`.
  - `services::Services { tmux: Option<Tmux>, raiser, presenter, frontmost, presence, sound, dnd_store: DndStore, clock: Clock, facts: DaemonFacts }` with the four `Arc<dyn ...>` fields.
  - `actor::DaemonConfig { tick: Duration, snapshot_every_ms: i64, serve: ServeOptions, intake_capacity: usize }` with `Default` (250 ms, 60 000, default, 1024).
  - `actor::Daemon` with `start(Arc<Services>, Listener, DaemonConfig) -> Daemon` (call inside a Tokio runtime), `handle(&self) -> DaemonHandle`, `send_ui(&self, UiIntent)`, `async view_model(&self) -> Option<PanelViewModel>`, `async stop(self)`.
  - `actor::DaemonHandle` (`Clone + Send + Sync`) with `send_ui(&self, UiIntent)` and `async view_model(&self) -> Option<PanelViewModel>`. Tauri callbacks hold a handle; only `main` holds the `Daemon`.
  - `actor::gather_focus(&Services, &Request) -> FocusSnapshot`.

**Ownership.** The actor task owns `DaemonState` and the decision log; nothing else reads or writes them. The intake task runs requests in arrival order and gathers focus facts for agent requests on a blocking thread. The worker thread runs focus plans, tmux writes, DND saves and pane snapshots in order, and sends results back as `DaemonInput`. The actor pushes a view to the presenter only when the cards, the finished row or the DND flag change.

- [ ] **Step 1: Write the failing `dnd_store` tests**

`daemon/src/dnd_store.rs`, tests first:

```rust
#[cfg(test)]
mod tests {
    use agent_notify_core::EpochMs;
    use agent_notify_core::state::Dnd;

    use super::*;

    #[test]
    fn dnd_round_trips_through_text() {
        for dnd in [Dnd::Off, Dnd::On { until: None }, Dnd::On { until: Some(EpochMs(1_760_500_000_000)) }] {
            assert_eq!(dnd_from_text(&dnd_to_text(&dnd)).expect("parses"), dnd);
        }
    }

    #[test]
    fn a_missing_file_reads_as_off() {
        let directory = tempfile::tempdir().expect("a directory");
        assert_eq!(DndStore::new(directory.path().join("dnd.json")).load(), Dnd::Off);
    }

    #[test]
    fn a_corrupt_dnd_file_reads_as_off() {
        let directory = tempfile::tempdir().expect("a directory");
        let path = directory.path().join("dnd.json");
        for text in ["", "{\"on\": tru", "[]", "{\"on\": true, \"until_ms\": \"soon\"}"] {
            std::fs::write(&path, text).expect("writes");
            assert_eq!(DndStore::new(path.clone()).load(), Dnd::Off, "{text:?}");
        }
    }

    #[test]
    fn save_then_load_keeps_the_value_and_leaves_no_temp_file() {
        let directory = tempfile::tempdir().expect("a directory");
        let store = DndStore::new(directory.path().join("state/dnd.json"));
        let dnd = Dnd::On { until: Some(EpochMs(5)) };
        store.save(&dnd).expect("saves");
        assert_eq!(store.load(), dnd);
        let names: Vec<String> =
            std::fs::read_dir(directory.path().join("state")).expect("lists").flatten().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["dnd.json".to_string()]);
    }
}
```

`tempfile` is a dev-dependency already.

- [ ] **Step 2: Implement `dnd_store.rs`**

Add `pub mod dnd_store;` to `lib.rs`. Above the tests:

```rust
//! Do-not-disturb on disk, so it survives a restart. A corrupt or
//! half-written file reads as off and is logged: a daemon that crashes on
//! it would crash-loop under launchd.

use std::fs;
use std::io;
use std::path::PathBuf;

use agent_notify_core::EpochMs;
use agent_notify_core::state::Dnd;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DndFile {
    on: bool,
    until_ms: Option<i64>,
}

/// Parses the file's text.
///
/// # Errors
///
/// The JSON error for text that is not a DND record.
pub fn dnd_from_text(text: &str) -> Result<Dnd, serde_json::Error> {
    let file: DndFile = serde_json::from_str(text)?;
    Ok(if file.on { Dnd::On { until: file.until_ms.map(EpochMs) } } else { Dnd::Off })
}

/// The file's text.
#[must_use]
pub fn dnd_to_text(dnd: &Dnd) -> String {
    let file = match dnd {
        Dnd::Off => DndFile { on: false, until_ms: None },
        Dnd::On { until } => DndFile { on: true, until_ms: until.map(|until| until.0) },
    };
    serde_json::to_string(&file).unwrap_or_else(|_| "{\"on\":false,\"until_ms\":null}".to_string())
}

/// The DND file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DndStore {
    path: PathBuf,
}

impl DndStore {
    /// The store at `path`.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        DndStore { path }
    }

    /// The saved value, or `Off` when the file is missing or unreadable.
    #[must_use]
    pub fn load(&self) -> Dnd {
        match fs::read_to_string(&self.path) {
            Ok(text) => dnd_from_text(&text).unwrap_or_else(|error| {
                tracing::error!(%error, path = %self.path.display(), "the DND file is corrupt; starting with DND off");
                Dnd::Off
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Dnd::Off,
            Err(error) => {
                tracing::error!(%error, path = %self.path.display(), "the DND file is unreadable; starting with DND off");
                Dnd::Off
            }
        }
    }

    /// Writes `dnd` to a temporary file, then renames it over the old one.
    ///
    /// # Errors
    ///
    /// Any filesystem error.
    pub fn save(&self, dnd: &Dnd) -> io::Result<()> {
        if let Some(directory) = self.path.parent() {
            fs::create_dir_all(directory)?;
        }
        let temporary = self.path.with_extension("json.tmp");
        fs::write(&temporary, dnd_to_text(dnd))?;
        fs::rename(&temporary, &self.path)
    }
}
```

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon dnd_store`
Expected: pass. If `EpochMs`'s field is private in plan 1, use its accessor instead of `.0`.

- [ ] **Step 3: Add the `Services` record**

Append to `services.rs` (extend the `use` lines with `agent_notify_core::status::DaemonFacts`, `crate::dnd_store::DndStore` and `crate::tmux::Tmux`):

```rust
/// Every service the daemon uses, built once by `main`.
pub struct Services {
    /// tmux, when found.
    pub tmux: Option<Tmux>,
    /// The terminal raiser.
    pub raiser: Arc<dyn WindowRaiser>,
    /// The toast panel.
    pub presenter: Arc<dyn ToastPresenter>,
    /// The frontmost-app reader.
    pub frontmost: Arc<dyn FrontmostReader>,
    /// The presence reader.
    pub presence: Arc<dyn PresenceReader>,
    /// The attention sound.
    pub sound: Arc<dyn SoundPlayer>,
    /// The DND file.
    pub dnd_store: DndStore,
    /// The wall clock.
    pub clock: Clock,
    /// Facts for `status`.
    pub facts: DaemonFacts,
}
```

- [ ] **Step 4: Write the daemon harness**

Append to `daemon/tests/support/mod.rs`:

```rust
use std::sync::{Arc, Condvar};
use std::time::{Duration, Instant};

use agent_notify_core::EpochMs;
use agent_notify_core::status::DaemonFacts;
use agent_notify_core::view::PanelViewModel;
use agent_notify_daemon::actor::{Daemon, DaemonConfig};
use agent_notify_daemon::dnd_store::DndStore;
use agent_notify_daemon::services::{NoPresenter, Services, Silent, ToastPresenter, UnknownFrontmost, UnknownPresence};
use agent_notify_daemon::socket::{BindOutcome, ServeOptions, bind};

/// A presenter that keeps every view it is given.
#[derive(Debug, Default)]
pub struct RecordingPresenter {
    views: Mutex<Vec<PanelViewModel>>,
    changed: Condvar,
}

impl ToastPresenter for RecordingPresenter {
    fn present(&self, view: &PanelViewModel) {
        self.views.lock().expect("unpoisoned").push(view.clone());
        self.changed.notify_all();
    }
}

impl RecordingPresenter {
    /// Waits up to 3 s for a presented view that satisfies `test`.
    pub fn wait_for(&self, test: impl Fn(&PanelViewModel) -> bool) -> PanelViewModel {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut views = self.views.lock().expect("unpoisoned");
        loop {
            if let Some(found) = views.iter().rev().find(|view| test(view)) {
                return found.clone();
            }
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "no matching view in 3 s; saw {views:#?}");
            views = self.changed.wait_timeout(views, left).expect("unpoisoned").0;
        }
    }
}

/// A running daemon on a private socket, a tmux server, and recorders.
pub struct DaemonWorld {
    pub daemon: Daemon,
    pub socket: PathBuf,
    pub presenter: Arc<RecordingPresenter>,
    pub raiser: Arc<RecordingRaiser>,
    pub server: Server,
    pub dnd_path: PathBuf,
    pub scratch: tempfile::TempDir,
}

/// Starts a daemon whose tmux adapter points at a fresh tmux server.
pub fn spawn_daemon(label: &str) -> DaemonWorld {
    let scratch = tempfile::Builder::new().prefix("an-").tempdir_in("/tmp").expect("a directory under /tmp");
    spawn_daemon_in(label, scratch)
}

/// Starts a daemon with its state in `scratch`, which may hold a DND file already.
pub fn spawn_daemon_in(label: &str, scratch: tempfile::TempDir) -> DaemonWorld {
    let socket = scratch.path().join("run/agent-notify.sock");
    let dnd_path = scratch.path().join("state/dnd.json");
    let presenter = Arc::new(RecordingPresenter::default());
    let raiser = Arc::new(RecordingRaiser::default());
    let services = Arc::new(Services {
        tmux: Some(adapter()),
        raiser: raiser.clone(),
        presenter: presenter.clone(),
        frontmost: Arc::new(UnknownFrontmost),
        presence: Arc::new(UnknownPresence),
        sound: Arc::new(Silent),
        dnd_store: DndStore::new(dnd_path.clone()),
        clock: Arc::new(|| EpochMs(1_760_500_000_000)),
        facts: DaemonFacts { stamp: "stamp-test".to_string(), started_ms: EpochMs(1_760_499_000_000), tools: Vec::new() },
    });
    let BindOutcome::Bound(listener) = bind(&socket).expect("binds") else { panic!("nothing else runs here") };
    let config = DaemonConfig { serve: ServeOptions { line_timeout: Duration::from_millis(500), ..ServeOptions::default() }, ..DaemonConfig::default() };
    let daemon = Daemon::start(services, listener, config);
    DaemonWorld { daemon, socket, presenter, raiser, server: tmux_server(label), dnd_path, scratch }
}

/// Sends one line and returns the reply as JSON.
pub async fn send_line(socket: &Path, line: String) -> serde_json::Value {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let mut stream = tokio::net::UnixStream::connect(socket).await.expect("connects");
    stream.write_all(format!("{line}\n").as_bytes()).await.expect("writes");
    let mut reply = String::new();
    tokio::time::timeout(Duration::from_secs(2), BufReader::new(stream).read_line(&mut reply)).await.expect("in time").expect("reads");
    serde_json::from_str(&reply).unwrap_or(serde_json::Value::Null)
}

/// An agent event line from `pane` on `server`, in the wire fixture's shape.
pub fn agent_line(server: &Server, pane: &PaneId, number: u32, session: &str, prompt: &str, event: serde_json::Value) -> String {
    let id = server_id(server);
    serde_json::json!({
        "type": "agent", "v": 1,
        "event_id": format!("{number:026}"),
        "sent_at_ms": 1_760_500_000_000_i64 + i64::from(number),
        "origin": {
            "agent": "claude_code", "session": session, "prompt": prompt,
            "pane": { "server": { "socket_path": id.socket_path.as_path(), "pid": id.pid, "start_time": id.start_time }, "pane": pane.as_str() },
            "window": { "index": 0, "name": "main" },
            "session_title": "Fix sync retry",
            "cwd": "/tmp"
        },
        "event": event
    })
    .to_string()
}

/// A control line.
pub fn control_line(number: u32, control: serde_json::Value) -> String {
    serde_json::json!({ "type": "control", "v": 1, "event_id": format!("{number:026}"), "sent_at_ms": 1_760_500_000_000_i64 + i64::from(number), "control": control })
        .to_string()
}

/// A query line.
pub fn query_line(number: u32, query: &str) -> String {
    serde_json::json!({ "type": "query", "v": 1, "event_id": format!("{number:026}"), "query": query }).to_string()
}

/// Polls `read` until it returns `expected`, for up to 3 s. tmux options
/// change on the worker thread, which has no hook a test can wait on.
pub fn eventually(expected: &str, read: impl Fn() -> String) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let value = read();
        if value == expected {
            return;
        }
        assert!(Instant::now() < deadline, "still {value:?}, wanted {expected:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}
```

Merge the new `use` lines with the ones at the top of the file. `NoPresenter` is imported for tests that do not need views; delete the import if clippy reports it unused in every target.

Event ids are 26 decimal digits. Decimal digits are valid Crockford base32, and a leading `0` keeps the value under the ULID maximum.

- [ ] **Step 5: Write the failing daemon tests**

`daemon/tests/daemon.rs`:

```rust
//! The daemon end to end: socket in, tmux and presenter out.

mod support;

use agent_notify_core::state::Dnd;
use agent_notify_core::update::UiIntent;
use agent_notify_core::status::PANE_STATE_OPTION;
use agent_notify_core::wire::Kind;
use agent_notify_daemon::dnd_store::DndStore;
use serde_json::json;
use support::{agent_line, control_line, eventually, first_pane, new_window, pane_option, query_line, send_line, spawn_daemon, spawn_daemon_in, tag};

#[tokio::test(flavor = "multi_thread")]
async fn a_finished_turn_shows_an_info_card_and_marks_the_pane() {
    let world = spawn_daemon("an-turn");
    let pane = first_pane(&world.server, "main:0");
    let started = send_line(&world.socket, agent_line(&world.server, &pane, 1, "session-a", "prompt-1", json!({ "type": "turn_started" }))).await;
    assert_eq!(started["ok"], true, "{started}");
    let server = &world.server;
    let read_state = || pane_option(server, &pane, PANE_STATE_OPTION);
    tokio::task::block_in_place(|| eventually("working", read_state));
    send_line(&world.socket, agent_line(&world.server, &pane, 2, "session-a", "prompt-1", json!({ "type": "turn_ended", "last_message": "Done." }))).await;
    let view = tokio::task::block_in_place(|| world.presenter.wait_for(|view| !view.cards.is_empty()));
    assert_eq!(view.cards[0].kind, Kind::Info);
    assert_eq!(view.cards[0].body.as_deref(), Some("Done."));
    tokio::task::block_in_place(|| eventually("finished", read_state));
    world.daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn status_reports_the_session_and_its_decisions() {
    let world = spawn_daemon("an-status");
    let pane = first_pane(&world.server, "main:0");
    send_line(&world.socket, agent_line(&world.server, &pane, 1, "session-a", "prompt-1", json!({ "type": "turn_started" }))).await;
    let reply = send_line(&world.socket, query_line(2, "status")).await;
    assert_eq!(reply["ok"], true, "{reply}");
    assert_eq!(reply["data"]["daemon"]["stamp"], "stamp-test");
    assert_eq!(reply["data"]["sessions"][0]["turn"], "working", "{reply}");
    assert_eq!(reply["data"]["decisions"].as_array().map(Vec::len), Some(1), "{reply}");
    world.daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_click_runs_the_focus_plan_on_the_worker() {
    let world = spawn_daemon("an-click");
    let pane = new_window(&world.server);
    tag(&world.server, &pane, "session-a");
    send_line(&world.socket, agent_line(&world.server, &pane, 1, "session-a", "prompt-1", json!({ "type": "turn_started" }))).await;
    send_line(&world.socket, agent_line(&world.server, &pane, 2, "session-a", "prompt-1", json!({ "type": "turn_ended" }))).await;
    let view = tokio::task::block_in_place(|| world.presenter.wait_for(|view| !view.cards.is_empty()));
    world.daemon.send_ui(UiIntent::Clicked { card_id: view.cards[0].card_id.clone() });
    let raiser = world.raiser.clone();
    tokio::task::block_in_place(|| eventually("1", || raiser.attaches.lock().expect("unpoisoned").len().to_string()));
    assert_eq!(world.raiser.attaches.lock().expect("unpoisoned")[0].session, "main");
    world.daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn dnd_hides_cards_and_survives_a_restart() {
    let world = spawn_daemon("an-dnd");
    let reply = send_line(&world.socket, control_line(1, json!({ "type": "dnd", "command": { "type": "on" } }))).await;
    assert_eq!(reply["ok"], true, "{reply}");
    let view = tokio::task::block_in_place(|| world.presenter.wait_for(|view| view.dnd));
    assert!(view.cards.is_empty());
    let dnd_path = world.dnd_path.clone();
    tokio::task::block_in_place(|| eventually("true", || DndStore::new(dnd_path.clone()).load().eq(&Dnd::On { until: None }).to_string()));
    let scratch = world.scratch;
    world.daemon.stop().await;
    let restarted = spawn_daemon_in("an-dnd2", scratch);
    let model = restarted.daemon.view_model().await.expect("the actor answers");
    assert!(model.dnd, "DND was loaded from disk");
    restarted.daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn stopping_frees_the_socket_for_the_next_daemon() {
    let world = spawn_daemon("an-stop");
    let socket = world.socket.clone();
    world.daemon.stop().await;
    assert!(!socket.exists());
}
```

The DND control JSON follows plan 1's wire (`{"type":"dnd","command":{"type":"on"}}`); check it against `tests/fixtures/wire-v1.jsonl` and match it exactly.

- [ ] **Step 6: Run them to see them fail**

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon --test daemon`
Expected: compile errors, `could not find actor`.

- [ ] **Step 7: Implement `actor.rs`**

Add `pub mod actor;` to `lib.rs`. Create `actor.rs`:

```rust
//! The daemon's tasks: socket, intake, actor, tick, and the effects worker
//! thread. See "Ownership" in the plan, Task 12.

use std::collections::{BTreeSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use agent_notify_core::state::{DaemonState, Dnd};
use agent_notify_core::update::{DaemonInput, Effect, FocusSnapshot, FocusTarget, PaneState, UiIntent, update};
use agent_notify_core::status::{Decision, MAX_DECISIONS, decision, status_report};
use agent_notify_core::view::{PanelViewModel, view};
use agent_notify_core::wire::{Query, Request};
use agent_notify_core::{CardId, PaneKey, ServerId};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::focus_run::run_focus;
use crate::services::Services;
use crate::socket::{Inbound, Listener, ServeOptions, serve};

/// Timing and queue sizes.
#[derive(Debug, Clone, Copy)]
pub struct DaemonConfig {
    /// The timer that drives expiry.
    pub tick: Duration,
    /// How often to read pane tags from each known tmux server.
    pub snapshot_every_ms: i64,
    /// Socket limits.
    pub serve: ServeOptions,
    /// Requests waiting for the intake task.
    pub intake_capacity: usize,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        DaemonConfig { tick: Duration::from_millis(250), snapshot_every_ms: 60_000, serve: ServeOptions::default(), intake_capacity: 1024 }
    }
}

enum ActorMessage {
    Input(DaemonInput),
    Query { query: Query, answer: oneshot::Sender<serde_json::Value> },
    ViewModel(oneshot::Sender<PanelViewModel>),
}

enum Job {
    Focus { card: CardId, target: FocusTarget },
    PaneState { pane: PaneKey, state: PaneState },
    SaveDnd(Dnd),
    Snapshot(ServerId),
}

/// A running daemon.
pub struct Daemon {
    actor: mpsc::UnboundedSender<ActorMessage>,
    tasks: Vec<JoinHandle<()>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Daemon {
    /// Starts every task. DND comes from the store.
    ///
    /// # Panics
    ///
    /// Outside a Tokio runtime.
    #[must_use]
    pub fn start(services: Arc<Services>, listener: Listener, config: DaemonConfig) -> Daemon {
        let (actor_sender, actor_receiver) = mpsc::unbounded_channel();
        let (inbound_sender, inbound_receiver) = mpsc::channel(config.intake_capacity);
        let (job_sender, job_receiver) = std::sync::mpsc::channel();
        let worker = {
            let services = Arc::clone(&services);
            let results = actor_sender.clone();
            std::thread::Builder::new().name("agent-notify-effects".to_string()).spawn(move || run_worker(&services, &job_receiver, &results))
        };
        let worker = match worker {
            Ok(handle) => Some(handle),
            Err(error) => {
                tracing::error!(%error, "the effects worker did not start; effects are dropped");
                None
            }
        };
        let stamp = services.facts.stamp.clone();
        let tasks = vec![
            tokio::spawn(async move {
                if let Err(error) = serve(listener, stamp, inbound_sender, config.serve).await {
                    tracing::error!(%error, "the socket server stopped");
                }
            }),
            tokio::spawn(run_intake(Arc::clone(&services), inbound_receiver, actor_sender.clone())),
            tokio::spawn(run_actor(Arc::clone(&services), actor_receiver, job_sender, config)),
            tokio::spawn(run_ticks(actor_sender.clone(), config.tick)),
        ];
        Daemon { actor: actor_sender, tasks, worker }
    }

    /// A cheap handle for callbacks that outlive a borrow of the daemon.
    #[must_use]
    pub fn handle(&self) -> DaemonHandle {
        DaemonHandle { actor: self.actor.clone() }
    }

    /// Passes a UI intent to the core.
    pub fn send_ui(&self, intent: UiIntent) {
        self.handle().send_ui(intent);
    }

    /// The current view model, for the page's bootstrap.
    pub async fn view_model(&self) -> Option<PanelViewModel> {
        self.handle().view_model().await
    }

    /// Stops every task, waits for the worker to finish its current job,
    /// and removes the socket file.
    pub async fn stop(mut self) {
        for task in self.tasks.drain(..) {
            task.abort();
            let _ = task.await;
        }
        drop(self.actor);
        if let Some(worker) = self.worker.take() {
            let _ = tokio::task::spawn_blocking(move || worker.join()).await;
        }
    }
}

/// Sends UI intents and reads the view model. Clone it into callbacks.
#[derive(Clone)]
pub struct DaemonHandle {
    actor: mpsc::UnboundedSender<ActorMessage>,
}

impl DaemonHandle {
    /// Passes a UI intent to the core.
    pub fn send_ui(&self, intent: UiIntent) {
        if self.actor.send(ActorMessage::Input(DaemonInput::Ui(intent))).is_err() {
            tracing::warn!("a UI intent arrived after the actor stopped");
        }
    }

    /// The current view model.
    pub async fn view_model(&self) -> Option<PanelViewModel> {
        let (sender, receiver) = oneshot::channel();
        self.actor.send(ActorMessage::ViewModel(sender)).ok()?;
        receiver.await.ok()
    }
}

async fn run_ticks(actor: mpsc::UnboundedSender<ActorMessage>, every: Duration) {
    let mut interval = tokio::time::interval(every);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        interval.tick().await;
        if actor.send(ActorMessage::Input(DaemonInput::Tick)).is_err() {
            return;
        }
    }
}

async fn run_intake(services: Arc<Services>, mut inbound: mpsc::Receiver<Inbound>, actor: mpsc::UnboundedSender<ActorMessage>) {
    while let Some(message) = inbound.recv().await {
        let next = match message {
            Inbound::Query { query, answer } => ActorMessage::Query { query, answer },
            Inbound::Request(request) => {
                let reading = Arc::clone(&services);
                let copy = request.clone();
                let focus = tokio::task::spawn_blocking(move || gather_focus(&reading, &copy)).await.unwrap_or_else(|error| {
                    tracing::error!(%error, "focus gathering panicked");
                    FocusSnapshot::unknown()
                });
                ActorMessage::Input(DaemonInput::Received { request, focus })
            }
        };
        if actor.send(next).is_err() {
            return;
        }
    }
}

/// Reads the focus facts an agent request needs. Other requests need none.
#[must_use]
pub fn gather_focus(services: &Services, request: &Request) -> FocusSnapshot {
    let Request::Agent { origin, .. } = request else { return FocusSnapshot::unknown() };
    let presence = services.presence.presence();
    let active_pane = match (&services.tmux, &origin.pane) {
        (Some(tmux), Some(pane)) => tmux.active_pane(&pane.server).unwrap_or_else(|error| {
            tracing::debug!(%error, "no active pane");
            None
        }),
        _ => None,
    };
    FocusSnapshot {
        frontmost: services.frontmost.frontmost(),
        active_pane,
        idle_seconds: presence.idle_seconds,
        screen_locked: presence.screen_locked,
        screen_shared: None,
    }
}

struct Shown {
    cards: Vec<agent_notify_core::view::CardView>,
    finished_row: Vec<agent_notify_core::view::FinishedEntry>,
    dnd: bool,
}

impl Shown {
    fn differs(&self, model: &PanelViewModel) -> bool {
        self.cards != model.cards || self.finished_row != model.finished_row || self.dnd != model.dnd
    }
}

async fn run_actor(services: Arc<Services>, mut inbox: mpsc::UnboundedReceiver<ActorMessage>, jobs: std::sync::mpsc::Sender<Job>, config: DaemonConfig) {
    let mut state = DaemonState { dnd: services.dnd_store.load(), ..DaemonState::default() };
    let mut decisions: VecDeque<Decision> = VecDeque::with_capacity(MAX_DECISIONS);
    let mut shown: Option<Shown> = None;
    let mut last_snapshot = (services.clock)();
    while let Some(message) = inbox.recv().await {
        let now = (services.clock)();
        match message {
            ActorMessage::Input(input) => {
                let received = match &input {
                    DaemonInput::Received { request, .. } => Some(request.clone()),
                    _ => None,
                };
                let is_tick = matches!(input, DaemonInput::Tick);
                let effects = update(&mut state, input, now);
                if let Some(entry) = received.as_ref().and_then(|request| decision(request, &effects, now)) {
                    if decisions.len() == MAX_DECISIONS {
                        decisions.pop_front();
                    }
                    decisions.push_back(entry);
                }
                apply(&services, &jobs, effects);
                if is_tick && now.since(last_snapshot) >= config.snapshot_every_ms {
                    last_snapshot = now;
                    for server in known_servers(&state) {
                        send_job(&jobs, Job::Snapshot(server));
                    }
                }
                let model = view(&state, now);
                if shown.as_ref().is_none_or(|previous| previous.differs(&model)) {
                    services.presenter.present(&model);
                    shown = Some(Shown { cards: model.cards.clone(), finished_row: model.finished_row.clone(), dnd: model.dnd });
                }
            }
            ActorMessage::Query { query, answer } => {
                let data = match query {
                    Query::Status => {
                        let log: Vec<Decision> = decisions.iter().cloned().collect();
                        serde_json::to_value(status_report(&state, now, &services.facts, &log))
                    }
                    Query::ViewModel => serde_json::to_value(view(&state, now)),
                    Query::Unknown => Ok(serde_json::Value::Null),
                };
                let _ = answer.send(data.unwrap_or(serde_json::Value::Null));
            }
            ActorMessage::ViewModel(answer) => {
                let _ = answer.send(view(&state, now));
            }
        }
    }
}

fn known_servers(state: &DaemonState) -> BTreeSet<ServerId> {
    state.sessions.values().filter_map(|record| record.pane.as_ref().map(|pane| pane.server.clone())).collect()
}

fn apply(services: &Services, jobs: &std::sync::mpsc::Sender<Job>, effects: Vec<Effect>) {
    for effect in effects {
        match effect {
            Effect::RunFocusPlan { card, target } => send_job(jobs, Job::Focus { card, target }),
            Effect::PlaySound => services.sound.play(),
            Effect::SetPaneState { pane, state } => send_job(jobs, Job::PaneState { pane, state }),
            Effect::PersistDnd(dnd) => send_job(jobs, Job::SaveDnd(dnd)),
            Effect::Suppressed { event_id, reason } => tracing::info!(%event_id, ?reason, "suppressed"),
        }
    }
}

fn send_job(jobs: &std::sync::mpsc::Sender<Job>, job: Job) {
    if jobs.send(job).is_err() {
        tracing::error!("the effects worker is gone; an effect was dropped");
    }
}

fn run_worker(services: &Services, jobs: &std::sync::mpsc::Receiver<Job>, results: &mpsc::UnboundedSender<ActorMessage>) {
    for job in jobs {
        match job {
            Job::Focus { card, target } => {
                let outcome = run_focus(services.tmux.as_ref(), services.raiser.as_ref(), &target);
                let _ = results.send(ActorMessage::Input(DaemonInput::FocusOutcome { card, outcome }));
            }
            Job::PaneState { pane, state } => {
                if let Some(tmux) = &services.tmux
                    && let Err(error) = tmux.set_pane_state(&pane, state)
                {
                    tracing::debug!(%error, pane = pane.pane.as_str(), "pane state not written");
                }
            }
            Job::SaveDnd(dnd) => {
                if let Err(error) = services.dnd_store.save(&dnd) {
                    tracing::error!(%error, "DND not saved; it resets on restart");
                }
            }
            Job::Snapshot(server) => match services.tmux.as_ref().map(|tmux| tmux.pane_snapshot(&server)) {
                Some(Ok(snapshot)) => {
                    let _ = results.send(ActorMessage::Input(DaemonInput::PaneSnapshot(snapshot)));
                }
                Some(Err(error)) => tracing::warn!(%error, "pane snapshot failed; sessions are kept"),
                None => {}
            },
        }
    }
}
```

Notes for the implementer:

- Run `cargo fmt` after pasting; it sorts the imports.
- `EventId` must implement `Display` for `%event_id`; if it does not, use `event_id = event_id.as_str()`.
- `Option::is_none_or` is stable since Rust 1.82. The `if let ... && let ...` chain needs edition 2024, which the workspace sets.
- The actor never blocks: `present`, `play` and `send_job` return at once, and every tmux call runs on the worker or a blocking thread.

- [ ] **Step 8: Run the tests and clippy**

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon && cargo clippy --workspace --all-targets -- -D warnings`
Expected: all pass.

If `a_finished_turn_shows_an_info_card_and_marks_the_pane` shows no card: the core may suppress the card because the user "looks at" the pane. In the test no tmux client is attached, so `active_pane` is `None` and `frontmost` is `Unknown`; read the decision log with a `status` query in the test and fix the cause it names. Do not change the core to make the test pass.

- [ ] **Step 9: Prove the view push filter is load-bearing**

Back up `actor.rs`, then change `previous.differs(&model)` to `false`. Run `cargo test -p agent-notify-daemon --test daemon a_finished_turn; echo "exit $?"`. Expected: a non-zero exit, because no view after the first is pushed. Restore, `touch`, `diff` against the backup, and rerun green.

- [ ] **Step 10: Commit**

```bash
cd ~ && config add tools/agent-notify
config diff --cached --name-only
config commit -m "Add the agent-notify daemon actor, effects worker and DND store" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- tools/agent-notify
```

---

### Task 13: The toast UI

The page is an Elm loop. `model.ts` holds a pure `update(model, msg) -> [model, cmds]`. `runtime.ts` is the only stateful code: it holds the current model, runs commands through `UiServices`, and hands the model to `<toast-stack>`. `main.ts` is the composition root: it builds the Tauri services and starts the runtime. Components render the model and report user actions as messages; they call no service.

**Files:**
- Create: `tools/agent-notify/ui/package.json`, `bun.lock`, `tsconfig.json`, `bunfig.toml`, `index.html`, `build.ts`
- Create: `tools/agent-notify/ui/src/main.ts`, `services.ts`, `tauri-services.ts`, `model.ts`, `runtime.ts`, `time.ts`, `styles.ts`
- Create: `tools/agent-notify/ui/src/components/toast-stack.ts`, `toast-card.ts`, `finished-row.ts`
- Create: `tools/agent-notify/ui/test/setup.ts`, `recording-services.ts`, `model.test.ts`, `runtime.test.ts`, `time.test.ts`, `components.test.ts`

**Interfaces:**
- Consumes: Task 10 bindings (`PanelViewModel`, `CardView`, `FinishedEntry`, `CardId`, `UiMessage`, `UiBootstrap`, `HoverPayload`, `VIEW_MODEL_EVENT`, `HOVER_EVENT`) and fixtures (`empty`, `mixed`, `dndOn`, `revisionTen`, `revisionNine`).
- Produces:
  - `services.ts`: `UiServices { bootstrap(): Promise<UiBootstrap>; send(message: UiMessage): void; onViewModel(listener): Promise<Unsubscribe>; onHover(listener): Promise<Unsubscribe>; now(): number; setTimer(callback, delayMs): Unsubscribe }`, `Unsubscribe = () => void`.
  - `model.ts`: `Model`, `Msg`, `Cmd`, `initialModel(now)`, `update(model, msg)`, `LEAVE_MS = 200`, `TICK_MS = 30_000`, `closeVisible(model)`.
  - `runtime.ts`: `start(root: ToastStack, services: UiServices): Promise<Runtime>`, `Runtime { dispatch(msg: Msg): void; current(): Model; stop(): void }`.
  - `time.ts`: `relativeTime(createdMs: number, nowMs: number): string`.
  - Custom elements `toast-stack` (`ToastStack`, properties `model`, `dispatch`), `toast-card`, `finished-row`.
  - `tauri-services.ts`: `tauriServices(): UiServices`, calling commands `bootstrap` and `ui_message` (Task 16 defines them with these names).

- [ ] **Step 1: Create the project files**

`ui/package.json`:

```json
{
  "name": "agent-notify-ui",
  "private": true,
  "type": "module",
  "scripts": {
    "build": "bun run build.ts",
    "test": "bun test",
    "typecheck": "tsc --noEmit -p ."
  },
  "dependencies": {
    "@tauri-apps/api": "2.12.4",
    "lit": "3.3.3"
  },
  "devDependencies": {
    "@happy-dom/global-registrator": "21.0.1",
    "@types/bun": "1.4.3",
    "typescript": "7.0.2"
  }
}
```

`ui/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "types": ["bun"],
    "strict": true,
    "noUncheckedIndexedAccess": true,
    "exactOptionalPropertyTypes": true,
    "noFallthroughCasesInSwitch": true,
    "verbatimModuleSyntax": true,
    "isolatedModules": true,
    "skipLibCheck": true,
    "noEmit": true
  },
  "include": ["src", "test", "build.ts"]
}
```

`ui/bunfig.toml`:

```toml
[test]
preload = ["./test/setup.ts"]
```

`ui/test/setup.ts`:

```ts
import { GlobalRegistrator } from "@happy-dom/global-registrator";

GlobalRegistrator.register();
```

`ui/index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width" />
    <title>agent-notify</title>
    <script type="module" src="./src/main.ts"></script>
  </head>
  <body></body>
</html>
```

`ui/build.ts`:

```ts
const result = await Bun.build({ entrypoints: ["./index.html"], outdir: "./dist", minify: true });
if (!result.success) {
  for (const log of result.logs) {
    console.error(log);
  }
  process.exit(1);
}
```

Run: `cd ~/tools/agent-notify/ui && bun install && bun --version`
Expected: `bun.lock` appears, and no install script runs (Bun runs none without `trustedDependencies`).

- [ ] **Step 2: Write the failing pure tests**

`ui/test/time.test.ts`:

```ts
import { expect, test } from "bun:test";
import { relativeTime } from "../src/time";

const base = 1_760_500_000_000;

test("recent cards read as now", () => {
  expect(relativeTime(base, base)).toBe("now");
  expect(relativeTime(base, base + 44_000)).toBe("now");
});

test("minutes, hours and days", () => {
  expect(relativeTime(base, base + 45_000)).toBe("1m");
  expect(relativeTime(base, base + 59 * 60_000)).toBe("59m");
  expect(relativeTime(base, base + 60 * 60_000)).toBe("1h");
  expect(relativeTime(base, base + 47 * 3_600_000)).toBe("1d");
});

test("a clock behind the card reads as now", () => {
  expect(relativeTime(base, base - 5_000)).toBe("now");
});
```

`ui/test/model.test.ts`:

```ts
import { expect, test } from "bun:test";
import { LEAVE_MS, closeVisible, initialModel, update, type Model, type Msg } from "../src/model";
import { mixed, revisionNine, revisionTen } from "./fixtures/view-models";

function run(model: Model, ...messages: Msg[]): [Model, unknown[]] {
  return messages.reduce<[Model, unknown[]]>(
    ([current, commands], message) => {
      const [next, more] = update(current, message);
      return [next, [...commands, ...more]];
    },
    [model, []],
  );
}

const booted = (nativeHover: boolean): Model =>
  update(initialModel(0), { type: "booted", bootstrap: { view_model: revisionTen, native_hover: nativeHover } })[0];

test("an_older_revision_is_dropped", () => {
  const [model] = run(booted(true), { type: "view_model", view: revisionNine });
  expect(model.view).toEqual(revisionTen);
});

test("a newer revision replaces the view", () => {
  const [model] = run(booted(true), { type: "view_model", view: { ...mixed, revision: 11 } });
  expect(model.view?.revision).toBe(11);
});

test("a push before the bootstrap answer is kept when it is newer", () => {
  const [early] = update(initialModel(0), { type: "view_model", view: { ...mixed, revision: 12 } });
  const [model] = update(early, { type: "booted", bootstrap: { view_model: revisionTen, native_hover: true } });
  expect(model.view?.revision).toBe(12);
  expect(model.nativeHover).toBe(true);
});

test("clicks and closes become messages for the shell", () => {
  const cardId = mixed.cards[0]?.card_id ?? "";
  const [, commands] = run(booted(true), { type: "clicked", cardId }, { type: "closed", cardId });
  expect(commands).toEqual([
    { type: "send", message: { type: "clicked", card_id: cardId } },
    { type: "send", message: { type: "closed", card_id: cardId } },
  ]);
});

test("native hover shows the close buttons and ignores DOM pointer events", () => {
  const [hovered, commands] = run(booted(true), { type: "native_hover", hovered: true }, { type: "pointer", inside: false });
  expect(closeVisible(hovered)).toBe(true);
  expect(commands).toEqual([]);
});

test("DOM hover reports enter at once and leave after a delay", () => {
  const [entered, enterCommands] = run(booted(false), { type: "pointer", inside: true });
  expect(enterCommands).toEqual([{ type: "cancel_leave" }, { type: "send", message: { type: "hover", hovered: true } }]);
  const [leaving, leaveCommands] = run(entered, { type: "pointer", inside: false });
  expect(leaveCommands).toEqual([{ type: "schedule_leave", delayMs: LEAVE_MS }]);
  expect(leaving.hovered).toBe(true);
  const [left, leftCommands] = run(leaving, { type: "left" });
  expect(left.hovered).toBe(false);
  expect(leftCommands).toEqual([{ type: "send", message: { type: "hover", hovered: false } }]);
});

test("re-entering before the delay cancels the leave", () => {
  const [model, commands] = run(booted(false), { type: "pointer", inside: true }, { type: "pointer", inside: false }, { type: "pointer", inside: true });
  expect(model.hovered).toBe(true);
  expect(commands.at(-1)).toEqual({ type: "cancel_leave" });
});

test("layout is reported once per height", () => {
  const [, commands] = run(booted(true), { type: "layout", heightPx: 212.4 }, { type: "layout", heightPx: 212 }, { type: "layout", heightPx: 300 });
  expect(commands).toEqual([
    { type: "send", message: { type: "layout", content_height_px: 212 } },
    { type: "send", message: { type: "layout", content_height_px: 300 } },
  ]);
});

test("ticks move the clock", () => {
  const [model] = run(booted(true), { type: "tick", now: 99 });
  expect(model.now).toBe(99);
});
```

- [ ] **Step 3: Run them to see them fail**

Run: `cd ~/tools/agent-notify/ui && bun test`
Expected: failures, `Cannot find module '../src/time'`.

- [ ] **Step 4: Implement `time.ts`, `services.ts` and `model.ts`**

`ui/src/time.ts`:

```ts
const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

export function relativeTime(createdMs: number, nowMs: number): string {
  const elapsed = Math.max(0, nowMs - createdMs);
  if (elapsed < 45_000) return "now";
  if (elapsed < HOUR) return `${Math.max(1, Math.round(elapsed / MINUTE))}m`;
  if (elapsed < DAY) return `${Math.floor(elapsed / HOUR)}h`;
  return `${Math.floor(elapsed / DAY)}d`;
}
```

`ui/src/services.ts`:

```ts
import type { PanelViewModel } from "./bindings/PanelViewModel";
import type { UiBootstrap } from "./bindings/UiBootstrap";
import type { UiMessage } from "./bindings/UiMessage";

export type Unsubscribe = () => void;

export interface UiServices {
  bootstrap(): Promise<UiBootstrap>;
  send(message: UiMessage): void;
  onViewModel(listener: (view: PanelViewModel) => void): Promise<Unsubscribe>;
  onHover(listener: (hovered: boolean) => void): Promise<Unsubscribe>;
  now(): number;
  setTimer(callback: () => void, delayMs: number): Unsubscribe;
}
```

`ui/src/model.ts`:

```ts
import type { CardId } from "./bindings/CardId";
import type { PanelViewModel } from "./bindings/PanelViewModel";
import type { UiBootstrap } from "./bindings/UiBootstrap";
import type { UiMessage } from "./bindings/UiMessage";

export const LEAVE_MS = 200;
export const TICK_MS = 30_000;

export type Model = {
  readonly view: PanelViewModel | null;
  readonly nativeHover: boolean;
  readonly hovered: boolean;
  readonly now: number;
  readonly reportedHeight: number | null;
};

export type Msg =
  | { readonly type: "booted"; readonly bootstrap: UiBootstrap }
  | { readonly type: "view_model"; readonly view: PanelViewModel }
  | { readonly type: "native_hover"; readonly hovered: boolean }
  | { readonly type: "pointer"; readonly inside: boolean }
  | { readonly type: "left" }
  | { readonly type: "clicked"; readonly cardId: CardId }
  | { readonly type: "closed"; readonly cardId: CardId }
  | { readonly type: "tick"; readonly now: number }
  | { readonly type: "layout"; readonly heightPx: number };

export type Cmd =
  | { readonly type: "send"; readonly message: UiMessage }
  | { readonly type: "schedule_leave"; readonly delayMs: number }
  | { readonly type: "cancel_leave" };

export function initialModel(now: number): Model {
  return { view: null, nativeHover: false, hovered: false, now, reportedHeight: null };
}

export function closeVisible(model: Model): boolean {
  return model.nativeHover && model.hovered;
}

function newer(current: PanelViewModel | null, incoming: PanelViewModel): PanelViewModel {
  return current !== null && incoming.revision < current.revision ? current : incoming;
}

const send = (message: UiMessage): Cmd => ({ type: "send", message });

export function update(model: Model, msg: Msg): readonly [Model, readonly Cmd[]] {
  switch (msg.type) {
    case "booted":
      return [{ ...model, view: newer(model.view, msg.bootstrap.view_model), nativeHover: msg.bootstrap.native_hover }, []];
    case "view_model":
      return [{ ...model, view: newer(model.view, msg.view) }, []];
    case "native_hover":
      return model.nativeHover ? [{ ...model, hovered: msg.hovered }, []] : [model, []];
    case "pointer":
      if (model.nativeHover) return [model, []];
      if (!msg.inside) return [model, [{ type: "schedule_leave", delayMs: LEAVE_MS }]];
      return model.hovered
        ? [model, [{ type: "cancel_leave" }]]
        : [{ ...model, hovered: true }, [{ type: "cancel_leave" }, send({ type: "hover", hovered: true })]];
    case "left":
      return model.hovered && !model.nativeHover ? [{ ...model, hovered: false }, [send({ type: "hover", hovered: false })]] : [model, []];
    case "clicked":
      return [model, [send({ type: "clicked", card_id: msg.cardId })]];
    case "closed":
      return [model, [send({ type: "closed", card_id: msg.cardId })]];
    case "tick":
      return [{ ...model, now: msg.now }, []];
    case "layout": {
      const height = Math.max(0, Math.round(msg.heightPx));
      return height === model.reportedHeight
        ? [model, []]
        : [{ ...model, reportedHeight: height }, [send({ type: "layout", content_height_px: height })]];
    }
  }
}
```

Run: `cd ~/tools/agent-notify/ui && bun test test/time.test.ts test/model.test.ts && bun run typecheck`
Expected: pass. If `tsc` reports the `UiMessage` variant shapes differently from what `send(...)` builds here, the generated binding is the contract: change the TypeScript, not the binding.

- [ ] **Step 5: Write the failing component and runtime tests**

`ui/test/recording-services.ts`:

```ts
import type { PanelViewModel } from "../src/bindings/PanelViewModel";
import type { UiBootstrap } from "../src/bindings/UiBootstrap";
import type { UiMessage } from "../src/bindings/UiMessage";
import type { UiServices, Unsubscribe } from "../src/services";

export type RecordingServices = UiServices & {
  readonly sent: UiMessage[];
  pushViewModel(view: PanelViewModel): void;
  pushHover(hovered: boolean): void;
  advance(milliseconds: number): void;
};

export function recordingServices(bootstrap: UiBootstrap, startMs = 1_760_500_000_000): RecordingServices {
  const sent: UiMessage[] = [];
  const viewListeners: Array<(view: PanelViewModel) => void> = [];
  const hoverListeners: Array<(hovered: boolean) => void> = [];
  const timers: Array<{ at: number; callback: () => void; live: boolean }> = [];
  let clock = startMs;
  return {
    sent,
    bootstrap: async () => bootstrap,
    send: (message) => {
      sent.push(message);
    },
    onViewModel: async (listener) => {
      viewListeners.push(listener);
      return () => {};
    },
    onHover: async (listener) => {
      hoverListeners.push(listener);
      return () => {};
    },
    now: () => clock,
    setTimer: (callback, delayMs): Unsubscribe => {
      const timer = { at: clock + delayMs, callback, live: true };
      timers.push(timer);
      return () => {
        timer.live = false;
      };
    },
    pushViewModel: (view) => viewListeners.forEach((listener) => listener(view)),
    pushHover: (hovered) => hoverListeners.forEach((listener) => listener(hovered)),
    advance: (milliseconds) => {
      clock += milliseconds;
      for (const timer of timers.filter((entry) => entry.live && entry.at <= clock)) {
        timer.live = false;
        timer.callback();
      }
    },
  };
}
```

The recorder mutates its own arrays; it is test scaffolding, not shared state between tests (each test builds its own).

`ui/test/components.test.ts`:

```ts
import { expect, test } from "bun:test";
import "../src/components/toast-stack";
import type { ToastStack } from "../src/components/toast-stack";
import { initialModel, update, type Model, type Msg } from "../src/model";
import { dndOn, mixed } from "./fixtures/view-models";

async function mount(model: Model): Promise<{ stack: ToastStack; messages: Msg[] }> {
  const messages: Msg[] = [];
  const stack = document.createElement("toast-stack") as ToastStack;
  stack.dispatch = (message) => messages.push(message);
  stack.model = model;
  document.body.replaceChildren(stack);
  await stack.updateComplete;
  for (const child of stack.renderRoot.querySelectorAll("toast-card, finished-row")) {
    await (child as HTMLElement & { updateComplete: Promise<unknown> }).updateComplete;
  }
  return { stack, messages };
}

const withView = (nativeHover: boolean, hovered = false): Model => {
  const [booted] = update(initialModel(1_760_500_400_000), { type: "booted", bootstrap: { view_model: mixed, native_hover: nativeHover } });
  return { ...booted, hovered };
};

function cards(stack: ToastStack): HTMLElement[] {
  return [...stack.renderRoot.querySelectorAll<HTMLElement>("toast-card")];
}

function inner(element: Element): ShadowRoot {
  const root = element.shadowRoot;
  if (root === null) throw new Error(`${element.tagName} has no shadow root`);
  return root;
}

test("every card renders with its kind and location", async () => {
  const { stack } = await mount(withView(true));
  const rendered = cards(stack);
  expect(rendered.length).toBe(3);
  expect(inner(rendered[0] as Element).querySelector(".card")?.classList.contains("attention")).toBe(true);
  expect(inner(rendered[1] as Element).textContent).toContain("2 · notes/main");
});

test("card text is text, never markup", async () => {
  const { stack } = await mount(withView(true));
  const info = inner(cards(stack)[1] as Element);
  expect(info.textContent).toContain("<b>not bold</b>");
  expect(info.querySelector("b")).toBeNull();
});

test("a note replaces nothing and shows under the body", async () => {
  const { stack } = await mount(withView(true));
  expect(inner(cards(stack)[2] as Element).querySelector(".note")?.textContent).toBe("Session ended");
});

test("clicking a card sends its id; the close button sends close only", async () => {
  const { stack, messages } = await mount(withView(true, true));
  const first = inner(cards(stack)[0] as Element);
  (first.querySelector(".card") as HTMLElement).click();
  (first.querySelector("button.close") as HTMLElement).click();
  const cardId = mixed.cards[0]?.card_id;
  expect(messages).toEqual([
    { type: "clicked", cardId },
    { type: "closed", cardId },
  ]);
});

test("close buttons are visible only while native hover is on", async () => {
  const { stack } = await mount(withView(true, true));
  expect(cards(stack).every((card) => card.hasAttribute("close-visible"))).toBe(true);
  const { stack: idle } = await mount(withView(true, false));
  expect(cards(idle).some((card) => card.hasAttribute("close-visible"))).toBe(false);
});

test("the finished row lists folded cards as buttons", async () => {
  const { stack, messages } = await mount(withView(true));
  const row = stack.renderRoot.querySelector("finished-row");
  const button = row === null ? null : inner(row).querySelector("button");
  expect(button?.textContent).toBe("6");
  button?.click();
  expect(messages).toEqual([{ type: "clicked", cardId: mixed.finished_row[0]?.card_id }]);
});

test("do-not-disturb and an empty view render nothing", async () => {
  const [model] = update(initialModel(0), { type: "booted", bootstrap: { view_model: dndOn, native_hover: true } });
  const { stack } = await mount(model);
  expect(cards(stack).length).toBe(0);
  expect(stack.renderRoot.querySelector("finished-row")).toBeNull();
});
```

`ui/test/runtime.test.ts`:

```ts
import { expect, test } from "bun:test";
import "../src/components/toast-stack";
import type { ToastStack } from "../src/components/toast-stack";
import { LEAVE_MS } from "../src/model";
import { start } from "../src/runtime";
import { revisionNine, revisionTen } from "./fixtures/view-models";
import { recordingServices } from "./recording-services";

async function boot(nativeHover: boolean) {
  const services = recordingServices({ view_model: revisionTen, native_hover: nativeHover });
  const root = document.createElement("toast-stack") as ToastStack;
  document.body.replaceChildren(root);
  const runtime = await start(root, services);
  return { services, root, runtime };
}

test("the runtime renders the bootstrap view", async () => {
  const { root, runtime } = await boot(true);
  await root.updateComplete;
  expect(root.model?.view?.revision).toBe(10);
  runtime.stop();
});

test("a stale push does not replace the view", async () => {
  const { services, runtime } = await boot(true);
  services.pushViewModel(revisionNine);
  expect(runtime.current().view?.revision).toBe(10);
  runtime.stop();
});

test("native hover from the shell reaches the model", async () => {
  const { services, runtime } = await boot(true);
  services.pushHover(true);
  expect(runtime.current().hovered).toBe(true);
  runtime.stop();
});

test("a DOM leave is reported after the delay, through the timer service", async () => {
  const { services, runtime } = await boot(false);
  runtime.dispatch({ type: "pointer", inside: true });
  runtime.dispatch({ type: "pointer", inside: false });
  expect(services.sent).toEqual([{ type: "hover", hovered: true }]);
  services.advance(LEAVE_MS);
  expect(services.sent).toEqual([
    { type: "hover", hovered: true },
    { type: "hover", hovered: false },
  ]);
  runtime.stop();
});
```

- [ ] **Step 6: Run them to see them fail**

Run: `cd ~/tools/agent-notify/ui && bun test`
Expected: failures, `Cannot find module '../src/components/toast-stack'`.

- [ ] **Step 7: Implement the styles and components**

`ui/src/styles.ts`:

```ts
import { css } from "lit";

export const tokens = css`
  :host {
    --card-width: 380px;
    --radius: 12px;
    --info-bg: rgba(30, 30, 32, 0.92);
    --info-border: rgba(255, 255, 255, 0.12);
    --attention-bg: rgba(92, 58, 18, 0.95);
    --attention-border: #e0a030;
    --text: #f2f2f2;
    --muted: rgba(242, 242, 242, 0.65);
    --close-bg: rgba(80, 80, 84, 0.95);
    font: 13px/1.35 -apple-system, BlinkMacSystemFont, "Inter", system-ui, sans-serif;
    color: var(--text);
  }
  @media (prefers-color-scheme: light) {
    :host {
      --info-bg: rgba(250, 250, 250, 0.96);
      --info-border: rgba(0, 0, 0, 0.12);
      --attention-bg: rgba(255, 243, 224, 0.97);
      --text: #1d1d1f;
      --muted: rgba(29, 29, 31, 0.6);
      --close-bg: rgba(225, 225, 228, 0.98);
    }
  }
`;
```

`ui/src/components/toast-card.ts`:

```ts
import { LitElement, css, html, nothing } from "lit";
import type { CardId } from "../bindings/CardId";
import type { CardView } from "../bindings/CardView";
import { tokens } from "../styles";
import { relativeTime } from "../time";

export class ToastCard extends LitElement {
  static properties = {
    card: { attribute: false },
    now: { type: Number },
    closeVisible: { type: Boolean, reflect: true, attribute: "close-visible" },
    nativeHover: { type: Boolean, reflect: true, attribute: "native-hover" },
    onActivate: { attribute: false },
    onClose: { attribute: false },
  };

  static styles = [
    tokens,
    css`
      :host {
        display: block;
        position: relative;
        width: var(--card-width);
      }
      .card {
        box-sizing: border-box;
        padding: 10px 14px 11px;
        border-radius: var(--radius);
        background: var(--info-bg);
        border: 1px solid var(--info-border);
        box-shadow: 0 6px 18px rgba(0, 0, 0, 0.28);
        cursor: default;
        transition: filter 120ms ease-out;
      }
      .card.attention {
        background: var(--attention-bg);
        border-color: var(--attention-border);
      }
      :host(:not([native-hover])) .card:hover {
        filter: brightness(1.08);
      }
      .top {
        display: flex;
        justify-content: space-between;
        gap: 8px;
        color: var(--muted);
        font-size: 11.5px;
      }
      .location {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }
      .title {
        font-weight: 600;
        margin-top: 2px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }
      .body {
        margin-top: 2px;
        display: -webkit-box;
        -webkit-line-clamp: 2;
        -webkit-box-orient: vertical;
        overflow: hidden;
        overflow-wrap: anywhere;
      }
      .note {
        margin-top: 4px;
        font-style: italic;
        color: var(--muted);
      }
      button.close {
        position: absolute;
        top: -6px;
        left: -6px;
        width: 18px;
        height: 18px;
        padding: 0;
        border-radius: 50%;
        border: 1px solid var(--info-border);
        background: var(--close-bg);
        color: var(--text);
        font-size: 12px;
        line-height: 16px;
        opacity: 0;
        transition: opacity 120ms ease-out;
      }
      :host([close-visible]) button.close,
      :host(:not([native-hover]):hover) button.close {
        opacity: 1;
      }
      @media (prefers-reduced-motion: reduce) {
        .card,
        button.close {
          transition: none;
        }
      }
    `,
  ];

  declare card: CardView | undefined;
  declare now: number;
  declare closeVisible: boolean;
  declare nativeHover: boolean;
  declare onActivate: (cardId: CardId) => void;
  declare onClose: (cardId: CardId) => void;

  constructor() {
    super();
    this.card = undefined;
    this.now = 0;
    this.closeVisible = false;
    this.nativeHover = false;
    this.onActivate = () => {};
    this.onClose = () => {};
  }

  override render() {
    const card = this.card;
    if (card === undefined) return nothing;
    const activate = () => this.onActivate(card.card_id);
    const close = (event: Event) => {
      event.stopPropagation();
      this.onClose(card.card_id);
    };
    return html`
      <article class="card ${card.kind}" aria-label=${card.location} @click=${activate}>
        <div class="top">
          <span class="location">${card.location}</span>
          <span class="time">${relativeTime(card.created_ms, this.now)}</span>
        </div>
        ${card.title === null ? nothing : html`<div class="title">${card.title}</div>`}
        ${card.body === null ? nothing : html`<div class="body">${card.body}</div>`}
        ${card.note === null ? nothing : html`<div class="note">${card.note}</div>`}
      </article>
      <button class="close" aria-label="Dismiss" @click=${close}>×</button>
    `;
  }
}

customElements.define("toast-card", ToastCard);
```

`ui/src/components/finished-row.ts`:

```ts
import { LitElement, css, html } from "lit";
import type { CardId } from "../bindings/CardId";
import type { FinishedEntry } from "../bindings/FinishedEntry";
import { tokens } from "../styles";

export class FinishedRow extends LitElement {
  static properties = {
    entries: { attribute: false },
    onActivate: { attribute: false },
  };

  static styles = [
    tokens,
    css`
      :host {
        display: block;
        width: var(--card-width);
      }
      .row {
        box-sizing: border-box;
        padding: 6px 14px;
        border-radius: var(--radius);
        background: var(--info-bg);
        border: 1px solid var(--info-border);
        color: var(--muted);
      }
      button {
        font: inherit;
        color: var(--text);
        background: none;
        border: none;
        padding: 0 3px;
        text-decoration: underline;
      }
    `,
  ];

  declare entries: readonly FinishedEntry[];
  declare onActivate: (cardId: CardId) => void;

  constructor() {
    super();
    this.entries = [];
    this.onActivate = () => {};
  }

  override render() {
    return html`<div class="row">
      Finished:
      ${this.entries.map((entry) => html`<button @click=${() => this.onActivate(entry.card_id)}>${entry.label}</button>`)}
    </div>`;
  }
}

customElements.define("finished-row", FinishedRow);
```

`ui/src/components/toast-stack.ts`:

```ts
import { LitElement, css, html, nothing } from "lit";
import { closeVisible, type Model, type Msg } from "../model";
import "./finished-row";
import "./toast-card";

export class ToastStack extends LitElement {
  static properties = {
    model: { attribute: false },
    dispatch: { attribute: false },
  };

  static styles = css`
    :host {
      display: flex;
      flex-direction: column;
      gap: 8px;
      padding: 8px 12px 12px;
      background: transparent;
    }
  `;

  declare model: Model | null;
  declare dispatch: (message: Msg) => void;

  constructor() {
    super();
    this.model = null;
    this.dispatch = () => {};
    this.addEventListener("pointerenter", () => this.dispatch({ type: "pointer", inside: true }));
    this.addEventListener("pointerleave", () => this.dispatch({ type: "pointer", inside: false }));
  }

  override render() {
    const model = this.model;
    const view = model?.view;
    if (model === null || view === null || view === undefined) return nothing;
    const activate = (cardId: string) => this.dispatch({ type: "clicked", cardId });
    const close = (cardId: string) => this.dispatch({ type: "closed", cardId });
    return html`
      ${view.cards.map(
        (card) => html`<toast-card
          .card=${card}
          .now=${model.now}
          .onActivate=${activate}
          .onClose=${close}
          ?close-visible=${closeVisible(model)}
          ?native-hover=${model.nativeHover}
        ></toast-card>`,
      )}
      ${view.finished_row.length === 0 ? nothing : html`<finished-row .entries=${view.finished_row} .onActivate=${activate}></finished-row>`}
    `;
  }

  override updated() {
    this.dispatch({ type: "layout", heightPx: this.getBoundingClientRect().height });
  }
}

customElements.define("toast-stack", ToastStack);
```

`?close-visible` and `?native-hover` set the attributes the card reflects, so the card's `:host([...])` selectors see them.

- [ ] **Step 8: Implement the runtime, the Tauri services and `main.ts`**

`ui/src/runtime.ts`:

```ts
import type { ToastStack } from "./components/toast-stack";
import { TICK_MS, initialModel, update, type Cmd, type Model, type Msg } from "./model";
import type { UiServices, Unsubscribe } from "./services";

export type Runtime = {
  dispatch(msg: Msg): void;
  current(): Model;
  stop(): void;
};

export async function start(root: ToastStack, services: UiServices): Promise<Runtime> {
  let model = initialModel(services.now());
  let cancelLeave: Unsubscribe | null = null;
  const subscriptions: Unsubscribe[] = [];

  const run = (cmd: Cmd): void => {
    switch (cmd.type) {
      case "send":
        services.send(cmd.message);
        return;
      case "schedule_leave":
        cancelLeave?.();
        cancelLeave = services.setTimer(() => {
          cancelLeave = null;
          dispatch({ type: "left" });
        }, cmd.delayMs);
        return;
      case "cancel_leave":
        cancelLeave?.();
        cancelLeave = null;
        return;
    }
  };

  const dispatch = (msg: Msg): void => {
    const [next, cmds] = update(model, msg);
    model = next;
    root.model = model;
    cmds.forEach(run);
  };

  const tick = (): void => {
    dispatch({ type: "tick", now: services.now() });
    subscriptions.push(services.setTimer(tick, TICK_MS));
  };

  root.dispatch = dispatch;
  root.model = model;
  subscriptions.push(await services.onViewModel((view) => dispatch({ type: "view_model", view })));
  subscriptions.push(await services.onHover((hovered) => dispatch({ type: "native_hover", hovered })));
  dispatch({ type: "booted", bootstrap: await services.bootstrap() });
  subscriptions.push(services.setTimer(tick, TICK_MS));

  return {
    dispatch,
    current: () => model,
    stop: () => {
      cancelLeave?.();
      subscriptions.forEach((unsubscribe) => unsubscribe());
    },
  };
}
```

The listeners are registered before `bootstrap()`, so a push that lands between the two is not lost; the revision guard in `update` sorts out the order.

`ui/src/tauri-services.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { HOVER_EVENT, VIEW_MODEL_EVENT } from "./bindings/events";
import type { HoverPayload } from "./bindings/HoverPayload";
import type { PanelViewModel } from "./bindings/PanelViewModel";
import type { UiBootstrap } from "./bindings/UiBootstrap";
import type { UiServices } from "./services";

export function tauriServices(): UiServices {
  return {
    bootstrap: () => invoke<UiBootstrap>("bootstrap"),
    send: (message) => {
      invoke("ui_message", { message }).catch((error: unknown) => console.error("ui_message failed", error));
    },
    onViewModel: (listener) => listen<PanelViewModel>(VIEW_MODEL_EVENT, (event) => listener(event.payload)),
    onHover: (listener) => listen<HoverPayload>(HOVER_EVENT, (event) => listener(event.payload.hovered)),
    now: () => Date.now(),
    setTimer: (callback, delayMs) => {
      const timer = setTimeout(callback, delayMs);
      return () => clearTimeout(timer);
    },
  };
}
```

`ui/src/main.ts`:

```ts
import "./components/toast-stack";
import type { ToastStack } from "./components/toast-stack";
import { start } from "./runtime";
import { tauriServices } from "./tauri-services";

const services = tauriServices();

function report(message: string, stack: string | null): void {
  services.send({ type: "error", message, stack });
}

window.addEventListener("error", (event) => report(String(event.message), event.error instanceof Error ? (event.error.stack ?? null) : null));
window.addEventListener("unhandledrejection", (event) => {
  const reason: unknown = event.reason;
  report(reason instanceof Error ? reason.message : String(reason), reason instanceof Error ? (reason.stack ?? null) : null);
});

const root = document.createElement("toast-stack") as ToastStack;
document.body.append(root);
start(root, services).catch((error: unknown) => report(`start failed: ${String(error)}`, null));
```

- [ ] **Step 9: Run the tests, the type check and the build**

Run: `cd ~/tools/agent-notify/ui && bun test && bun run typecheck && bun run build && ls dist`
Expected: all tests pass, `tsc` prints nothing, and `dist/` holds `index.html` plus one script. `grep -c "innerHTML\|unsafeHTML" src -r` prints `0`.

If happy-dom does not run Lit's first render inside `updateComplete` (an empty `renderRoot`), add `await new Promise((resolve) => setTimeout(resolve, 0))` after `updateComplete` in `mount`, and say so in the commit message.

- [ ] **Step 10: Prove the revision guard is load-bearing**

Back up `model.ts`, then make `newer` always return `incoming`. Run `bun test test/model.test.ts; echo "exit $?"`. Expected: non-zero, with `an_older_revision_is_dropped` failing. Restore, `diff` against the backup, and rerun green.

- [ ] **Step 11: Commit**

```bash
cd ~ && config add tools/agent-notify/ui
config diff --cached --name-only
config commit -m "Add the agent-notify toast UI" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- tools/agent-notify/ui
```

The list must not contain `node_modules/` or `dist/`.

---

### Task 14: The app crate and the macOS services

**Files:**
- Create: `tools/agent-notify/app/Cargo.toml`, `build.rs`, `tauri.conf.json`, `capabilities/default.json`, `icons/icon.png`
- Create: `tools/agent-notify/app/src/main.rs`, `src/geometry.rs`, `src/logging.rs`, `src/macos/mod.rs`, `src/macos/services.rs`
- Create: `tools/agent-notify/scripts/make-icon.ts`
- Modify: `tools/agent-notify/Cargo.toml` (member `app`)

**Interfaces:**
- Consumes: Task 8 `Aerospace`, `frontmost_from_bundle`, `IoregPresence`, `TERMINAL_BUNDLE_ID`, service traits; plan 1 `log_directory`.
- Produces (crate `agent-notifyd`, binary):
  - `geometry::{Rect { x, y, width, height }, PANEL_WIDTH = 404.0, MARGIN = 16.0, panel_frame(visible: Rect, content_height_px: u32) -> Rect}`. AppKit coordinates: origin at the bottom left.
  - `logging::{level_from(Option<&str>) -> tracing::level_filters::LevelFilter, init(&Path, LevelFilter) -> io::Result<WorkerGuard>, install_panic_hook()}`.
  - `macos::services::{on_main<T>(&AppHandle, Duration, work) -> Option<T>, MacFrontmost, AfplaySound, MacRaiser}`.
  - `STAMP: &str`.

- [ ] **Step 1: Create the crate files**

Add `"app"` to `members` in `tools/agent-notify/Cargo.toml`: `members = ["daemon", "app"]`.

`tools/agent-notify/app/Cargo.toml`:

```toml
[package]
name = "agent-notifyd"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
publish.workspace = true
description = "The agent-notify daemon: toast panel, socket and tmux focus."

[lints]
workspace = true

[dependencies]
agent-notify-core = { path = "../../../crates/agent-notify-core", features = ["ts"] }
agent-notify-daemon = { path = "../daemon" }
serde = { version = "=1.0.229", features = ["derive"] }
serde_json = "=1.0.154"
tauri = { version = "=2.12.4", features = ["macos-private-api"] }
tokio = { version = "=1.53.3", features = ["rt-multi-thread", "sync", "time"] }
tracing = "=0.1.44"
tracing-appender = "=0.2.5"
tracing-subscriber = { version = "=0.3.23", default-features = false, features = ["fmt", "std"] }

[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "=0.6.5"
objc2-app-kit = { version = "=0.3.2", features = ["NSApplication", "NSPanel", "NSResponder", "NSRunningApplication", "NSScreen", "NSWindow", "NSWorkspace"] }
objc2-foundation = "=0.3.2"
tauri-nspanel = "=2.1.1"

[build-dependencies]
tauri-build = { version = "=2.7.3", features = [] }
```

`app/build.rs`:

```rust
fn main() {
    tauri_build::build();
}
```

`app/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "agent-notifyd",
  "identifier": "dev.dotfiles.agent-notifyd",
  "build": { "frontendDist": "../ui/dist" },
  "app": {
    "windows": [],
    "macOSPrivateApi": true,
    "security": { "csp": "default-src 'self' ipc: http://ipc.localhost; style-src 'self' 'unsafe-inline'" }
  },
  "bundle": { "active": false, "icon": ["icons/icon.png"] }
}
```

`app/capabilities/default.json`:

```json
{ "identifier": "default", "windows": ["toast"], "permissions": ["core:event:default"] }
```

App-defined commands (`bootstrap`, `ui_message`) need no permission entry; `core:event:default` lets the page listen to the two events.

`tools/agent-notify/scripts/make-icon.ts` writes a 128x128 amber disc. It was run once on 2026-10-15 and produced a valid RGBA PNG:

```ts
import { deflateSync } from "node:zlib";

const size = 128;
const raw = new Uint8Array(size * (size * 4 + 1));
for (let row = 0; row < size; row++) {
  const start = row * (size * 4 + 1);
  for (let column = 0; column < size; column++) {
    const inside = (column - 63.5) ** 2 + (row - 63.5) ** 2 <= 60 ** 2;
    raw.set(inside ? [0xe0, 0xa0, 0x30, 0xff] : [0, 0, 0, 0], start + 1 + column * 4);
  }
}
const chunk = (type: string, data: Uint8Array): Uint8Array => {
  const body = new Uint8Array([...new TextEncoder().encode(type), ...data]);
  const out = new DataView(new ArrayBuffer(12 + data.length));
  out.setUint32(0, data.length);
  new Uint8Array(out.buffer).set(body, 4);
  out.setUint32(8 + data.length, Bun.hash.crc32(body));
  return new Uint8Array(out.buffer);
};
const header = new DataView(new ArrayBuffer(13));
header.setUint32(0, size);
header.setUint32(4, size);
new Uint8Array(header.buffer).set([8, 6, 0, 0, 0], 8);
const png = [
  new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]),
  chunk("IHDR", new Uint8Array(header.buffer)),
  chunk("IDAT", deflateSync(raw)),
  chunk("IEND", new Uint8Array()),
];
await Bun.write(process.argv[2] ?? "icon.png", new Blob(png));
```

Run: `cd ~/tools/agent-notify && mkdir -p app/icons && bun scripts/make-icon.ts app/icons/icon.png && file app/icons/icon.png`
Expected: `PNG image data, 128 x 128, 8-bit/color RGBA`.

- [ ] **Step 2: Write the failing pure tests**

`app/src/geometry.rs`, tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const DISPLAY: Rect = Rect { x: 0.0, y: 0.0, width: 2560.0, height: 1415.0 };

    #[test]
    fn the_panel_sits_top_right_inside_the_margin() {
        let frame = panel_frame(DISPLAY, 212);
        assert_eq!(frame, Rect { x: 2560.0 - PANEL_WIDTH - MARGIN, y: 1415.0 - 212.0 - MARGIN, width: PANEL_WIDTH, height: 212.0 });
    }

    #[test]
    fn a_second_display_offsets_the_frame() {
        let left = Rect { x: -1920.0, y: 200.0, width: 1920.0, height: 1055.0 };
        let frame = panel_frame(left, 100);
        assert_eq!(frame.x, -1920.0 + 1920.0 - PANEL_WIDTH - MARGIN);
        assert_eq!(frame.y, 200.0 + 1055.0 - 100.0 - MARGIN);
    }

    #[test]
    fn tall_content_is_clamped_to_the_screen() {
        let frame = panel_frame(DISPLAY, 5000);
        assert_eq!(frame.height, 1415.0 - 2.0 * MARGIN);
        assert_eq!(frame.y, DISPLAY.y + MARGIN);
    }

    #[test]
    fn zero_height_is_one_point_and_a_tiny_screen_still_fits() {
        assert_eq!(panel_frame(DISPLAY, 0).height, 1.0);
        let tiny = Rect { x: 0.0, y: 0.0, width: 300.0, height: 200.0 };
        let frame = panel_frame(tiny, 500);
        assert!(frame.width <= 300.0 - 2.0 * MARGIN && frame.x >= MARGIN, "{frame:?}");
    }
}
```

`app/src/logging.rs`, tests first:

```rust
#[cfg(test)]
mod tests {
    use tracing::level_filters::LevelFilter;

    use super::*;

    #[test]
    fn the_level_reads_the_variable_and_defaults_to_info() {
        assert_eq!(level_from(None), LevelFilter::INFO);
        assert_eq!(level_from(Some("debug")), LevelFilter::DEBUG);
        assert_eq!(level_from(Some("TRACE")), LevelFilter::TRACE);
        assert_eq!(level_from(Some("loud")), LevelFilter::INFO);
    }
}
```

`app/src/main.rs` for now:

```rust
//! `agent-notifyd`: the composition root. Task 16 fills in `run`.

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

mod geometry;
mod logging;
#[cfg(target_os = "macos")]
mod macos;

use std::process::ExitCode;

/// The build stamp `config build --tools` embeds.
const STAMP: &str = match option_env!("CONFIG_MANIFEST_STAMP") {
    Some(stamp) => stamp,
    None => "unstamped",
};

fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("--stamp") {
        println!("{STAMP}");
        return ExitCode::SUCCESS;
    }
    eprintln!("agent-notifyd: not wired yet");
    ExitCode::from(1)
}
```

`app/src/macos/mod.rs`:

```rust
//! The macOS services and panel.

pub mod services;
```

- [ ] **Step 3: Run them to see them fail**

Run: `cd ~/tools/agent-notify && cargo test -p agent-notifyd`
Expected: compile errors, `cannot find function panel_frame`. The first build compiles Tauri: allow several minutes.

- [ ] **Step 4: Implement `geometry.rs` and `logging.rs`**

Above the tests in `geometry.rs`:

```rust
//! Where the panel goes. Pure: `panel.rs` reads the screen and applies it.

/// The panel width in points: a 380-point card plus 12 points each side
/// for the shadow and the close button that overhangs the corner.
pub const PANEL_WIDTH: f64 = 404.0;
/// The gap to the screen's top and right edges.
pub const MARGIN: f64 = 16.0;

/// A rectangle in AppKit screen coordinates, origin at the bottom left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f64,
    /// Bottom edge.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

/// The panel frame for content `content_height_px` tall (CSS pixels equal
/// points) in the screen's `visible` frame, which excludes the menu bar.
#[must_use]
pub fn panel_frame(visible: Rect, content_height_px: u32) -> Rect {
    let width = PANEL_WIDTH.min(visible.width - 2.0 * MARGIN).max(1.0);
    let height = f64::from(content_height_px).min(visible.height - 2.0 * MARGIN).max(1.0);
    Rect { x: visible.x + visible.width - width - MARGIN, y: visible.y + visible.height - height - MARGIN, width, height }
}
```

Above the tests in `logging.rs`:

```rust
//! Daily log files, seven kept, in a private directory.

use std::fs::{self, Permissions};
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use tracing::level_filters::LevelFilter;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};

/// The level from `$AGENT_NOTIFY_LOG`. Unknown values read as `info`.
#[must_use]
pub fn level_from(value: Option<&str>) -> LevelFilter {
    match value.map(str::to_ascii_lowercase).as_deref() {
        Some("error") => LevelFilter::ERROR,
        Some("warn") => LevelFilter::WARN,
        Some("debug") => LevelFilter::DEBUG,
        Some("trace") => LevelFilter::TRACE,
        _ => LevelFilter::INFO,
    }
}

/// Starts logging to `directory/agent-notifyd.<date>.log`. Keep the guard
/// alive until exit, or the last lines are lost.
///
/// # Errors
///
/// When the directory cannot be created or the appender cannot open a file.
pub fn init(directory: &Path, level: LevelFilter) -> io::Result<WorkerGuard> {
    fs::create_dir_all(directory)?;
    fs::set_permissions(directory, Permissions::from_mode(0o700))?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("agent-notifyd")
        .filename_suffix("log")
        .max_log_files(7)
        .build(directory)
        .map_err(io::Error::other)?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt().with_writer(writer).with_ansi(false).with_max_level(level).init();
    Ok(guard)
}

/// Logs a panic with its location before the default hook runs.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(panic = %info, "agent-notifyd panicked");
        previous(info);
    }));
}
```

- [ ] **Step 5: Implement the macOS services**

`app/src/macos/services.rs`:

```rust
//! The macOS service implementations. AppKit calls run on the main thread
//! through Tauri, and every wait for the main thread is bounded.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use agent_notify_core::update::Frontmost;
use agent_notify_daemon::aerospace::Aerospace;
use agent_notify_daemon::presence::{TERMINAL_BUNDLE_ID, frontmost_from_bundle};
use agent_notify_daemon::services::{AttachRequest, FrontmostReader, SoundPlayer, WindowRaiser};
use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};
use objc2_foundation::NSString;
use tauri::AppHandle;

const MAIN_THREAD_WAIT: Duration = Duration::from_secs(1);
const AFPLAY: &str = "/usr/bin/afplay";
const SOUND: &str = "/System/Library/Sounds/Glass.aiff";
const OPEN: &str = "/usr/bin/open";

/// Runs `work` on the main thread and waits up to `limit` for its answer.
pub fn on_main<T: Send + 'static>(app: &AppHandle, limit: Duration, work: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (sender, receiver) = mpsc::channel();
    if let Err(error) = app.run_on_main_thread(move || {
        let _ = sender.send(work());
    }) {
        tracing::warn!(%error, "the main thread is gone");
        return None;
    }
    receiver.recv_timeout(limit).inspect_err(|_| tracing::warn!("the main thread did not answer in time")).ok()
}

/// The frontmost app from `NSWorkspace`.
pub struct MacFrontmost {
    app: AppHandle,
}

impl MacFrontmost {
    /// A reader that hops to `app`'s main thread.
    pub fn new(app: AppHandle) -> Self {
        MacFrontmost { app }
    }
}

impl FrontmostReader for MacFrontmost {
    fn frontmost(&self) -> Frontmost {
        on_main(&self.app, MAIN_THREAD_WAIT, || {
            let bundle = NSWorkspace::sharedWorkspace().frontmostApplication().and_then(|app| app.bundleIdentifier()).map(|id| id.to_string());
            frontmost_from_bundle(bundle.as_deref())
        })
        .unwrap_or(Frontmost::Unknown)
    }
}

/// The Glass system sound through `afplay`.
#[derive(Debug, Clone, Copy, Default)]
pub struct AfplaySound;

impl SoundPlayer for AfplaySound {
    fn play(&self) {
        match Command::new(AFPLAY).arg(SOUND).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            Ok(mut child) => {
                std::thread::spawn(move || child.wait());
            }
            Err(error) => tracing::warn!(%error, "afplay did not start"),
        }
    }
}

/// AeroSpace focus when present, then app activation (spike D2).
pub struct MacRaiser {
    app: AppHandle,
    aerospace: Option<Aerospace>,
}

impl MacRaiser {
    /// A raiser for `app`, with AeroSpace at `aerospace` when found.
    pub fn new(app: AppHandle, aerospace: Option<PathBuf>) -> Self {
        MacRaiser { app, aerospace: aerospace.map(Aerospace::new) }
    }
}

impl WindowRaiser for MacRaiser {
    fn raise(&self) -> bool {
        if let Some(aerospace) = &self.aerospace
            && let Some(window) = aerospace.terminal_window()
            && !aerospace.focus(window)
        {
            tracing::debug!(window, "aerospace focus failed; activating anyway");
        }
        on_main(&self.app, MAIN_THREAD_WAIT, || {
            let apps = NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(TERMINAL_BUNDLE_ID));
            apps.firstObject().is_some_and(|terminal| terminal.activateWithOptions(NSApplicationActivationOptions::empty()))
        })
        .unwrap_or(false)
    }

    fn open_terminal(&self, request: &AttachRequest) -> bool {
        let status = Command::new(OPEN)
            .args(["-n", "-b", TERMINAL_BUNDLE_ID, "--args", "-e"])
            .arg(&request.tmux)
            .arg("-S")
            .arg(&request.socket)
            .args(["attach-session", "-t", &format!("={}", request.session)])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output();
        match status {
            Ok(output) if output.status.success() => true,
            Ok(output) => {
                tracing::warn!(stderr = %String::from_utf8_lossy(&output.stderr).trim(), "open did not start the terminal");
                false
            }
            Err(error) => {
                tracing::warn!(%error, "open did not start");
                false
            }
        }
    }
}
```

`=name` makes tmux match the session name exactly, not as a prefix.

`objc2-app-kit` 0.3.2 marks `NSWorkspace::sharedWorkspace`, `frontmostApplication`, `bundleIdentifier`, `runningApplicationsWithBundleIdentifier` and `activateWithOptions` safe (checked 2026-10-15). If a call needs a `MainThreadMarker`, take it with `objc2::MainThreadMarker::new()` inside the closure; the closure runs on the main thread, so it is `Some`.

- [ ] **Step 6: Run the tests and clippy**

Run: `cd ~/tools/agent-notify && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: all pass. `MacFrontmost`, `AfplaySound`, `MacRaiser`, `on_main` and `init` are not called until Task 16; if clippy reports them as dead code, add `#[cfg_attr(not(test), expect(dead_code, reason = "wired in Task 16"))]` on each, and delete those attributes in Task 16.

- [ ] **Step 7: Run cargo deny with Tauri in the tree**

Run: `cd ~/tools/agent-notify && cargo deny check 2>&1 | tail -3`
Expected: `advisories ok, bans ok, licenses ok, sources ok` and no warnings. Apply the license rule from Task 8 Step 1 to anything new.

- [ ] **Step 8: Commit**

```bash
cd ~ && config add tools/agent-notify/Cargo.toml tools/agent-notify/Cargo.lock tools/agent-notify/app tools/agent-notify/scripts/make-icon.ts
config diff --cached --name-only
config commit -m "Add the agent-notifyd app crate and macOS services" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- tools/agent-notify/Cargo.toml tools/agent-notify/Cargo.lock tools/agent-notify/app tools/agent-notify/scripts/make-icon.ts
```

`app/gen/` holds Tauri's generated schemas. Add `app/gen/` to `tools/agent-notify/.gitignore` in this commit if it appeared.

---

### Task 15: The macOS panel and presenter

**Files:**
- Create: `tools/agent-notify/app/src/macos/panel.rs`
- Modify: `tools/agent-notify/app/src/macos/mod.rs`

**Interfaces:**
- Consumes: Task 10 `VIEW_MODEL_EVENT`, `HOVER_EVENT`, `HoverPayload`; Task 12 `DaemonHandle`; Task 14 `geometry`, `on_main`.
- Produces (`macos::panel`): `LABEL = "toast"`, `build(app: &AppHandle, daemon: DaemonHandle) -> tauri::Result<()>`, `PanelPresenter::new(AppHandle)` implementing `ToastPresenter`, `resize(app: &AppHandle, content_height_px: u32)`.

The settings below are the ones spike 1 verified (S1-1 to S1-6). Do not change one without rerunning the matching check from the spike report.

- [ ] **Step 1: Write the panel module**

`app/src/macos/panel.rs`:

```rust
//! The non-activating toast panel (spike 1). `tauri_panel!` expands to
//! Objective-C class definitions, which need `unsafe`.

#![allow(unsafe_code)] // tauri_panel! defines Objective-C classes; see spike S1.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use agent_notify_core::update::UiIntent;
use agent_notify_core::view::PanelViewModel;
use agent_notify_daemon::actor::DaemonHandle;
use agent_notify_daemon::bridge::{HOVER_EVENT, HoverPayload, VIEW_MODEL_EVENT};
use agent_notify_daemon::services::ToastPresenter;
use objc2_app_kit::{NSScreen, NSWindowSharingType};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, WebviewUrl};
use tauri_nspanel::{CollectionBehavior, ManagerExt, PanelBuilder, PanelLevel, StyleMask, tauri_panel};

use crate::geometry::{PANEL_WIDTH, Rect, panel_frame};

tauri_panel! {
    panel!(ToastPanel {
        config: {
            can_become_key_window: false,
            can_become_main_window: false,
            is_floating_panel: true
        }
        with: {
            tracking_area: {
                options: tauri_nspanel::TrackingAreaOptions::new().active_always().mouse_entered_and_exited(),
                auto_resize: true
            }
        }
    })
    panel_event!(ToastEvents {})
}

/// The panel's window label.
pub const LABEL: &str = "toast";
const INITIAL_HEIGHT: u32 = 120;

static CONTENT_HEIGHT: AtomicU32 = AtomicU32::new(INITIAL_HEIGHT);

/// Builds the hidden panel and wires native hover to the daemon and page.
///
/// # Errors
///
/// When Tauri cannot build the panel's web view.
pub fn build(app: &AppHandle, daemon: DaemonHandle) -> tauri::Result<()> {
    let panel = PanelBuilder::<_, ToastPanel>::new(app, LABEL)
        .url(WebviewUrl::App("index.html".into()))
        .position(LogicalPosition::new(0.0, 0.0).into())
        .size(LogicalSize::new(PANEL_WIDTH, f64::from(INITIAL_HEIGHT)).into())
        .level(PanelLevel::Status)
        .transparent(true)
        .has_shadow(false)
        .hides_on_deactivate(false)
        .no_activate(true)
        .style_mask(StyleMask::empty().borderless().nonactivating_panel())
        .collection_behavior(CollectionBehavior::new().can_join_all_spaces().full_screen_auxiliary().stationary().ignores_cycle())
        .with_window(|window| window.decorations(false).transparent(true).accept_first_mouse(true).skip_taskbar(true).focused(false).visible(false))
        .build()?;
    panel.as_panel().setSharingType(NSWindowSharingType::None);
    panel.hide();
    let events = ToastEvents::new();
    let entered = hover_reporter(app.clone(), daemon.clone(), true);
    events.on_mouse_entered(move |_| entered());
    let exited = hover_reporter(app.clone(), daemon, false);
    events.on_mouse_exited(move |_| exited());
    panel.set_event_handler(Some(events.as_ref()));
    Ok(())
}

fn hover_reporter(app: AppHandle, daemon: DaemonHandle, hovered: bool) -> impl Fn() {
    move || {
        daemon.send_ui(UiIntent::HoverChanged { hovered });
        if let Err(error) = app.emit_to(LABEL, HOVER_EVENT, HoverPayload { hovered }) {
            tracing::warn!(%error, "hover event not delivered");
        }
    }
}

/// Shows the view in the panel.
pub struct PanelPresenter {
    app: AppHandle,
}

impl PanelPresenter {
    /// A presenter for `app`'s panel.
    pub fn new(app: AppHandle) -> Self {
        PanelPresenter { app }
    }
}

fn has_content(view: &PanelViewModel) -> bool {
    !view.cards.is_empty() || !view.finished_row.is_empty()
}

impl ToastPresenter for PanelPresenter {
    fn present(&self, view: &PanelViewModel) {
        if let Err(error) = self.app.emit_to(LABEL, VIEW_MODEL_EVENT, view) {
            tracing::warn!(%error, "view model not delivered");
        }
        let visible = has_content(view);
        let app = self.app.clone();
        let scheduled = self.app.run_on_main_thread(move || {
            let Ok(panel) = app.get_webview_panel(LABEL) else { return };
            if visible {
                place(&app, CONTENT_HEIGHT.load(Ordering::Relaxed));
                panel.show();
            } else {
                panel.hide();
            }
        });
        if let Err(error) = scheduled {
            tracing::warn!(%error, "panel update not scheduled");
        }
    }
}

/// Records the page's content height and resizes the panel. Call on the main thread.
pub fn resize(app: &AppHandle, content_height_px: u32) {
    CONTENT_HEIGHT.store(content_height_px, Ordering::Relaxed);
    place(app, content_height_px);
}

fn place(app: &AppHandle, content_height_px: u32) {
    let Some(marker) = objc2::MainThreadMarker::new() else {
        tracing::error!("place ran off the main thread");
        return;
    };
    let Some(screen) = NSScreen::mainScreen(marker) else { return };
    let visible = screen.visibleFrame();
    let frame = panel_frame(
        Rect { x: visible.origin.x, y: visible.origin.y, width: visible.size.width, height: visible.size.height },
        content_height_px,
    );
    if let Ok(panel) = app.get_webview_panel(LABEL) {
        let rect = NSRect::new(NSPoint::new(frame.x, frame.y), NSSize::new(frame.width, frame.height));
        panel.as_panel().setFrame_display(rect, true);
    }
}
```

Add `pub mod panel;` to `macos/mod.rs`.

`CONTENT_HEIGHT` is process state with one writer (the `ui_message` command) and one reader (the presenter). It is the only module-level mutable value in the app; it lives here because both callers already share the panel's lifetime.

- [ ] **Step 2: Build and clippy**

Run: `cd ~/tools/agent-notify && cargo build -p agent-notifyd && cargo clippy --workspace --all-targets -- -D warnings`
Expected: builds. If `setSharingType` or `setFrame_display` is `unsafe` in objc2-app-kit 0.3.2, wrap that one call in `unsafe { }` with a `// SAFETY:` line naming the reason (a plain property setter on a live window, on the main thread). If `NSWindowSharingType` lives under another name, find it with `grep -rn "NSWindowSharingNone\|SharingType" ~/.cargo/registry/src/*/objc2-app-kit-0.3.2/src/generated/NSWindow.rs`.

The panel has no automated test: it needs a window server. Task 16 Step 5 and the final smoke test check it by hand.

- [ ] **Step 3: Commit**

```bash
cd ~ && config add tools/agent-notify/app
config commit -m "Add the agent-notifyd macOS toast panel" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- tools/agent-notify/app
```

---

### Task 16: The composition root

**Files:**
- Create: `tools/agent-notify/app/src/shell.rs`
- Modify: `tools/agent-notify/app/src/main.rs`, `app/src/macos/mod.rs`

**Interfaces:**
- Consumes: everything above.
- Produces: the running daemon. `#[tauri::command] async fn bootstrap(...) -> Result<UiBootstrap, String>` and `#[tauri::command] fn ui_message(...)`, with exactly those names (Task 13's `tauri-services.ts` calls them).

`main` builds every service once, in this order: environment, logging, socket bind (exit 0 if another daemon runs), tools, state directory, services, Tokio runtime, Tauri. Nothing below `main` constructs a service.

- [ ] **Step 1: Write the command shell**

`app/src/shell.rs`:

```rust
//! The Tauri commands. Each is thin: it routes to the daemon or the panel.

use agent_notify_daemon::actor::DaemonHandle;
use agent_notify_daemon::bridge::{Routed, UiBootstrap, UiMessage, route};
use tauri::{AppHandle, State};

/// Shared with the commands through Tauri's managed state.
pub struct Shell {
    /// The daemon.
    pub daemon: DaemonHandle,
    /// True on macOS, where the panel reports hover natively.
    pub native_hover: bool,
}

/// The page's first read.
#[tauri::command]
pub async fn bootstrap(shell: State<'_, Shell>) -> Result<UiBootstrap, String> {
    let view_model = shell.daemon.view_model().await.ok_or_else(|| "the daemon stopped".to_string())?;
    Ok(UiBootstrap { view_model, native_hover: shell.native_hover })
}

/// Every message from the page. Runs on the main thread (a sync command).
#[tauri::command]
pub fn ui_message(app: AppHandle, shell: State<'_, Shell>, message: UiMessage) {
    match route(message) {
        Routed::Intent(intent) => shell.daemon.send_ui(intent),
        Routed::Layout { content_height_px } => resize(&app, content_height_px),
        Routed::Error { message, stack } => tracing::error!(%message, stack = stack.as_deref().unwrap_or(""), "the page reported an error"),
    }
}

#[cfg(target_os = "macos")]
fn resize(app: &AppHandle, content_height_px: u32) {
    crate::macos::panel::resize(app, content_height_px);
}

#[cfg(not(target_os = "macos"))]
fn resize(_app: &AppHandle, _content_height_px: u32) {}
```

- [ ] **Step 2: Write `macos::run`**

Append to `app/src/macos/mod.rs`:

```rust
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use agent_notify_core::paths::{SocketLocation, log_directory, socket_location, state_directory};
use agent_notify_core::status::DaemonFacts;
use agent_notify_daemon::actor::{Daemon, DaemonConfig};
use agent_notify_daemon::dnd_store::DndStore;
use agent_notify_daemon::environment::DaemonEnvironment;
use agent_notify_daemon::presence::IoregPresence;
use agent_notify_daemon::services::{Services, system_clock};
use agent_notify_daemon::socket::{BindOutcome, bind};
use agent_notify_daemon::tmux::Tmux;
use agent_notify_daemon::tools::ToolPaths;
use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSString};
use tauri::{Manager, RunEvent};

use crate::shell::{Shell, bootstrap, ui_message};
use crate::{STAMP, logging};

/// Runs the daemon until the app exits.
pub fn run() -> ExitCode {
    let environment = DaemonEnvironment::read();
    let inputs = environment.path_inputs();
    let level = logging::level_from(std::env::var("AGENT_NOTIFY_LOG").ok().as_deref());
    let _log_guard = match log_directory(&inputs).map(|directory| logging::init(&directory, level)) {
        Some(Ok(guard)) => Some(guard),
        Some(Err(error)) => {
            eprintln!("agent-notifyd: logging did not start: {error}");
            None
        }
        None => None,
    };
    logging::install_panic_hook();
    let SocketLocation::At(socket) = socket_location(&inputs) else {
        tracing::error!("no socket path on this machine");
        return ExitCode::from(1);
    };
    let listener = match bind(&socket) {
        Ok(BindOutcome::Bound(listener)) => listener,
        Ok(BindOutcome::AlreadyRunning) => {
            tracing::info!("another agent-notifyd runs; exiting");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            tracing::error!(%error, socket = %socket.display(), "the socket did not bind");
            return ExitCode::from(1);
        }
    };
    let Some(state) = state_directory(&inputs) else {
        tracing::error!("no state directory on this machine");
        return ExitCode::from(1);
    };
    let tools = ToolPaths::discover(&environment);
    tracing::info!(stamp = STAMP, ?tools, socket = %socket.display(), "starting");
    let runtime = match tokio::runtime::Builder::new_multi_thread().worker_threads(2).thread_name("agent-notifyd").enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::error!(%error, "the runtime did not start");
            return ExitCode::from(1);
        }
    };
    let _activity = NSProcessInfo::processInfo().beginActivityWithOptions_reason(
        NSActivityOptions::UserInitiatedAllowingIdleSystemSleep,
        &NSString::from_str("agent-notifyd delivers agent notifications"),
    );
    let daemon_slot: Arc<Mutex<Option<Daemon>>> = Arc::new(Mutex::new(None));
    let setup_slot = Arc::clone(&daemon_slot);
    let runtime_handle = runtime.handle().clone();
    let clock = system_clock();
    let facts = DaemonFacts { stamp: STAMP.to_string(), started_ms: clock(), tools: tools.statuses() };
    let built = tauri::Builder::default()
        .plugin(tauri_nspanel::init())
        .invoke_handler(tauri::generate_handler![bootstrap, ui_message])
        .setup(move |app| {
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let handle = app.handle().clone();
            let services = Arc::new(Services {
                tmux: tools.tmux.clone().map(Tmux::new),
                raiser: Arc::new(services::MacRaiser::new(handle.clone(), tools.aerospace.clone())),
                presenter: Arc::new(panel::PanelPresenter::new(handle.clone())),
                frontmost: Arc::new(services::MacFrontmost::new(handle.clone())),
                presence: Arc::new(IoregPresence),
                sound: Arc::new(services::AfplaySound),
                dnd_store: DndStore::new(state.join("dnd.json")),
                clock,
                facts,
            });
            let daemon = {
                let _entered = runtime_handle.enter();
                Daemon::start(services, listener, DaemonConfig::default())
            };
            panel::build(&handle, daemon.handle())?;
            app.manage(Shell { daemon: daemon.handle(), native_hover: true });
            if let Ok(mut slot) = setup_slot.lock() {
                *slot = Some(daemon);
            }
            Ok(())
        })
        .build(tauri::generate_context!());
    let app = match built {
        Ok(app) => app,
        Err(error) => {
            tracing::error!(%error, "tauri did not start");
            return ExitCode::from(1);
        }
    };
    let exit_handle = runtime.handle().clone();
    app.run(move |_, event| {
        if let RunEvent::Exit = event
            && let Some(daemon) = daemon_slot.lock().ok().and_then(|mut slot| slot.take())
        {
            exit_handle.block_on(daemon.stop());
            tracing::info!("stopped");
        }
    });
    ExitCode::SUCCESS
}
```

Notes:

- `Builder::setup` takes an `FnOnce`, so `listener`, `services` inputs and `state` move into it once.
- `log_directory` and `state_directory` return `Option<PathBuf>` (plan 1 and Task 2). A logging failure is printed, not dropped: the `eprintln!` reaches launchd's `StandardErrorPath`, and the daemon keeps running without a log file.
- `beginActivityWithOptions_reason` returns a token; holding `_activity` on `run`'s stack keeps App Nap off for the process lifetime.
- `app.run` blocks until the app exits; on macOS it may call `std::process::exit` after `RunEvent::Exit`, which is why the daemon stops inside that event.

- [ ] **Step 3: Wire `main.rs`**

Replace `main` in `app/src/main.rs` and add `mod shell;`:

```rust
mod shell;

fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("--stamp") {
        println!("{STAMP}");
        return ExitCode::SUCCESS;
    }
    run()
}

#[cfg(target_os = "macos")]
fn run() -> ExitCode {
    macos::run()
}

#[cfg(not(target_os = "macos"))]
fn run() -> ExitCode {
    eprintln!("agent-notifyd: this platform is not supported yet (plan 3 adds Linux)");
    ExitCode::from(1)
}
```

Add `pub mod panel;` (if not there) and the `use` lines above to `macos/mod.rs`, and delete the `expect(dead_code)` attributes from Task 14.

- [ ] **Step 4: Build, test and clippy**

Run: `cd ~/tools/agent-notify && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo build -p agent-notifyd`
Expected: all pass.

- [ ] **Step 5: Run it by hand against a private socket**

This step does not touch launchd or the live setup. In one tmux pane:

```bash
cd ~/tools/agent-notify/ui && bun run build
export AGENT_NOTIFY_SOCKET=/tmp/an-dev/agent-notify.sock AGENT_NOTIFY_LOG_DIR=/tmp/an-dev/logs AGENT_NOTIFY_LOG=debug
~/.cache/agent-notify/target/debug/agent-notifyd
```

In a second pane, with the same `AGENT_NOTIFY_SOCKET` exported, using the CLI built in Tasks 4 to 7 (`~/crates`, `cargo build -p agent-notify`, binary at `~/crates/target/debug/agent-notify` or the workspace's target directory):

```bash
export AGENT_NOTIFY_SOCKET=/tmp/an-dev/agent-notify.sock
agent-notify status
agent-notify send info 'Hello from the dev daemon'
agent-notify send attention 'Needs you: <b>raw</b> & text'
agent-notify dnd on && agent-notify dnd status && agent-notify dnd off
agent-notify jump
```

Check each, and write what you saw in the commit message body:

1. `status` prints the stamp `unstamped`, the tmux path and the AeroSpace path.
2. The info card appears top right; the attention card above it in amber; `<b>raw</b> & text` shows literally.
3. Alacritty stays frontmost while the cards appear (`aerospace list-windows --focused` still names Alacritty).
4. Moving the pointer over the panel shows the close buttons; leaving hides them.
5. Clicking the attention card switches tmux to the second pane's window and raises Alacritty. Clicking × on the info card removes it without switching.
6. `dnd on` hides the panel; `dnd off` brings the remaining card back.
7. A second `agent-notifyd` started with the same environment exits 0 at once and logs "another agent-notifyd runs".
8. `ls -l /tmp/an-dev` shows the socket as `srw-------`.

Stop the daemon with Ctrl-C and confirm `/tmp/an-dev/agent-notify.sock` is gone. Any failed check is a bug in this plan's code: fix it before you commit.

- [ ] **Step 6: Commit**

```bash
cd ~ && config add tools/agent-notify/app
config commit -m "Wire the agent-notifyd composition root" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- tools/agent-notify/app
```

---

### Task 17: Install: app bundle, launch agent, `config build --tools`

The install is a Bun script with a pure planner. `install-plan.ts` takes plain data (home, uid, resolved tool paths, the stamp) and returns the files to write and their text. `install.ts` is the shell: it gathers that data, builds, writes the files, and calls `launchctl`. Only the planner has unit tests; the shell is checked by running it.

**Files:**
- Create: `tools/agent-notify/scripts/install-plan.ts`, `tools/agent-notify/scripts/install.ts`
- Create: `tools/agent-notify/tests/install-plan.test.ts`
- Modify: `.scripts/config/config-build` (the `--tools` option), `README.md` (the `config build` bullet), `deps/deps.toml` (`[cargo-deny]`)
- Test: `crates/config-cli/tests/config_dispatcher.rs` (one new test), `tools/agent-notify/tests/install-plan.test.ts`

**Interfaces:**
- Consumes: Task 14 `--stamp` and `CONFIG_MANIFEST_STAMP`; Task 7 `LAUNCHD_LABEL = "dev.dotfiles.agent-notifyd"`; plan 1 state directory (`~/Library/Application Support/agent-notify` on macOS; the `paths::state_directory` value).
- Produces (`install-plan.ts`):
  - `LABEL = "dev.dotfiles.agent-notifyd"`, `BUNDLE_ID = "dev.dotfiles.agent-notifyd"`, `EXECUTABLE = "agent-notifyd"`.
  - `type InstallInputs = { home: string; uid: number; tmux: string | null; aerospace: string | null; brewBin: string; stamp: string }`.
  - `type InstallPlan = { appDir: string; executablePath: string; infoPlist: string; launchAgentPath: string; launchAgent: string; stateDir: string; stampPath: string; logDir: string; domain: string }`.
  - `xmlEscape(text)`, `renderInfoPlist()`, `renderLaunchAgent(inputs, executablePath, logDir)`, `planInstall(inputs)`, `stampFrom(files: { path: string; bytes: Uint8Array }[])`.

- [ ] **Step 1: Write the failing planner tests**

`tools/agent-notify/tests/install-plan.test.ts`:

```ts
import { expect, test } from "bun:test";
import { LABEL, planInstall, renderInfoPlist, stampFrom, xmlEscape, type InstallInputs } from "../scripts/install-plan";

const inputs: InstallInputs = {
  home: "/Users/dev",
  uid: 501,
  tmux: "/opt/homebrew/bin/tmux",
  aerospace: "/opt/homebrew/bin/aerospace",
  brewBin: "/opt/homebrew/bin",
  stamp: "abc123",
};

test("paths sit under the home directory", () => {
  const plan = planInstall(inputs);
  expect(plan.executablePath).toBe("/Users/dev/Applications/agent-notifyd.app/Contents/MacOS/agent-notifyd");
  expect(plan.launchAgentPath).toBe(`/Users/dev/Library/LaunchAgents/${LABEL}.plist`);
  expect(plan.stampPath).toBe("/Users/dev/Library/Application Support/agent-notify/installed-stamp");
  expect(plan.domain).toBe("gui/501");
});

test("the launch agent restarts only after a failure", () => {
  const agent = planInstall(inputs).launchAgent;
  expect(agent).toContain("<key>KeepAlive</key>\n  <dict>\n    <key>SuccessfulExit</key>\n    <false/>\n  </dict>");
  expect(agent).toContain("<key>LimitLoadToSessionType</key>\n  <string>Aqua</string>");
  expect(agent).toContain("<key>ProcessType</key>\n  <string>Interactive</string>");
  expect(agent).toContain("<key>RunAtLoad</key>\n  <true/>");
});

test("the launch agent carries absolute tool paths and a PATH with Homebrew first", () => {
  const agent = planInstall(inputs).launchAgent;
  expect(agent).toContain("<key>AGENT_NOTIFY_TMUX</key>\n    <string>/opt/homebrew/bin/tmux</string>");
  expect(agent).toContain("<key>AGENT_NOTIFY_AEROSPACE</key>\n    <string>/opt/homebrew/bin/aerospace</string>");
  expect(agent).toContain("<string>/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>");
  expect(agent).toContain("<key>RUST_BACKTRACE</key>\n    <string>1</string>");
});

test("a missing tool leaves its variable out", () => {
  const agent = planInstall({ ...inputs, aerospace: null }).launchAgent;
  expect(agent).not.toContain("AGENT_NOTIFY_AEROSPACE");
  expect(agent).toContain("AGENT_NOTIFY_TMUX");
});

test("text that reaches XML is escaped", () => {
  expect(xmlEscape(`a&b<c>"d'`)).toBe("a&amp;b&lt;c&gt;&quot;d&apos;");
  const agent = planInstall({ ...inputs, home: "/Users/a&b" }).launchAgent;
  expect(agent).toContain("/Users/a&amp;b/Applications/agent-notifyd.app/Contents/MacOS/agent-notifyd");
  expect(agent).not.toContain("/Users/a&b/");
});

test("the app bundle is a background agent with the macOS floor", () => {
  const info = renderInfoPlist();
  expect(info).toContain("<key>LSUIElement</key>\n  <true/>");
  expect(info).toContain("<key>LSMinimumSystemVersion</key>\n  <string>26.0</string>");
  expect(info).toContain("<key>CFBundleExecutable</key>\n  <string>agent-notifyd</string>");
  expect(info).toContain("<key>CFBundleIdentifier</key>\n  <string>dev.dotfiles.agent-notifyd</string>");
});

test("the stamp depends on content and path, not on input order", () => {
  const one = { path: "a.rs", bytes: new TextEncoder().encode("one") };
  const two = { path: "b.rs", bytes: new TextEncoder().encode("two") };
  expect(stampFrom([one, two])).toBe(stampFrom([two, one]));
  expect(stampFrom([one, two])).not.toBe(stampFrom([{ ...one, path: "c.rs" }, two]));
  expect(stampFrom([one, two])).not.toBe(stampFrom([one, { ...two, bytes: new TextEncoder().encode("twO") }]));
  expect(stampFrom([one])).toMatch(/^[0-9a-f]{16}$/);
});
```

Run: `cd ~/tools/agent-notify && bun test tests/install-plan.test.ts`
Expected: failure, `Cannot find module '../scripts/install-plan'`. The root `bunfig.toml` from Task 8 sets the test root to `tests`, so the UI's tests do not run here.

- [ ] **Step 2: Implement the planner**

`tools/agent-notify/scripts/install-plan.ts`:

```ts
export const LABEL = "dev.dotfiles.agent-notifyd";
export const BUNDLE_ID = "dev.dotfiles.agent-notifyd";
export const EXECUTABLE = "agent-notifyd";

export type InstallInputs = {
  home: string;
  uid: number;
  tmux: string | null;
  aerospace: string | null;
  brewBin: string;
  stamp: string;
};

export type InstallPlan = {
  appDir: string;
  executablePath: string;
  infoPlist: string;
  launchAgentPath: string;
  launchAgent: string;
  stateDir: string;
  stampPath: string;
  logDir: string;
  domain: string;
};

export function xmlEscape(text: string): string {
  return text.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;").replaceAll("'", "&apos;");
}

const HEADER = `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>`;
const FOOTER = `</dict>
</plist>
`;

const entry = (key: string, value: string, indent = "  "): string =>
  `${indent}<key>${xmlEscape(key)}</key>\n${indent}<string>${xmlEscape(value)}</string>`;

export function renderInfoPlist(): string {
  return [
    HEADER,
    entry("CFBundleIdentifier", BUNDLE_ID),
    entry("CFBundleExecutable", EXECUTABLE),
    entry("CFBundleName", EXECUTABLE),
    entry("CFBundlePackageType", "APPL"),
    entry("LSMinimumSystemVersion", "26.0"),
    "  <key>LSUIElement</key>\n  <true/>",
    FOOTER,
  ].join("\n");
}

export function renderLaunchAgent(inputs: InstallInputs, executablePath: string, logDir: string): string {
  const environment: Array<[string, string]> = [
    ["PATH", `${inputs.brewBin}:/usr/bin:/bin:/usr/sbin:/sbin`],
    ["LANG", "en_US.UTF-8"],
    ["RUST_BACKTRACE", "1"],
  ];
  if (inputs.tmux !== null) environment.push(["AGENT_NOTIFY_TMUX", inputs.tmux]);
  if (inputs.aerospace !== null) environment.push(["AGENT_NOTIFY_AEROSPACE", inputs.aerospace]);
  return [
    HEADER,
    entry("Label", LABEL),
    `  <key>ProgramArguments</key>\n  <array>\n    <string>${xmlEscape(executablePath)}</string>\n  </array>`,
    "  <key>RunAtLoad</key>\n  <true/>",
    "  <key>KeepAlive</key>\n  <dict>\n    <key>SuccessfulExit</key>\n    <false/>\n  </dict>",
    entry("LimitLoadToSessionType", "Aqua"),
    entry("ProcessType", "Interactive"),
    entry("StandardErrorPath", `${logDir}/launchd-stderr.log`),
    `  <key>EnvironmentVariables</key>\n  <dict>\n${environment.map(([key, value]) => entry(key, value, "    ")).join("\n")}\n  </dict>`,
    FOOTER,
  ].join("\n");
}

export function planInstall(inputs: InstallInputs): InstallPlan {
  const appDir = `${inputs.home}/Applications/agent-notifyd.app`;
  const executablePath = `${appDir}/Contents/MacOS/${EXECUTABLE}`;
  const stateDir = `${inputs.home}/Library/Application Support/agent-notify`;
  const logDir = `${inputs.home}/Library/Logs/agent-notify`;
  return {
    appDir,
    executablePath,
    infoPlist: renderInfoPlist(),
    launchAgentPath: `${inputs.home}/Library/LaunchAgents/${LABEL}.plist`,
    launchAgent: renderLaunchAgent(inputs, executablePath, logDir),
    stateDir,
    stampPath: `${stateDir}/installed-stamp`,
    logDir,
    domain: `gui/${inputs.uid}`,
  };
}

export function stampFrom(files: ReadonlyArray<{ path: string; bytes: Uint8Array }>): string {
  const hasher = new Bun.CryptoHasher("sha256");
  for (const file of [...files].sort((left, right) => (left.path < right.path ? -1 : left.path > right.path ? 1 : 0))) {
    hasher.update(file.path);
    hasher.update("\0");
    hasher.update(String(file.bytes.length));
    hasher.update("\0");
    hasher.update(file.bytes);
  }
  return hasher.digest("hex").slice(0, 16);
}
```

Before you write this, check two values against plan 1: the state directory (`crates/agent-notify-core/src/paths.rs`, `state_directory` on macOS) and the log directory (`log_directory`). If plan 1 uses other paths, use plan 1's and change the test's expected strings. The daemon and the installer must agree, or `doctor` reports a missing stamp.

Run: `cd ~/tools/agent-notify && bun test tests/install-plan.test.ts`
Expected: 7 pass.

- [ ] **Step 3: Write the install shell**

`tools/agent-notify/scripts/install.ts`:

```ts
import { chmod, mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { LABEL, planInstall, stampFrom, type InstallPlan } from "./install-plan";

const toolRoot = join(import.meta.dir, "..");
const home = process.env.HOME ?? "";
const skipLaunchd = process.argv.includes("--no-launchd");

if (process.platform !== "darwin") {
  console.error("agent-notify install: macOS only in this version (plan 3 adds Linux)");
  process.exit(1);
}
if (home === "") {
  console.error("agent-notify install: HOME is not set");
  process.exit(1);
}

function run(command: string[], options: { cwd?: string; env?: Record<string, string>; allowFailure?: boolean } = {}): string {
  const result = Bun.spawnSync(command, { cwd: options.cwd ?? toolRoot, env: { ...process.env, ...options.env }, stdout: "pipe", stderr: "inherit" });
  if (!result.success && options.allowFailure !== true) {
    console.error(`agent-notify install: ${command.join(" ")} failed with exit ${result.exitCode}`);
    process.exit(1);
  }
  return result.stdout.toString();
}

async function writeAtomic(path: string, text: string): Promise<void> {
  await mkdir(dirname(path), { recursive: true });
  const temporary = `${path}.tmp-${process.pid}`;
  await writeFile(temporary, text);
  await rename(temporary, path);
}

async function sourceStamp(): Promise<string> {
  const listed = run(["git", `--git-dir=${home}/.cfg`, `--work-tree=${home}`, "ls-files", "--", "tools/agent-notify", "crates/agent-notify-core"], { cwd: home });
  const paths = listed.split("\n").filter((path) => path !== "");
  const files = await Promise.all(paths.map(async (path) => ({ path, bytes: new Uint8Array(await readFile(join(home, path))) })));
  return stampFrom(files);
}

async function install(plan: InstallPlan, stamp: string): Promise<void> {
  run(["bun", "install", "--frozen-lockfile"], { cwd: join(toolRoot, "ui") });
  run(["bun", "run", "build"], { cwd: join(toolRoot, "ui") });
  run(["cargo", "deny", "check"]);
  run(["cargo", "build", "--release", "--locked", "-p", "agent-notifyd"], { env: { CONFIG_MANIFEST_STAMP: stamp } });
  const built = join(home, ".cache/agent-notify/target/release/agent-notifyd");
  const contents = join(plan.appDir, "Contents");
  await mkdir(join(contents, "MacOS"), { recursive: true });
  await writeAtomic(join(contents, "Info.plist"), plan.infoPlist);
  const staged = `${plan.executablePath}.tmp-${process.pid}`;
  await Bun.write(staged, Bun.file(built));
  await chmod(staged, 0o755);
  await rename(staged, plan.executablePath);
  run(["/usr/bin/codesign", "--force", "--sign", "-", plan.appDir]);
  const reported = run([plan.executablePath, "--stamp"]).trim();
  if (reported !== stamp) {
    console.error(`agent-notify install: the bundle reports stamp ${reported}, expected ${stamp}`);
    process.exit(1);
  }
  await mkdir(plan.stateDir, { recursive: true, mode: 0o700 });
  await mkdir(plan.logDir, { recursive: true, mode: 0o700 });
  await writeAtomic(plan.launchAgentPath, plan.launchAgent);
  await writeAtomic(plan.stampPath, `${stamp}\n`);
}

function reload(plan: InstallPlan): void {
  run(["/bin/launchctl", "bootout", `${plan.domain}/${LABEL}`], { allowFailure: true });
  run(["/bin/launchctl", "bootstrap", plan.domain, plan.launchAgentPath]);
  run(["/bin/launchctl", "kickstart", "-k", `${plan.domain}/${LABEL}`]);
}

const brewBin = Bun.which("brew") === null ? "/usr/local/bin" : dirname(Bun.which("brew") ?? "/usr/local/bin/brew");
const stamp = await sourceStamp();
const plan = planInstall({
  home,
  uid: process.getuid?.() ?? 0,
  tmux: Bun.which("tmux"),
  aerospace: Bun.which("aerospace"),
  brewBin,
  stamp,
});
await install(plan, stamp);
if (skipLaunchd) {
  console.log(`agent-notify install: installed ${plan.appDir} (stamp ${stamp}); launchd not touched`);
} else {
  reload(plan);
  console.log(`agent-notify install: installed and restarted ${LABEL} (stamp ${stamp})`);
}
```

The `bootout` failure is allowed: on the first install there is nothing to boot out.

`kickstart -k` restarts a running daemon, so the new binary replaces the old one at once.

- [ ] **Step 4: Run the install without launchd**

Run: `cd ~/tools/agent-notify && bun scripts/install.ts --no-launchd && ~/Applications/agent-notifyd.app/Contents/MacOS/agent-notifyd --stamp && plutil -lint ~/Library/LaunchAgents/dev.dotfiles.agent-notifyd.plist && codesign --verify --verbose ~/Applications/agent-notifyd.app`
Expected: the stamp prints, `plutil` prints `OK`, and `codesign` prints `valid on disk`.

This writes the plist file but does not load it. Do not run the script without `--no-launchd` until the user says yes: that step replaces the live notification path. Ask: "The daemon is built and installed at ~/Applications/agent-notifyd.app. May I load it with launchctl now? The old Stop hook keeps running until Task 19."

- [ ] **Step 5: Add `--tools` to `config build`, test first**

Add this test to `crates/config-cli/tests/config_dispatcher.rs`, beside the other `config-build` tests (it does not take `build_lock`, because the stub install script builds nothing):

```rust
/// `--tools` runs each tool's install script after the crate build, and
/// passes `--no-launchd` through so a CI or doctor run never touches launchd.
#[test]
fn build_with_tools_runs_each_install_script() {
    let root = tempfile::tempdir().expect("a temp root");
    let script_dir = root.path().join("tools/sample/scripts");
    std::fs::create_dir_all(&script_dir).expect("the script directory");
    let marker = root.path().join("ran");
    std::fs::write(
        script_dir.join("install.ts"),
        format!("await Bun.write({:?}, process.argv.slice(2).join(' '));\n", marker.to_str().expect("utf-8")),
    )
    .expect("the stub script");
    let output = Command::new(config_dir().join("config-build"))
        .args(["--tools-only", "--no-launchd"])
        .env("DOTFILES_ROOT", root.path())
        .output()
        .expect("config-build runs");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(std::fs::read_to_string(&marker).expect("the script ran"), "--no-launchd");
}
```

`--tools-only` skips the crate loop, so this test runs in well under a second. Run it and see it fail: `cd ~/crates && cargo test -p config-cli --test config_dispatcher build_with_tools`. Expected: failure, the script prints usage or the marker is missing.

Then change `.scripts/config/config-build`. Replace the `# usage:` and `# Takes no options.` lines with:

```sh
# usage: config build [--tools | --tools-only] [--no-launchd]
#
#   --tools        Also run tools/*/scripts/install.ts with bun, after the
#                  crates. Each one builds and installs a desktop tool.
#   --tools-only   Run only the tool installs.
#   --no-launchd   Passed to each install script: install files, but do not
#                  load or restart a launch agent.
```

Below `usage_if_requested "${1:-}"`, add the option parse:

```sh
build_crates=yes
build_tools=no
tool_args=''
for argument in "$@"; do
    case $argument in
        --tools) build_tools=yes ;;
        --tools-only) build_tools=yes; build_crates=no ;;
        --no-launchd) tool_args='--no-launchd' ;;
        *) printf 'config-build: unknown option %s\n' "$argument" >&2; exit 2 ;;
    esac
done
```

Wrap the existing `stamps=...` through `done || exit 1` block in `if [ "$build_crates" = yes ]; then ... fi`, and append:

```sh
if [ "$build_tools" = yes ]; then
    for script in "$ROOT"/tools/*/scripts/install.ts; do
        [ -f "$script" ] || continue
        # Unquoted on purpose, as with $quiet above.
        bun "$script" $tool_args || exit 1
    done
fi
```

Change the README bullet in `README.md` (line 74) to:

```markdown
- `config build` builds every workspace crate and installs each stamped
  binary. `--tools` also builds and installs the desktop tools under
  `tools/` (the agent-notify daemon), and `--no-launchd` installs them
  without restarting their launch agents.
```

Keep the rest of the bullet's existing wrap. If `config_docs.rs` compares the bullet against the script's `# help:` line, run it and follow its failure message.

Add to `deps/deps.toml`, after `[rustup]`:

```toml
# The agent-notify install runs `cargo deny check` before it builds the
# daemon. Install with `cargo install --locked cargo-deny`.
[cargo-deny]
command = "cargo-deny"
docs = "https://github.com/EmbarkStudios/cargo-deny"
```

Run: `cd ~/crates && cargo test -p config-cli --test config_dispatcher build_with_tools && cargo test -p config-cli --test config_docs && sh -n ~/.scripts/config/config-build && shellcheck ~/.scripts/config/config-build`
Expected: all pass, no shellcheck findings. `SC2086` on `$tool_args` is expected to be silent because the variable has no spaces; if shellcheck reports it, add `# shellcheck disable=SC2086` above that line with the reason "one optional word".

Also run the full dispatcher suite once, because it rebuilds the real crates: `cargo test -p config-cli --test config_dispatcher`. Expected: pass.

- [ ] **Step 6: Prove the stamp check is load-bearing**

Back up `install.ts`, then change the `cargo build` line to pass `CONFIG_MANIFEST_STAMP: "wrong"`. Run `bun scripts/install.ts --no-launchd; echo "exit $?"`. Expected: `exit 1` with "the bundle reports stamp wrong". Restore the file and `diff` it against the backup.

- [ ] **Step 7: Commit**

```bash
cd ~ && config add tools/agent-notify/scripts tools/agent-notify/tests .scripts/config/config-build README.md deps/deps.toml crates/config-cli/tests/config_dispatcher.rs
config diff --cached --name-only
config commit -m "Install agent-notifyd as a launch agent through config build --tools" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- tools/agent-notify/scripts tools/agent-notify/tests .scripts/config/config-build README.md deps/deps.toml crates/config-cli/tests/config_dispatcher.rs
```

---

### Task 18: tmux integration

**Files:**
- Create: `.config/tmux/agent-notify.conf`
- Modify: `.config/tmux/tmux.conf` (one `source-file` line after the tpm `run` line)
- Test: `crates/agent-notify/tests/tmux_conf.rs`

**Interfaces:**
- Consumes: Task 5 CLI `focus-changed --socket --pid --start-time --pane --in|--out`, `jump`, `status --tmux`; Task 2 `WINDOW_STATE_OPTION = "@agent_notify_window_state"` and its values.
- Produces: the tmux hooks, the `M-n` binding, the `@agent_notify_marker` format, the window marker and the status segment.

The spec puts the hooks in `tmux.conf` itself. This plan puts them in `agent-notify.conf`, sourced from `tmux.conf`, so that `tmux.conf` stays readable and the guard can skip the whole file on a machine without the CLI.

- [ ] **Step 1: Write the failing test**

`crates/agent-notify/tests/tmux_conf.rs`:

```rust
//! The tracked agent-notify tmux file loads on a throwaway server and sets
//! the hooks, the binding and the status formats.

use std::path::PathBuf;

use dotfiles_test_support::tmux_server::Server;

fn conf() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.config/tmux/agent-notify.conf")
}

fn loaded(label: &str) -> Server {
    let server = Server::new(label);
    let dir = tempfile::tempdir().expect("a temp dir");
    server.new_session("main", dir.path());
    let path = conf();
    let sourced = server.tmux(&["source-file", path.to_str().expect("utf-8")]);
    assert!(sourced.status.success(), "{}", String::from_utf8_lossy(&sourced.stderr));
    server
}

#[test]
fn focus_hooks_report_in_and_out_with_the_server_identity() {
    let server = loaded("an-conf-hooks");
    let hooks = server.stdout(&["show-hooks", "-g"]);
    for direction in ["--in", "--out"] {
        let line = hooks
            .lines()
            .find(|line| line.contains("agent-notify focus-changed") && line.contains(direction))
            .unwrap_or_else(|| panic!("no focus hook with {direction} in:\n{hooks}"));
        for field in ["--socket", "#{q:socket_path}", "--pid", "#{pid}", "--start-time", "#{start_time}", "--pane", "#{pane_id}"] {
            assert!(line.contains(field), "{direction} hook lacks {field}: {line}");
        }
        assert!(line.contains("run-shell -b"), "the hook must not block tmux: {line}");
    }
}

#[test]
fn meta_n_jumps_without_blocking() {
    let server = loaded("an-conf-jump");
    let keys = server.stdout(&["list-keys", "-T", "root", "M-n"]);
    assert!(keys.contains("run-shell -b") && keys.contains("agent-notify jump"), "{keys}");
}

#[test]
fn window_formats_and_status_right_gain_the_marker_and_segment() {
    let server = loaded("an-conf-formats");
    for option in ["window-status-format", "window-status-current-format"] {
        let value = server.stdout(&["show-options", "-gv", option]);
        assert!(value.contains("@agent_notify_marker"), "{option}: {value}");
    }
    let right = server.stdout(&["show-options", "-gv", "status-right"]);
    assert!(right.contains("agent-notify status --tmux"), "{right}");
}

#[test]
fn the_marker_names_each_window_state() {
    let server = loaded("an-conf-marker");
    for (state, marker) in [("needs-you", " !"), ("failed", " x"), ("finished", " +"), ("working", " ~"), ("", "")] {
        let set = if state.is_empty() {
            server.tmux(&["set-option", "-wu", "-t", "main:", "@agent_notify_window_state"])
        } else {
            server.tmux(&["set-option", "-w", "-t", "main:", "@agent_notify_window_state", state])
        };
        assert!(set.status.success());
        let shown = server.stdout(&["display-message", "-p", "-t", "main:", "#{E:@agent_notify_marker}"]);
        assert_eq!(shown.trim_end_matches('\n'), marker, "state {state:?}");
    }
}

#[test]
fn sourcing_twice_does_not_duplicate_the_segment() {
    let server = loaded("an-conf-twice");
    let path = conf();
    let again = server.tmux(&["source-file", path.to_str().expect("utf-8")]);
    assert!(again.status.success());
    let right = server.stdout(&["show-options", "-gv", "status-right"]);
    assert_eq!(right.matches("agent-notify status --tmux").count(), 1, "{right}");
    let format = server.stdout(&["show-options", "-gv", "window-status-format"]);
    assert_eq!(format.matches("@agent_notify_marker").count(), 1, "{format}");
}
```

`config reload` re-sources `tmux.conf`, so a plain `set -ga` would append the segment again on every reload. The last test pins that.

If `Server::stdout` or `new_session` has another signature in plan 1 Task 13, adapt the calls; do not add a second harness. Add `tempfile` to the crate's dev-dependencies if plan 1 did not.

Run: `cd ~/crates && cargo test -p agent-notify --test tmux_conf`
Expected: failure, `source-file` cannot open the file.

- [ ] **Step 2: Write the tmux file**

`.config/tmux/agent-notify.conf`:

```tmux
# agent-notify: tmux side. Sourced from tmux.conf after tpm, because the nord
# theme sets the window-status formats and status-right when tpm runs, and
# anything appended earlier is overwritten.
#
# Every command runs with run-shell -b, so a slow or missing daemon never
# stalls tmux. The CLI exits quietly when the daemon is not running.

set-hook -g pane-focus-in[42] "run-shell -b '~/.local/bin/agent-notify focus-changed --socket #{q:socket_path} --pid #{pid} --start-time #{start_time} --pane #{pane_id} --in'"
set-hook -g pane-focus-out[42] "run-shell -b '~/.local/bin/agent-notify focus-changed --socket #{q:socket_path} --pid #{pid} --start-time #{start_time} --pane #{pane_id} --out'"

bind-key -n M-n run-shell -b '~/.local/bin/agent-notify jump'

# One marker per window: ! needs you, x failed, + finished, ~ working.
# Plain text, no colours: the theme's formats carry their own, and a colour
# here would bleed into the separator that follows.
set -g @agent_notify_marker '#{?#{==:#{@agent_notify_window_state},needs-you}, !,#{?#{==:#{@agent_notify_window_state},failed}, x,#{?#{==:#{@agent_notify_window_state},finished}, +,#{?#{==:#{@agent_notify_window_state},working}, ~,}}}}'

# Appended only once, so `config reload` does not stack copies.
if-shell -F '#{m:*agent_notify_marker*,#{window-status-format}}' '' \
    "set -ga window-status-format '#{E:@agent_notify_marker}'"
if-shell -F '#{m:*agent_notify_marker*,#{window-status-current-format}}' '' \
    "set -ga window-status-current-format '#{E:@agent_notify_marker}'"
if-shell -F '#{m:*agent-notify status*,#{status-right}}' '' \
    "set -ga status-right '#(~/.local/bin/agent-notify status --tmux)'"
```

The `if-shell -F` guards assume `#{window-status-format}` expands to the option's raw text, so the pattern can see the marker name. The `sourcing_twice_does_not_duplicate_the_segment` test checks that assumption. If it fails, replace each guard with a flag: `if-shell -F '#{@agent_notify_appended}' '' 'set -ga ... ; set -g @agent_notify_appended 1'`, and say so in the commit message.

The `[42]` index gives each hook a fixed slot, so a re-source replaces it instead of adding a second copy.

`@agent_notify_window_state` holds the state name Task 2 defines (`working`, `finished`, `needs-you`, `failed`, unset when idle). Task 9 writes it. The marker format maps those names to one character each.

Add to the end of `.config/tmux/tmux.conf`, after the `run '~/.tmux/plugins/tpm/tpm'` line:

```tmux

# agent-notify hooks and status (see that file). After tpm on purpose: tpm's
# `run` is synchronous, and the theme it loads overwrites the formats this
# file appends to. tpm's "last line" rule is about plugin declarations, which
# all sit above.
if-shell 'command -v ~/.local/bin/agent-notify >/dev/null' 'source-file -q ~/.config/tmux/agent-notify.conf'
```

- [ ] **Step 3: Run the tests**

Run: `cd ~/crates && cargo test -p agent-notify --test tmux_conf && cargo test -p tmux-tools --test conf_split`
Expected: all pass. `conf_split` loads the real `tmux.conf`; it must still pass with the new line.

- [ ] **Step 4: Check it on the live server**

Run `config reload`, then `tmux show-hooks -g | grep agent-notify` and `tmux show-options -gv status-right`. Expected: the two hooks and one copy of the segment. Run `config reload` again and check that the segment still appears once.

- [ ] **Step 5: Commit**

```bash
cd ~ && config add .config/tmux/agent-notify.conf .config/tmux/tmux.conf crates/agent-notify/tests/tmux_conf.rs crates/agent-notify/Cargo.toml crates/Cargo.lock
config diff --cached --name-only
config commit -m "Wire agent-notify into tmux: focus hooks, M-n jump, window marker, status segment" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- .config/tmux/agent-notify.conf .config/tmux/tmux.conf crates/agent-notify/tests/tmux_conf.rs crates/agent-notify/Cargo.toml crates/Cargo.lock
```

Leave `crates/agent-notify/Cargo.toml` and `crates/Cargo.lock` out of both commands if they did not change.

---

### Task 19: Retire the old Stop-hook notifier

The user approves this task before it starts. It deletes the only working notifier, so do it after Task 17's launch agent runs and Task 20's checks 1 to 5 pass.

**Files:**
- Delete: `.claude/hooks/notify.sh`, `crates/config-cli/tests/notify.rs`
- Modify: `crates/config-cli/tests/workflow_labels.rs:231-233` (doc comment), `.github/workflows/test-suite.yml:11-12` (comment), `tests/run-in-docker.sh:30` (comment), `deps/deps-mac.toml:6-7` (comment), `README-MAC.md` ("Claude Code notifications" section), `README-LINUX.md` ("Claude Code notifications" section)
- Not tracked, ask first: the `Stop` hook entry in `~/.claude/settings.json`

- [ ] **Step 1: Ask before the untracked change**

Ask the user: "Task 19 removes the old notifier. May I delete the `Stop` hook entry that calls `~/.claude/hooks/notify.sh` from `~/.claude/settings.json`? The agent-notify plugin hooks replace it." Do not edit `settings.json` without a yes. If the answer is no, stop this task: deleting `notify.sh` while settings still call it makes every turn end with a hook error.

- [ ] **Step 2: Remove the hook and its test**

```bash
cd ~ && rg -n "notify\.sh|notify\.test" --glob '!docs/superpowers/**' --glob '!.cache/**' --glob '!Library/**' crates .github tests deps .scripts README*.md .claude/hooks .claude/settings.json
```

Expected: the files listed above, and no others. If another file names `notify.sh`, add it to this task before you delete anything.

Delete the two files with `config rm .claude/hooks/notify.sh crates/config-cli/tests/notify.rs`. Remove the `Stop` entry from `~/.claude/settings.json` (after the yes in Step 1).

- [ ] **Step 3: Rewrite the comments that named it**

The macOS CI runner stays: the Alacritty app-bundle check still needs it. Change the reasons, not the matrix.

- `crates/config-cli/tests/workflow_labels.rs`: the doc comment on `the_matrix_includes_a_macos_runner` becomes `/// macOS reaches the Darwin-gated Alacritty app-bundle check in deps-manifest.test.sh. Deleting this entry once left a whole-file grep green.` (split over the same line width as its neighbours).
- `.github/workflows/test-suite.yml`: the first bullet becomes `#   - macOS reaches the Darwin-gated Alacritty app-bundle check in` / `#     deps-manifest.test.sh.`
- `tests/run-in-docker.sh:30`: replace "The notify tests are macOS-only" with the remaining macOS-only reason (read the full comment first, and keep its meaning for the Alacritty check).
- `deps/deps-mac.toml:6-7`: `# aerospace lets agent-notifyd raise the right Alacritty window when a` / `# card is clicked (tools/agent-notify). It is optional: without it the` / `# daemon activates Alacritty and tmux still switches pane.`
- `README-MAC.md`: replace the whole "Claude Code notifications" section with:

```markdown
## Claude Code notifications

`agent-notifyd` (`tools/agent-notify`) shows a card at the top right when a
Claude Code session finishes or needs you, and a click switches tmux to that
session's pane and raises Alacritty. The `agent-notify` plugin's hooks send
the events; agents can also call `agent-notify send` themselves.

Install or update it with `config build --tools`. It runs as the launch
agent `dev.dotfiles.agent-notifyd`. `agent-notify doctor` checks the setup,
`agent-notify dnd on` silences it, and Meta-n jumps to the newest card's
pane. Logs are in `~/Library/Logs/agent-notify/`.
```

- `README-LINUX.md`: replace its "Claude Code notifications" section with `Not wired up yet. tools/agent-notify runs on macOS only until its Linux plan lands. The CLI's fallback (notify-send) works on Pop!_OS COSMIC today.` Check that last sentence against plan 1's fallback task before you write it; if plan 1's fallback is not on Linux, drop the sentence.

The old README said `terminal-notifier` notifications were silently dropped on this machine. The spec says `terminal-notifier` worked on 2026-10-15. The new section does not mention `terminal-notifier`, so the conflict goes away; mention it in the commit body so the history keeps the note.

- [ ] **Step 4: Run the suites that read these files**

Run: `cd ~/crates && cargo test -p config-cli && cd ~ && tests/leak-check.sh`
Expected: pass. `cargo test -p config-cli` includes `workflow_labels` and the README checks.

- [ ] **Step 5: Commit**

```bash
cd ~ && config add crates/config-cli/tests/workflow_labels.rs .github/workflows/test-suite.yml tests/run-in-docker.sh deps/deps-mac.toml README-MAC.md README-LINUX.md
config diff --cached --name-only
config commit -m "Retire the Stop-hook notifier in favour of agent-notify" -m "notify.sh drove osascript notifications. agent-notifyd replaces it. The old README said terminal-notifier notifications were dropped on this machine; a check on 2026-10-15 found them working, and nothing here uses terminal-notifier now." -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- .claude/hooks/notify.sh crates/config-cli/tests/notify.rs crates/config-cli/tests/workflow_labels.rs .github/workflows/test-suite.yml tests/run-in-docker.sh deps/deps-mac.toml README-MAC.md README-LINUX.md
```

The two deleted paths are in the `--` list so the commit records their removal.

---

### Task 20: Manual smoke test on the live setup

No code. Run each check with the launch agent loaded (Task 17, after the user's yes) and the plugin hooks on. Write the results into `docs/superpowers/spikes/2026-10-15-agent-notify-spikes.md` under a new "Plan 2 smoke test" heading, one row per check: id, result, evidence. Commit that file.

- [ ] **Step 1: Daemon and setup**

1. `launchctl print gui/$(id -u)/dev.dotfiles.agent-notifyd | grep -E "state|pid"` shows `state = running`.
2. `agent-notify doctor` exits 0, and every line reads OK or names an optional tool.
3. `agent-notify status` shows the stamp from `~/Library/Application Support/agent-notify/installed-stamp`.
4. System Settings > General > Login Items lists `agent-notifyd` under "Allow in the Background". Record what macOS shows; if it warns the user, note the text.
5. `launchctl kickstart -k gui/$(id -u)/dev.dotfiles.agent-notifyd` restarts it. After `kill -9` of the pid, launchd starts it again (KeepAlive). After `agent-notify` reports it running, start a second copy by hand: it exits 0, and launchd does not loop (`launchctl print` shows one pid, run count rises by at most one).

- [ ] **Step 2: Cards and clicks**

6. Start a Claude Code session in a tmux window, give it a short task, and switch to another window. When it ends, an info card names the window and repo.
7. Ask the session for something that needs a permission prompt, then switch away. An amber attention card appears and the Glass sound plays once.
8. Click the attention card from another tmux window: tmux switches to the session's pane, and Alacritty comes to the front (also from a browser in front).
9. Detach the tmux client, then click a card: `open -n` starts a new Alacritty window attached to that session.
10. Hover the stack: the close buttons fade in within about 200 ms. Click ×: the card goes, and the pane does not change.
11. Meta-n jumps to the newest card's pane. The status segment and the window marker match the cards.

- [ ] **Step 3: Suppression and DND**

12. With the session's own pane focused in the front Alacritty window, a finished turn shows no card.
13. `agent-notify dnd on`: new events show no card and no sound. `agent-notify dnd status` reports on. `dnd off` shows the waiting cards.
14. Lock the screen with Ctrl-Cmd-Q, wait 10 s, unlock. Then run `ioreg -n Root -d1 -a | plutil -extract IOConsoleUsers xml1 -o - - | grep -A1 CGSSessionScreenIsLocked` while locked (from an SSH session or a `sleep 5;` prefix). Record the output: Task 8 parses `"CGSSessionScreenIsLocked"=Yes`, which no one has seen on this machine yet. If the shape differs, file a fix to `presence::parse_screen_locked` with that output as its test.

- [ ] **Step 4: The checks the spikes left open**

15. **Fullscreen (S1-4).** Put Safari in native fullscreen, then trigger a card. Record whether it shows. The spec accepts a failure.
16. **Intel (S1-13).** On the home Intel Mac, run `config build --tools`, then checks 1, 6, 8 and 10. Record the result. If the build fails there, record the error; do not hold this plan for it.
17. **Screen share (D5).** Start a screen share (a Zoom or Meet test call). The card shows in the share; the spike predicted this. Record it, and check that `dnd on` hides it.

- [ ] **Step 5: Commit the results**

```bash
cd ~ && config add docs/superpowers/spikes/2026-10-15-agent-notify-spikes.md
config commit -m "Record the agent-notify plan 2 smoke test" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- docs/superpowers/spikes/2026-10-15-agent-notify-spikes.md
```

Every failed check gets a fix task appended to this plan, or a line in plan 3 if it is Linux-only. Do not close the plan with a failed check that has neither.

