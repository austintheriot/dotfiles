# agent-notify Plan 1: Core Crate and CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the pure `agent-notify-core` crate and the `agent-notify` CLI, so Claude Code hooks tag tmux panes, build typed requests, and fall back to a plain OS notification for attention events while no daemon exists.

**Architecture:** Functional core, imperative shell. `agent-notify-core` holds every decision (wire types, hook mapping, labels, the daemon state machine, the view, the focus plan) as pure functions over plain data, with time passed in as `EpochMs`. The `agent-notify` CLI gathers facts (env, stdin, one tmux read, clock, randomness), calls one pure planning function, and then runs the plan (tmux tag writes, one socket request, an optional fallback notification).

**Tech Stack:** Rust 2024 (rustc 1.94), serde, serde_json, clap 4, tracing, tracing-subscriber, proptest. tmux 3.4 for integration tests.

**Spec:** `~/docs/superpowers/specs/2026-10-15-agent-notify-design.md` (approved, commit `534dcfad`). Read it before starting. This plan argues from it.

## Plan series

The spec covers four subsystems. Each gets its own plan, and each plan ends with working, tested software:

1. **This plan:** core crate, CLI (`hook`, `send`, `quiet`), Claude plugin, fallback notifications. `notify.sh` stays in place.
2. **Plan 2 (after spike 1):** spike 1 on macOS, then the Tauri daemon `agent-notifyd`, the remaining CLI commands (`focus-changed`, `dnd`, `jump`, `status`, `doctor`), tmux.conf hooks and status line, launchd, and the removal of `notify.sh` and its dependents.
3. **Plan 3:** spike 2 on COSMIC, then the Linux adapters and the systemd unit.

## Global Constraints

- Rust edition 2024. Every new crate sets `[lints] workspace = true`, so `unsafe_code = "forbid"`, `missing_docs = "warn"` and `clippy::all = "warn"` apply. The gates run with `-D warnings`: every `pub` item, field and variant needs a one-line `///` doc comment.
- Every binary starts with `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]` and answers `--stamp` with `option_env!("CONFIG_MANIFEST_STAMP").unwrap_or("unstamped")`.
- `agent-notify-core` performs no IO, reads no clock, and makes no random numbers. Time is `EpochMs` data. Randomness arrives as `[u8; 10]`.
- No single-letter names (except `i`, `j`, `k` and math values). No em dashes anywhere. No comments except a non-obvious why.
- Wire: tag field `type`, values `snake_case`, `WIRE_VERSION = 1`, accepted range `1..=1`. Unknown fields are ignored. Unknown variants decode to `Unknown`. Golden fixtures are append-only.
- `DisplayText`: at most 4096 bytes, cut on a character boundary. C0/C1 controls (except `\n`), bidi controls and zero-width characters are removed. Cleaned again on decode.
- `MAX_LINE_BYTES = 65_536` for one request line (see Deviations).
- `agent-notify hook claude-code` always exits 0, also after a panic. `send` and `quiet` exit 2 on a usage error with one stderr line.
- The CLI has 300 ms for connect, write and reply together.
- Fallback only for `NeedsInput`, `TurnFailed`, `Explicit`. Never for `TurnEnded`. macOS: `terminal-notifier -title -subtitle -message -activate org.alacritty`, never `-execute`, never a shell string. Linux: `notify-send` with `<`, `>` and `&` replaced by fullwidth look-alikes (spike report, section S2-4: COSMIC parses tags but shows entities literally).
- Timing constants (core `update.rs`): `INFO_SHOW_MS = 6_000`, `HOVER_GRACE_MS = 300`, `HOVER_RESUME_MS = 2_000`, `FOCUS_DWELL_MS = 1_000`, `SNOOZE_MS = 600_000`, `SOUND_GAP_MS = 3_000`, `SESSION_GONE_MS = 3_000`, `NOT_RAISED_MS = 2_000`, `IDLE_LIMIT_SECONDS = 30`. Quiet default `--max-minutes 120`.
- Pane options: `@agent_notify_session`, `@agent_notify_prompt`. Every tmux call passes `-S <socket path>` and targets a `%pane_id`.
- The `config` repo is public. No employer, product or ticket names in code, fixtures or commit messages. The pre-commit leak check scans every staged file.
- Commit only your own paths: `config add <paths>` then `config commit -m "<subject>" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- <paths>`. Never `--no-verify`.
- Minimum macOS version: 26.

## Review Focus

These inputs are implied by the spec but no rule table names them. Each has a test in the task named.

1. **A request built from maximal input exceeds the line limit.** Six 4 KB text fields plus a 4 KB `cwd`, JSON-escaped, must still encode under `MAX_LINE_BYTES`, or the daemon rejects a legitimate notification. Test: Task 15, `largest_possible_request_fits_one_line`.
2. **A window name contains the tmux field separator or a newline.** The pane read must still parse, and the name must keep its text. Test: Task 16, `window_name_with_separator_and_newline_parses`.
3. **`$TMUX` names a tmux server that is gone.** The hook must exit 0 within the budget and still send the fallback for a permission prompt. Test: Task 17, `dead_tmux_server_still_falls_back`.
4. **A hook payload has a malformed `session_id`.** No request, the reason is logged, exit 0. Test: Task 15, `malformed_session_id_sends_nothing`.
5. **Agent text starts with `-` and reaches terminal-notifier.** terminal-notifier reads its options through the Cocoa argument domain, which treats a leading `-` as an option name. Test: Task 17, `leading_dash_is_guarded` (unit) and `fallback_passes_text_as_single_arguments` (process).

## Deviations from the spec

Tell the user about these at handoff. Each one closes a gap the spec did not cover.

- `MAX_LINE_BYTES` is 64 KB, not 16 KB. Six 4 KB `DisplayText` fields plus `cwd` exceed 16 KB after JSON escaping.
- `TurnEnded` for the same prompt clears a `NeedsInput` card. Without it, a "needs permission" card outlives the turn the user already answered.
- `prompt_id` is optional in Claude Code payloads (absent before the first prompt), so every `Turn` variant holds `Option<PromptId>`.
- The plugin's `Notification` matcher lists only the four mapped types, so `idle_prompt` never starts the binary. The spec said "logged".
- The `SessionEnd` hook runs synchronously. An async hook at process exit can be dropped.
- `Effect::Log` is dropped. The daemon shell logs every input and effect.
- `UiIntent::HoverChanged` carries no `card_id`. `LayoutChanged` and `UiError` belong to the app shell in plan 2, not the core.
- The leak check needs no new path. It already scans every staged file.

## File map

Create:

- `crates/agent-notify-core/Cargo.toml`
- `crates/agent-notify-core/src/lib.rs`: module list and re-exports
- `crates/agent-notify-core/src/text.rs`: `DisplayText`
- `crates/agent-notify-core/src/ids.rs`: `EpochMs`, checked identifiers, `ServerId`, `PaneKey`, `SocketPath`
- `crates/agent-notify-core/src/wire.rs`: `Request`, `Origin`, `AgentEvent`, `Control`, `Query`, `Reply`, `decode_request`
- `crates/agent-notify-core/src/hook.rs`: Claude hook payload and `decide_hook`
- `crates/agent-notify-core/src/label.rs`: `location_line`, `first_sentence`, card bodies
- `crates/agent-notify-core/src/paths.rs`: socket and log locations
- `crates/agent-notify-core/src/state.rs`: `DaemonState` and its records
- `crates/agent-notify-core/src/update.rs`: inputs, effects, `update`, `merge`
- `crates/agent-notify-core/src/view.rs`: `PanelViewModel`, `view`
- `crates/agent-notify-core/src/focus.rs`: `FocusWorld`, `FocusPlan`, `plan_focus`
- `crates/agent-notify-core/src/fallback.rs`: `FallbackNote`, `fallback_note`
- `crates/agent-notify-core/src/test_support.rs`: `#[cfg(test)]` builders
- `crates/agent-notify-core/tests/fixtures/wire-v1.jsonl`, `reply-v1.jsonl`
- `crates/agent-notify-core/tests/wire_fixtures.rs`, `tests/properties.rs`
- `crates/agent-notify/Cargo.toml`
- `crates/agent-notify/src/main.rs`: composition root and execution
- `crates/agent-notify/src/cli.rs`: clap types
- `crates/agent-notify/src/invocation.rs`: pure `plan_invocation`
- `crates/agent-notify/src/context.rs`: gathers `Inputs`
- `crates/agent-notify/src/tmux.rs`: pane read and tag writes
- `crates/agent-notify/src/client.rs`: socket request with budget
- `crates/agent-notify/src/fallback.rs`: notifier processes
- `crates/agent-notify/src/log.rs`: file logging
- `crates/agent-notify/tests/process.rs`, `tests/tmux_tags.rs`
- `crates/dotfiles-test-support/src/tmux_server.rs` (moved), `crates/dotfiles-test-support/tests/tmux_server.rs` (moved)
- `crates/tmux-tools/src/lib.rs`
- `tools/agent-notify/.claude-plugin/marketplace.json`
- `tools/agent-notify/claude-plugin/.claude-plugin/plugin.json`
- `tools/agent-notify/claude-plugin/hooks/hooks.json`
- `tools/agent-notify/claude-plugin/skills/agent-notify/SKILL.md`
- `tools/agent-notify/tests/plugin.test.ts`

Modify:

- `crates/Cargo.toml`: members, workspace dependencies
- `crates/dotfiles-test-support/Cargo.toml`, `src/lib.rs`
- `crates/tmux-tools/Cargo.toml`, `src/main.rs`, the 12 test files that declare `mod support;`
- `deps/deps-mac.toml`, `deps/deps-linux.toml`
- `.claude/skills/monitor-ci/SKILL.md`

Delete: `crates/tmux-tools/tests/support/mod.rs`, `crates/tmux-tools/tests/support_fixture.rs` (both moved).

---

### Task 0: Confirm hook payload fields and `$TMUX_PANE`

A check, not code. Section 5.2 of the spec depends on `$TMUX_PANE` in hook and Bash environments. The decoder in Task 4 depends on the field names. Spike 1 confirmed every expectation below on Claude Code 2.1.301 (`docs/superpowers/spikes/2026-10-15-agent-notify-spikes.md`, section S1-H). If `claude --version` still prints 2.1.301, skip to Task 1. Otherwise run the steps. If any expectation fails, stop and report to the user before Task 1.

**Files:** none committed. Scratch only.

- [ ] **Step 1: Write a capture settings file**

Run inside a tmux pane:

```bash
export CAPTURE=$(mktemp -d /tmp/an-capture.XXXXXX)
cat > "$CAPTURE/settings.json" <<'JSON'
{
  "hooks": {
    "SessionStart":     [{ "hooks": [{ "type": "command", "command": "{ cat; echo; env | grep '^TMUX'; echo ---; } >> \"$CAPTURE/hooks.log\"" }] }],
    "UserPromptSubmit": [{ "hooks": [{ "type": "command", "command": "{ cat; echo; env | grep '^TMUX'; echo ---; } >> \"$CAPTURE/hooks.log\"" }] }],
    "Stop":             [{ "hooks": [{ "type": "command", "command": "{ cat; echo; env | grep '^TMUX'; echo ---; } >> \"$CAPTURE/hooks.log\"", "async": true }] }],
    "SessionEnd":       [{ "hooks": [{ "type": "command", "command": "{ cat; echo; env | grep '^TMUX'; echo ---; } >> \"$CAPTURE/hooks.log\"" }] }]
  }
}
JSON
```

- [ ] **Step 2: Run one headless session**

```bash
claude -p 'Run the shell command: echo "pane=$TMUX_PANE"' --settings "$CAPTURE/settings.json" --allowedTools 'Bash(echo:*)'
sleep 2; cat "$CAPTURE/hooks.log"
```

Expected:
- The Bash output prints `pane=%<digits>`.
- Every payload has `session_id`, `cwd`, `hook_event_name`, and a `TMUX_PANE=%<digits>` line follows it.
- `UserPromptSubmit` and `Stop` payloads have `prompt_id`, and the two values are equal.
- `SessionStart` has `source`, and no `prompt_id` and no `session_title`. `Stop` has `last_assistant_message`.

- [ ] **Step 3: Record and clean up**

Write the observed field list into the task report. Do not commit the log, because it holds real session content. Then `rm -rf "$CAPTURE"`.

---

### Task 1: Scaffold the core crate with `DisplayText`

**Files:**
- Modify: `crates/Cargo.toml`
- Create: `crates/agent-notify-core/Cargo.toml`, `src/lib.rs`, `src/text.rs`

**Interfaces:**
- Produces: `agent_notify_core::DisplayText` with `new(&str) -> DisplayText`, `as_str(&self) -> &str`, `is_blank(&self) -> bool`; `DISPLAY_TEXT_MAX_BYTES: usize = 4096`. Serde: transparent string, cleaned on decode.

- [ ] **Step 1: Add the crate to the workspace**

In `crates/Cargo.toml`, replace the members line (keep it on one line, `config-stamp` reads it with sed):

```toml
members = ["agent-notify", "agent-notify-core", "config-cli", "config-manifest", "deps-core", "dotfiles-path", "dotfiles-test-support", "tmux-core", "tmux-tools"]
```

`agent-notify` does not exist until Task 15. Until then, add only `"agent-notify-core"` and add `"agent-notify"` in Task 15.

Append to `[workspace.dependencies]`:

```toml
# The socket protocol and the Claude hook payload are JSON.
serde_json = "1.0.154"
# Property tests for the decision core and the CLI's pure planner.
proptest = "1.12.0"
tracing = "0.1.44"
# fmt only: the CLI writes one log file, and env-filter would add a regex engine for one level setting.
tracing-subscriber = { version = "0.3.23", default-features = false, features = ["fmt", "std"] }
```

- [ ] **Step 2: Create the manifest and lib**

`crates/agent-notify-core/Cargo.toml`:

```toml
[package]
name = "agent-notify-core"
edition = "2024"
version = "0.1.0"
publish = false

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }

[dev-dependencies]
proptest = { workspace = true }

[lints]
workspace = true
```

`crates/agent-notify-core/src/lib.rs`:

```rust
//! Pure decisions for agent-notify: wire types, hook mapping, labels, and the
//! daemon's state machine. No IO, no clock, no randomness: time arrives as
//! `EpochMs` data, so every rule is testable with plain values.

pub mod text;

pub use text::DisplayText;
```

- [ ] **Step 3: Write the failing tests**

`crates/agent-notify-core/src/text.rs`, tests first (the type below them does not exist yet):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn removes_bidi_overrides_and_zero_width_characters() {
        let text = DisplayText::new("safe\u{202E}txt.exe\u{200B}\u{FEFF}");
        assert_eq!(text.as_str(), "safetxt.exe");
    }

    #[test]
    fn removes_terminal_control_characters() {
        assert_eq!(DisplayText::new("a\u{1b}[31mred\u{7}\u{9b}").as_str(), "a[31mred");
    }

    #[test]
    fn keeps_newlines_and_non_latin_text() {
        assert_eq!(DisplayText::new("完了。\nnext").as_str(), "完了。\nnext");
    }

    #[test]
    fn cuts_on_a_character_boundary() {
        let text = DisplayText::new(&"é".repeat(3000));
        assert_eq!(text.as_str().len(), DISPLAY_TEXT_MAX_BYTES);
        assert!(text.as_str().chars().all(|character| character == 'é'));
    }

    #[test]
    fn decoding_cleans_again() {
        let text: DisplayText = serde_json::from_str("\"a\\u202eb\"").expect("decodes");
        assert_eq!(text.as_str(), "ab");
    }

    #[test]
    fn blank_means_only_whitespace() {
        assert!(DisplayText::new(" \n ").is_blank());
        assert!(!DisplayText::new(" x ").is_blank());
    }

    fn long_strings() -> impl Strategy<Value = String> {
        proptest::collection::vec(any::<char>(), 0..6000).prop_map(String::from_iter)
    }

    proptest! {
        #[test]
        fn output_is_a_bounded_clean_prefix_of_the_cleaned_input(raw in long_strings()) {
            let text = DisplayText::new(&raw);
            let cleaned: String = raw.chars().filter(|character| !is_removed(*character)).collect();
            prop_assert!(text.as_str().len() <= DISPLAY_TEXT_MAX_BYTES);
            prop_assert!(cleaned.starts_with(text.as_str()));
            prop_assert!(!text.as_str().chars().any(is_removed));
        }
    }
}
```

- [ ] **Step 4: Run the tests to see them fail**

Run: `cd ~/crates && cargo test -p agent-notify-core`
Expected: compile errors, `cannot find type DisplayText`.

- [ ] **Step 5: Implement**

Put above the tests in `text.rs`:

```rust
//! Text that came from an agent or a hook and will be shown on screen.

use serde::{Deserialize, Serialize};

/// The largest [`DisplayText`], in bytes.
pub const DISPLAY_TEXT_MAX_BYTES: usize = 4096;

/// Agent-supplied text after cleaning. Never markup, never part of a command.
///
/// Decoding runs the same cleaning, so a daemon never holds text that skipped it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub struct DisplayText(String);

impl DisplayText {
    /// Removes control, bidi and zero-width characters, then cuts to
    /// [`DISPLAY_TEXT_MAX_BYTES`] on a character boundary.
    #[must_use]
    pub fn new(raw: &str) -> Self {
        let mut cleaned = String::with_capacity(raw.len().min(DISPLAY_TEXT_MAX_BYTES));
        for character in raw.chars().filter(|character| !is_removed(*character)) {
            if cleaned.len() + character.len_utf8() > DISPLAY_TEXT_MAX_BYTES {
                break;
            }
            cleaned.push(character);
        }
        DisplayText(cleaned)
    }

    /// The cleaned text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// True when the text holds only whitespace.
    #[must_use]
    pub fn is_blank(&self) -> bool {
        self.0.trim().is_empty()
    }
}

impl From<String> for DisplayText {
    fn from(raw: String) -> Self {
        DisplayText::new(&raw)
    }
}

impl From<DisplayText> for String {
    fn from(text: DisplayText) -> Self {
        text.0
    }
}

/// `\n` survives because a card body's first sentence ends at a line break.
fn is_removed(character: char) -> bool {
    (character.is_control() && character != '\n')
        || matches!(
            character,
            '\u{061C}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
        )
}
```

- [ ] **Step 6: Run the tests and clippy**

Run: `cd ~/crates && cargo test -p agent-notify-core && cargo clippy -p agent-notify-core --all-targets -- -D warnings`
Expected: 7 tests pass, no warnings.

- [ ] **Step 7: Commit**

```bash
cd ~ && config add crates/Cargo.toml crates/Cargo.lock crates/agent-notify-core
config commit -m "Add agent-notify-core with DisplayText" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/Cargo.toml crates/Cargo.lock crates/agent-notify-core
```

---

### Task 2: Identifiers and time

**Files:**
- Create: `crates/agent-notify-core/src/ids.rs`, `src/test_support.rs`
- Modify: `crates/agent-notify-core/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `EpochMs(pub i64)`: `Copy`, `Ord`; `plus_ms(self, i64) -> EpochMs`; `since(self, earlier: EpochMs) -> i64`.
  - Checked string ids, each with `parse(&str) -> Result<Self, InvalidId>`, `as_str(&self) -> &str`, `Display`, serde as a string: `SessionId`, `PromptId` (1 to 128 of `[A-Za-z0-9_-]`), `PaneId` (`^%[0-9]+$`, plus `number(&self) -> u32`), `EventId` (26 Crockford base32 characters, plus `from_parts(EpochMs, [u8; 10]) -> EventId`), `CardId` (`card-` + token, plus `for_event(&EventId) -> CardId`).
  - `SocketPath`: absolute, no NUL; `parse(&Path) -> Result<Self, InvalidId>`, `as_path(&self) -> &Path`.
  - `ServerId { socket_path: SocketPath, pid: u32, start_time: i64 }`, `PaneKey { server: ServerId, pane: PaneId }`.
  - `InvalidId { kind: &'static str, value: String }`.
  - Test builders (crate-private, `#[cfg(test)]`): `event_id(i64)`, `session(&str)`, `prompt(&str)`, `server()`, `pane(u32)`.

- [ ] **Step 1: Register the modules**

`lib.rs` becomes:

```rust
//! Pure decisions for agent-notify: wire types, hook mapping, labels, and the
//! daemon's state machine. No IO, no clock, no randomness: time arrives as
//! `EpochMs` data, so every rule is testable with plain values.

pub mod ids;
pub mod text;

#[cfg(test)]
pub(crate) mod test_support;

pub use ids::{
    CardId, EpochMs, EventId, InvalidId, PaneId, PaneKey, PromptId, ServerId, SessionId,
    SocketPath,
};
pub use text::DisplayText;
```

`src/test_support.rs`:

```rust
//! Builders for unit tests. Panics are fine here: a bad literal is a test bug.

use std::path::Path;

use crate::{EpochMs, EventId, PaneId, PaneKey, PromptId, ServerId, SessionId, SocketPath};

pub fn event_id(number: i64) -> EventId {
    EventId::from_parts(EpochMs(number), [0; 10])
}

pub fn session(name: &str) -> SessionId {
    SessionId::parse(name).expect("a valid session id literal")
}

pub fn prompt(name: &str) -> PromptId {
    PromptId::parse(name).expect("a valid prompt id literal")
}

pub fn server() -> ServerId {
    ServerId {
        socket_path: SocketPath::parse(Path::new("/tmp/tmux-501/default")).expect("absolute"),
        pid: 100,
        start_time: 1_700_000_000,
    }
}

pub fn pane(number: u32) -> PaneKey {
    PaneKey { server: server(), pane: PaneId::parse(&format!("%{number}")).expect("a pane id") }
}
```

- [ ] **Step 2: Write the failing tests**

In `src/ids.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_ids_accept_uuid_text_and_reject_other_characters() {
        assert!(SessionId::parse("0b6e8a8c-1f2d-4c3b-9a7e-5d4c3b2a1f00").is_ok());
        assert!(SessionId::parse("").is_err());
        assert!(SessionId::parse("a b").is_err());
        assert!(SessionId::parse("a;rm").is_err());
        assert!(SessionId::parse(&"x".repeat(129)).is_err());
    }

    #[test]
    fn pane_ids_need_a_percent_and_digits() {
        assert_eq!(PaneId::parse("%12").map(|pane| pane.number()), Ok(12));
        for bad in ["12", "%", "%1a", "%-1", "%%1"] {
            assert!(PaneId::parse(bad).is_err(), "{bad} must be rejected");
        }
    }

    #[test]
    fn event_ids_encode_time_first() {
        assert_eq!(
            EventId::from_parts(EpochMs(0), [0; 10]).as_str(),
            "00000000000000000000000000"
        );
        assert_eq!(
            EventId::from_parts(EpochMs(1), [0; 10]).as_str(),
            "00000000010000000000000000"
        );
        let earlier = EventId::from_parts(EpochMs(1_760_000_000_000), [255; 10]);
        let later = EventId::from_parts(EpochMs(1_760_000_000_001), [0; 10]);
        assert!(earlier < later);
    }

    #[test]
    fn event_ids_round_trip_through_parse() {
        let id = EventId::from_parts(EpochMs(1_760_000_000_000), [9; 10]);
        assert_eq!(EventId::parse(id.as_str()), Ok(id.clone()));
        assert!(EventId::parse("0000000000000000000000000I").is_err());
    }

    #[test]
    fn card_ids_derive_from_event_ids() {
        let event = EventId::from_parts(EpochMs(5), [1; 10]);
        let card = CardId::for_event(&event);
        assert_eq!(card.as_str(), format!("card-{}", event.as_str()));
        assert!(CardId::parse(card.as_str()).is_ok());
    }

    #[test]
    fn socket_paths_must_be_absolute_without_nul() {
        assert!(SocketPath::parse(Path::new("/tmp/tmux-501/default")).is_ok());
        assert!(SocketPath::parse(Path::new("tmux-501/default")).is_err());
        assert!(SocketPath::parse(Path::new("/tmp/a\0b")).is_err());
    }

    #[test]
    fn ids_decode_with_the_same_checks() {
        assert!(serde_json::from_str::<PaneId>("\"%3\"").is_ok());
        assert!(serde_json::from_str::<PaneId>("\"3\"").is_err());
        assert!(serde_json::from_str::<SocketPath>("\"relative\"").is_err());
    }

    #[test]
    fn epoch_arithmetic_saturates() {
        assert_eq!(EpochMs(i64::MAX).plus_ms(1), EpochMs(i64::MAX));
        assert_eq!(EpochMs(10).since(EpochMs(4)), 6);
    }
}
```

- [ ] **Step 3: Run the tests to see them fail**

Run: `cd ~/crates && cargo test -p agent-notify-core ids`
Expected: compile errors for the missing types.

- [ ] **Step 4: Implement**

Above the tests in `src/ids.rs`:

```rust
//! Identifiers and timestamps that cross the socket. Each identifier is
//! checked when it is built, so holding one proves its format.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Milliseconds since the Unix epoch. The only form time takes in this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EpochMs(pub i64);

impl EpochMs {
    /// This time plus `milliseconds`, saturating.
    #[must_use]
    pub fn plus_ms(self, milliseconds: i64) -> Self {
        EpochMs(self.0.saturating_add(milliseconds))
    }

    /// Milliseconds from `earlier` to this time. Negative when `earlier` is later.
    #[must_use]
    pub fn since(self, earlier: EpochMs) -> i64 {
        self.0.saturating_sub(earlier.0)
    }
}

/// A value failed the format check of the identifier it was meant to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidId {
    /// Which identifier.
    pub kind: &'static str,
    /// The rejected text, cut to 64 characters.
    pub value: String,
}

impl fmt::Display for InvalidId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} has an invalid format: {:?}", self.kind, self.value)
    }
}

impl std::error::Error for InvalidId {}

fn invalid(kind: &'static str, value: &str) -> InvalidId {
    InvalidId { kind, value: value.chars().take(64).collect() }
}

macro_rules! checked_id {
    ($(#[$doc:meta])* $name:ident, $kind:literal, $check:expr) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// Checks `value` and wraps it.
            ///
            /// # Errors
            ///
            /// Returns [`InvalidId`] when `value` does not have this identifier's format.
            pub fn parse(value: &str) -> Result<Self, InvalidId> {
                let check: fn(&str) -> bool = $check;
                if check(value) { Ok($name(value.to_string())) } else { Err(invalid($kind, value)) }
            }

            /// The identifier as text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = InvalidId;
            fn try_from(value: String) -> Result<Self, InvalidId> {
                $name::parse(&value)
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> String {
                id.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

fn is_token(value: &str) -> bool {
    (1..=128).contains(&value.len())
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const ULID_TIME_MASK: u128 = (1 << 48) - 1;

checked_id!(
    /// A Claude Code `session_id`.
    SessionId, "session id", is_token
);
checked_id!(
    /// A Claude Code `prompt_id`. One per user turn.
    PromptId, "prompt id", is_token
);
checked_id!(
    /// A tmux pane id such as `%12`.
    PaneId, "pane id",
    |value| value.len() >= 2 && value.starts_with('%') && value[1..].bytes().all(|byte| byte.is_ascii_digit())
);
checked_id!(
    /// A ULID that names one CLI invocation in every log line.
    EventId, "event id",
    |value| value.len() == 26 && value.bytes().all(|byte| CROCKFORD.contains(&byte))
);
checked_id!(
    /// One card instance. Fresh for every card, never reused.
    CardId, "card id",
    |value| value.strip_prefix("card-").is_some_and(is_token)
);

impl PaneId {
    /// The number after `%`. Used only to order panes.
    #[must_use]
    pub fn number(&self) -> u32 {
        self.0[1..].parse().unwrap_or(u32::MAX)
    }
}

impl EventId {
    /// Builds a ULID from a timestamp and 80 random bits. Later times sort later.
    #[must_use]
    pub fn from_parts(time: EpochMs, random: [u8; 10]) -> Self {
        let millis = u128::try_from(time.0).unwrap_or(0) & ULID_TIME_MASK;
        let random_bits = random.iter().fold(0_u128, |bits, byte| (bits << 8) | u128::from(*byte));
        let value = (millis << 80) | random_bits;
        let encoded = (0..26)
            .rev()
            .map(|position| char::from(CROCKFORD[((value >> (position * 5)) & 0x1f) as usize]))
            .collect();
        EventId(encoded)
    }
}

impl CardId {
    /// The card created by `event`. Event ids are unique, so card ids are too.
    #[must_use]
    pub fn for_event(event: &EventId) -> Self {
        CardId(format!("card-{}", event.as_str()))
    }
}

/// An absolute socket path with no NUL byte.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "PathBuf", into = "PathBuf")]
pub struct SocketPath(PathBuf);

impl SocketPath {
    /// Checks `path` and wraps it.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidId`] for a relative path or a path with a NUL byte.
    pub fn parse(path: &Path) -> Result<Self, InvalidId> {
        let has_nul = path.as_os_str().as_encoded_bytes().contains(&0);
        if path.is_absolute() && !has_nul {
            Ok(SocketPath(path.to_path_buf()))
        } else {
            Err(invalid("socket path", &path.display().to_string()))
        }
    }

    /// The path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl TryFrom<PathBuf> for SocketPath {
    type Error = InvalidId;
    fn try_from(path: PathBuf) -> Result<Self, InvalidId> {
        SocketPath::parse(&path)
    }
}

impl From<SocketPath> for PathBuf {
    fn from(path: SocketPath) -> PathBuf {
        path.0
    }
}

/// One tmux server. Pane ids restart at `%0` on a new server with the same
/// socket path, so the pid and start time are part of the identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ServerId {
    /// The server's socket path.
    pub socket_path: SocketPath,
    /// `#{pid}`.
    pub pid: u32,
    /// `#{start_time}`, seconds since the epoch.
    pub start_time: i64,
}

/// One pane on one tmux server.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PaneKey {
    /// The server.
    pub server: ServerId,
    /// The pane on that server.
    pub pane: PaneId,
}
```

- [ ] **Step 5: Run the tests and clippy**

Run: `cd ~/crates && cargo test -p agent-notify-core && cargo clippy -p agent-notify-core --all-targets -- -D warnings`
Expected: all pass. If clippy flags `test_support` items as unused, that is expected until Task 7; add `#![allow(dead_code)]` at the top of `test_support.rs` with the comment `// Each builder is used by some test module, not by all of them.`

- [ ] **Step 6: Commit**

```bash
cd ~ && config add crates/agent-notify-core
config commit -m "Add checked identifiers and EpochMs to agent-notify-core" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/agent-notify-core
```

---

### Task 3: Wire protocol and golden fixtures

**Files:**
- Create: `crates/agent-notify-core/src/wire.rs`, `tests/wire_fixtures.rs`, `tests/fixtures/wire-v1.jsonl`, `tests/fixtures/reply-v1.jsonl`
- Modify: `src/lib.rs` (add `pub mod wire;`)

**Interfaces:**
- Consumes: Task 1 `DisplayText`; Task 2 ids, `EpochMs`, `PaneKey`.
- Produces (all in `agent_notify_core::wire`):
  - Constants `WIRE_VERSION: u16 = 1`, `OLDEST_ACCEPTED_VERSION: u16 = 1`, `MAX_LINE_BYTES: usize = 65_536`.
  - `Request::{Agent { v, event_id, sent_at_ms, origin, event }, Control { v, event_id, sent_at_ms, control }, Query { v, event_id, query }, Unknown}`.
  - `Origin { agent: AgentKind, session: Option<SessionId>, prompt: Option<PromptId>, pane: Option<PaneKey>, window: Option<WindowLabel>, repo: Option<RepoInfo>, session_title: Option<DisplayText>, cwd: PathBuf }`.
  - `AgentKind::{ClaudeCode, Unknown}`, `WindowLabel { index: u32, name: DisplayText }`, `RepoInfo { main_repo_name: DisplayText, head: RepoHead }`, `RepoHead::{Branch { name }, Detached { short_sha }}`.
  - `AgentEvent::{SessionStarted { source }, TurnStarted, TurnEnded { last_message: Option<DisplayText> }, TurnFailed { error }, NeedsInput { reason, message }, InputResolved, Explicit { kind, body }, Quiet { until, max_minutes: u16 }, QuietOff, SessionEnded, Unknown}`.
  - `Copy` enums: `Kind::{Info, Attention}` (`Ord`, `Attention > Info`), `SessionSource::{Startup, Resume, Clear, Compact, Fork, Unknown}`, `InputReason::{Permission, Elicitation, Unknown}`, `QuietUntil::{NextSend, NextPrompt}`.
  - `Control::{FocusChanged { pane: PaneKey, focused: bool }, Dnd { command: DndCommand }, Unknown}`, `DndCommand::{On { for_minutes: Option<u16> }, Off}`, `Query::{Status, ViewModel, Unknown}`.
  - `Reply { ok, daemon, code, accepts, data }` with `Reply::accepted(&str)`, `Reply::rejected(ReplyCode)`; `ReplyCode::{BadRequest, UnsupportedVersion, UnknownKind, LineTooLong, Unknown}`.
  - `decode_request(&str) -> Result<Request, DecodeError>`; `DecodeError::{LineTooLong { bytes }, Malformed { message }, UnsupportedVersion { version }, UnknownKind}` with `reply_code(&self) -> ReplyCode`.
  - `encode_line<T: Serialize>(&T) -> Result<String, serde_json::Error>` (JSON plus `\n`).

- [ ] **Step 1: Write the fixtures**

`crates/agent-notify-core/tests/fixtures/wire-v1.jsonl` (one request per line; append-only from now on):

```
{"type":"agent","v":1,"event_id":"00000000000000000000000001","sent_at_ms":1760500000000,"origin":{"agent":"claude_code","session":"session-a","prompt":"prompt-1","pane":{"server":{"socket_path":"/tmp/tmux-501/default","pid":1799,"start_time":1760400000},"pane":"%12"},"window":{"index":4,"name":"code/feature-x"},"repo":{"main_repo_name":"code","head":{"type":"branch","name":"feature-x"}},"session_title":"Fix sync retry","cwd":"/home/dev/code"},"event":{"type":"session_started","source":"startup"}}
{"type":"agent","v":1,"event_id":"00000000000000000000000002","sent_at_ms":1760500000001,"origin":{"agent":"claude_code","session":"session-a","prompt":"prompt-1","cwd":"/home/dev/code"},"event":{"type":"turn_started"}}
{"type":"agent","v":1,"event_id":"00000000000000000000000003","sent_at_ms":1760500000002,"origin":{"agent":"claude_code","session":"session-a","prompt":"prompt-1","cwd":"/home/dev/code"},"event":{"type":"turn_ended","last_message":"Done. All tests pass."}}
{"type":"agent","v":1,"event_id":"00000000000000000000000004","sent_at_ms":1760500000003,"origin":{"agent":"claude_code","session":"session-a","cwd":"/home/dev/code"},"event":{"type":"turn_ended"}}
{"type":"agent","v":1,"event_id":"00000000000000000000000005","sent_at_ms":1760500000004,"origin":{"agent":"claude_code","session":"session-a","prompt":"prompt-1","cwd":"/home/dev/code"},"event":{"type":"turn_failed","error":"API Error: Rate limit reached"}}
{"type":"agent","v":1,"event_id":"00000000000000000000000006","sent_at_ms":1760500000005,"origin":{"agent":"claude_code","session":"session-a","prompt":"prompt-1","cwd":"/home/dev/code"},"event":{"type":"needs_input","reason":"permission","message":"Claude needs your permission to use Bash"}}
{"type":"agent","v":1,"event_id":"00000000000000000000000007","sent_at_ms":1760500000006,"origin":{"agent":"claude_code","session":"session-a","prompt":"prompt-1","cwd":"/home/dev/code"},"event":{"type":"needs_input","reason":"elicitation","message":"Which branch?"}}
{"type":"agent","v":1,"event_id":"00000000000000000000000008","sent_at_ms":1760500000007,"origin":{"agent":"claude_code","session":"session-a","prompt":"prompt-1","cwd":"/home/dev/code"},"event":{"type":"input_resolved"}}
{"type":"agent","v":1,"event_id":"00000000000000000000000009","sent_at_ms":1760500000008,"origin":{"agent":"claude_code","session":"session-a","prompt":"prompt-1","cwd":"/home/dev/code"},"event":{"type":"explicit","kind":"attention","body":"CI is red: 2 jobs failed"}}
{"type":"agent","v":1,"event_id":"0000000000000000000000000A","sent_at_ms":1760500000009,"origin":{"agent":"claude_code","session":"session-a","prompt":"prompt-1","cwd":"/home/dev/code"},"event":{"type":"quiet","until":"next_send","max_minutes":180}}
{"type":"agent","v":1,"event_id":"0000000000000000000000000B","sent_at_ms":1760500000010,"origin":{"agent":"claude_code","session":"session-a","cwd":"/home/dev/code"},"event":{"type":"quiet_off"}}
{"type":"agent","v":1,"event_id":"0000000000000000000000000C","sent_at_ms":1760500000011,"origin":{"agent":"claude_code","session":"session-a","cwd":"/home/dev/code"},"event":{"type":"session_ended"}}
{"type":"agent","v":1,"event_id":"0000000000000000000000000D","sent_at_ms":1760500000012,"origin":{"agent":"claude_code","cwd":"/home/dev/notes"},"event":{"type":"explicit","kind":"info","body":"Report written"}}
{"type":"control","v":1,"event_id":"0000000000000000000000000E","sent_at_ms":1760500000013,"control":{"type":"focus_changed","pane":{"server":{"socket_path":"/tmp/tmux-501/default","pid":1799,"start_time":1760400000},"pane":"%12"},"focused":true}}
{"type":"control","v":1,"event_id":"0000000000000000000000000F","sent_at_ms":1760500000014,"control":{"type":"dnd","command":{"type":"on","for_minutes":30}}}
{"type":"control","v":1,"event_id":"0000000000000000000000000G","sent_at_ms":1760500000015,"control":{"type":"dnd","command":{"type":"on"}}}
{"type":"control","v":1,"event_id":"0000000000000000000000000H","sent_at_ms":1760500000016,"control":{"type":"dnd","command":{"type":"off"}}}
{"type":"query","v":1,"event_id":"0000000000000000000000000J","query":"status"}
{"type":"query","v":1,"event_id":"0000000000000000000000000K","query":"view_model"}
```

`crates/agent-notify-core/tests/fixtures/reply-v1.jsonl`:

```
{"ok":true,"daemon":"stamp-abc123"}
{"ok":false,"code":"unsupported_version","accepts":[1,1]}
{"ok":false,"code":"unknown_kind","accepts":[1,1]}
```

- [ ] **Step 2: Write the failing tests**

`crates/agent-notify-core/tests/wire_fixtures.rs`:

```rust
//! Pins the JSON shape of the socket protocol. The fixture files are
//! append-only: a line that stops decoding means an old CLI can no longer
//! reach a new daemon.

use agent_notify_core::wire::{
    AgentEvent, DecodeError, MAX_LINE_BYTES, Reply, Request, decode_request, encode_line,
};

const REQUESTS: &str = include_str!("fixtures/wire-v1.jsonl");
const REPLIES: &str = include_str!("fixtures/reply-v1.jsonl");

fn as_value(text: &str) -> serde_json::Value {
    serde_json::from_str(text).expect("valid JSON")
}

#[test]
fn every_request_fixture_decodes_and_re_encodes_to_the_same_json() {
    let mut count = 0;
    for line in REQUESTS.lines().filter(|line| !line.trim().is_empty()) {
        let request = decode_request(line).unwrap_or_else(|error| panic!("{error:?}: {line}"));
        let encoded = encode_line(&request).expect("encodes");
        assert!(encoded.ends_with('\n'));
        assert_eq!(as_value(&encoded), as_value(line), "round trip changed {line}");
        count += 1;
    }
    assert_eq!(count, 19, "a fixture line went missing");
}

#[test]
fn every_reply_fixture_round_trips() {
    for line in REPLIES.lines().filter(|line| !line.trim().is_empty()) {
        let reply: Reply = serde_json::from_str(line).expect("decodes");
        assert_eq!(as_value(&serde_json::to_string(&reply).expect("encodes")), as_value(line));
    }
}

fn first_request_with(change: impl Fn(&mut serde_json::Value)) -> String {
    let mut value = as_value(REQUESTS.lines().next().expect("a first line"));
    change(&mut value);
    value.to_string()
}

#[test]
fn unknown_fields_are_ignored() {
    let line = first_request_with(|value| {
        value["future_field"] = serde_json::json!(1);
        value["origin"]["future_origin_field"] = serde_json::json!("x");
    });
    assert!(decode_request(&line).is_ok());
}

#[test]
fn versions_outside_the_accepted_range_are_rejected() {
    for version in [0, 2] {
        let line = first_request_with(|value| value["v"] = serde_json::json!(version));
        assert_eq!(decode_request(&line), Err(DecodeError::UnsupportedVersion { version }));
    }
}

#[test]
fn a_missing_version_is_malformed() {
    let line = first_request_with(|value| {
        value.as_object_mut().expect("an object").remove("v");
    });
    assert!(matches!(decode_request(&line), Err(DecodeError::Malformed { .. })));
}

#[test]
fn unknown_request_and_event_kinds_are_reported_as_unknown_kind() {
    let unknown_request = first_request_with(|value| value["type"] = serde_json::json!("teleport"));
    assert_eq!(decode_request(&unknown_request), Err(DecodeError::UnknownKind));
    let unknown_event =
        first_request_with(|value| value["event"]["type"] = serde_json::json!("future_event"));
    assert_eq!(decode_request(&unknown_event), Err(DecodeError::UnknownKind));
}

#[test]
fn an_oversized_line_is_rejected_before_parsing() {
    let line = "x".repeat(MAX_LINE_BYTES + 1);
    assert_eq!(decode_request(&line), Err(DecodeError::LineTooLong { bytes: MAX_LINE_BYTES + 1 }));
}

#[test]
fn text_fields_are_cleaned_on_decode() {
    let line = first_request_with(|value| {
        value["event"] = serde_json::json!({"type": "explicit", "kind": "info", "body": "ok\u{202e}gpj.exe"});
    });
    let Ok(Request::Agent { event: AgentEvent::Explicit { body, .. }, .. }) = decode_request(&line) else {
        panic!("expected an explicit event");
    };
    assert_eq!(body.as_str(), "okgpj.exe");
}

#[test]
fn an_invalid_pane_id_is_malformed() {
    let line = first_request_with(|value| value["origin"]["pane"]["pane"] = serde_json::json!("12"));
    assert!(matches!(decode_request(&line), Err(DecodeError::Malformed { .. })));
}
```

- [ ] **Step 3: Run the tests to see them fail**

Run: `cd ~/crates && cargo test -p agent-notify-core --test wire_fixtures`
Expected: compile error, `could not find wire in agent_notify_core`.

- [ ] **Step 4: Implement**

Add `pub mod wire;` to `lib.rs`. Create `src/wire.rs`:

```rust
//! The request and reply that cross the Unix socket, one JSON object per line.
//!
//! Evolution: an added optional field keeps `v`. An unknown variant decodes to
//! `Unknown` and the daemon replies `unknown_kind`. A removed or renamed field
//! or variant, or a changed meaning, increments `v`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{DisplayText, EpochMs, EventId, PaneKey, PromptId, SessionId};

/// The version this build writes.
pub const WIRE_VERSION: u16 = 1;
/// The oldest version this build accepts.
pub const OLDEST_ACCEPTED_VERSION: u16 = 1;
/// The longest request line the daemon reads, in bytes.
pub const MAX_LINE_BYTES: usize = 65_536;

/// One request from the CLI to the daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    /// Something an agent did.
    Agent {
        /// Wire version.
        v: u16,
        /// Names this invocation in every log line.
        event_id: EventId,
        /// When the CLI built the request.
        sent_at_ms: EpochMs,
        /// Where the event came from.
        origin: Origin,
        /// What happened.
        event: AgentEvent,
    },
    /// Something the user or tmux did.
    Control {
        /// Wire version.
        v: u16,
        /// Names this invocation in every log line.
        event_id: EventId,
        /// When the CLI built the request.
        sent_at_ms: EpochMs,
        /// What to change.
        control: Control,
    },
    /// A read of daemon state.
    Query {
        /// Wire version.
        v: u16,
        /// Names this invocation in every log line.
        event_id: EventId,
        /// What to read.
        query: Query,
    },
    /// A request type from a newer CLI.
    #[serde(other)]
    Unknown,
}

/// Where an agent event came from. The CLI fills it from the hook payload,
/// the pane tags, and one tmux read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    /// Which agent.
    pub agent: AgentKind,
    /// The Claude session. `None` only for `send` with no session tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionId>,
    /// The turn, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<PromptId>,
    /// The tmux pane, when inside tmux.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane: Option<PaneKey>,
    /// The tmux window, when inside tmux.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowLabel>,
    /// The git repository, read only outside tmux.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<RepoInfo>,
    /// The latest session title known to the sender.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_title: Option<DisplayText>,
    /// The agent's working directory.
    pub cwd: PathBuf,
}

/// The agent that produced an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    /// Claude Code.
    ClaudeCode,
    /// An agent from a newer CLI.
    #[serde(other)]
    Unknown,
}

/// A tmux window's index and name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowLabel {
    /// `#{window_index}`.
    pub index: u32,
    /// `#{window_name}`.
    pub name: DisplayText,
}

/// Git facts for a working directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoInfo {
    /// The main repository's name, also inside a linked worktree.
    pub main_repo_name: DisplayText,
    /// Branch or detached commit.
    pub head: RepoHead,
}

/// The state of HEAD.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RepoHead {
    /// On a branch.
    Branch {
        /// The branch name.
        name: DisplayText,
    },
    /// Detached.
    Detached {
        /// The short commit sha.
        short_sha: DisplayText,
    },
}

/// What an agent did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// A session started, resumed, cleared, compacted or forked.
    SessionStarted {
        /// Why it started.
        source: SessionSource,
    },
    /// The user submitted a prompt.
    TurnStarted,
    /// The agent finished its turn.
    TurnEnded {
        /// The agent's last message.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_message: Option<DisplayText>,
    },
    /// The turn stopped on an API error.
    TurnFailed {
        /// The rendered error.
        error: DisplayText,
    },
    /// The agent waits for the user.
    NeedsInput {
        /// Why it waits.
        reason: InputReason,
        /// The prompt text.
        message: DisplayText,
    },
    /// An elicitation was answered.
    InputResolved,
    /// The agent called `agent-notify send`.
    Explicit {
        /// Info or attention.
        kind: Kind,
        /// The agent's text.
        body: DisplayText,
    },
    /// The agent silenced the `Stop` backup.
    Quiet {
        /// When quiet ends.
        until: QuietUntil,
        /// The latest end, in minutes.
        max_minutes: u16,
    },
    /// The agent ended quiet.
    QuietOff,
    /// The session ended.
    SessionEnded,
    /// An event from a newer CLI.
    #[serde(other)]
    Unknown,
}

/// The two card kinds. `Attention` outranks `Info`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Finished work. Fades after a few seconds.
    Info,
    /// The user must act. Stays until handled.
    Attention,
}

/// Why a session started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionSource {
    /// A new session.
    Startup,
    /// `--resume`.
    Resume,
    /// `/clear`.
    Clear,
    /// After compaction.
    Compact,
    /// A fork.
    Fork,
    /// A source from a newer Claude Code.
    #[serde(other)]
    Unknown,
}

/// Why an agent waits for the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputReason {
    /// A permission prompt.
    Permission,
    /// A question to the user.
    Elicitation,
    /// A reason from a newer CLI.
    #[serde(other)]
    Unknown,
}

/// When quiet ends on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuietUntil {
    /// At the session's next `send`.
    NextSend,
    /// At the session's next user prompt.
    NextPrompt,
}

/// A change requested by the user or by tmux.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Control {
    /// A tmux pane gained or lost focus.
    FocusChanged {
        /// The pane.
        pane: PaneKey,
        /// True on focus-in.
        focused: bool,
    },
    /// Do-not-disturb.
    Dnd {
        /// On or off.
        command: DndCommand,
    },
    /// A control from a newer CLI.
    #[serde(other)]
    Unknown,
}

/// A do-not-disturb change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DndCommand {
    /// Turn on, until `dnd off` or for a number of minutes.
    On {
        /// Optional end, in minutes from now.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        for_minutes: Option<u16>,
    },
    /// Turn off.
    Off,
}

/// A read of daemon state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Query {
    /// The `status` report.
    Status,
    /// The panel view model.
    ViewModel,
    /// A query from a newer CLI.
    #[serde(other)]
    Unknown,
}

/// The daemon's answer to one request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reply {
    /// True when the daemon accepted the request.
    pub ok: bool,
    /// The daemon's build stamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon: Option<String>,
    /// Why the daemon refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<ReplyCode>,
    /// The accepted version range, on refusal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepts: Option<[u16; 2]>,
    /// The answer to a query.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl Reply {
    /// An acknowledgement from a daemon with build stamp `daemon`.
    #[must_use]
    pub fn accepted(daemon: &str) -> Self {
        Reply { ok: true, daemon: Some(daemon.to_string()), code: None, accepts: None, data: None }
    }

    /// A refusal, with the accepted version range.
    #[must_use]
    pub fn rejected(code: ReplyCode) -> Self {
        Reply {
            ok: false,
            daemon: None,
            code: Some(code),
            accepts: Some([OLDEST_ACCEPTED_VERSION, WIRE_VERSION]),
            data: None,
        }
    }
}

/// Why the daemon refused a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyCode {
    /// The line was not a valid request.
    BadRequest,
    /// `v` is outside the accepted range.
    UnsupportedVersion,
    /// The request or event type is unknown to this daemon.
    UnknownKind,
    /// The line was longer than [`MAX_LINE_BYTES`].
    LineTooLong,
    /// A code from a newer daemon.
    #[serde(other)]
    Unknown,
}

/// Why a request line did not decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Longer than [`MAX_LINE_BYTES`].
    LineTooLong {
        /// The line length.
        bytes: usize,
    },
    /// Not JSON, or a field failed its check.
    Malformed {
        /// The parser's message.
        message: String,
    },
    /// `v` outside the accepted range.
    UnsupportedVersion {
        /// The version sent.
        version: u16,
    },
    /// A request, event, control or query type this build does not know.
    UnknownKind,
}

impl DecodeError {
    /// The reply code the daemon sends for this error.
    #[must_use]
    pub fn reply_code(&self) -> ReplyCode {
        match self {
            DecodeError::LineTooLong { .. } => ReplyCode::LineTooLong,
            DecodeError::Malformed { .. } => ReplyCode::BadRequest,
            DecodeError::UnsupportedVersion { .. } => ReplyCode::UnsupportedVersion,
            DecodeError::UnknownKind => ReplyCode::UnknownKind,
        }
    }
}

/// Decodes and checks one request line.
///
/// # Errors
///
/// Returns a [`DecodeError`] for an oversized line, malformed JSON, a version
/// outside the accepted range, or an unknown kind.
pub fn decode_request(line: &str) -> Result<Request, DecodeError> {
    if line.len() > MAX_LINE_BYTES {
        return Err(DecodeError::LineTooLong { bytes: line.len() });
    }
    let request: Request = serde_json::from_str(line)
        .map_err(|error| DecodeError::Malformed { message: error.to_string() })?;
    let version = match &request {
        Request::Agent { v, .. } | Request::Control { v, .. } | Request::Query { v, .. } => *v,
        Request::Unknown => return Err(DecodeError::UnknownKind),
    };
    if !(OLDEST_ACCEPTED_VERSION..=WIRE_VERSION).contains(&version) {
        return Err(DecodeError::UnsupportedVersion { version });
    }
    match &request {
        Request::Agent { event: AgentEvent::Unknown, .. }
        | Request::Control { control: Control::Unknown, .. }
        | Request::Query { query: Query::Unknown, .. } => Err(DecodeError::UnknownKind),
        _ => Ok(request),
    }
}

/// Encodes `value` as one JSON line, newline included.
///
/// # Errors
///
/// Returns the serializer's error. The types in this module do not produce one.
pub fn encode_line<Value: Serialize>(value: &Value) -> Result<String, serde_json::Error> {
    let mut line = serde_json::to_string(value)?;
    line.push('\n');
    Ok(line)
}
```

- [ ] **Step 5: Run the tests and clippy**

Run: `cd ~/crates && cargo test -p agent-notify-core && cargo clippy -p agent-notify-core --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
cd ~ && config add crates/agent-notify-core
config commit -m "Add the agent-notify wire protocol with golden fixtures" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/agent-notify-core
```

---

### Task 4: Claude Code hook mapping

**Files:**
- Create: `crates/agent-notify-core/src/hook.rs`
- Modify: `src/lib.rs` (add `pub mod hook;`)

**Interfaces:**
- Consumes: Task 3 `AgentEvent`, `SessionSource`, `InputReason`; Task 2 ids.
- Produces:
  - `SESSION_TAG_OPTION: &str = "@agent_notify_session"`, `PROMPT_TAG_OPTION: &str = "@agent_notify_prompt"`.
  - `ClaudeHookInput` (serde `Deserialize`): `session_id: SessionId`, `prompt_id: Option<PromptId>`, `cwd: PathBuf`, `hook_event_name: String`, `source: Option<SessionSource>`, `session_title: Option<DisplayText>`, `last_assistant_message: Option<DisplayText>`, `error: Option<String>`, `notification_type: Option<String>`, `message: Option<DisplayText>`. Unknown fields ignored.
  - `TagWrite::{BindSession(SessionId), BindPrompt(PromptId), UnbindSession}`. `BindSession` and `UnbindSession` also clear the prompt tag.
  - `HookDecision::{Send { tags: Vec<TagWrite>, event: AgentEvent }, Ignore { reason: String }}`.
  - `decide_hook(&ClaudeHookInput, pane_session_tag: Option<&SessionId>) -> HookDecision`.

- [ ] **Step 1: Write the failing tests**

In `src/hook.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::session;

    fn input(event: &str, extra: serde_json::Value) -> ClaudeHookInput {
        let mut payload = serde_json::json!({
            "session_id": "session-a",
            "prompt_id": "prompt-1",
            "transcript_path": "/home/dev/.claude/projects/x/session-a.jsonl",
            "cwd": "/home/dev/code",
            "permission_mode": "default",
            "hook_event_name": event,
        });
        for (key, value) in extra.as_object().expect("an object") {
            payload[key] = value.clone();
        }
        serde_json::from_value(payload).expect("a valid synthetic payload")
    }

    fn sent(decision: HookDecision) -> (Vec<TagWrite>, AgentEvent) {
        match decision {
            HookDecision::Send { tags, event } => (tags, event),
            HookDecision::Ignore { reason } => panic!("expected a send, got ignore: {reason}"),
        }
    }

    #[test]
    fn session_start_binds_the_session_tag() {
        let (tags, event) = sent(decide_hook(&input("SessionStart", serde_json::json!({"source": "clear"})), None));
        assert_eq!(tags, vec![TagWrite::BindSession(session("session-a"))]);
        assert_eq!(event, AgentEvent::SessionStarted { source: SessionSource::Clear });
    }

    #[test]
    fn user_prompt_submit_binds_the_prompt_tag() {
        let (tags, event) = sent(decide_hook(&input("UserPromptSubmit", serde_json::json!({"prompt": "hi"})), None));
        assert_eq!(tags, vec![TagWrite::BindPrompt(crate::test_support::prompt("prompt-1"))]);
        assert_eq!(event, AgentEvent::TurnStarted);
    }

    #[test]
    fn stop_carries_the_last_message() {
        let (tags, event) = sent(decide_hook(
            &input("Stop", serde_json::json!({"last_assistant_message": "Done. Tests pass.", "stop_hook_active": false})),
            None,
        ));
        assert!(tags.is_empty());
        assert_eq!(event, AgentEvent::TurnEnded { last_message: Some(DisplayText::new("Done. Tests pass.")) });
    }

    #[test]
    fn stop_failure_prefers_the_rendered_message_then_the_error_type() {
        let (_, rendered) = sent(decide_hook(
            &input("StopFailure", serde_json::json!({"error": "rate_limit", "last_assistant_message": "API Error: Rate limit reached"})),
            None,
        ));
        assert_eq!(rendered, AgentEvent::TurnFailed { error: DisplayText::new("API Error: Rate limit reached") });
        let (_, bare) = sent(decide_hook(&input("StopFailure", serde_json::json!({"error": "overloaded"})), None));
        assert_eq!(bare, AgentEvent::TurnFailed { error: DisplayText::new("overloaded") });
    }

    #[test]
    fn mapped_notification_types() {
        let cases = [
            ("permission_prompt", Some(AgentEvent::NeedsInput { reason: InputReason::Permission, message: DisplayText::new("Claude needs your permission") })),
            ("elicitation_dialog", Some(AgentEvent::NeedsInput { reason: InputReason::Elicitation, message: DisplayText::new("Claude needs your permission") })),
            ("elicitation_url_dialog", Some(AgentEvent::NeedsInput { reason: InputReason::Elicitation, message: DisplayText::new("Claude needs your permission") })),
            ("elicitation_complete", Some(AgentEvent::InputResolved)),
            ("idle_prompt", None),
            ("agent_completed", None),
            ("a_future_type", None),
        ];
        for (notification_type, expected) in cases {
            let decision = decide_hook(
                &input("Notification", serde_json::json!({"notification_type": notification_type, "message": "Claude needs your permission"})),
                None,
            );
            match (decision, expected) {
                (HookDecision::Send { event, .. }, Some(expected)) => assert_eq!(event, expected, "{notification_type}"),
                (HookDecision::Ignore { .. }, None) => {}
                (other, expected) => panic!("{notification_type}: got {other:?}, expected {expected:?}"),
            }
        }
    }

    #[test]
    fn a_blank_notification_message_gets_a_default() {
        let (_, event) = sent(decide_hook(
            &input("Notification", serde_json::json!({"notification_type": "permission_prompt", "message": "  "})),
            None,
        ));
        assert_eq!(event, AgentEvent::NeedsInput { reason: InputReason::Permission, message: DisplayText::new("Needs your permission") });
    }

    #[test]
    fn session_end_unbinds_only_a_matching_tag() {
        let ending = input("SessionEnd", serde_json::json!({"reason": "clear"}));
        let (matching, event) = sent(decide_hook(&ending, Some(&session("session-a"))));
        assert_eq!(matching, vec![TagWrite::UnbindSession]);
        assert_eq!(event, AgentEvent::SessionEnded);
        let (other, _) = sent(decide_hook(&ending, Some(&session("session-b"))));
        assert!(other.is_empty(), "a newer session's tag must survive a late SessionEnd");
    }

    #[test]
    fn unknown_hook_events_are_ignored_with_a_reason() {
        let decision = decide_hook(&input("PreCompact", serde_json::json!({})), None);
        assert!(matches!(decision, HookDecision::Ignore { reason } if reason.contains("PreCompact")));
    }

    #[test]
    fn a_missing_prompt_id_is_allowed() {
        let payload = serde_json::json!({"session_id": "session-a", "cwd": "/x", "hook_event_name": "SessionStart", "source": "startup"});
        let parsed: ClaudeHookInput = serde_json::from_value(payload).expect("decodes");
        assert_eq!(parsed.prompt_id, None);
    }
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cd ~/crates && cargo test -p agent-notify-core hook`
Expected: compile errors for the missing items.

- [ ] **Step 3: Implement**

Above the tests:

```rust
//! Maps a Claude Code hook payload to the pane-tag writes and the agent event
//! it implies. See the spec's section 5.4 for the table.

use std::path::PathBuf;

use serde::Deserialize;

use crate::wire::{AgentEvent, InputReason, SessionSource};
use crate::{DisplayText, PromptId, SessionId};

/// The pane option that names the Claude session living in a pane.
pub const SESSION_TAG_OPTION: &str = "@agent_notify_session";
/// The pane option that names the current turn in a pane.
pub const PROMPT_TAG_OPTION: &str = "@agent_notify_prompt";

/// The fields of a Claude Code hook payload this crate reads.
#[derive(Debug, Clone, Deserialize)]
pub struct ClaudeHookInput {
    /// The session.
    pub session_id: SessionId,
    /// The turn. Absent before the first prompt.
    #[serde(default)]
    pub prompt_id: Option<PromptId>,
    /// The session's working directory.
    pub cwd: PathBuf,
    /// Which hook fired.
    pub hook_event_name: String,
    /// `SessionStart` only.
    #[serde(default)]
    pub source: Option<SessionSource>,
    /// Present when the session has a title.
    #[serde(default)]
    pub session_title: Option<DisplayText>,
    /// `Stop` and `StopFailure`.
    #[serde(default)]
    pub last_assistant_message: Option<DisplayText>,
    /// `StopFailure`: the error type.
    #[serde(default)]
    pub error: Option<String>,
    /// `Notification`.
    #[serde(default)]
    pub notification_type: Option<String>,
    /// `Notification`.
    #[serde(default)]
    pub message: Option<DisplayText>,
}

/// A change to the pane tags, applied before the request is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagWrite {
    /// Set the session tag and clear the prompt tag.
    BindSession(SessionId),
    /// Set the prompt tag.
    BindPrompt(PromptId),
    /// Clear both tags.
    UnbindSession,
}

/// What one hook invocation does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookDecision {
    /// Write `tags`, then send `event`.
    Send {
        /// Tag writes, in order.
        tags: Vec<TagWrite>,
        /// The event to send.
        event: AgentEvent,
    },
    /// Do nothing. `reason` goes to the log.
    Ignore {
        /// Why nothing happens.
        reason: String,
    },
}

/// Decides the tag writes and event for one hook payload.
///
/// `pane_session_tag` is the pane's current session tag, so a late
/// `SessionEnd` from an old session does not erase a new session's tag.
#[must_use]
pub fn decide_hook(input: &ClaudeHookInput, pane_session_tag: Option<&SessionId>) -> HookDecision {
    let send = |tags: Vec<TagWrite>, event: AgentEvent| HookDecision::Send { tags, event };
    match input.hook_event_name.as_str() {
        "SessionStart" => send(
            vec![TagWrite::BindSession(input.session_id.clone())],
            AgentEvent::SessionStarted { source: input.source.unwrap_or(SessionSource::Unknown) },
        ),
        "UserPromptSubmit" => send(
            input.prompt_id.iter().cloned().map(TagWrite::BindPrompt).collect(),
            AgentEvent::TurnStarted,
        ),
        "Stop" => send(
            Vec::new(),
            AgentEvent::TurnEnded { last_message: input.last_assistant_message.clone() },
        ),
        "StopFailure" => send(Vec::new(), AgentEvent::TurnFailed { error: failure_text(input) }),
        "Notification" => notification(input),
        "SessionEnd" => {
            let tags = if pane_session_tag == Some(&input.session_id) {
                vec![TagWrite::UnbindSession]
            } else {
                Vec::new()
            };
            send(tags, AgentEvent::SessionEnded)
        }
        other => HookDecision::Ignore { reason: format!("hook event {other:?} is not mapped") },
    }
}

fn failure_text(input: &ClaudeHookInput) -> DisplayText {
    match (&input.last_assistant_message, &input.error) {
        (Some(message), _) if !message.is_blank() => message.clone(),
        (_, Some(error)) => DisplayText::new(error),
        _ => DisplayText::new("unknown error"),
    }
}

fn notification(input: &ClaudeHookInput) -> HookDecision {
    let message = |default: &str| {
        input
            .message
            .clone()
            .filter(|message| !message.is_blank())
            .unwrap_or_else(|| DisplayText::new(default))
    };
    let event = match input.notification_type.as_deref() {
        Some("permission_prompt") => AgentEvent::NeedsInput {
            reason: InputReason::Permission,
            message: message("Needs your permission"),
        },
        Some("elicitation_dialog" | "elicitation_url_dialog") => AgentEvent::NeedsInput {
            reason: InputReason::Elicitation,
            message: message("Needs an answer"),
        },
        Some("elicitation_complete") => AgentEvent::InputResolved,
        other => {
            return HookDecision::Ignore { reason: format!("notification type {other:?} is not mapped") };
        }
    };
    HookDecision::Send { tags: Vec::new(), event }
}
```

- [ ] **Step 4: Run the tests and clippy**

Run: `cd ~/crates && cargo test -p agent-notify-core && cargo clippy -p agent-notify-core --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
cd ~ && config add crates/agent-notify-core
config commit -m "Map Claude Code hook payloads to agent-notify events" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/agent-notify-core
```

---

### Task 5: Labels and first sentence

**Files:**
- Create: `crates/agent-notify-core/src/label.rs`
- Modify: `src/lib.rs` (add `pub mod label;`)

**Interfaces:**
- Consumes: Task 3 `WindowLabel`, `RepoInfo`, `RepoHead`.
- Produces:
  - `location_line(window: Option<&WindowLabel>, repo: Option<&RepoInfo>, cwd: &Path) -> String`
  - `first_sentence(text: &str) -> &str`
  - `turn_ended_body(last_message: Option<&DisplayText>) -> DisplayText`
  - `turn_failed_body(error: &DisplayText) -> DisplayText`

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::RepoHead;
    use proptest::prelude::*;

    fn window(index: u32, name: &str) -> WindowLabel {
        WindowLabel { index, name: DisplayText::new(name) }
    }

    fn repo(head: RepoHead) -> RepoInfo {
        RepoInfo { main_repo_name: DisplayText::new("code"), head }
    }

    #[test]
    fn location_prefers_the_tmux_window() {
        let branch = repo(RepoHead::Branch { name: DisplayText::new("main") });
        assert_eq!(location_line(Some(&window(4, "code/feature-x")), Some(&branch), Path::new("/x")), "4 · code/feature-x");
        assert_eq!(location_line(Some(&window(4, "  ")), None, Path::new("/x")), "4");
    }

    #[test]
    fn location_outside_tmux_uses_repo_and_branch() {
        let branch = repo(RepoHead::Branch { name: DisplayText::new("feature-x") });
        let detached = repo(RepoHead::Detached { short_sha: DisplayText::new("abc1234") });
        assert_eq!(location_line(None, Some(&branch), Path::new("/x")), "code · feature-x");
        assert_eq!(location_line(None, Some(&detached), Path::new("/x")), "code");
        assert_eq!(location_line(None, None, Path::new("/home/dev/notes")), "notes");
        assert_eq!(location_line(None, None, Path::new("/")), "/");
    }

    #[test]
    fn first_sentence_cases() {
        let cases = [
            ("Done. All 41 tests pass.", "Done."),
            ("  Fixed it  ", "Fixed it"),
            ("No punctuation at all", "No punctuation at all"),
            ("Use a map, e.g. a BTreeMap. Then sort.", "Use a map, e.g. a BTreeMap."),
            ("Bumped to v1.2.3 today. Next.", "Bumped to v1.2.3 today."),
            ("See https://example.com/a.b/c for details. More.", "See https://example.com/a.b/c for details."),
            ("Summary line\nSecond line.", "Summary line"),
            ("- Fixed X\n- Fixed Y", "- Fixed X"),
            ("完了しました。次へ", "完了しました。"),
            ("Is it green? Yes.", "Is it green?"),
            ("```rust\nfn main() {}\n```\nAfter.", "```rust\nfn main() {}\n```"),
            ("```\nunclosed", "```\nunclosed"),
            ("", ""),
            ("   \n  ", ""),
        ];
        for (input, expected) in cases {
            assert_eq!(first_sentence(input), expected, "input {input:?}");
        }
    }

    #[test]
    fn bodies() {
        assert_eq!(turn_ended_body(None).as_str(), "Finished");
        assert_eq!(turn_ended_body(Some(&DisplayText::new("  "))).as_str(), "Finished");
        assert_eq!(turn_ended_body(Some(&DisplayText::new("Done. More."))).as_str(), "Done.");
        assert_eq!(turn_failed_body(&DisplayText::new("API Error: overloaded")).as_str(), "Stopped: API Error: overloaded");
    }

    proptest! {
        #[test]
        fn first_sentence_is_a_non_empty_prefix_of_the_trimmed_input(text in ".{0,300}") {
            let trimmed = text.trim();
            let sentence = first_sentence(&text);
            prop_assert!(trimmed.starts_with(sentence));
            prop_assert_eq!(sentence.is_empty(), trimmed.is_empty());
        }
    }
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cd ~/crates && cargo test -p agent-notify-core label`
Expected: compile errors.

- [ ] **Step 3: Implement**

```rust
//! The text a card shows. Built only from origin data and agent text.

use std::path::Path;

use crate::DisplayText;
use crate::wire::{RepoHead, RepoInfo, WindowLabel};

const ABBREVIATIONS: [&str; 9] = ["e.g", "i.e", "etc", "vs", "cf", "approx", "mr", "mrs", "dr"];

/// Line 1 of a card: `<window index> · <window name>` inside tmux, else
/// `<repo> · <branch>`, else the directory name.
#[must_use]
pub fn location_line(window: Option<&WindowLabel>, repo: Option<&RepoInfo>, cwd: &Path) -> String {
    if let Some(window) = window {
        return if window.name.is_blank() {
            window.index.to_string()
        } else {
            format!("{} · {}", window.index, window.name.as_str())
        };
    }
    match repo {
        Some(RepoInfo { main_repo_name, head: RepoHead::Branch { name } }) => {
            format!("{} · {}", main_repo_name.as_str(), name.as_str())
        }
        Some(RepoInfo { main_repo_name, head: RepoHead::Detached { .. } }) => {
            main_repo_name.as_str().to_string()
        }
        None => {
            let name = cwd.file_name().map_or_else(
                || cwd.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
            DisplayText::new(&name).as_str().to_string()
        }
    }
}

/// The first sentence of `text`, as a prefix of the trimmed text.
///
/// Ends at `.`, `!` or `?` followed by whitespace or the end (not after a
/// common abbreviation), at a CJK full stop, or at a line break. A leading
/// code fence runs to its closing fence.
#[must_use]
pub fn first_sentence(text: &str) -> &str {
    let trimmed = text.trim();
    if trimmed.starts_with("```") {
        return through_closing_fence(trimmed);
    }
    for (index, character) in trimmed.char_indices() {
        let end = index + character.len_utf8();
        match character {
            '\n' => return trimmed[..index].trim_end(),
            '。' | '！' | '？' => return &trimmed[..end],
            '.' | '!' | '?' => {
                let at_boundary = trimmed[end..].chars().next().is_none_or(char::is_whitespace);
                let is_abbreviation = character == '.' && ends_with_abbreviation(&trimmed[..index]);
                if at_boundary && !is_abbreviation {
                    return &trimmed[..end];
                }
            }
            _ => {}
        }
    }
    trimmed
}

fn ends_with_abbreviation(before: &str) -> bool {
    let word = before.rsplit(char::is_whitespace).next().unwrap_or("");
    let word = word.trim_start_matches(|character: char| !character.is_alphanumeric());
    ABBREVIATIONS.iter().any(|abbreviation| word.eq_ignore_ascii_case(abbreviation))
}

fn through_closing_fence(trimmed: &str) -> &str {
    let body_start = trimmed.find('\n').map_or(trimmed.len(), |index| index + 1);
    match trimmed[body_start..].find("```") {
        Some(offset) => &trimmed[..body_start + offset + 3],
        None => trimmed,
    }
}

/// The body of a `TurnEnded` card.
#[must_use]
pub fn turn_ended_body(last_message: Option<&DisplayText>) -> DisplayText {
    let sentence = last_message.map_or("", |message| first_sentence(message.as_str()));
    if sentence.is_empty() { DisplayText::new("Finished") } else { DisplayText::new(sentence) }
}

/// The body of a `TurnFailed` card.
#[must_use]
pub fn turn_failed_body(error: &DisplayText) -> DisplayText {
    DisplayText::new(&format!("Stopped: {}", error.as_str()))
}
```

- [ ] **Step 4: Run the tests and clippy**

Run: `cd ~/crates && cargo test -p agent-notify-core && cargo clippy -p agent-notify-core --all-targets -- -D warnings`
Expected: all pass. If the URL case fails, check that `.` inside the URL is followed by a non-space character: that is what keeps it from ending the sentence.

- [ ] **Step 5: Commit**

```bash
cd ~ && config add crates/agent-notify-core
config commit -m "Add card labels and first-sentence extraction" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/agent-notify-core
```

---

### Task 6: Socket and log locations, fallback note

**Files:**
- Create: `crates/agent-notify-core/src/paths.rs`, `src/fallback.rs`
- Modify: `src/lib.rs` (add `pub mod paths;` and `pub mod fallback;`)

**Interfaces:**
- Consumes: Task 3 `Origin`, `AgentEvent`, `Kind`; Task 5 `location_line`, `turn_failed_body`.
- Produces:
  - `Platform::{MacOs, Linux, Other}` with `Platform::current() -> Platform` (a `cfg!` check, no IO).
  - `PathInputs<'a> { platform, home: Option<&'a Path>, xdg_runtime_dir, xdg_state_home, socket_override, log_dir_override }` (all `Option<&'a Path>` except `platform`).
  - `SocketLocation::{At(PathBuf), Unavailable(&'static str)}`; `socket_location(&PathInputs<'_>) -> SocketLocation`.
  - `log_directory(&PathInputs<'_>) -> Option<PathBuf>`.
  - `SUN_PATH_MAX_BYTES: usize = 103`.
  - `FallbackNote { title: String, subtitle: &'static str, message: String, urgent: bool }`, `OFFLINE_SUBTITLE = "agent-notify offline"`, `fallback_note(&Origin, &AgentEvent) -> Option<FallbackNote>`.

- [ ] **Step 1: Write the failing tests**

In `src/paths.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(platform: Platform) -> PathInputs<'static> {
        PathInputs {
            platform,
            home: Some(Path::new("/Users/dev")),
            xdg_runtime_dir: Some(Path::new("/run/user/1000")),
            xdg_state_home: None,
            socket_override: None,
            log_dir_override: None,
        }
    }

    #[test]
    fn macos_socket_lives_in_application_support() {
        assert_eq!(
            socket_location(&inputs(Platform::MacOs)),
            SocketLocation::At(PathBuf::from("/Users/dev/Library/Application Support/agent-notify/agent-notify.sock"))
        );
    }

    #[test]
    fn linux_socket_lives_in_the_runtime_dir_and_fails_closed_without_it() {
        assert_eq!(
            socket_location(&inputs(Platform::Linux)),
            SocketLocation::At(PathBuf::from("/run/user/1000/agent-notify/agent-notify.sock"))
        );
        let missing = PathInputs { xdg_runtime_dir: None, ..inputs(Platform::Linux) };
        assert!(matches!(socket_location(&missing), SocketLocation::Unavailable(_)));
        let relative = PathInputs { xdg_runtime_dir: Some(Path::new("run")), ..inputs(Platform::Linux) };
        assert!(matches!(socket_location(&relative), SocketLocation::Unavailable(_)));
    }

    #[test]
    fn the_override_wins_when_absolute() {
        let overridden = PathInputs { socket_override: Some(Path::new("/tmp/test.sock")), ..inputs(Platform::Linux) };
        assert_eq!(socket_location(&overridden), SocketLocation::At(PathBuf::from("/tmp/test.sock")));
        let relative = PathInputs { socket_override: Some(Path::new("test.sock")), ..inputs(Platform::MacOs) };
        assert!(matches!(socket_location(&relative), SocketLocation::Unavailable(_)));
    }

    #[test]
    fn an_overlong_socket_path_is_unavailable() {
        let long_home = format!("/Users/{}", "d".repeat(80));
        let long = PathInputs { home: Some(Path::new(&long_home)), ..inputs(Platform::MacOs) };
        assert!(matches!(socket_location(&long), SocketLocation::Unavailable(_)));
    }

    #[test]
    fn log_directories() {
        assert_eq!(log_directory(&inputs(Platform::MacOs)), Some(PathBuf::from("/Users/dev/Library/Logs/agent-notify")));
        assert_eq!(log_directory(&inputs(Platform::Linux)), Some(PathBuf::from("/Users/dev/.local/state/agent-notify")));
        let state = PathInputs { xdg_state_home: Some(Path::new("/state")), ..inputs(Platform::Linux) };
        assert_eq!(log_directory(&state), Some(PathBuf::from("/state/agent-notify")));
        let overridden = PathInputs { log_dir_override: Some(Path::new("/tmp/logs")), ..inputs(Platform::MacOs) };
        assert_eq!(log_directory(&overridden), Some(PathBuf::from("/tmp/logs")));
        let homeless = PathInputs { home: None, ..inputs(Platform::MacOs) };
        assert_eq!(log_directory(&homeless), None);
    }
}
```

In `src/fallback.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{AgentKind, InputReason, WindowLabel};
    use std::path::PathBuf;

    fn origin() -> Origin {
        Origin {
            agent: AgentKind::ClaudeCode,
            session: None,
            prompt: None,
            pane: None,
            window: Some(WindowLabel { index: 4, name: DisplayText::new("code/x") }),
            repo: None,
            session_title: None,
            cwd: PathBuf::from("/home/dev/code"),
        }
    }

    #[test]
    fn attention_events_and_explicit_sends_get_a_note() {
        let needs = AgentEvent::NeedsInput { reason: InputReason::Permission, message: DisplayText::new("Allow Bash?") };
        let note = fallback_note(&origin(), &needs).expect("a note");
        assert_eq!(note.title, "4 · code/x");
        assert_eq!(note.subtitle, OFFLINE_SUBTITLE);
        assert_eq!(note.message, "Allow Bash?");
        assert!(note.urgent);

        let failed = AgentEvent::TurnFailed { error: DisplayText::new("overloaded") };
        assert_eq!(fallback_note(&origin(), &failed).map(|note| note.message), Some("Stopped: overloaded".to_string()));

        let info = AgentEvent::Explicit { kind: Kind::Info, body: DisplayText::new("Report ready") };
        assert_eq!(fallback_note(&origin(), &info).map(|note| note.urgent), Some(false));
    }

    #[test]
    fn turn_ended_and_bookkeeping_events_get_no_note() {
        for event in [
            AgentEvent::TurnEnded { last_message: Some(DisplayText::new("Done.")) },
            AgentEvent::TurnStarted,
            AgentEvent::QuietOff,
            AgentEvent::SessionEnded,
            AgentEvent::InputResolved,
        ] {
            assert_eq!(fallback_note(&origin(), &event), None, "{event:?}");
        }
    }
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cd ~/crates && cargo test -p agent-notify-core paths fallback`
Expected: compile errors. (Cargo takes one filter; run `cargo test -p agent-notify-core` instead if the two-filter form errors.)

- [ ] **Step 3: Implement `paths.rs`**

```rust
//! Where the socket and the log files live. Inputs are plain values read by
//! the caller, so every platform's rule is testable on any machine.

use std::path::{Path, PathBuf};

/// The longest Unix socket path, in bytes, before the terminating NUL. macOS
/// caps `sun_path` at 104 bytes.
pub const SUN_PATH_MAX_BYTES: usize = 103;

/// The platform rules to apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// macOS.
    MacOs,
    /// Linux.
    Linux,
    /// Anything else. No socket.
    Other,
}

impl Platform {
    /// The platform this binary was built for.
    #[must_use]
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Platform::MacOs
        } else if cfg!(target_os = "linux") {
            Platform::Linux
        } else {
            Platform::Other
        }
    }
}

/// Environment facts the path rules read.
#[derive(Debug, Clone, Copy)]
pub struct PathInputs<'a> {
    /// The platform.
    pub platform: Platform,
    /// `$HOME`.
    pub home: Option<&'a Path>,
    /// `$XDG_RUNTIME_DIR`.
    pub xdg_runtime_dir: Option<&'a Path>,
    /// `$XDG_STATE_HOME`.
    pub xdg_state_home: Option<&'a Path>,
    /// `$AGENT_NOTIFY_SOCKET`.
    pub socket_override: Option<&'a Path>,
    /// `$AGENT_NOTIFY_LOG_DIR`.
    pub log_dir_override: Option<&'a Path>,
}

/// Where the daemon socket is, or why there is none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocketLocation {
    /// The socket path.
    At(PathBuf),
    /// No socket. The CLI uses the fallback and `doctor` reports the reason.
    Unavailable(&'static str),
}

/// Resolves the socket path. Fails closed: a missing or relative directory
/// gives `Unavailable`, never a path in a shared directory.
#[must_use]
pub fn socket_location(inputs: &PathInputs<'_>) -> SocketLocation {
    let candidate = match (inputs.socket_override, inputs.platform) {
        (Some(path), _) if path.is_absolute() => path.to_path_buf(),
        (Some(_), _) => return SocketLocation::Unavailable("AGENT_NOTIFY_SOCKET is not absolute"),
        (None, Platform::MacOs) => match absolute(inputs.home) {
            Some(home) => home.join("Library/Application Support/agent-notify/agent-notify.sock"),
            None => return SocketLocation::Unavailable("HOME is unset or relative"),
        },
        (None, Platform::Linux) => match absolute(inputs.xdg_runtime_dir) {
            Some(runtime) => runtime.join("agent-notify/agent-notify.sock"),
            None => return SocketLocation::Unavailable("XDG_RUNTIME_DIR is unset or relative"),
        },
        (None, Platform::Other) => return SocketLocation::Unavailable("unsupported platform"),
    };
    if candidate.as_os_str().len() > SUN_PATH_MAX_BYTES {
        return SocketLocation::Unavailable("socket path is longer than 103 bytes");
    }
    SocketLocation::At(candidate)
}

/// The directory for log files, or `None` when there is nowhere safe to write.
#[must_use]
pub fn log_directory(inputs: &PathInputs<'_>) -> Option<PathBuf> {
    if let Some(directory) = absolute(inputs.log_dir_override) {
        return Some(directory.to_path_buf());
    }
    let home = absolute(inputs.home);
    match inputs.platform {
        Platform::MacOs => home.map(|home| home.join("Library/Logs/agent-notify")),
        Platform::Linux | Platform::Other => absolute(inputs.xdg_state_home)
            .map(Path::to_path_buf)
            .or_else(|| home.map(|home| home.join(".local/state")))
            .map(|state| state.join("agent-notify")),
    }
}

fn absolute(path: Option<&Path>) -> Option<&Path> {
    path.filter(|path| path.is_absolute())
}
```

- [ ] **Step 4: Implement `fallback.rs`**

```rust
//! The plain OS notification the CLI shows when the daemon does not answer.
//! Only events that would produce a card get one, and never `TurnEnded`: the
//! CLI cannot know whether the agent already sent its own notification.

use crate::label::{location_line, turn_failed_body};
use crate::wire::{AgentEvent, Kind, Origin};

/// The subtitle that tells the user the daemon is down.
pub const OFFLINE_SUBTITLE: &str = "agent-notify offline";

/// The text of one fallback notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackNote {
    /// The location line.
    pub title: String,
    /// Always [`OFFLINE_SUBTITLE`].
    pub subtitle: &'static str,
    /// The card body.
    pub message: String,
    /// True for attention events. Linux maps it to `--urgency=critical`.
    pub urgent: bool,
}

/// The fallback for `event`, or `None` when the event shows no fallback.
#[must_use]
pub fn fallback_note(origin: &Origin, event: &AgentEvent) -> Option<FallbackNote> {
    let (message, urgent) = match event {
        AgentEvent::NeedsInput { message, .. } => (message.as_str().to_string(), true),
        AgentEvent::TurnFailed { error } => (turn_failed_body(error).as_str().to_string(), true),
        AgentEvent::Explicit { kind, body } => (body.as_str().to_string(), *kind == Kind::Attention),
        _ => return None,
    };
    Some(FallbackNote {
        title: location_line(origin.window.as_ref(), origin.repo.as_ref(), &origin.cwd),
        subtitle: OFFLINE_SUBTITLE,
        message,
        urgent,
    })
}
```

- [ ] **Step 5: Run the tests and clippy**

Run: `cd ~/crates && cargo test -p agent-notify-core && cargo clippy -p agent-notify-core --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
cd ~ && config add crates/agent-notify-core
config commit -m "Add socket and log locations and the fallback note" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/agent-notify-core
```

---

### Task 7: Daemon state and the session and turn rules

This task covers section 6.2 of the spec for agent events. Tasks 8 and 10 add the remaining inputs to the same `update` function.

**Files:**
- Create: `crates/agent-notify-core/src/state.rs`, `src/update.rs`
- Modify: `src/lib.rs` (add `pub mod state;`, `pub mod update;`), `src/test_support.rs`

**Interfaces:**
- Consumes: Tasks 2, 3, 5.
- Produces (`agent_notify_core::state`):
  - `DaemonState { sessions: BTreeMap<SessionId, SessionRecord>, oneoff_cards: Vec<OneoffCard>, dnd: Dnd, focus: Option<FocusedPane>, stack_hovered: bool, frozen_view: Option<PanelViewModel>, screen_shared: bool, last_sound_at: Option<EpochMs>, revision: u64 }`, `Default`, `dnd_active(&self, EpochMs) -> bool`.
  - `SessionRecord { pane, window, repo, cwd, title, turn: Turn, quiet: QuietMode, card: Option<Card> }`.
  - `OneoffCard { location: String, pane: Option<PaneKey>, card: Card }`.
  - `Card { id: CardId, kind: Kind, cause: CardCause, body: DisplayText, event_id: EventId, created: EpochMs, phase: CardPhase, note: Option<CardNote> }`.
  - `CardCause::{Explicit, TurnEnded, TurnFailed, NeedsInput(InputReason)}`; `CardPhase::{Showing { fade_at: Option<EpochMs> }, Folded, Snoozed { until }, Expiring { remove_at }}` (`Copy`); `CardNote::{SessionEnded, NotRaised, SwitchFailed(DisplayText)}`.
  - `Turn::{Idle, Working { prompt: Option<PromptId>, explicit_sent: bool }, Finished { prompt }, NeedsYou { prompt, reason }, Failed { prompt }}` with `prompt(&self) -> Option<&PromptId>`.
  - `QuietMode::{Armed, Until { until: QuietUntil, deadline: EpochMs }}`; `Dnd::{Off, On { until: Option<EpochMs> }}`; `FocusedPane { pane: PaneKey, since: EpochMs }`.
- Produces (`agent_notify_core::update`):
  - The timing constants listed in Global Constraints.
  - `DaemonInput::{Received { request: Request, focus: FocusSnapshot }, Ui(UiIntent), Tick, PaneSnapshot(PaneSnapshot), FocusOutcome { card: CardId, outcome: FocusOutcome }}`.
  - `UiIntent::{Clicked { card_id }, Closed { card_id }, HoverChanged { hovered: bool }}`.
  - `FocusSnapshot { frontmost: Frontmost, active_pane: Option<PaneKey>, idle_seconds: Option<u32>, screen_locked: Option<bool>, screen_shared: Option<bool> }` with `FocusSnapshot::unknown()`; `Frontmost::{Terminal, Other, Unknown}`.
  - `PaneSnapshot { server: ServerId, panes: Vec<PaneTag> }`, `PaneTag { pane: PaneId, session: Option<SessionId> }`.
  - `FocusOutcome::{Done, SessionGone, TmuxFailed(DisplayText), RaiseFailed}`.
  - `Effect::{RunFocusPlan { card: CardId, target: FocusTarget }, PlaySound, SetPaneState { pane: PaneKey, state: PaneState }, PersistDnd(Dnd), Suppressed { event_id: EventId, reason: SuppressReason }}`.
  - `FocusTarget::{Session { session: SessionId, pane: PaneKey }, RaiseOnly}`; `PaneState::{Idle, Working, Finished, NeedsYou, Failed}`; `SuppressReason::{ExplicitSent, Quiet, Outranked, Focused, Dnd, Stale, NoCardForEvent}`.
  - `update(&mut DaemonState, DaemonInput, EpochMs) -> Vec<Effect>`.
  - `merge(Option<Card>, Card) -> Merged`, `Merged::{Kept(Card), Replaced { card: Card, replaced_attention: bool }}`.
  - `is_suppressed_by_focus(&FocusSnapshot, Option<&PaneKey>) -> bool`.

`frozen_view` has the type `Option<PanelViewModel>` from Task 9. In this task declare it as `Option<crate::view::PanelViewModel>` and create `src/view.rs` with only the type (Step 1), so the crate compiles. Task 9 adds `view()`.

- [ ] **Step 1: Create the state types and the view-model type**

`src/view.rs` (Task 9 completes it):

```rust
//! The panel view model: a pure function of [`DaemonState`](crate::state::DaemonState).

use serde::Serialize;

use crate::wire::Kind;
use crate::{CardId, EpochMs};

/// Everything the toast panel draws.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PanelViewModel {
    /// Increases with every update. The UI drops a push with a lower value.
    pub revision: u64,
    /// Cards, newest first.
    pub cards: Vec<CardView>,
    /// The "Finished: 2, 4, 6" row, oldest first.
    pub finished_row: Vec<FinishedEntry>,
    /// True while do-not-disturb hides everything.
    pub dnd: bool,
}

/// One card as drawn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardView {
    /// The id the UI sends back in intents.
    pub card_id: CardId,
    /// Info or attention.
    pub kind: Kind,
    /// Line 1.
    pub location: String,
    /// Line 2, the session title.
    pub title: Option<String>,
    /// Line 3. `None` while the screen is shared.
    pub body: Option<String>,
    /// For the relative time.
    pub created_ms: EpochMs,
    /// A status line after a click.
    pub note: Option<String>,
}

/// One entry in the finished row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FinishedEntry {
    /// The folded card.
    pub card_id: CardId,
    /// The window index, or the location line outside tmux.
    pub label: String,
}
```

`src/state.rs`:

```rust
//! Daemon state. Owned by one actor in the daemon and changed only by
//! [`update`](crate::update::update).

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::view::PanelViewModel;
use crate::wire::{InputReason, Kind, Origin, QuietUntil, RepoInfo, WindowLabel};
use crate::{CardId, DisplayText, EpochMs, EventId, PaneKey, PromptId, SessionId};

/// All decision state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DaemonState {
    /// One record per Claude session.
    pub sessions: BTreeMap<SessionId, SessionRecord>,
    /// Cards from `send` with no session.
    pub oneoff_cards: Vec<OneoffCard>,
    /// Do-not-disturb.
    pub dnd: Dnd,
    /// The focused tmux pane, from the pane focus hooks.
    pub focus: Option<FocusedPane>,
    /// True while the pointer is over the stack.
    pub stack_hovered: bool,
    /// The view shown when the pointer entered the stack.
    pub frozen_view: Option<PanelViewModel>,
    /// True while the screen is shared. Hides card bodies.
    pub screen_shared: bool,
    /// The last time a sound played.
    pub last_sound_at: Option<EpochMs>,
    /// Increases with every update.
    pub revision: u64,
}

impl DaemonState {
    /// True while do-not-disturb is on at `now`.
    #[must_use]
    pub fn dnd_active(&self, now: EpochMs) -> bool {
        match self.dnd {
            Dnd::Off => false,
            Dnd::On { until: None } => true,
            Dnd::On { until: Some(until) } => now < until,
        }
    }
}

/// What the daemon knows about one session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    /// The latest known pane.
    pub pane: Option<PaneKey>,
    /// The latest known window.
    pub window: Option<WindowLabel>,
    /// The repository, outside tmux.
    pub repo: Option<RepoInfo>,
    /// The working directory.
    pub cwd: PathBuf,
    /// The latest session title.
    pub title: Option<DisplayText>,
    /// The current turn.
    pub turn: Turn,
    /// Whether the `Stop` backup is silenced.
    pub quiet: QuietMode,
    /// At most one card per session.
    pub card: Option<Card>,
}

impl SessionRecord {
    /// A record for a session first seen in `origin`.
    #[must_use]
    pub fn new(origin: &Origin) -> Self {
        SessionRecord {
            pane: origin.pane.clone(),
            window: origin.window.clone(),
            repo: origin.repo.clone(),
            cwd: origin.cwd.clone(),
            title: origin.session_title.clone(),
            turn: Turn::Idle,
            quiet: QuietMode::Armed,
            card: None,
        }
    }

    /// Takes the newer location and title facts from `origin`. Absent facts
    /// keep the old value, so a Bash `send` without a title keeps the title.
    pub fn absorb(&mut self, origin: &Origin) {
        if origin.pane.is_some() {
            self.pane.clone_from(&origin.pane);
        }
        if origin.window.is_some() {
            self.window.clone_from(&origin.window);
        }
        if origin.repo.is_some() {
            self.repo.clone_from(&origin.repo);
        }
        if origin.session_title.is_some() {
            self.title.clone_from(&origin.session_title);
        }
        self.cwd.clone_from(&origin.cwd);
    }
}

/// A card from `send` with no session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneoffCard {
    /// Line 1, fixed when the card was made.
    pub location: String,
    /// The pane, when known.
    pub pane: Option<PaneKey>,
    /// The card.
    pub card: Card,
}

/// One card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    /// Fresh for every card.
    pub id: CardId,
    /// Info or attention.
    pub kind: Kind,
    /// The event that made it.
    pub cause: CardCause,
    /// Line 3.
    pub body: DisplayText,
    /// The request that made it.
    pub event_id: EventId,
    /// When it was made.
    pub created: EpochMs,
    /// Where it is in its life.
    pub phase: CardPhase,
    /// A status line after a click.
    pub note: Option<CardNote>,
}

/// The event that made a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardCause {
    /// `agent-notify send`.
    Explicit,
    /// The `Stop` backup.
    TurnEnded,
    /// `StopFailure`.
    TurnFailed,
    /// A permission prompt or a question.
    NeedsInput(InputReason),
}

/// Where a card is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardPhase {
    /// On screen. Info cards fold at `fade_at`.
    Showing {
        /// When an info card folds. `None` for attention cards.
        fade_at: Option<EpochMs>,
    },
    /// In the finished row.
    Folded,
    /// Hidden by the close button until `until`.
    Snoozed {
        /// When it shows again.
        until: EpochMs,
    },
    /// Showing a note, removed at `remove_at`.
    Expiring {
        /// When it goes.
        remove_at: EpochMs,
    },
}

/// A status line shown on a card after a click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardNote {
    /// No pane carries the session tag any more.
    SessionEnded,
    /// tmux switched, the window did not come forward.
    NotRaised,
    /// tmux refused the switch.
    SwitchFailed(DisplayText),
}

/// The current turn of a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Turn {
    /// No turn seen yet.
    Idle,
    /// The agent is working.
    Working {
        /// The turn.
        prompt: Option<PromptId>,
        /// True once the agent called `send` in this turn.
        explicit_sent: bool,
    },
    /// The turn ended.
    Finished {
        /// The turn.
        prompt: Option<PromptId>,
    },
    /// The agent waits for the user.
    NeedsYou {
        /// The turn.
        prompt: Option<PromptId>,
        /// Why it waits.
        reason: InputReason,
    },
    /// The turn stopped on an error.
    Failed {
        /// The turn.
        prompt: Option<PromptId>,
    },
}

impl Turn {
    /// The turn's prompt id, when known.
    #[must_use]
    pub fn prompt(&self) -> Option<&PromptId> {
        match self {
            Turn::Idle => None,
            Turn::Working { prompt, .. }
            | Turn::Finished { prompt }
            | Turn::NeedsYou { prompt, .. }
            | Turn::Failed { prompt } => prompt.as_ref(),
        }
    }
}

/// Whether the `Stop` backup is silenced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuietMode {
    /// `TurnEnded` may show a card.
    Armed,
    /// `TurnEnded` shows no card until `until` happens or `deadline` passes.
    Until {
        /// The event that ends quiet.
        until: QuietUntil,
        /// The latest end.
        deadline: EpochMs,
    },
}

impl QuietMode {
    /// True when `event` ends this quiet mode.
    #[must_use]
    pub fn ends_on(self, event: QuietUntil) -> bool {
        matches!(self, QuietMode::Until { until, .. } if until == event)
    }
}

/// Do-not-disturb.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dnd {
    /// Cards and sounds as usual.
    #[default]
    Off,
    /// No cards on screen, no sounds.
    On {
        /// The end, or `None` until `dnd off`.
        until: Option<EpochMs>,
    },
}

/// The tmux pane that has focus, and since when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusedPane {
    /// The pane.
    pub pane: PaneKey,
    /// When it gained focus.
    pub since: EpochMs,
}
```

- [ ] **Step 2: Extend the test builders**

Append to `src/test_support.rs`:

```rust
use std::path::PathBuf;

use crate::DisplayText;
use crate::state::{Card, DaemonState};
use crate::update::{DaemonInput, Effect, FocusSnapshot, SuppressReason, update};
use crate::wire::{AgentEvent, AgentKind, Origin, Request, WIRE_VERSION, WindowLabel};

pub const WINDOW_INDEX: u32 = 4;

pub fn origin(session_name: Option<&str>, prompt_name: Option<&str>) -> Origin {
    Origin {
        agent: AgentKind::ClaudeCode,
        session: session_name.map(session),
        prompt: prompt_name.map(prompt),
        pane: Some(pane(1)),
        window: Some(WindowLabel { index: WINDOW_INDEX, name: DisplayText::new("code/feature-x") }),
        repo: None,
        session_title: None,
        cwd: PathBuf::from("/home/dev/code"),
    }
}

pub fn agent_input(number: i64, origin: Origin, event: AgentEvent) -> DaemonInput {
    DaemonInput::Received {
        request: Request::Agent {
            v: WIRE_VERSION,
            event_id: event_id(number),
            sent_at_ms: EpochMs(number),
            origin,
            event,
        },
        focus: FocusSnapshot::unknown(),
    }
}

/// Sends `event` for session `session_name` at time `number`, which is also
/// the event id, so a test can name the card an event made.
pub fn send(
    state: &mut DaemonState,
    number: i64,
    session_name: &str,
    prompt_name: Option<&str>,
    event: AgentEvent,
) -> Vec<Effect> {
    update(state, agent_input(number, origin(Some(session_name), prompt_name), event), EpochMs(number))
}

pub fn card<'a>(state: &'a DaemonState, session_name: &str) -> Option<&'a Card> {
    state.sessions.get(&session(session_name)).and_then(|record| record.card.as_ref())
}

pub fn reasons(effects: &[Effect]) -> Vec<SuppressReason> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Suppressed { reason, .. } => Some(*reason),
            _ => None,
        })
        .collect()
}
```

- [ ] **Step 3: Write the failing tests**

In `src/update.rs`, tests module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{CardCause, QuietMode, Turn};
    use crate::test_support::{agent_input, card, event_id, origin, pane, reasons, send, session};
    use crate::wire::{AgentEvent, InputReason, Kind, QuietUntil, SessionSource};
    use crate::DisplayText;

    fn text(value: &str) -> DisplayText {
        DisplayText::new(value)
    }

    fn turn_ended(message: &str) -> AgentEvent {
        AgentEvent::TurnEnded { last_message: Some(text(message)) }
    }

    fn needs_permission() -> AgentEvent {
        AgentEvent::NeedsInput { reason: InputReason::Permission, message: text("Allow Bash?") }
    }

    fn explicit(kind: Kind, body: &str) -> AgentEvent {
        AgentEvent::Explicit { kind, body: text(body) }
    }

    #[test]
    fn turn_ended_without_an_explicit_send_shows_an_info_card() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", Some("p1"), turn_ended("Done. All pass."));
        let shown = card(&state, "a").expect("a card");
        assert_eq!((shown.kind, shown.cause), (Kind::Info, CardCause::TurnEnded));
        assert_eq!(shown.body.as_str(), "Done.");
        assert_eq!(shown.id, CardId::for_event(&event_id(2)));
    }

    #[test]
    fn turn_ended_after_an_explicit_send_keeps_the_explicit_card() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", None, explicit(Kind::Info, "CI green"));
        let effects = send(&mut state, 3, "a", Some("p1"), turn_ended("Done."));
        assert_eq!(reasons(&effects), vec![SuppressReason::ExplicitSent]);
        assert_eq!(card(&state, "a").map(|shown| shown.cause), Some(CardCause::Explicit));
    }

    #[test]
    fn turn_ended_while_quiet_shows_nothing() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", None, AgentEvent::Quiet { until: QuietUntil::NextSend, max_minutes: 120 });
        let effects = send(&mut state, 3, "a", Some("p1"), turn_ended("Done."));
        assert_eq!(reasons(&effects), vec![SuppressReason::Quiet]);
        assert!(card(&state, "a").is_none());
    }

    #[test]
    fn quiet_until_next_prompt_ends_on_turn_started() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", None, AgentEvent::Quiet { until: QuietUntil::NextPrompt, max_minutes: 120 });
        send(&mut state, 2, "a", Some("p1"), AgentEvent::TurnStarted);
        assert_eq!(state.sessions[&session("a")].quiet, QuietMode::Armed);
    }

    #[test]
    fn quiet_until_next_send_ends_on_explicit_and_not_on_turn_started() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", None, AgentEvent::Quiet { until: QuietUntil::NextSend, max_minutes: 120 });
        send(&mut state, 2, "a", Some("p1"), AgentEvent::TurnStarted);
        assert_ne!(state.sessions[&session("a")].quiet, QuietMode::Armed);
        send(&mut state, 3, "a", None, explicit(Kind::Info, "x"));
        assert_eq!(state.sessions[&session("a")].quiet, QuietMode::Armed);
    }

    #[test]
    fn quiet_off_rearms() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", None, AgentEvent::Quiet { until: QuietUntil::NextSend, max_minutes: 120 });
        send(&mut state, 2, "a", None, AgentEvent::QuietOff);
        assert_eq!(state.sessions[&session("a")].quiet, QuietMode::Armed);
    }

    #[test]
    fn needs_input_shows_attention_and_the_turn_end_replaces_it_with_info() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", Some("p1"), needs_permission());
        assert_eq!(card(&state, "a").map(|shown| shown.kind), Some(Kind::Attention));
        send(&mut state, 3, "a", Some("p1"), turn_ended("Done."));
        let shown = card(&state, "a").expect("a card");
        assert_eq!((shown.kind, shown.cause), (Kind::Info, CardCause::TurnEnded));
    }

    #[test]
    fn a_late_needs_input_for_an_ended_turn_is_stale() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", Some("p1"), turn_ended("Done."));
        let effects = send(&mut state, 3, "a", Some("p1"), needs_permission());
        assert_eq!(reasons(&effects), vec![SuppressReason::Stale]);
        assert_eq!(card(&state, "a").map(|shown| shown.kind), Some(Kind::Info));
    }

    #[test]
    fn events_for_an_older_prompt_are_stale() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", Some("p2"), AgentEvent::TurnStarted);
        for (number, event) in [(3, turn_ended("old")), (4, needs_permission()), (5, AgentEvent::TurnFailed { error: text("x") })] {
            let effects = send(&mut state, number, "a", Some("p1"), event);
            assert_eq!(reasons(&effects), vec![SuppressReason::Stale]);
        }
        assert!(card(&state, "a").is_none());
    }

    #[test]
    fn a_duplicate_turn_end_is_stale() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", Some("p1"), turn_ended("Done."));
        let effects = send(&mut state, 3, "a", Some("p1"), turn_ended("Done."));
        assert_eq!(reasons(&effects), vec![SuppressReason::Stale]);
        assert_eq!(card(&state, "a").map(|shown| shown.id.clone()), Some(CardId::for_event(&event_id(2))));
    }

    #[test]
    fn info_never_replaces_attention_and_attention_replaces_info() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnFailed { error: text("overloaded") });
        let effects = send(&mut state, 2, "a", None, explicit(Kind::Info, "x"));
        assert_eq!(reasons(&effects), vec![SuppressReason::Outranked]);
        assert_eq!(card(&state, "a").map(|shown| shown.cause), Some(CardCause::TurnFailed));

        send(&mut state, 3, "b", None, explicit(Kind::Info, "x"));
        send(&mut state, 4, "b", None, explicit(Kind::Attention, "y"));
        assert_eq!(card(&state, "b").map(|shown| shown.kind), Some(Kind::Attention));
    }

    #[test]
    fn turn_failed_shows_attention_even_while_quiet() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", None, AgentEvent::Quiet { until: QuietUntil::NextSend, max_minutes: 120 });
        send(&mut state, 2, "a", Some("p1"), AgentEvent::TurnFailed { error: text("overloaded") });
        let shown = card(&state, "a").expect("a card");
        assert_eq!((shown.kind, shown.body.as_str()), (Kind::Attention, "Stopped: overloaded"));
    }

    #[test]
    fn turn_started_clears_the_card() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnFailed { error: text("x") });
        send(&mut state, 2, "a", Some("p2"), AgentEvent::TurnStarted);
        assert!(card(&state, "a").is_none());
        assert!(matches!(state.sessions[&session("a")].turn, Turn::Working { explicit_sent: false, .. }));
    }

    #[test]
    fn a_new_session_in_a_pane_drops_the_old_record() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "old", Some("p1"), AgentEvent::TurnFailed { error: text("x") });
        send(&mut state, 2, "new", None, AgentEvent::SessionStarted { source: SessionSource::Clear });
        assert!(!state.sessions.contains_key(&session("old")));
        assert!(state.sessions.contains_key(&session("new")));
    }

    #[test]
    fn compact_with_the_same_id_keeps_the_record() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnFailed { error: text("x") });
        send(&mut state, 2, "a", None, AgentEvent::SessionStarted { source: SessionSource::Compact });
        assert!(card(&state, "a").is_some());
    }

    #[test]
    fn session_ended_removes_the_record_and_idles_the_pane() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        let effects = send(&mut state, 2, "a", None, AgentEvent::SessionEnded);
        assert!(state.sessions.is_empty());
        assert!(effects.contains(&Effect::SetPaneState { pane: pane(1), state: PaneState::Idle }));
    }

    #[test]
    fn an_event_for_an_unknown_session_creates_the_record() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), turn_ended("Done."));
        assert_eq!(card(&state, "a").map(|shown| shown.kind), Some(Kind::Info));
    }

    #[test]
    fn input_resolved_clears_only_an_elicitation_card() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), needs_permission());
        send(&mut state, 2, "a", Some("p1"), AgentEvent::InputResolved);
        assert!(card(&state, "a").is_some(), "a permission card is not an elicitation");

        send(&mut state, 3, "b", Some("p1"), AgentEvent::NeedsInput { reason: InputReason::Elicitation, message: text("Which?") });
        send(&mut state, 4, "b", Some("p1"), AgentEvent::InputResolved);
        assert!(card(&state, "b").is_none());
        assert!(matches!(state.sessions[&session("b")].turn, Turn::Working { .. }));
    }

    #[test]
    fn an_explicit_send_without_a_session_is_a_oneoff_card() {
        let mut state = DaemonState::default();
        let input = agent_input(1, origin(None, None), explicit(Kind::Info, "Report written"));
        update(&mut state, input, EpochMs(1));
        assert!(state.sessions.is_empty());
        assert_eq!(state.oneoff_cards.len(), 1);
        assert_eq!(state.oneoff_cards[0].location, "4 · code/feature-x");
    }

    #[test]
    fn a_title_survives_a_send_without_one() {
        let mut state = DaemonState::default();
        let mut titled = origin(Some("a"), Some("p1"));
        titled.session_title = Some(text("Fix sync retry"));
        update(&mut state, agent_input(1, titled, AgentEvent::TurnStarted), EpochMs(1));
        send(&mut state, 2, "a", None, explicit(Kind::Info, "x"));
        assert_eq!(state.sessions[&session("a")].title, Some(text("Fix sync retry")));
    }

    #[test]
    fn agent_events_report_the_pane_state() {
        let mut state = DaemonState::default();
        let effects = send(&mut state, 1, "a", Some("p1"), needs_permission());
        assert!(effects.contains(&Effect::SetPaneState { pane: pane(1), state: PaneState::NeedsYou }));
    }

    #[test]
    fn every_update_increases_the_revision() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", Some("p1"), AgentEvent::TurnStarted);
        assert_eq!(state.revision, 2);
    }
}
```

- [ ] **Step 4: Run the tests to see them fail**

Run: `cd ~/crates && cargo test -p agent-notify-core update`
Expected: compile errors for the missing `update` items.

- [ ] **Step 5: Implement `update.rs`**

```rust
//! The daemon's state machine: `update(state, input, now) -> effects`.
//!
//! Cards are not effects. The shell calls [`view`](crate::view::view) after
//! every update and pushes the result.

use crate::label::{location_line, turn_ended_body, turn_failed_body};
use crate::state::{Card, CardCause, CardNote, CardPhase, DaemonState, Dnd, FocusedPane, OneoffCard, QuietMode, SessionRecord, Turn};
use crate::wire::{AgentEvent, Control, DndCommand, InputReason, Kind, Origin, QuietUntil, Request};
use crate::{CardId, DisplayText, EpochMs, EventId, PaneId, PaneKey, PromptId, ServerId, SessionId};

/// How long an info card shows before it folds.
pub const INFO_SHOW_MS: i64 = 6_000;
/// Extra time before a fold, for a hover message still in transit.
pub const HOVER_GRACE_MS: i64 = 300;
/// The least time left on an info card after the pointer leaves the stack.
pub const HOVER_RESUME_MS: i64 = 2_000;
/// Continuous pane focus that clears the pane's card.
pub const FOCUS_DWELL_MS: i64 = 1_000;
/// How long the close button hides an attention card.
pub const SNOOZE_MS: i64 = 600_000;
/// The least time between two sounds.
pub const SOUND_GAP_MS: i64 = 3_000;
/// How long a "Session ended" card stays.
pub const SESSION_GONE_MS: i64 = 3_000;
/// How long a "window not raised" card stays.
pub const NOT_RAISED_MS: i64 = 2_000;
/// The user counts as present after input within this many seconds.
pub const IDLE_LIMIT_SECONDS: u32 = 30;

/// One input to [`update`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonInput {
    /// A decoded request, with the focus facts read when it arrived.
    Received {
        /// The request.
        request: Request,
        /// Focus facts at arrival.
        focus: FocusSnapshot,
    },
    /// A UI intent.
    Ui(UiIntent),
    /// The 250 ms timer.
    Tick,
    /// Pane tags read from one tmux server.
    PaneSnapshot(PaneSnapshot),
    /// The result of a focus plan.
    FocusOutcome {
        /// The clicked card.
        card: CardId,
        /// What happened.
        outcome: FocusOutcome,
    },
}

/// What the user did on the panel. Intents carry only ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiIntent {
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
    /// The pointer entered or left the stack.
    HoverChanged {
        /// True on enter.
        hovered: bool,
    },
}

/// Focus facts the shell reads when a request arrives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusSnapshot {
    /// Which app is frontmost.
    pub frontmost: Frontmost,
    /// The active pane of the client's current window.
    pub active_pane: Option<PaneKey>,
    /// Seconds since the last user input.
    pub idle_seconds: Option<u32>,
    /// True while the screen is locked.
    pub screen_locked: Option<bool>,
    /// True while the screen is shared or recorded.
    pub screen_shared: Option<bool>,
}

impl FocusSnapshot {
    /// No facts known. Every card shows.
    #[must_use]
    pub fn unknown() -> Self {
        FocusSnapshot { frontmost: Frontmost::Unknown, active_pane: None, idle_seconds: None, screen_locked: None, screen_shared: None }
    }
}

/// The frontmost app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frontmost {
    /// The terminal that hosts tmux.
    Terminal,
    /// Another app.
    Other,
    /// Not known on this platform.
    Unknown,
}

/// Pane tags read from one tmux server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneSnapshot {
    /// The server read.
    pub server: ServerId,
    /// Every pane on it. Empty when the server is gone.
    pub panes: Vec<PaneTag>,
}

/// One pane and its session tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneTag {
    /// The pane.
    pub pane: PaneId,
    /// `@agent_notify_session`.
    pub session: Option<SessionId>,
}

/// What a focus plan did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusOutcome {
    /// Switched and raised.
    Done,
    /// No pane carries the session tag.
    SessionGone,
    /// tmux refused.
    TmuxFailed(DisplayText),
    /// Switched, but the window did not come forward.
    RaiseFailed,
}

/// Work for the shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Read the focus world for `target` and run the focus plan.
    RunFocusPlan {
        /// The clicked card.
        card: CardId,
        /// Where to go.
        target: FocusTarget,
    },
    /// Play the attention sound.
    PlaySound,
    /// Write `@agent_notify_state` on a pane.
    SetPaneState {
        /// The pane.
        pane: PaneKey,
        /// The state.
        state: PaneState,
    },
    /// Save do-not-disturb to disk.
    PersistDnd(Dnd),
    /// An event made no new card, and why. Goes to the log and `status`.
    Suppressed {
        /// The event.
        event_id: EventId,
        /// Why.
        reason: SuppressReason,
    },
}

/// Where a click goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusTarget {
    /// The pane that carries `session`'s tag. `pane` was its last location.
    Session {
        /// The session.
        session: SessionId,
        /// Its last known pane.
        pane: PaneKey,
    },
    /// No pane: raise the terminal only.
    RaiseOnly,
}

/// The value of `@agent_notify_state`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneState {
    /// No turn, or the session ended.
    Idle,
    /// Working.
    Working,
    /// Finished.
    Finished,
    /// Waiting for the user.
    NeedsYou,
    /// Stopped on an error.
    Failed,
}

/// Why an event made no new card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuppressReason {
    /// The agent already sent its own notification this turn.
    ExplicitSent,
    /// The session is quiet.
    Quiet,
    /// An attention card outranks the new info card.
    Outranked,
    /// The user is looking at the pane.
    Focused,
    /// Do-not-disturb holds the card off screen.
    Dnd,
    /// The event belongs to an older or already-ended turn.
    Stale,
    /// The event never makes a card.
    NoCardForEvent,
}

/// The result of [`merge`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Merged {
    /// The existing card stays.
    Kept(Card),
    /// The incoming card replaces the old one.
    Replaced {
        /// The new card.
        card: Card,
        /// True when it replaced an attention card.
        replaced_attention: bool,
    },
}

/// One card per session: the higher kind wins, and on a tie the newer card.
/// An expiring card always gives way.
#[must_use]
pub fn merge(existing: Option<Card>, incoming: Card) -> Merged {
    match existing {
        Some(current) if current.kind > incoming.kind && !matches!(current.phase, CardPhase::Expiring { .. }) => Merged::Kept(current),
        Some(current) => Merged::Replaced { replaced_attention: current.kind == Kind::Attention, card: incoming },
        None => Merged::Replaced { card: incoming, replaced_attention: false },
    }
}

/// True when the user is looking at `pane`. Any unknown fact means "show".
#[must_use]
pub fn is_suppressed_by_focus(snapshot: &FocusSnapshot, pane: Option<&PaneKey>) -> bool {
    let Some(pane) = pane else { return false };
    snapshot.frontmost == Frontmost::Terminal
        && snapshot.active_pane.as_ref() == Some(pane)
        && snapshot.idle_seconds.is_some_and(|seconds| seconds < IDLE_LIMIT_SECONDS)
        && snapshot.screen_locked == Some(false)
}

/// Applies one input and returns the work the shell must do.
pub fn update(state: &mut DaemonState, input: DaemonInput, now: EpochMs) -> Vec<Effect> {
    state.revision += 1;
    let mut effects = Vec::new();
    match input {
        DaemonInput::Received { request, focus } => receive(state, request, &focus, now, &mut effects),
        DaemonInput::Ui(_) | DaemonInput::Tick | DaemonInput::PaneSnapshot(_) | DaemonInput::FocusOutcome { .. } => {}
    }
    effects
}

fn receive(state: &mut DaemonState, request: Request, focus: &FocusSnapshot, now: EpochMs, effects: &mut Vec<Effect>) {
    if let Some(shared) = focus.screen_shared {
        state.screen_shared = shared;
    }
    match request {
        Request::Agent { event_id, origin, event, .. } => agent_event(state, event_id, origin, event, focus, now, effects),
        Request::Control { control, .. } => control_event(state, control, now, effects),
        Request::Query { .. } | Request::Unknown => {}
    }
}

fn agent_event(state: &mut DaemonState, event_id: EventId, origin: Origin, event: AgentEvent, focus: &FocusSnapshot, now: EpochMs, effects: &mut Vec<Effect>) {
    let Some(session) = origin.session.clone() else {
        oneoff_event(state, event_id, &origin, event, focus, now, effects);
        return;
    };
    if matches!(event, AgentEvent::SessionStarted { .. })
        && let Some(pane) = &origin.pane
    {
        state.sessions.retain(|id, record| *id == session || record.pane.as_ref() != Some(pane));
    }
    if matches!(event, AgentEvent::SessionEnded) {
        if let Some(record) = state.sessions.remove(&session)
            && let Some(pane) = record.pane
        {
            effects.push(Effect::SetPaneState { pane, state: PaneState::Idle });
        }
        effects.push(Effect::Suppressed { event_id, reason: SuppressReason::NoCardForEvent });
        return;
    }
    let dnd = state.dnd_active(now);
    let record = state.sessions.entry(session).or_insert_with(|| SessionRecord::new(&origin));
    record.absorb(&origin);
    match step(record, origin.prompt.as_ref(), event, now) {
        Step::Offer(offer) => {
            if is_suppressed_by_focus(focus, record.pane.as_ref()) {
                effects.push(Effect::Suppressed { event_id, reason: SuppressReason::Focused });
            } else {
                offer_card(&mut record.card, offer, event_id, now, dnd, &mut state.last_sound_at, effects);
            }
        }
        Step::Clear => {
            record.card = None;
            effects.push(Effect::Suppressed { event_id, reason: SuppressReason::NoCardForEvent });
        }
        Step::Nothing(reason) => effects.push(Effect::Suppressed { event_id, reason }),
    }
    if let Some(pane) = &record.pane {
        effects.push(Effect::SetPaneState { pane: pane.clone(), state: pane_state(&record.turn) });
    }
}

fn oneoff_event(state: &mut DaemonState, event_id: EventId, origin: &Origin, event: AgentEvent, focus: &FocusSnapshot, now: EpochMs, effects: &mut Vec<Effect>) {
    let AgentEvent::Explicit { kind, body } = event else {
        effects.push(Effect::Suppressed { event_id, reason: SuppressReason::NoCardForEvent });
        return;
    };
    if is_suppressed_by_focus(focus, origin.pane.as_ref()) {
        effects.push(Effect::Suppressed { event_id, reason: SuppressReason::Focused });
        return;
    }
    let dnd = state.dnd_active(now);
    let mut slot = None;
    offer_card(&mut slot, CardOffer { kind, cause: CardCause::Explicit, body }, event_id, now, dnd, &mut state.last_sound_at, effects);
    if let Some(card) = slot {
        state.oneoff_cards.push(OneoffCard {
            location: location_line(origin.window.as_ref(), origin.repo.as_ref(), &origin.cwd),
            pane: origin.pane.clone(),
            card,
        });
    }
}

fn control_event(state: &mut DaemonState, control: Control, now: EpochMs, effects: &mut Vec<Effect>) {
    match control {
        Control::FocusChanged { pane, focused: true } => state.focus = Some(FocusedPane { pane, since: now }),
        Control::FocusChanged { pane, focused: false } => {
            if state.focus.as_ref().is_some_and(|focus| focus.pane == pane) {
                state.focus = None;
            }
        }
        Control::Dnd { command: DndCommand::On { for_minutes } } => {
            state.dnd = Dnd::On { until: for_minutes.map(|minutes| now.plus_ms(i64::from(minutes) * 60_000)) };
            effects.push(Effect::PersistDnd(state.dnd));
        }
        Control::Dnd { command: DndCommand::Off } => {
            end_dnd(state);
            effects.push(Effect::PersistDnd(Dnd::Off));
        }
        Control::Unknown => {}
    }
}

/// Turns DND off. Info cards that waited go to the finished row, so the
/// return to the desk is not a burst of stale toasts.
fn end_dnd(state: &mut DaemonState) {
    state.dnd = Dnd::Off;
    let sessions = state.sessions.values_mut().filter_map(|record| record.card.as_mut());
    let oneoffs = state.oneoff_cards.iter_mut().map(|oneoff| &mut oneoff.card);
    for card in sessions.chain(oneoffs) {
        if card.kind == Kind::Info && matches!(card.phase, CardPhase::Showing { .. }) {
            card.phase = CardPhase::Folded;
        }
    }
}

struct CardOffer {
    kind: Kind,
    cause: CardCause,
    body: DisplayText,
}

enum Step {
    Offer(CardOffer),
    Clear,
    Nothing(SuppressReason),
}

fn step(record: &mut SessionRecord, prompt: Option<&PromptId>, event: AgentEvent, now: EpochMs) -> Step {
    match event {
        AgentEvent::TurnStarted => {
            record.turn = Turn::Working { prompt: prompt.cloned(), explicit_sent: false };
            if record.quiet.ends_on(QuietUntil::NextPrompt) {
                record.quiet = QuietMode::Armed;
            }
            Step::Clear
        }
        AgentEvent::Explicit { kind, body } => {
            if let Turn::Working { explicit_sent, .. } = &mut record.turn {
                *explicit_sent = true;
            }
            if record.quiet.ends_on(QuietUntil::NextSend) {
                record.quiet = QuietMode::Armed;
            }
            Step::Offer(CardOffer { kind, cause: CardCause::Explicit, body })
        }
        AgentEvent::TurnEnded { last_message } => {
            if is_stale(&record.turn, prompt) {
                return Step::Nothing(SuppressReason::Stale);
            }
            let explicit_sent = matches!(record.turn, Turn::Working { explicit_sent: true, .. });
            record.turn = Turn::Finished { prompt: current_prompt(&record.turn, prompt) };
            if record.card.as_ref().is_some_and(|card| matches!(card.cause, CardCause::NeedsInput(_))) {
                record.card = None;
            }
            if explicit_sent {
                Step::Nothing(SuppressReason::ExplicitSent)
            } else if record.quiet != QuietMode::Armed {
                Step::Nothing(SuppressReason::Quiet)
            } else {
                Step::Offer(CardOffer { kind: Kind::Info, cause: CardCause::TurnEnded, body: turn_ended_body(last_message.as_ref()) })
            }
        }
        AgentEvent::TurnFailed { error } => {
            if is_stale(&record.turn, prompt) {
                return Step::Nothing(SuppressReason::Stale);
            }
            record.turn = Turn::Failed { prompt: current_prompt(&record.turn, prompt) };
            Step::Offer(CardOffer { kind: Kind::Attention, cause: CardCause::TurnFailed, body: turn_failed_body(&error) })
        }
        AgentEvent::NeedsInput { reason, message } => {
            if is_stale(&record.turn, prompt) {
                return Step::Nothing(SuppressReason::Stale);
            }
            record.turn = Turn::NeedsYou { prompt: current_prompt(&record.turn, prompt), reason };
            Step::Offer(CardOffer { kind: Kind::Attention, cause: CardCause::NeedsInput(reason), body: message })
        }
        AgentEvent::InputResolved => {
            if let Turn::NeedsYou { prompt: waiting, .. } = &record.turn {
                record.turn = Turn::Working { prompt: waiting.clone(), explicit_sent: false };
            }
            let elicitation = record.card.as_ref().is_some_and(|card| card.cause == CardCause::NeedsInput(InputReason::Elicitation));
            if elicitation { Step::Clear } else { Step::Nothing(SuppressReason::NoCardForEvent) }
        }
        AgentEvent::Quiet { until, max_minutes } => {
            record.quiet = QuietMode::Until { until, deadline: now.plus_ms(i64::from(max_minutes) * 60_000) };
            Step::Nothing(SuppressReason::NoCardForEvent)
        }
        AgentEvent::QuietOff => {
            record.quiet = QuietMode::Armed;
            Step::Nothing(SuppressReason::NoCardForEvent)
        }
        AgentEvent::SessionStarted { .. } | AgentEvent::SessionEnded | AgentEvent::Unknown => Step::Nothing(SuppressReason::NoCardForEvent),
    }
}

/// An event is stale when it names another turn than the record's, or
/// repeats the end of a turn that already ended.
fn is_stale(turn: &Turn, prompt: Option<&PromptId>) -> bool {
    let Some(event_prompt) = prompt else { return false };
    match turn.prompt() {
        Some(current) if current != event_prompt => true,
        Some(_) => matches!(turn, Turn::Finished { .. } | Turn::Failed { .. }),
        None => false,
    }
}

fn current_prompt(turn: &Turn, prompt: Option<&PromptId>) -> Option<PromptId> {
    prompt.cloned().or_else(|| turn.prompt().cloned())
}

fn offer_card(slot: &mut Option<Card>, offer: CardOffer, event_id: EventId, now: EpochMs, dnd: bool, last_sound_at: &mut Option<EpochMs>, effects: &mut Vec<Effect>) {
    let incoming = Card {
        id: CardId::for_event(&event_id),
        kind: offer.kind,
        cause: offer.cause,
        body: offer.body,
        event_id: event_id.clone(),
        created: now,
        phase: CardPhase::Showing { fade_at: (offer.kind == Kind::Info).then(|| now.plus_ms(INFO_SHOW_MS)) },
        note: None,
    };
    match merge(slot.take(), incoming) {
        Merged::Kept(existing) => {
            *slot = Some(existing);
            effects.push(Effect::Suppressed { event_id, reason: SuppressReason::Outranked });
        }
        Merged::Replaced { card, replaced_attention } => {
            let attention = card.kind == Kind::Attention;
            *slot = Some(card);
            if dnd {
                effects.push(Effect::Suppressed { event_id, reason: SuppressReason::Dnd });
                return;
            }
            let sound_allowed = last_sound_at.is_none_or(|at| now.since(at) >= SOUND_GAP_MS);
            if attention && !replaced_attention && sound_allowed {
                *last_sound_at = Some(now);
                effects.push(Effect::PlaySound);
            }
        }
    }
}

fn pane_state(turn: &Turn) -> PaneState {
    match turn {
        Turn::Idle => PaneState::Idle,
        Turn::Working { .. } => PaneState::Working,
        Turn::Finished { .. } => PaneState::Finished,
        Turn::NeedsYou { .. } => PaneState::NeedsYou,
        Turn::Failed { .. } => PaneState::Failed,
    }
}
```

Unused-import warnings for `CardNote`, `OneoffCard` uses, and the Task 10 inputs are expected to disappear in Task 10. If clippy fails on them now, remove the unused names from the `use` lines and add them back in Task 10.

- [ ] **Step 6: Run the tests and clippy**

Run: `cd ~/crates && cargo test -p agent-notify-core && cargo clippy -p agent-notify-core --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
cd ~ && config add crates/agent-notify-core
config commit -m "Add the agent-notify state machine for session and turn events" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/agent-notify-core
```

---

### Task 8: Suppression, do-not-disturb and sound

The code for these rules landed in Task 7 (`is_suppressed_by_focus`, `control_event`, `offer_card`). This task pins them with tests and fixes what the tests find. The `notify.sh` focus cases in `crates/config-cli/tests/notify.rs` map to the first four tests here.

**Files:**
- Modify: `crates/agent-notify-core/src/update.rs` (tests module)

**Interfaces:**
- Consumes: Task 7.
- Produces: nothing new.

- [ ] **Step 1: Write the tests**

Append inside the `tests` module of `update.rs`:

```rust
    use crate::state::Dnd;
    use crate::update::Frontmost;
    use crate::wire::{Control, DndCommand, Request, WIRE_VERSION};

    fn present_at(pane_number: u32) -> FocusSnapshot {
        FocusSnapshot {
            frontmost: Frontmost::Terminal,
            active_pane: Some(pane(pane_number)),
            idle_seconds: Some(5),
            screen_locked: Some(false),
            screen_shared: Some(false),
        }
    }

    fn with_focus(input: DaemonInput, focus: FocusSnapshot) -> DaemonInput {
        match input {
            DaemonInput::Received { request, .. } => DaemonInput::Received { request, focus },
            other => other,
        }
    }

    fn control(number: i64, control: Control) -> DaemonInput {
        DaemonInput::Received {
            request: Request::Control { v: WIRE_VERSION, event_id: event_id(number), sent_at_ms: EpochMs(number), control },
            focus: FocusSnapshot::unknown(),
        }
    }

    #[test]
    fn a_user_looking_at_the_pane_gets_no_card() {
        let mut state = DaemonState::default();
        let input = with_focus(agent_input(1, origin(Some("a"), Some("p1")), needs_permission()), present_at(1));
        let effects = update(&mut state, input, EpochMs(1));
        assert_eq!(reasons(&effects), vec![SuppressReason::Focused]);
        assert!(card(&state, "a").is_none());
    }

    #[test]
    fn each_missing_presence_fact_means_show() {
        let variants = [
            FocusSnapshot { frontmost: Frontmost::Other, ..present_at(1) },
            FocusSnapshot { frontmost: Frontmost::Unknown, ..present_at(1) },
            FocusSnapshot { active_pane: Some(pane(2)), ..present_at(1) },
            FocusSnapshot { idle_seconds: Some(IDLE_LIMIT_SECONDS), ..present_at(1) },
            FocusSnapshot { idle_seconds: None, ..present_at(1) },
            FocusSnapshot { screen_locked: Some(true), ..present_at(1) },
            FocusSnapshot { screen_locked: None, ..present_at(1) },
        ];
        for focus in variants {
            let mut state = DaemonState::default();
            let input = with_focus(agent_input(1, origin(Some("a"), Some("p1")), needs_permission()), focus.clone());
            update(&mut state, input, EpochMs(1));
            assert!(card(&state, "a").is_some(), "expected a card for {focus:?}");
        }
    }

    #[test]
    fn a_shared_screen_is_remembered_from_the_snapshot() {
        let mut state = DaemonState::default();
        let focus = FocusSnapshot { screen_shared: Some(true), ..FocusSnapshot::unknown() };
        update(&mut state, with_focus(agent_input(1, origin(Some("a"), None), AgentEvent::TurnStarted), focus), EpochMs(1));
        assert!(state.screen_shared);
    }

    #[test]
    fn the_first_attention_card_plays_a_sound_and_info_cards_do_not() {
        let mut state = DaemonState::default();
        let info = send(&mut state, 1, "a", None, explicit(Kind::Info, "x"));
        assert!(!info.contains(&Effect::PlaySound));
        let attention = send(&mut state, 2, "b", Some("p1"), needs_permission());
        assert!(attention.contains(&Effect::PlaySound));
    }

    #[test]
    fn sounds_are_at_least_three_seconds_apart() {
        let mut state = DaemonState::default();
        send(&mut state, 1_000, "a", Some("p1"), needs_permission());
        let too_soon = send(&mut state, 2_000, "b", Some("p1"), needs_permission());
        assert!(!too_soon.contains(&Effect::PlaySound));
        let later = send(&mut state, 1_000 + SOUND_GAP_MS, "c", Some("p1"), needs_permission());
        assert!(later.contains(&Effect::PlaySound));
    }

    #[test]
    fn attention_replacing_attention_in_one_session_is_silent() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), needs_permission());
        let replaced = send(&mut state, 10_000, "a", Some("p1"), needs_permission());
        assert!(!replaced.contains(&Effect::PlaySound));
    }

    #[test]
    fn dnd_keeps_cards_in_state_without_sound_and_persists() {
        let mut state = DaemonState::default();
        let on = update(&mut state, control(1, Control::Dnd { command: DndCommand::On { for_minutes: None } }), EpochMs(1));
        assert_eq!(on, vec![Effect::PersistDnd(Dnd::On { until: None })]);
        let effects = send(&mut state, 2, "a", Some("p1"), needs_permission());
        assert!(!effects.contains(&Effect::PlaySound));
        assert_eq!(reasons(&effects), vec![SuppressReason::Dnd]);
        assert!(card(&state, "a").is_some());
    }

    #[test]
    fn dnd_off_folds_waiting_info_cards_and_keeps_attention_showing() {
        let mut state = DaemonState::default();
        update(&mut state, control(1, Control::Dnd { command: DndCommand::On { for_minutes: None } }), EpochMs(1));
        send(&mut state, 2, "a", None, explicit(Kind::Info, "x"));
        send(&mut state, 3, "b", Some("p1"), needs_permission());
        let off = update(&mut state, control(4, Control::Dnd { command: DndCommand::Off }), EpochMs(4));
        assert_eq!(off, vec![Effect::PersistDnd(Dnd::Off)]);
        assert_eq!(card(&state, "a").map(|shown| shown.phase), Some(CardPhase::Folded));
        assert!(matches!(card(&state, "b").map(|shown| shown.phase), Some(CardPhase::Showing { .. })));
    }

    #[test]
    fn timed_dnd_is_active_until_its_end() {
        let mut state = DaemonState::default();
        update(&mut state, control(0, Control::Dnd { command: DndCommand::On { for_minutes: Some(30) } }), EpochMs(0));
        assert!(state.dnd_active(EpochMs(30 * 60_000 - 1)));
        assert!(!state.dnd_active(EpochMs(30 * 60_000)));
    }

    #[test]
    fn focus_events_track_the_focused_pane() {
        let mut state = DaemonState::default();
        update(&mut state, control(5, Control::FocusChanged { pane: pane(1), focused: true }), EpochMs(5));
        assert_eq!(state.focus.as_ref().map(|focus| (focus.pane.clone(), focus.since)), Some((pane(1), EpochMs(5))));
        update(&mut state, control(6, Control::FocusChanged { pane: pane(2), focused: false }), EpochMs(6));
        assert!(state.focus.is_some(), "focus-out of another pane changes nothing");
        update(&mut state, control(7, Control::FocusChanged { pane: pane(1), focused: false }), EpochMs(7));
        assert!(state.focus.is_none());
    }
```

Add `use crate::state::CardPhase;` to the tests module imports if the compiler asks for it.

- [ ] **Step 2: Run the tests**

Run: `cd ~/crates && cargo test -p agent-notify-core update`
Expected: all pass. A failure here means Task 7's code differs from the rule. Fix the code, not the test, unless the test contradicts spec section 6.4 or 6.5.

- [ ] **Step 3: Run clippy and commit**

```bash
cd ~/crates && cargo clippy -p agent-notify-core --all-targets -- -D warnings
cd ~ && config add crates/agent-notify-core
config commit -m "Pin suppression, do-not-disturb and sound rules with tests" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/agent-notify-core
```

---

### Task 9: The view

**Files:**
- Modify: `crates/agent-notify-core/src/view.rs`

**Interfaces:**
- Consumes: Task 7 state; Task 5 `location_line`.
- Produces: `view(&DaemonState, EpochMs) -> PanelViewModel`, `MAX_VISIBLE_INFO_CARDS: usize = 3`.

- [ ] **Step 1: Write the failing tests**

Append to `view.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{CardNote, CardPhase, DaemonState, Dnd, FocusedPane};
    use crate::test_support::{card, pane, send, session};
    use crate::wire::{AgentEvent, InputReason};
    use crate::DisplayText;

    fn needs(state: &mut DaemonState, number: i64, name: &str) {
        send(state, number, name, Some("p1"), AgentEvent::NeedsInput { reason: InputReason::Permission, message: DisplayText::new("Allow?") });
    }

    fn info(state: &mut DaemonState, number: i64, name: &str) {
        send(state, number, name, None, AgentEvent::Explicit { kind: Kind::Info, body: DisplayText::new("done") });
    }

    #[test]
    fn cards_are_newest_first_with_their_lines() {
        let mut state = DaemonState::default();
        needs(&mut state, 1, "a");
        info(&mut state, 2, "b");
        let model = view(&state, EpochMs(3));
        assert_eq!(model.revision, state.revision);
        let kinds: Vec<Kind> = model.cards.iter().map(|shown| shown.kind).collect();
        assert_eq!(kinds, vec![Kind::Info, Kind::Attention]);
        assert_eq!(model.cards[1].location, "4 · code/feature-x");
        assert_eq!(model.cards[1].body.as_deref(), Some("Allow?"));
    }

    #[test]
    fn at_most_three_info_cards_show_and_the_rest_fold() {
        let mut state = DaemonState::default();
        for (number, name) in [(1, "a"), (2, "b"), (3, "c"), (4, "d")] {
            info(&mut state, number, name);
        }
        let model = view(&state, EpochMs(5));
        assert_eq!(model.cards.len(), MAX_VISIBLE_INFO_CARDS);
        assert_eq!(model.finished_row.len(), 1);
        assert_eq!(model.finished_row[0].card_id, card(&state, "a").map(|shown| shown.id.clone()).expect("a card"));
        assert_eq!(model.finished_row[0].label, "4");
    }

    #[test]
    fn folded_and_snoozed_cards() {
        let mut state = DaemonState::default();
        info(&mut state, 1, "a");
        needs(&mut state, 2, "b");
        if let Some(record) = state.sessions.get_mut(&session("a")) && let Some(shown) = &mut record.card {
            shown.phase = CardPhase::Folded;
        }
        if let Some(record) = state.sessions.get_mut(&session("b")) && let Some(shown) = &mut record.card {
            shown.phase = CardPhase::Snoozed { until: EpochMs(1_000_000) };
        }
        let model = view(&state, EpochMs(3));
        assert!(model.cards.is_empty());
        assert_eq!(model.finished_row.len(), 1);
    }

    #[test]
    fn a_pane_focused_after_its_card_hides_the_card() {
        let mut state = DaemonState::default();
        needs(&mut state, 1, "a");
        state.focus = Some(FocusedPane { pane: pane(1), since: EpochMs(2) });
        assert!(view(&state, EpochMs(2)).cards.is_empty());
        state.focus = Some(FocusedPane { pane: pane(1), since: EpochMs(0) });
        assert_eq!(view(&state, EpochMs(2)).cards.len(), 1, "a card that arrived during focus still shows");
    }

    #[test]
    fn dnd_hides_everything() {
        let mut state = DaemonState::default();
        needs(&mut state, 1, "a");
        state.dnd = Dnd::On { until: None };
        let model = view(&state, EpochMs(2));
        assert!(model.dnd);
        assert!(model.cards.is_empty() && model.finished_row.is_empty());
    }

    #[test]
    fn a_shared_screen_hides_bodies() {
        let mut state = DaemonState::default();
        needs(&mut state, 1, "a");
        state.screen_shared = true;
        assert_eq!(view(&state, EpochMs(2)).cards[0].body, None);
    }

    #[test]
    fn notes_render_with_the_window_index() {
        let mut state = DaemonState::default();
        needs(&mut state, 1, "a");
        if let Some(record) = state.sessions.get_mut(&session("a")) && let Some(shown) = &mut record.card {
            shown.note = Some(CardNote::NotRaised);
        }
        assert_eq!(view(&state, EpochMs(2)).cards[0].note.as_deref(), Some("Switched to 4, window not raised"));
    }

    #[test]
    fn a_frozen_view_is_returned_with_the_current_revision() {
        let mut state = DaemonState::default();
        needs(&mut state, 1, "a");
        state.frozen_view = Some(view(&state, EpochMs(1)));
        info(&mut state, 2, "b");
        let model = view(&state, EpochMs(3));
        assert_eq!(model.cards.len(), 1, "new cards wait while the pointer is over the stack");
        assert_eq!(model.revision, state.revision);
    }
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cd ~/crates && cargo test -p agent-notify-core view`
Expected: `cannot find function view`.

- [ ] **Step 3: Implement**

Add to `view.rs`, after the types (extend the `use` lines as shown):

```rust
use crate::label::location_line;
use crate::state::{Card, CardNote, CardPhase, DaemonState, FocusedPane};
use crate::{DisplayText, PaneKey};

/// The most info cards on screen at once. Older ones join the finished row.
pub const MAX_VISIBLE_INFO_CARDS: usize = 3;

/// Builds the panel view model from `state`.
#[must_use]
pub fn view(state: &DaemonState, now: EpochMs) -> PanelViewModel {
    if let Some(frozen) = &state.frozen_view {
        return PanelViewModel { revision: state.revision, ..frozen.clone() };
    }
    let dnd = state.dnd_active(now);
    if dnd {
        return PanelViewModel { revision: state.revision, cards: Vec::new(), finished_row: Vec::new(), dnd };
    }
    let mut showing = Vec::new();
    let mut folded = Vec::new();
    for placed in placed_cards(state) {
        if hidden_by_focus(state.focus.as_ref(), &placed) {
            continue;
        }
        match placed.card.phase {
            CardPhase::Showing { .. } | CardPhase::Expiring { .. } => showing.push(placed),
            CardPhase::Folded => folded.push(placed),
            CardPhase::Snoozed { .. } => {}
        }
    }
    showing.sort_by(|left, right| right.card.created.cmp(&left.card.created));
    let mut cards = Vec::new();
    let mut info_shown = 0;
    for placed in showing {
        if placed.card.kind == Kind::Info {
            info_shown += 1;
            if info_shown > MAX_VISIBLE_INFO_CARDS {
                folded.push(placed);
                continue;
            }
        }
        cards.push(card_view(&placed, state.screen_shared));
    }
    folded.sort_by(|left, right| left.card.created.cmp(&right.card.created));
    let finished_row = folded.iter().map(finished_entry).collect();
    PanelViewModel { revision: state.revision, cards, finished_row, dnd }
}

struct Placed<'a> {
    location: String,
    window_index: Option<u32>,
    title: Option<&'a DisplayText>,
    pane: Option<&'a PaneKey>,
    card: &'a Card,
}

fn placed_cards(state: &DaemonState) -> Vec<Placed<'_>> {
    let sessions = state.sessions.values().filter_map(|record| {
        record.card.as_ref().map(|card| Placed {
            location: location_line(record.window.as_ref(), record.repo.as_ref(), &record.cwd),
            window_index: record.window.as_ref().map(|window| window.index),
            title: record.title.as_ref(),
            pane: record.pane.as_ref(),
            card,
        })
    });
    let oneoffs = state.oneoff_cards.iter().map(|oneoff| Placed {
        location: oneoff.location.clone(),
        window_index: None,
        title: None,
        pane: oneoff.pane.as_ref(),
        card: &oneoff.card,
    });
    sessions.chain(oneoffs).collect()
}

/// Focusing a pane hides its card at once. A card that arrived while the pane
/// already had focus still shows: the focus check at arrival let it through.
fn hidden_by_focus(focus: Option<&FocusedPane>, placed: &Placed<'_>) -> bool {
    focus.is_some_and(|focus| placed.pane == Some(&focus.pane) && focus.since >= placed.card.created)
}

fn card_view(placed: &Placed<'_>, screen_shared: bool) -> CardView {
    CardView {
        card_id: placed.card.id.clone(),
        kind: placed.card.kind,
        location: placed.location.clone(),
        title: placed.title.map(|title| title.as_str().to_string()),
        body: (!screen_shared).then(|| placed.card.body.as_str().to_string()),
        created_ms: placed.card.created,
        note: placed.card.note.as_ref().map(|note| note_text(note, placed.window_index)),
    }
}

fn note_text(note: &CardNote, window_index: Option<u32>) -> String {
    match note {
        CardNote::SessionEnded => "Session ended".to_string(),
        CardNote::NotRaised => window_index.map_or_else(
            || "Switched, window not raised".to_string(),
            |index| format!("Switched to {index}, window not raised"),
        ),
        CardNote::SwitchFailed(reason) => format!("Switch failed: {}", reason.as_str()),
    }
}

fn finished_entry(placed: &Placed<'_>) -> FinishedEntry {
    FinishedEntry {
        card_id: placed.card.id.clone(),
        label: placed.window_index.map_or_else(|| placed.location.clone(), |index| index.to_string()),
    }
}
```

- [ ] **Step 4: Run the tests and clippy**

Run: `cd ~/crates && cargo test -p agent-notify-core && cargo clippy -p agent-notify-core --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
cd ~ && config add crates/agent-notify-core
config commit -m "Add the panel view model" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/agent-notify-core
```

---

### Task 10: UI intents, ticks, pane snapshots and focus outcomes

**Files:**
- Modify: `crates/agent-notify-core/src/update.rs`

**Interfaces:**
- Consumes: Tasks 7 and 9.
- Produces: `update` handles every `DaemonInput` variant.

- [ ] **Step 1: Write the failing tests**

Append inside the `tests` module of `update.rs`:

```rust
    use crate::state::CardNote;
    use crate::update::{FocusOutcome, PaneSnapshot, PaneTag, UiIntent};
    use crate::test_support::server;
    use crate::{PaneId, ServerId};

    fn card_id_of(state: &DaemonState, name: &str) -> CardId {
        card(state, name).map(|shown| shown.id.clone()).expect("a card")
    }

    fn tick(state: &mut DaemonState, now: i64) -> Vec<Effect> {
        update(state, DaemonInput::Tick, EpochMs(now))
    }

    #[test]
    fn an_info_card_folds_after_its_show_time_plus_grace() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", None, explicit(Kind::Info, "x"));
        tick(&mut state, INFO_SHOW_MS + HOVER_GRACE_MS - 1);
        assert!(matches!(card(&state, "a").map(|shown| shown.phase), Some(CardPhase::Showing { .. })));
        tick(&mut state, INFO_SHOW_MS + HOVER_GRACE_MS);
        assert_eq!(card(&state, "a").map(|shown| shown.phase), Some(CardPhase::Folded));
    }

    #[test]
    fn hover_holds_the_fold_and_leaving_gives_time_back() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", None, explicit(Kind::Info, "x"));
        update(&mut state, DaemonInput::Ui(UiIntent::HoverChanged { hovered: true }), EpochMs(1_000));
        assert!(state.frozen_view.is_some());
        tick(&mut state, 60_000);
        assert!(matches!(card(&state, "a").map(|shown| shown.phase), Some(CardPhase::Showing { .. })));
        update(&mut state, DaemonInput::Ui(UiIntent::HoverChanged { hovered: false }), EpochMs(60_000));
        assert!(state.frozen_view.is_none());
        tick(&mut state, 60_000 + HOVER_RESUME_MS + HOVER_GRACE_MS - 1);
        assert!(matches!(card(&state, "a").map(|shown| shown.phase), Some(CardPhase::Showing { .. })));
        tick(&mut state, 60_000 + HOVER_RESUME_MS + HOVER_GRACE_MS);
        assert_eq!(card(&state, "a").map(|shown| shown.phase), Some(CardPhase::Folded));
    }

    #[test]
    fn attention_cards_never_fold() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", Some("p1"), needs_permission());
        tick(&mut state, 3_600_000);
        assert!(matches!(card(&state, "a").map(|shown| shown.phase), Some(CardPhase::Showing { fade_at: None })));
    }

    #[test]
    fn a_focus_dwell_clears_a_card_that_was_there_before_the_focus() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", Some("p1"), needs_permission());
        update(&mut state, control(10, Control::FocusChanged { pane: pane(1), focused: true }), EpochMs(10));
        tick(&mut state, 10 + FOCUS_DWELL_MS - 1);
        assert!(card(&state, "a").is_some());
        tick(&mut state, 10 + FOCUS_DWELL_MS);
        assert!(card(&state, "a").is_none());
    }

    #[test]
    fn leaving_before_the_dwell_keeps_the_card() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", Some("p1"), needs_permission());
        update(&mut state, control(10, Control::FocusChanged { pane: pane(1), focused: true }), EpochMs(10));
        update(&mut state, control(500, Control::FocusChanged { pane: pane(1), focused: false }), EpochMs(500));
        tick(&mut state, 5_000);
        assert!(card(&state, "a").is_some());
    }

    #[test]
    fn a_card_that_arrives_during_focus_is_not_cleared_by_that_focus() {
        let mut state = DaemonState::default();
        update(&mut state, control(0, Control::FocusChanged { pane: pane(1), focused: true }), EpochMs(0));
        send(&mut state, 100, "a", Some("p1"), needs_permission());
        tick(&mut state, 10_000);
        assert!(card(&state, "a").is_some());
    }

    #[test]
    fn quiet_ends_at_its_deadline() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", None, AgentEvent::Quiet { until: QuietUntil::NextSend, max_minutes: 2 });
        tick(&mut state, 2 * 60_000 - 1);
        assert_ne!(state.sessions[&session("a")].quiet, QuietMode::Armed);
        tick(&mut state, 2 * 60_000);
        assert_eq!(state.sessions[&session("a")].quiet, QuietMode::Armed);
    }

    #[test]
    fn timed_dnd_ends_on_a_tick() {
        let mut state = DaemonState::default();
        update(&mut state, control(0, Control::Dnd { command: DndCommand::On { for_minutes: Some(1) } }), EpochMs(0));
        let effects = tick(&mut state, 60_000);
        assert_eq!(state.dnd, Dnd::Off);
        assert_eq!(effects, vec![Effect::PersistDnd(Dnd::Off)]);
    }

    #[test]
    fn a_click_asks_for_the_focus_plan() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", Some("p1"), needs_permission());
        let id = card_id_of(&state, "a");
        let effects = update(&mut state, DaemonInput::Ui(UiIntent::Clicked { card_id: id.clone() }), EpochMs(1));
        assert_eq!(effects, vec![Effect::RunFocusPlan { card: id, target: FocusTarget::Session { session: session("a"), pane: pane(1) } }]);
    }

    #[test]
    fn intents_for_unknown_cards_do_nothing() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", Some("p1"), needs_permission());
        let before = state.clone();
        let stale = CardId::for_event(&event_id(999));
        assert!(update(&mut state, DaemonInput::Ui(UiIntent::Clicked { card_id: stale.clone() }), EpochMs(1)).is_empty());
        update(&mut state, DaemonInput::Ui(UiIntent::Closed { card_id: stale }), EpochMs(1));
        assert_eq!(state.sessions, before.sessions);
    }

    #[test]
    fn closing_info_removes_it_and_closing_attention_snoozes_it() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", None, explicit(Kind::Info, "x"));
        send(&mut state, 1, "b", Some("p1"), needs_permission());
        let info = card_id_of(&state, "a");
        let attention = card_id_of(&state, "b");
        update(&mut state, DaemonInput::Ui(UiIntent::Closed { card_id: info }), EpochMs(2));
        update(&mut state, DaemonInput::Ui(UiIntent::Closed { card_id: attention }), EpochMs(2));
        assert!(card(&state, "a").is_none());
        assert_eq!(card(&state, "b").map(|shown| shown.phase), Some(CardPhase::Snoozed { until: EpochMs(2 + SNOOZE_MS) }));
        tick(&mut state, 2 + SNOOZE_MS);
        assert_eq!(card(&state, "b").map(|shown| shown.phase), Some(CardPhase::Showing { fade_at: None }));
    }

    #[test]
    fn focus_outcomes() {
        let mut state = DaemonState::default();
        for (number, name) in [(0, "done"), (1, "gone"), (2, "tmux"), (3, "raise")] {
            send(&mut state, number, name, Some("p1"), needs_permission());
        }
        let outcome = |state: &mut DaemonState, name: &str, outcome: FocusOutcome| {
            let card_id = card_id_of(state, name);
            update(state, DaemonInput::FocusOutcome { card: card_id, outcome }, EpochMs(10));
        };
        outcome(&mut state, "done", FocusOutcome::Done);
        outcome(&mut state, "gone", FocusOutcome::SessionGone);
        outcome(&mut state, "tmux", FocusOutcome::TmuxFailed(DisplayText::new("no client")));
        outcome(&mut state, "raise", FocusOutcome::RaiseFailed);
        assert!(card(&state, "done").is_none());
        assert_eq!(card(&state, "gone").map(|shown| (shown.note.clone(), shown.phase)), Some((Some(CardNote::SessionEnded), CardPhase::Expiring { remove_at: EpochMs(10 + SESSION_GONE_MS) })));
        assert_eq!(card(&state, "tmux").and_then(|shown| shown.note.clone()), Some(CardNote::SwitchFailed(DisplayText::new("no client"))));
        assert_eq!(card(&state, "raise").map(|shown| shown.phase), Some(CardPhase::Expiring { remove_at: EpochMs(10 + NOT_RAISED_MS) }));
        tick(&mut state, 10 + SESSION_GONE_MS);
        assert!(card(&state, "gone").is_none() && card(&state, "raise").is_none());
    }

    #[test]
    fn a_pane_snapshot_drops_dead_and_retagged_sessions() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "kept", None, AgentEvent::TurnStarted);
        let mut other = origin(Some("moved"), None);
        other.pane = Some(pane(2));
        update(&mut state, agent_input(1, other, AgentEvent::TurnStarted), EpochMs(1));
        let mut gone = origin(Some("gone"), None);
        gone.pane = Some(pane(3));
        update(&mut state, agent_input(2, gone, AgentEvent::TurnStarted), EpochMs(2));
        let snapshot = PaneSnapshot {
            server: server(),
            panes: vec![
                PaneTag { pane: PaneId::parse("%1").expect("pane"), session: Some(session("kept")) },
                PaneTag { pane: PaneId::parse("%2").expect("pane"), session: Some(session("someone-else")) },
            ],
        };
        update(&mut state, DaemonInput::PaneSnapshot(snapshot), EpochMs(3));
        let names: Vec<&str> = state.sessions.keys().map(SessionId::as_str).collect();
        assert_eq!(names, vec!["kept"]);
    }

    #[test]
    fn a_new_server_on_the_same_socket_drops_the_old_servers_sessions() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", None, AgentEvent::TurnStarted);
        let restarted = ServerId { pid: 200, ..server() };
        update(&mut state, DaemonInput::PaneSnapshot(PaneSnapshot { server: restarted, panes: Vec::new() }), EpochMs(1));
        assert!(state.sessions.is_empty());
    }
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cd ~/crates && cargo test -p agent-notify-core update`
Expected: the new tests fail (ticks and intents do nothing yet).

- [ ] **Step 3: Implement**

In `update`, replace the no-op arm:

```rust
        DaemonInput::Ui(intent) => ui_intent(state, intent, now, &mut effects),
        DaemonInput::Tick => tick(state, now, &mut effects),
        DaemonInput::PaneSnapshot(snapshot) => reap(state, &snapshot),
        DaemonInput::FocusOutcome { card, outcome } => focus_outcome(state, &card, outcome, now),
```

Add these functions to `update.rs`:

```rust
fn cards_mut(state: &mut DaemonState) -> impl Iterator<Item = &mut Card> {
    let sessions = state.sessions.values_mut().filter_map(|record| record.card.as_mut());
    let oneoffs = state.oneoff_cards.iter_mut().map(|oneoff| &mut oneoff.card);
    sessions.chain(oneoffs)
}

fn remove_card(state: &mut DaemonState, card_id: &CardId) {
    for record in state.sessions.values_mut() {
        if record.card.as_ref().is_some_and(|card| &card.id == card_id) {
            record.card = None;
        }
    }
    state.oneoff_cards.retain(|oneoff| &oneoff.card.id != card_id);
}

fn ui_intent(state: &mut DaemonState, intent: UiIntent, now: EpochMs, effects: &mut Vec<Effect>) {
    match intent {
        UiIntent::Clicked { card_id } => {
            if let Some(target) = click_target(state, &card_id) {
                effects.push(Effect::RunFocusPlan { card: card_id, target });
            }
        }
        UiIntent::Closed { card_id } => close(state, &card_id, now),
        UiIntent::HoverChanged { hovered: true } => {
            if !state.stack_hovered {
                state.frozen_view = Some(crate::view::view(state, now));
                state.stack_hovered = true;
            }
        }
        UiIntent::HoverChanged { hovered: false } => {
            state.stack_hovered = false;
            state.frozen_view = None;
            let resume_at = now.plus_ms(HOVER_RESUME_MS);
            for card in cards_mut(state) {
                if let CardPhase::Showing { fade_at: Some(fade_at) } = &mut card.phase {
                    *fade_at = (*fade_at).max(resume_at);
                }
            }
        }
    }
}

fn click_target(state: &DaemonState, card_id: &CardId) -> Option<FocusTarget> {
    for (session, record) in &state.sessions {
        if record.card.as_ref().is_some_and(|card| &card.id == card_id) {
            return Some(match &record.pane {
                Some(pane) => FocusTarget::Session { session: session.clone(), pane: pane.clone() },
                None => FocusTarget::RaiseOnly,
            });
        }
    }
    state.oneoff_cards.iter().any(|oneoff| &oneoff.card.id == card_id).then_some(FocusTarget::RaiseOnly)
}

fn close(state: &mut DaemonState, card_id: &CardId, now: EpochMs) {
    for record in state.sessions.values_mut() {
        let Some(kind) = record.card.as_ref().filter(|card| &card.id == card_id).map(|card| card.kind) else {
            continue;
        };
        match kind {
            Kind::Info => record.card = None,
            Kind::Attention => {
                if let Some(card) = &mut record.card {
                    card.phase = CardPhase::Snoozed { until: now.plus_ms(SNOOZE_MS) };
                }
            }
        }
        return;
    }
    state.oneoff_cards.retain(|oneoff| &oneoff.card.id != card_id);
}

fn tick(state: &mut DaemonState, now: EpochMs, effects: &mut Vec<Effect>) {
    if let Dnd::On { until: Some(until) } = state.dnd
        && now >= until
    {
        end_dnd(state);
        effects.push(Effect::PersistDnd(Dnd::Off));
    }
    let dwelled = state.focus.as_ref().filter(|focus| now.since(focus.since) >= FOCUS_DWELL_MS).cloned();
    let hovered = state.stack_hovered;
    for record in state.sessions.values_mut() {
        if let QuietMode::Until { deadline, .. } = record.quiet
            && now >= deadline
        {
            record.quiet = QuietMode::Armed;
        }
        let dwell_clears = dwelled.as_ref().is_some_and(|focus| {
            record.pane.as_ref() == Some(&focus.pane) && record.card.as_ref().is_some_and(|card| card.created <= focus.since)
        });
        if dwell_clears {
            record.card = None;
        }
        let keep = record.card.as_mut().is_none_or(|card| advance(card, now, hovered));
        if !keep {
            record.card = None;
        }
    }
    state.oneoff_cards.retain_mut(|oneoff| advance(&mut oneoff.card, now, hovered));
}

/// Moves a card along its timers. Returns false when the card must go.
fn advance(card: &mut Card, now: EpochMs, hovered: bool) -> bool {
    match card.phase {
        CardPhase::Showing { fade_at: Some(fade_at) } if !hovered && now.since(fade_at) >= HOVER_GRACE_MS => {
            card.phase = CardPhase::Folded;
        }
        CardPhase::Snoozed { until } if now >= until => card.phase = CardPhase::Showing { fade_at: None },
        CardPhase::Expiring { remove_at } if now >= remove_at => return false,
        _ => {}
    }
    true
}

/// Drops records whose pane is gone, whose pane now names another session, or
/// whose server was replaced by a new one on the same socket.
fn reap(state: &mut DaemonState, snapshot: &PaneSnapshot) {
    state.sessions.retain(|session, record| {
        let Some(pane) = &record.pane else { return true };
        if pane.server.socket_path != snapshot.server.socket_path {
            return true;
        }
        if pane.server != snapshot.server {
            return false;
        }
        snapshot.panes.iter().any(|tag| tag.pane == pane.pane && tag.session.as_ref() == Some(session))
    });
}

fn focus_outcome(state: &mut DaemonState, card_id: &CardId, outcome: FocusOutcome, now: EpochMs) {
    if outcome == FocusOutcome::Done {
        remove_card(state, card_id);
        return;
    }
    let Some(card) = cards_mut(state).find(|card| &card.id == card_id) else { return };
    match outcome {
        FocusOutcome::Done => {}
        FocusOutcome::SessionGone => {
            card.note = Some(CardNote::SessionEnded);
            card.phase = CardPhase::Expiring { remove_at: now.plus_ms(SESSION_GONE_MS) };
        }
        FocusOutcome::TmuxFailed(reason) => card.note = Some(CardNote::SwitchFailed(reason)),
        FocusOutcome::RaiseFailed => {
            card.note = Some(CardNote::NotRaised);
            card.phase = CardPhase::Expiring { remove_at: now.plus_ms(NOT_RAISED_MS) };
        }
    }
}
```

Use `cards_mut` in `end_dnd` too, replacing its two local iterators.

- [ ] **Step 4: Run the tests and clippy**

Run: `cd ~/crates && cargo test -p agent-notify-core && cargo clippy -p agent-notify-core --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
cd ~ && config add crates/agent-notify-core
config commit -m "Handle UI intents, ticks, pane snapshots and focus outcomes" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/agent-notify-core
```

---

### Task 11: The focus plan

**Files:**
- Create: `crates/agent-notify-core/src/focus.rs`
- Modify: `src/lib.rs` (add `pub mod focus;`)

**Interfaces:**
- Consumes: Task 7 `FocusTarget`.
- Produces:
  - `FocusWorld { panes: Vec<WorldPane>, clients: Vec<WorldClient> }`, `WorldPane { pane: PaneId, session_tag: Option<SessionId>, tmux_session: String }`, `WorldClient { tty: String, tmux_session: String, activity: i64 }`.
  - `FocusPlan::{SessionGone, RaiseOnly, Focus { pane: PaneId, switch: TmuxSwitch }}`; `TmuxSwitch::{InAttachedClient, SwitchClient { tty: String }, AttachNewTerminal { tmux_session: String }}`.
  - `plan_focus(&FocusTarget, world: Option<&FocusWorld>) -> FocusPlan`. `world` is `None` when the target's tmux server is gone.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{pane, session};

    fn world_pane(number: u32, tag: Option<&str>, tmux_session: &str) -> WorldPane {
        WorldPane { pane: pane(number).pane, session_tag: tag.map(session), tmux_session: tmux_session.to_string() }
    }

    fn client(tty: &str, tmux_session: &str, activity: i64) -> WorldClient {
        WorldClient { tty: tty.to_string(), tmux_session: tmux_session.to_string(), activity }
    }

    fn target(last_pane: u32) -> FocusTarget {
        FocusTarget::Session { session: session("a"), pane: pane(last_pane) }
    }

    #[test]
    fn targets_the_tagged_pane_in_an_attached_client() {
        let world = FocusWorld { panes: vec![world_pane(1, Some("b"), "code"), world_pane(5, Some("a"), "code")], clients: vec![client("/dev/ttys000", "code", 10)] };
        assert_eq!(plan_focus(&target(1), Some(&world)), FocusPlan::Focus { pane: pane(5).pane, switch: TmuxSwitch::InAttachedClient });
    }

    #[test]
    fn switches_the_most_recent_client_when_none_shows_the_session() {
        let world = FocusWorld {
            panes: vec![world_pane(5, Some("a"), "work")],
            clients: vec![client("/dev/ttys001", "code", 10), client("/dev/ttys002", "code", 20)],
        };
        assert_eq!(plan_focus(&target(5), Some(&world)), FocusPlan::Focus { pane: pane(5).pane, switch: TmuxSwitch::SwitchClient { tty: "/dev/ttys002".to_string() } });
    }

    #[test]
    fn opens_a_terminal_when_no_client_exists() {
        let world = FocusWorld { panes: vec![world_pane(5, Some("a"), "work")], clients: Vec::new() };
        assert_eq!(plan_focus(&target(5), Some(&world)), FocusPlan::Focus { pane: pane(5).pane, switch: TmuxSwitch::AttachNewTerminal { tmux_session: "work".to_string() } });
    }

    #[test]
    fn no_tag_or_no_server_means_the_session_is_gone() {
        let untagged = FocusWorld { panes: vec![world_pane(5, None, "code")], clients: Vec::new() };
        assert_eq!(plan_focus(&target(5), Some(&untagged)), FocusPlan::SessionGone);
        assert_eq!(plan_focus(&target(5), None), FocusPlan::SessionGone);
    }

    #[test]
    fn a_tag_on_two_panes_prefers_the_last_known_pane_then_the_lowest() {
        let world = FocusWorld { panes: vec![world_pane(9, Some("a"), "code"), world_pane(3, Some("a"), "code")], clients: vec![client("/dev/ttys000", "code", 1)] };
        assert!(matches!(plan_focus(&target(9), Some(&world)), FocusPlan::Focus { pane, .. } if pane == pane_id(9)));
        assert!(matches!(plan_focus(&target(1), Some(&world)), FocusPlan::Focus { pane, .. } if pane == pane_id(3)));
    }

    #[test]
    fn raise_only_targets_raise_only() {
        assert_eq!(plan_focus(&FocusTarget::RaiseOnly, None), FocusPlan::RaiseOnly);
    }

    fn pane_id(number: u32) -> PaneId {
        pane(number).pane
    }
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cd ~/crates && cargo test -p agent-notify-core focus`
Expected: compile errors.

- [ ] **Step 3: Implement**

```rust
//! Where a click goes. The shell reads a [`FocusWorld`] snapshot for the
//! card's tmux server, [`plan_focus`] decides, and the shell runs the plan
//! with argv only and checks the tag once more just before the switch.

use crate::update::FocusTarget;
use crate::{PaneId, SessionId};

/// One tmux server as the shell read it at click time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusWorld {
    /// Every pane on the server.
    pub panes: Vec<WorldPane>,
    /// Every attached client.
    pub clients: Vec<WorldClient>,
}

/// One pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldPane {
    /// The pane.
    pub pane: PaneId,
    /// `@agent_notify_session`.
    pub session_tag: Option<SessionId>,
    /// The tmux session that holds the pane.
    pub tmux_session: String,
}

/// One attached client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldClient {
    /// `#{client_tty}`.
    pub tty: String,
    /// `#{client_session}`.
    pub tmux_session: String,
    /// `#{client_activity}`.
    pub activity: i64,
}

/// What the shell does for a click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusPlan {
    /// No pane carries the tag. The card says "Session ended".
    SessionGone,
    /// Raise the terminal only.
    RaiseOnly,
    /// Select `pane`, switch a client to it, then raise the terminal.
    Focus {
        /// The target pane.
        pane: PaneId,
        /// How a client gets there.
        switch: TmuxSwitch,
    },
}

/// How a tmux client reaches the target pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TmuxSwitch {
    /// A client already shows the pane's session: select window and pane.
    InAttachedClient,
    /// Switch the most recently active client.
    SwitchClient {
        /// The client.
        tty: String,
    },
    /// No client: start the terminal attached to the session.
    AttachNewTerminal {
        /// The tmux session.
        tmux_session: String,
    },
}

/// Decides the focus plan for `target` in `world`.
#[must_use]
pub fn plan_focus(target: &FocusTarget, world: Option<&FocusWorld>) -> FocusPlan {
    let FocusTarget::Session { session, pane: last_pane } = target else {
        return FocusPlan::RaiseOnly;
    };
    let Some(world) = world else { return FocusPlan::SessionGone };
    let mut tagged: Vec<&WorldPane> = world.panes.iter().filter(|pane| pane.session_tag.as_ref() == Some(session)).collect();
    tagged.sort_by_key(|pane| pane.pane.number());
    let preferred = tagged.iter().copied().find(|pane| pane.pane == last_pane.pane);
    let Some(chosen) = preferred.or_else(|| tagged.first().copied()) else {
        return FocusPlan::SessionGone;
    };
    let shown = world.clients.iter().any(|client| client.tmux_session == chosen.tmux_session);
    let switch = if shown {
        TmuxSwitch::InAttachedClient
    } else {
        match world.clients.iter().max_by_key(|client| client.activity) {
            Some(client) => TmuxSwitch::SwitchClient { tty: client.tty.clone() },
            None => TmuxSwitch::AttachNewTerminal { tmux_session: chosen.tmux_session.clone() },
        }
    };
    FocusPlan::Focus { pane: chosen.pane.clone(), switch }
}
```

- [ ] **Step 4: Run the tests and clippy, then commit**

```bash
cd ~/crates && cargo test -p agent-notify-core && cargo clippy -p agent-notify-core --all-targets -- -D warnings
cd ~ && config add crates/agent-notify-core
config commit -m "Add the focus plan" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/agent-notify-core
```

---

### Task 12: Property tests over event sequences

**Files:**
- Create: `crates/agent-notify-core/tests/properties.rs`

**Interfaces:**
- Consumes: the public API of Tasks 2 to 10.
- Produces: nothing.

- [ ] **Step 1: Write the tests**

```rust
//! Properties of `update` over generated event sequences for one session.
//! The reference model restates only the `TurnEnded` card rule (spec 6.2),
//! because that rule combines four inputs: staleness, explicit sends, quiet,
//! and a pending attention card.

use std::path::{Path, PathBuf};

use agent_notify_core::state::{CardCause, DaemonState};
use agent_notify_core::update::{DaemonInput, Effect, FocusSnapshot, update};
use agent_notify_core::wire::{AgentEvent, AgentKind, InputReason, Kind, Origin, QuietUntil, Request, WIRE_VERSION};
use agent_notify_core::{CardId, DisplayText, EpochMs, EventId, PaneId, PaneKey, PromptId, ServerId, SessionId, SocketPath};
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Op {
    Start(u8),
    Explicit(Kind),
    End(u8),
    Fail(u8),
    Needs(u8),
    Resolved,
    Quiet(QuietUntil),
    QuietOff,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0_u8..3).prop_map(Op::Start),
        prop_oneof![Just(Kind::Info), Just(Kind::Attention)].prop_map(Op::Explicit),
        (0_u8..3).prop_map(Op::End),
        (0_u8..3).prop_map(Op::Fail),
        (0_u8..3).prop_map(Op::Needs),
        Just(Op::Resolved),
        prop_oneof![Just(QuietUntil::NextSend), Just(QuietUntil::NextPrompt)].prop_map(Op::Quiet),
        Just(Op::QuietOff),
    ]
}

fn prompt_id(number: u8) -> PromptId {
    PromptId::parse(&format!("p{number}")).expect("a prompt id")
}

fn event_id(number: usize) -> EventId {
    EventId::from_parts(EpochMs(i64::try_from(number).expect("small")), [0; 10])
}

fn to_event(op: &Op) -> (Option<PromptId>, AgentEvent) {
    let text = DisplayText::new;
    match op {
        Op::Start(prompt) => (Some(prompt_id(*prompt)), AgentEvent::TurnStarted),
        Op::Explicit(kind) => (None, AgentEvent::Explicit { kind: *kind, body: text("x") }),
        Op::End(prompt) => (Some(prompt_id(*prompt)), AgentEvent::TurnEnded { last_message: Some(text("Done.")) }),
        Op::Fail(prompt) => (Some(prompt_id(*prompt)), AgentEvent::TurnFailed { error: text("x") }),
        Op::Needs(prompt) => (Some(prompt_id(*prompt)), AgentEvent::NeedsInput { reason: InputReason::Permission, message: text("x") }),
        Op::Resolved => (None, AgentEvent::InputResolved),
        Op::Quiet(until) => (None, AgentEvent::Quiet { until: *until, max_minutes: 120 }),
        Op::QuietOff => (None, AgentEvent::QuietOff),
    }
}

fn input(number: usize, op: &Op) -> DaemonInput {
    let (prompt, event) = to_event(op);
    let server = ServerId { socket_path: SocketPath::parse(Path::new("/tmp/tmux-501/default")).expect("absolute"), pid: 1, start_time: 1 };
    let origin = Origin {
        agent: AgentKind::ClaudeCode,
        session: Some(SessionId::parse("s").expect("id")),
        prompt,
        pane: Some(PaneKey { server, pane: PaneId::parse("%1").expect("pane") }),
        window: None,
        repo: None,
        session_title: None,
        cwd: PathBuf::from("/x"),
    };
    DaemonInput::Received {
        request: Request::Agent { v: WIRE_VERSION, event_id: event_id(number), sent_at_ms: EpochMs(0), origin, event },
        focus: FocusSnapshot::unknown(),
    }
}

fn the_card(state: &DaemonState) -> Option<&agent_notify_core::state::Card> {
    state.sessions.values().next().and_then(|record| record.card.as_ref())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Working,
    Needs,
    Ended,
}

#[derive(Debug, Clone)]
struct Model {
    prompt: Option<u8>,
    phase: Phase,
    explicit: bool,
    quiet: Option<QuietUntil>,
}

impl Model {
    fn stale(&self, prompt: u8) -> bool {
        match self.prompt {
            Some(current) if current != prompt => true,
            Some(_) => self.phase == Phase::Ended,
            None => false,
        }
    }

    fn apply(&mut self, op: &Op) {
        match op {
            Op::Start(prompt) => {
                self.prompt = Some(*prompt);
                self.phase = Phase::Working;
                self.explicit = false;
                if self.quiet == Some(QuietUntil::NextPrompt) {
                    self.quiet = None;
                }
            }
            Op::Explicit(_) => {
                if self.phase == Phase::Working {
                    self.explicit = true;
                }
                if self.quiet == Some(QuietUntil::NextSend) {
                    self.quiet = None;
                }
            }
            Op::End(prompt) | Op::Fail(prompt) if !self.stale(*prompt) => {
                self.prompt = Some(*prompt);
                self.phase = Phase::Ended;
            }
            Op::Needs(prompt) if !self.stale(*prompt) => {
                self.prompt = Some(*prompt);
                self.phase = Phase::Needs;
            }
            Op::Resolved if self.phase == Phase::Needs => {
                self.phase = Phase::Working;
                self.explicit = false;
            }
            Op::Quiet(until) => self.quiet = Some(*until),
            Op::QuietOff => self.quiet = None,
            _ => {}
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn every_agent_event_yields_a_new_card_or_a_reason(ops in proptest::collection::vec(op(), 1..40)) {
        let mut state = DaemonState::default();
        for (number, op) in ops.iter().enumerate() {
            let before = the_card(&state).map(|card| card.id.clone());
            let effects = update(&mut state, input(number, op), EpochMs(0));
            let after = the_card(&state).map(|card| card.id.clone());
            let explained = effects.iter().any(|effect| matches!(effect, Effect::Suppressed { .. }));
            prop_assert!(explained || (after.is_some() && after != before), "{op:?} gave neither a new card nor a reason");
        }
    }

    #[test]
    fn no_card_comes_from_a_turn_other_than_the_latest_started(ops in proptest::collection::vec(op(), 1..40)) {
        let mut state = DaemonState::default();
        let mut latest_started: Option<u8> = None;
        let mut card_prompts: Vec<(CardId, u8)> = Vec::new();
        for (number, op) in ops.iter().enumerate() {
            if let Op::Start(prompt) = op {
                latest_started = Some(*prompt);
            }
            if let Op::End(prompt) | Op::Fail(prompt) | Op::Needs(prompt) = op {
                card_prompts.push((CardId::for_event(&event_id(number)), *prompt));
            }
            update(&mut state, input(number, op), EpochMs(0));
            let shown = the_card(&state).and_then(|card| card_prompts.iter().find(|(id, _)| *id == card.id).map(|(_, prompt)| *prompt));
            if let (Some(card_prompt), Some(started)) = (shown, latest_started) {
                prop_assert_eq!(card_prompt, started);
            }
        }
    }

    #[test]
    fn turn_ended_shows_info_exactly_when_the_model_says_so(ops in proptest::collection::vec(op(), 1..40)) {
        let mut state = DaemonState::default();
        let mut model = Model { prompt: None, phase: Phase::Idle, explicit: false, quiet: None };
        for (number, op) in ops.iter().enumerate() {
            let expectation = match op {
                Op::End(prompt) => {
                    let pending_attention = the_card(&state).is_some_and(|card| card.kind == Kind::Attention && !matches!(card.cause, CardCause::NeedsInput(_)));
                    let explicit_this_turn = model.phase == Phase::Working && model.explicit;
                    Some(!model.stale(*prompt) && !explicit_this_turn && model.quiet.is_none() && !pending_attention)
                }
                _ => None,
            };
            update(&mut state, input(number, op), EpochMs(0));
            model.apply(op);
            if let Some(expect_info) = expectation {
                let shown = the_card(&state).is_some_and(|card| card.id == CardId::for_event(&event_id(number)) && card.cause == CardCause::TurnEnded);
                prop_assert_eq!(shown, expect_info, "after {:?} in {:?}", op, ops);
            }
        }
    }
}
```

- [ ] **Step 2: Run the tests**

Run: `cd ~/crates && cargo test -p agent-notify-core --test properties`
Expected: all pass. A failure prints a minimal sequence. Decide from spec 6.2 whether the model or `update` is wrong, fix that side, and write the minimal sequence as a unit test in `update.rs` before the fix.

- [ ] **Step 3: Run clippy and commit**

```bash
cd ~/crates && cargo clippy -p agent-notify-core --all-targets -- -D warnings
cd ~ && config add crates/agent-notify-core
config commit -m "Add property tests for agent-notify event sequences" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/agent-notify-core
```

---

### Task 13: Promote the tmux test server to `dotfiles-test-support`

agent-notify is the third user of the throwaway tmux server (after `tmux-tools` and `config-cli`'s `notify.rs`), so the rule of three says promote it. `notify.rs` keeps its own copy, because plan 2 deletes that file.

**Files:**
- Move: `crates/tmux-tools/tests/support/mod.rs` to `crates/dotfiles-test-support/src/tmux_server.rs`
- Move: `crates/tmux-tools/tests/support_fixture.rs` to `crates/dotfiles-test-support/tests/tmux_server.rs`
- Modify: `crates/dotfiles-test-support/Cargo.toml`, `src/lib.rs`; `crates/tmux-tools/Cargo.toml`; the 12 files under `crates/tmux-tools/tests/` that declare `mod support;`

**Interfaces:**
- Produces: `dotfiles_test_support::tmux_server::{Server, shared_socket_path}` with the existing API, plus `Server::socket_path(&self) -> PathBuf` (the absolute socket path tmux reports; valid once a session exists) and `Server::pid(&self) -> u32`.

- [ ] **Step 1: Move the files**

```bash
cd ~/crates
mv tmux-tools/tests/support/mod.rs dotfiles-test-support/src/tmux_server.rs
rmdir tmux-tools/tests/support
mv tmux-tools/tests/support_fixture.rs dotfiles-test-support/tests/tmux_server.rs
```

- [ ] **Step 2: Wire the module**

In `dotfiles-test-support/src/tmux_server.rs`, delete the `#![allow(dead_code)]` line and the two comment lines above it (they explained per-binary dead code, which a library does not have). Add after `wait_until_inert`'s impl block, inside `impl Server`:

```rust
    /// The absolute socket path, as tmux reports it. Valid once a session
    /// exists, because `display-message` needs a session to answer.
    #[must_use]
    pub fn socket_path(&self) -> PathBuf {
        PathBuf::from(self.stdout(&["display-message", "-p", "#{socket_path}"]))
    }

    /// The server's pid, for building a `$TMUX` value.
    ///
    /// # Panics
    ///
    /// Panics when tmux prints something that is not a pid.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.stdout(&["display-message", "-p", "#{pid}"]).parse().expect("tmux prints a numeric pid")
    }
```

In `dotfiles-test-support/src/lib.rs`, add near the other modules:

```rust
pub mod tmux_server;
```

In `dotfiles-test-support/Cargo.toml`, move `tempfile` from `[dev-dependencies]` to `[dependencies]`:

```toml
[dependencies]
yaml_serde = "0.10"
# The throwaway tmux server keeps its socket in a temporary directory.
tempfile = "3.27"
```

In `dotfiles-test-support/tests/tmux_server.rs`, replace `mod support;` and `use support::{Server, shared_socket_path};` with:

```rust
use dotfiles_test_support::tmux_server::{Server, shared_socket_path};
```

In `tmux-tools/Cargo.toml`, add under `[dev-dependencies]`:

```toml
dotfiles-test-support = { path = "../dotfiles-test-support" }
```

Point the tmux-tools tests at the library:

```bash
cd ~/crates/tmux-tools/tests
grep -l '^mod support;$' *.rs | xargs sed -i '' 's/^mod support;$/use dotfiles_test_support::tmux_server as support;/'
```

On Linux, use `sed -i` without `''`.

- [ ] **Step 3: Run the moved and the dependent tests**

Run: `cd ~/crates && cargo test -p dotfiles-test-support && cargo test -p tmux-tools`
Expected: the same tests pass as before the move. Count them before and after with `cargo test -p tmux-tools 2>&1 | grep 'test result'` and compare the totals; the fixture tests now count under `dotfiles-test-support`.

- [ ] **Step 4: Clippy and commit**

```bash
cd ~/crates && cargo clippy -p dotfiles-test-support -p tmux-tools --all-targets -- -D warnings
cd ~ && config add crates/dotfiles-test-support crates/tmux-tools crates/Cargo.lock
config commit -m "Promote the throwaway tmux server to dotfiles-test-support" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/dotfiles-test-support crates/tmux-tools crates/Cargo.lock
```

`config add crates/tmux-tools` stages the deletions of the moved files too. Confirm with `config status -- crates/tmux-tools` before the commit.

---

### Task 14: Expose the git inspector as a library

**Files:**
- Create: `crates/tmux-tools/src/lib.rs`
- Modify: `crates/tmux-tools/src/main.rs`

**Interfaces:**
- Produces: `tmux_tools::repo::inspect(&Path) -> Option<tmux_core::RepositoryFacts>`.

- [ ] **Step 1: Add the library target**

`crates/tmux-tools/src/lib.rs`:

```rust
//! The parts of tmux-tools that other crates reuse. Today that is the git
//! inspector, which agent-notify uses to label a notification from outside
//! tmux with the same repository name a window would show.

pub mod repo;
```

In `src/main.rs`, delete `mod repo;` and add `use tmux_tools::repo;` beside the other `use` lines. Cargo builds `src/lib.rs` and `src/main.rs` as two crates of one package, so no `Cargo.toml` change is needed.

- [ ] **Step 2: Run the tests and clippy**

Run: `cd ~/crates && cargo test -p tmux-tools && cargo clippy -p tmux-tools --all-targets -- -D warnings`
Expected: the same pass count as Task 13 Step 3. The four `repo.rs` unit tests now run under the lib target.

- [ ] **Step 3: Commit**

```bash
cd ~ && config add crates/tmux-tools/src
config commit -m "Expose the tmux-tools git inspector as a library" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/tmux-tools/src
```

---

### Task 15: CLI arguments and the pure invocation planner

**Files:**
- Modify: `crates/Cargo.toml` (members: add `"agent-notify"`)
- Create: `crates/agent-notify/Cargo.toml`, `src/main.rs` (stub), `src/cli.rs`, `src/invocation.rs`

**Interfaces:**
- Consumes: the core crate.
- Produces:
  - `cli::Cli { command: Command }`; `cli::Command::{Hook { agent: HookAgent }, Send { kind: KindArg, body: String }, Quiet { until: Option<UntilArg>, max_minutes: u16, off: bool }}`; `HookAgent::ClaudeCode { dry_run: bool }`; `KindArg::{Info, Attention}` (`From<KindArg> for Kind`); `UntilArg::{NextSend, NextPrompt}` (`From<UntilArg> for QuietUntil`). `Command::is_dry_run(&self) -> bool`, `Command::is_hook(&self) -> bool`.
  - `invocation::Inputs { now: EpochMs, event_id: EventId, cwd: PathBuf, pane: PaneLookup, repo: Option<RepoInfo>, stdin: Option<String> }`.
  - `invocation::PaneLookup::{NotInTmux, Found(PaneFacts), Unreadable { reason: String }}`; `PaneFacts { key: PaneKey, window: WindowLabel, session_tag: Option<SessionId>, prompt_tag: Option<PromptId> }`.
  - `invocation::Invocation { tags: Vec<TagWrite>, request: Option<Request>, fallback: Option<FallbackNote>, exit: Exit, stderr: Option<String>, log: Option<String> }`; `Exit::{Success, Usage}`.
  - `invocation::plan_invocation(&Command, &Inputs) -> Invocation`.

- [ ] **Step 1: Scaffold the crate**

Add `"agent-notify"` to the members line in `crates/Cargo.toml` (keep alphabetical order and one line).

`crates/agent-notify/Cargo.toml`:

```toml
[package]
name = "agent-notify"
edition = "2024"
version = "0.1.0"
publish = false

[dependencies]
agent-notify-core = { path = "../agent-notify-core" }
tmux-core = { path = "../tmux-core" }
tmux-tools = { path = "../tmux-tools" }
clap = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }

[dev-dependencies]
dotfiles-test-support = { path = "../dotfiles-test-support" }
proptest = { workspace = true }
tempfile = { workspace = true }

[lints]
workspace = true
```

`crates/agent-notify/src/main.rs` (Task 17 replaces the body):

```rust
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
//! The agent-notify CLI. Hooks and agents call it. It tags tmux panes, sends
//! one request to the daemon, and falls back to a plain OS notification.

mod cli;
mod invocation;

fn main() {}
```

- [ ] **Step 2: Write `cli.rs`**

```rust
//! Command-line arguments.

use agent_notify_core::wire::{Kind, QuietUntil};
use clap::{Parser, Subcommand, ValueEnum};

/// The agent-notify command line.
#[derive(Debug, Parser)]
#[command(name = "agent-notify", version, about = "Contextual notifications for coding agents in tmux")]
pub struct Cli {
    /// The subcommand.
    #[command(subcommand)]
    pub command: Command,
}

/// One subcommand.
#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Read a hook payload on stdin and send the mapped event. Always exits 0.
    Hook {
        /// The agent whose hook fired.
        #[command(subcommand)]
        agent: HookAgent,
    },
    /// Send a notification from an agent.
    Send {
        /// info for finished work, attention when the user must act.
        #[arg(long, value_enum)]
        kind: KindArg,
        /// One or two short sentences.
        #[arg(long)]
        body: String,
    },
    /// Silence the end-of-turn backup notification for this session.
    Quiet {
        /// When quiet ends on its own.
        #[arg(long, value_enum, conflicts_with = "off", required_unless_present = "off")]
        until: Option<UntilArg>,
        /// The latest end, in minutes.
        #[arg(long, default_value_t = 120, value_parser = clap::value_parser!(u16).range(1..=1440))]
        max_minutes: u16,
        /// End quiet now.
        #[arg(long)]
        off: bool,
    },
}

/// The agent behind a hook.
#[derive(Debug, Clone, Subcommand)]
pub enum HookAgent {
    /// Claude Code.
    ClaudeCode {
        /// Print the request and send nothing.
        #[arg(long)]
        dry_run: bool,
    },
}

/// `--kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum KindArg {
    /// Finished work.
    Info,
    /// The user must act.
    Attention,
}

impl From<KindArg> for Kind {
    fn from(kind: KindArg) -> Kind {
        match kind {
            KindArg::Info => Kind::Info,
            KindArg::Attention => Kind::Attention,
        }
    }
}

/// `--until`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum UntilArg {
    /// The session's next `send`.
    NextSend,
    /// The session's next user prompt.
    NextPrompt,
}

impl From<UntilArg> for QuietUntil {
    fn from(until: UntilArg) -> QuietUntil {
        match until {
            UntilArg::NextSend => QuietUntil::NextSend,
            UntilArg::NextPrompt => QuietUntil::NextPrompt,
        }
    }
}

impl Command {
    /// True for `hook ... --dry-run`.
    pub fn is_dry_run(&self) -> bool {
        matches!(self, Command::Hook { agent: HookAgent::ClaudeCode { dry_run: true } })
    }

    /// True for `hook`, which must exit 0 whatever happens.
    pub fn is_hook(&self) -> bool {
        matches!(self, Command::Hook { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(arguments: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("agent-notify").chain(arguments.iter().copied()))
    }

    #[test]
    fn accepted_command_lines() {
        assert!(parse(&["hook", "claude-code"]).is_ok());
        assert!(parse(&["hook", "claude-code", "--dry-run"]).is_ok_and(|cli| cli.command.is_dry_run()));
        assert!(parse(&["send", "--kind", "attention", "--body", "CI is red"]).is_ok());
        assert!(parse(&["quiet", "--until", "next-send", "--max-minutes", "180"]).is_ok());
        assert!(parse(&["quiet", "--off"]).is_ok());
    }

    #[test]
    fn usage_errors() {
        for arguments in [
            &["send", "--kind", "loud", "--body", "x"][..],
            &["send", "--body", "x"],
            &["quiet"],
            &["quiet", "--off", "--until", "next-send"],
            &["quiet", "--until", "next-send", "--max-minutes", "0"],
            &["hook", "codex"],
        ] {
            let error = parse(arguments).expect_err("a usage error");
            assert_eq!(error.exit_code(), 2, "{arguments:?}");
        }
    }
}
```

- [ ] **Step 3: Write the failing planner tests**

`src/invocation.rs`, tests module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{HookAgent, KindArg, UntilArg};
    use agent_notify_core::hook::TagWrite;
    use agent_notify_core::wire::{AgentEvent, MAX_LINE_BYTES, QuietUntil, encode_line};
    use agent_notify_core::{DisplayText, PaneId, ServerId, SocketPath};
    use proptest::prelude::*;
    use std::path::Path;

    fn pane_key() -> PaneKey {
        PaneKey {
            server: ServerId { socket_path: SocketPath::parse(Path::new("/tmp/tmux-501/default")).expect("absolute"), pid: 1799, start_time: 1 },
            pane: PaneId::parse("%12").expect("pane"),
        }
    }

    fn found(session_tag: Option<&str>, prompt_tag: Option<&str>) -> PaneLookup {
        PaneLookup::Found(PaneFacts {
            key: pane_key(),
            window: WindowLabel { index: 4, name: DisplayText::new("code/feature-x") },
            session_tag: session_tag.map(|tag| SessionId::parse(tag).expect("id")),
            prompt_tag: prompt_tag.map(|tag| PromptId::parse(tag).expect("id")),
        })
    }

    fn inputs(pane: PaneLookup, stdin: Option<&str>) -> Inputs {
        Inputs {
            now: EpochMs(1_000),
            event_id: EventId::from_parts(EpochMs(1_000), [7; 10]),
            cwd: PathBuf::from("/home/dev/code"),
            pane,
            repo: None,
            stdin: stdin.map(str::to_string),
        }
    }

    fn hook() -> Command {
        Command::Hook { agent: HookAgent::ClaudeCode { dry_run: false } }
    }

    fn payload(event: &str, extra: &str) -> String {
        format!(r#"{{"session_id":"session-a","prompt_id":"prompt-1","cwd":"/home/dev/code","hook_event_name":"{event}"{extra}}}"#)
    }

    fn agent_parts(invocation: &Invocation) -> (&Origin, &AgentEvent) {
        match &invocation.request {
            Some(Request::Agent { origin, event, .. }) => (origin, event),
            other => panic!("expected an agent request, got {other:?}"),
        }
    }

    #[test]
    fn session_start_in_tmux_binds_the_tag_and_sends() {
        let invocation = plan_invocation(&hook(), &inputs(found(None, None), Some(&payload("SessionStart", r#","source":"startup","session_title":"Fix sync""#))));
        assert_eq!(invocation.tags, vec![TagWrite::BindSession(SessionId::parse("session-a").expect("id"))]);
        let (origin, _) = agent_parts(&invocation);
        assert_eq!(origin.pane, Some(pane_key()));
        assert_eq!(origin.session_title, Some(DisplayText::new("Fix sync")));
        assert_eq!(invocation.exit, Exit::Success);
    }

    #[test]
    fn a_hook_outside_tmux_sends_without_tags() {
        let invocation = plan_invocation(&hook(), &inputs(PaneLookup::NotInTmux, Some(&payload("SessionStart", r#","source":"startup""#))));
        assert!(invocation.tags.is_empty());
        assert_eq!(agent_parts(&invocation).0.pane, None);
    }

    #[test]
    fn an_unreadable_pane_still_sends_without_tags() {
        let invocation = plan_invocation(&hook(), &inputs(PaneLookup::Unreadable { reason: "no server".into() }, Some(&payload("Stop", ""))));
        assert!(invocation.tags.is_empty());
        assert!(invocation.request.is_some());
    }

    #[test]
    fn session_end_unbinds_only_its_own_tag() {
        let ending = payload("SessionEnd", r#","reason":"clear""#);
        assert_eq!(plan_invocation(&hook(), &inputs(found(Some("session-a"), None), Some(&ending))).tags, vec![TagWrite::UnbindSession]);
        assert!(plan_invocation(&hook(), &inputs(found(Some("session-b"), None), Some(&ending))).tags.is_empty());
    }

    #[test]
    fn a_permission_prompt_carries_a_fallback_and_stop_does_not() {
        let permission = plan_invocation(&hook(), &inputs(found(None, None), Some(&payload("Notification", r#","notification_type":"permission_prompt","message":"Allow Bash?""#))));
        assert_eq!(permission.fallback.map(|note| note.message), Some("Allow Bash?".to_string()));
        let stop = plan_invocation(&hook(), &inputs(found(None, None), Some(&payload("Stop", r#","last_assistant_message":"Done.""#))));
        assert_eq!(stop.fallback, None);
    }

    #[test]
    fn bad_hook_input_sends_nothing_and_exits_zero() {
        for stdin in [None, Some(""), Some("not json"), Some(r#"{"hook_event_name":"Stop"}"#)] {
            let invocation = plan_invocation(&hook(), &inputs(PaneLookup::NotInTmux, stdin));
            assert!(invocation.request.is_none(), "{stdin:?}");
            assert_eq!(invocation.exit, Exit::Success);
            assert!(invocation.log.is_some(), "the reason must be logged for {stdin:?}");
        }
    }

    #[test]
    fn malformed_session_id_sends_nothing() {
        let stdin = r#"{"session_id":"a b;rm -rf","cwd":"/x","hook_event_name":"Stop"}"#;
        let invocation = plan_invocation(&hook(), &inputs(PaneLookup::NotInTmux, Some(stdin)));
        assert!(invocation.request.is_none());
        assert_eq!(invocation.exit, Exit::Success);
        assert!(invocation.log.as_deref().is_some_and(|reason| reason.contains("session id")));
    }

    #[test]
    fn an_ignored_hook_event_logs_its_reason() {
        let invocation = plan_invocation(&hook(), &inputs(PaneLookup::NotInTmux, Some(&payload("PreCompact", ""))));
        assert!(invocation.request.is_none());
        assert!(invocation.log.as_deref().is_some_and(|reason| reason.contains("PreCompact")));
    }

    #[test]
    fn send_reads_session_and_prompt_from_the_pane_tags() {
        let command = Command::Send { kind: KindArg::Attention, body: "CI is red".into() };
        let invocation = plan_invocation(&command, &inputs(found(Some("session-a"), Some("prompt-9")), None));
        let (origin, event) = agent_parts(&invocation);
        assert_eq!(origin.session.as_ref().map(SessionId::as_str), Some("session-a"));
        assert_eq!(origin.prompt.as_ref().map(PromptId::as_str), Some("prompt-9"));
        assert_eq!(*event, AgentEvent::Explicit { kind: Kind::Attention, body: DisplayText::new("CI is red") });
        assert!(invocation.fallback.is_some_and(|note| note.urgent));
    }

    #[test]
    fn send_outside_tmux_has_no_session() {
        let command = Command::Send { kind: KindArg::Info, body: "Report ready".into() };
        let invocation = plan_invocation(&command, &inputs(PaneLookup::NotInTmux, None));
        assert_eq!(agent_parts(&invocation).0.session, None);
    }

    #[test]
    fn a_blank_body_is_a_usage_error() {
        let command = Command::Send { kind: KindArg::Info, body: " \u{200B} ".into() };
        let invocation = plan_invocation(&command, &inputs(found(Some("session-a"), None), None));
        assert_eq!(invocation.exit, Exit::Usage);
        assert!(invocation.request.is_none());
        assert!(invocation.stderr.as_deref().is_some_and(|message| message.contains("--body")));
    }

    #[test]
    fn quiet_needs_a_tagged_pane() {
        let command = Command::Quiet { until: Some(UntilArg::NextSend), max_minutes: 120, off: false };
        for pane in [PaneLookup::NotInTmux, found(None, None)] {
            let invocation = plan_invocation(&command, &inputs(pane, None));
            assert_eq!(invocation.exit, Exit::Usage);
            assert!(invocation.stderr.is_some());
        }
        let tagged = plan_invocation(&command, &inputs(found(Some("session-a"), None), None));
        assert_eq!(*agent_parts(&tagged).1, AgentEvent::Quiet { until: QuietUntil::NextSend, max_minutes: 120 });
        assert_eq!(tagged.fallback, None);
    }

    #[test]
    fn quiet_off() {
        let command = Command::Quiet { until: None, max_minutes: 120, off: true };
        let invocation = plan_invocation(&command, &inputs(found(Some("session-a"), None), None));
        assert_eq!(*agent_parts(&invocation).1, AgentEvent::QuietOff);
    }

    #[test]
    fn largest_possible_request_fits_one_line() {
        let big = "\"\\".repeat(4096);
        let mut facts = match found(Some("session-a"), Some("prompt-1")) {
            PaneLookup::Found(facts) => facts,
            _ => unreachable!(),
        };
        facts.window.name = DisplayText::new(&big);
        let mut maximal = inputs(PaneLookup::Found(facts), None);
        maximal.repo = Some(RepoInfo {
            main_repo_name: DisplayText::new(&big),
            head: agent_notify_core::wire::RepoHead::Branch { name: DisplayText::new(&big) },
        });
        let payload = serde_json::json!({
            "session_id": "session-a",
            "prompt_id": "prompt-1",
            "cwd": format!("/{}", "\"".repeat(4095)),
            "hook_event_name": "Stop",
            "session_title": big,
            "last_assistant_message": big,
        });
        maximal.stdin = Some(payload.to_string());
        let invocation = plan_invocation(&hook(), &maximal);
        let request = invocation.request.expect("a request");
        let line = encode_line(&request).expect("encodes");
        assert!(line.len() <= MAX_LINE_BYTES, "a maximal request is {} bytes", line.len());
    }

    proptest! {
        #[test]
        fn any_stdin_plans_without_panicking_and_exits_zero(stdin in proptest::collection::vec(any::<u8>(), 0..2000)) {
            let text = String::from_utf8_lossy(&stdin).into_owned();
            let invocation = plan_invocation(&hook(), &inputs(PaneLookup::NotInTmux, Some(&text)));
            prop_assert_eq!(invocation.exit, Exit::Success);
        }
    }
}
```

`largest_possible_request_fits_one_line` fills every text field with `"` and `\`, so JSON escaping doubles every byte. The hook's `cwd` comes from the payload and is not a `DisplayText`, so the test caps it at 4096 bytes, the Linux `PATH_MAX`. If the assertion fails, the limit is wrong, not the test: raise `MAX_LINE_BYTES` in the core and update the Global Constraints line.

- [ ] **Step 4: Run the tests to see them fail**

Run: `cd ~/crates && cargo test -p agent-notify`
Expected: `cli` tests pass; `invocation` tests fail to compile.

- [ ] **Step 5: Implement the planner**

Above the tests in `src/invocation.rs`:

```rust
//! Decides what one CLI invocation does. Pure: the shell gathers [`Inputs`],
//! this module returns an [`Invocation`], and the shell runs it.

use std::path::PathBuf;

use agent_notify_core::fallback::{FallbackNote, fallback_note};
use agent_notify_core::hook::{ClaudeHookInput, HookDecision, TagWrite, decide_hook};
use agent_notify_core::wire::{AgentEvent, AgentKind, Kind, Origin, RepoInfo, Request, WIRE_VERSION, WindowLabel};
use agent_notify_core::{DisplayText, EpochMs, EventId, PaneKey, PromptId, SessionId};

use crate::cli::{Command, HookAgent};

/// Everything the planner reads.
#[derive(Debug, Clone)]
pub struct Inputs {
    /// The clock, read once.
    pub now: EpochMs,
    /// This invocation's id.
    pub event_id: EventId,
    /// The process working directory.
    pub cwd: PathBuf,
    /// What the tmux read found.
    pub pane: PaneLookup,
    /// Git facts, read only outside tmux.
    pub repo: Option<RepoInfo>,
    /// The hook payload, for `hook` only.
    pub stdin: Option<String>,
}

/// The result of reading the current tmux pane.
#[derive(Debug, Clone)]
pub enum PaneLookup {
    /// `$TMUX` or `$TMUX_PANE` is unset.
    NotInTmux,
    /// The pane and its tags.
    Found(PaneFacts),
    /// Inside tmux, but the read failed. The request goes without pane facts.
    Unreadable {
        /// For the log.
        reason: String,
    },
}

/// One pane and its agent-notify tags.
#[derive(Debug, Clone)]
pub struct PaneFacts {
    /// The pane.
    pub key: PaneKey,
    /// Its window.
    pub window: WindowLabel,
    /// `@agent_notify_session`.
    pub session_tag: Option<SessionId>,
    /// `@agent_notify_prompt`.
    pub prompt_tag: Option<PromptId>,
}

/// What the shell does.
#[derive(Debug, Clone)]
pub struct Invocation {
    /// Pane tag writes, before the request.
    pub tags: Vec<TagWrite>,
    /// The request to send.
    pub request: Option<Request>,
    /// Shown when the daemon does not acknowledge.
    pub fallback: Option<FallbackNote>,
    /// The exit status.
    pub exit: Exit,
    /// One line for stderr.
    pub stderr: Option<String>,
    /// Why nothing was sent, for the log.
    pub log: Option<String>,
}

/// The exit status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// 0.
    Success,
    /// 2.
    Usage,
}

impl Invocation {
    fn nothing(reason: String) -> Self {
        Invocation { tags: Vec::new(), request: None, fallback: None, exit: Exit::Success, stderr: None, log: Some(reason) }
    }

    fn usage(message: &str) -> Self {
        Invocation { tags: Vec::new(), request: None, fallback: None, exit: Exit::Usage, stderr: Some(message.to_string()), log: Some(message.to_string()) }
    }

    fn agent(inputs: &Inputs, origin: Origin, event: AgentEvent, tags: Vec<TagWrite>) -> Self {
        let fallback = fallback_note(&origin, &event);
        let request = Request::Agent { v: WIRE_VERSION, event_id: inputs.event_id.clone(), sent_at_ms: inputs.now, origin, event };
        Invocation { tags, request: Some(request), fallback, exit: Exit::Success, stderr: None, log: None }
    }
}

/// Decides what `command` does with `inputs`.
#[must_use]
pub fn plan_invocation(command: &Command, inputs: &Inputs) -> Invocation {
    match command {
        Command::Hook { agent: HookAgent::ClaudeCode { .. } } => plan_hook(inputs),
        Command::Send { kind, body } => plan_send(Kind::from(*kind), body, inputs),
        Command::Quiet { until, max_minutes, off } => plan_quiet(until.map(Into::into), *max_minutes, *off, inputs),
    }
}

fn plan_hook(inputs: &Inputs) -> Invocation {
    let Some(raw) = inputs.stdin.as_deref().filter(|raw| !raw.trim().is_empty()) else {
        return Invocation::nothing("no hook payload on stdin".to_string());
    };
    let payload: ClaudeHookInput = match serde_json::from_str(raw) {
        Ok(payload) => payload,
        Err(error) => return Invocation::nothing(format!("hook payload did not parse: {error}")),
    };
    let pane = found(inputs);
    match decide_hook(&payload, pane.and_then(|facts| facts.session_tag.as_ref())) {
        HookDecision::Ignore { reason } => Invocation::nothing(reason),
        HookDecision::Send { tags, event } => {
            let origin = origin(inputs, Some(payload.session_id.clone()), payload.prompt_id.clone(), payload.session_title.clone(), payload.cwd.clone());
            let tags = if pane.is_some() { tags } else { Vec::new() };
            Invocation::agent(inputs, origin, event, tags)
        }
    }
}

fn plan_send(kind: Kind, body: &str, inputs: &Inputs) -> Invocation {
    let body = DisplayText::new(body);
    if body.as_str().trim().is_empty() {
        return Invocation::usage("--body is empty after removing control and invisible characters");
    }
    let (session, prompt) = tags(inputs);
    let origin = origin(inputs, session, prompt, None, inputs.cwd.clone());
    Invocation::agent(inputs, origin, AgentEvent::Explicit { kind, body }, Vec::new())
}

fn plan_quiet(until: Option<agent_notify_core::wire::QuietUntil>, max_minutes: u16, off: bool, inputs: &Inputs) -> Invocation {
    let (Some(session), prompt) = tags(inputs) else {
        return Invocation::usage("quiet needs a tmux pane with a Claude session tag: $TMUX_PANE is unset, or no SessionStart hook tagged this pane");
    };
    let event = match (off, until) {
        (true, _) => AgentEvent::QuietOff,
        (false, Some(until)) => AgentEvent::Quiet { until, max_minutes },
        (false, None) => return Invocation::usage("quiet needs --until or --off"),
    };
    let origin = origin(inputs, Some(session), prompt, None, inputs.cwd.clone());
    Invocation { fallback: None, ..Invocation::agent(inputs, origin, event, Vec::new()) }
}

fn found(inputs: &Inputs) -> Option<&PaneFacts> {
    match &inputs.pane {
        PaneLookup::Found(facts) => Some(facts),
        PaneLookup::NotInTmux | PaneLookup::Unreadable { .. } => None,
    }
}

fn tags(inputs: &Inputs) -> (Option<SessionId>, Option<PromptId>) {
    found(inputs).map_or((None, None), |facts| (facts.session_tag.clone(), facts.prompt_tag.clone()))
}

fn origin(inputs: &Inputs, session: Option<SessionId>, prompt: Option<PromptId>, session_title: Option<DisplayText>, cwd: PathBuf) -> Origin {
    let pane = found(inputs);
    Origin {
        agent: AgentKind::ClaudeCode,
        session,
        prompt,
        pane: pane.map(|facts| facts.key.clone()),
        window: pane.map(|facts| facts.window.clone()),
        repo: inputs.repo.clone(),
        session_title,
        cwd,
    }
}
```

`fallback_note` already returns `None` for `Quiet`, so `plan_quiet`'s `fallback: None` override only states the rule where a reader looks for it. Delete the override if clippy flags it as needless.

- [ ] **Step 6: Run the tests and clippy**

Run: `cd ~/crates && cargo test -p agent-notify && cargo clippy -p agent-notify --all-targets -- -D warnings`
Expected: all pass. `dead_code` warnings for items only `main` will use are expected until Task 17; if `-D warnings` blocks on them, add `#[allow(dead_code)] // used from main in Task 17` to the `mod` lines in `main.rs` and remove it in Task 17.

- [ ] **Step 7: Commit**

```bash
cd ~ && config add crates/Cargo.toml crates/Cargo.lock crates/agent-notify
config commit -m "Add the agent-notify CLI arguments and invocation planner" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/Cargo.toml crates/Cargo.lock crates/agent-notify
```

---

### Task 16: tmux adapter and input gathering

**Files:**
- Create: `crates/agent-notify/src/tmux.rs`, `src/context.rs`, `tests/support/mod.rs`, `tests/tmux_tags.rs`
- Modify: `crates/agent-notify/src/main.rs` (add `mod context;` and `mod tmux;`)

**Interfaces:**
- Consumes: Task 15 `Inputs`, `PaneLookup`, `PaneFacts`, `Command::is_hook`; Task 14 `tmux_tools::repo::inspect`; core `hook::{TagWrite, SESSION_TAG_OPTION, PROMPT_TAG_OPTION}`, `paths::{PathInputs, Platform}`.
- Produces:
  - `tmux::socket_from_tmux_env(&str) -> Option<SocketPath>`
  - `tmux::parse_pane_line(&str, SocketPath, PaneId) -> Result<PaneFacts, String>`
  - `tmux::read_pane(&SocketPath, &PaneId) -> io::Result<String>`
  - `tmux::apply_tag(&SocketPath, &PaneId, &TagWrite) -> io::Result<()>`
  - `context::EnvSnapshot { tmux, tmux_pane, home, xdg_runtime_dir, xdg_state_home, socket_override, log_dir_override, log_level, trace }` with `read() -> EnvSnapshot` and `path_inputs(&self) -> PathInputs<'_>`
  - `context::gather(&Command, &EnvSnapshot, EpochMs, [u8; 10]) -> Inputs`, `context::clock() -> EpochMs`, `context::random_bytes() -> [u8; 10]`
  - Test support (`tests/support/mod.rs`): `Sandbox::{new, command, run, fallback_calls, log_text}`, `Sandbox.socket: PathBuf`, `ReplyMode::{Ack, Reject, Never}`, `listen(&Path, ReplyMode) -> JoinHandle<Option<String>>`

- [ ] **Step 1: Write the failing unit tests**

In `src/tmux.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn socket() -> SocketPath {
        SocketPath::parse(Path::new("/tmp/tmux-501/default")).expect("absolute")
    }

    fn pane() -> PaneId {
        PaneId::parse("%12").expect("pane")
    }

    fn line(fields: &[&str]) -> String {
        let mut joined = fields.join(&FIELD_SEPARATOR.to_string());
        joined.push('\n');
        joined
    }

    #[test]
    fn the_tmux_variable_names_the_socket() {
        assert_eq!(socket_from_tmux_env("/private/tmp/tmux-501/default,1799,0"), SocketPath::parse(Path::new("/private/tmp/tmux-501/default")).ok());
        assert_eq!(socket_from_tmux_env("/tmp/a,b/sock,1,0"), SocketPath::parse(Path::new("/tmp/a,b/sock")).ok());
        assert_eq!(socket_from_tmux_env("garbage"), None);
        assert_eq!(socket_from_tmux_env("relative/sock,1,0"), None);
    }

    #[test]
    fn a_pane_line_parses_into_facts() {
        let facts = parse_pane_line(&line(&["1799", "1760400000", "4", "session-a", "prompt-1", "code/feature-x"]), socket(), pane()).expect("parses");
        assert_eq!(facts.key.server.pid, 1799);
        assert_eq!(facts.key.server.start_time, 1_760_400_000);
        assert_eq!(facts.window.index, 4);
        assert_eq!(facts.window.name.as_str(), "code/feature-x");
        assert_eq!(facts.session_tag.as_ref().map(SessionId::as_str), Some("session-a"));
        assert_eq!(facts.prompt_tag.as_ref().map(PromptId::as_str), Some("prompt-1"));
    }

    #[test]
    fn window_name_with_separator_and_newline_parses() {
        let name = format!("a{FIELD_SEPARATOR}b\nc");
        let facts = parse_pane_line(&line(&["1", "2", "3", "", "", &name]), socket(), pane()).expect("parses");
        assert_eq!(facts.window.name.as_str(), "ab\nc", "the separator is a control character, so DisplayText drops it");
        assert_eq!(facts.window.index, 3);
    }

    #[test]
    fn empty_or_invalid_tags_are_absent() {
        let facts = parse_pane_line(&line(&["1", "2", "3", "", "bad tag;", "w"]), socket(), pane()).expect("parses");
        assert_eq!(facts.session_tag, None);
        assert_eq!(facts.prompt_tag, None);
    }

    #[test]
    fn bad_numbers_are_errors() {
        assert!(parse_pane_line(&line(&["x", "2", "3", "", "", "w"]), socket(), pane()).is_err());
        assert!(parse_pane_line("1\u{1f}2", socket(), pane()).is_err());
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd ~/crates && cargo test -p agent-notify tmux`
Expected: compile errors.

- [ ] **Step 3: Implement `tmux.rs`**

```rust
//! The tmux side of the CLI: one pane read and the tag writes. Every call
//! passes `-S <socket>` and targets a `%pane_id`, so it reaches the server
//! the agent runs in and never a default server.

use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

use agent_notify_core::hook::{PROMPT_TAG_OPTION, SESSION_TAG_OPTION, TagWrite};
use agent_notify_core::wire::WindowLabel;
use agent_notify_core::{DisplayText, PaneId, PaneKey, PromptId, ServerId, SessionId, SocketPath};

use crate::invocation::PaneFacts;

const FIELD_SEPARATOR: char = '\u{1f}';

/// The socket path from `$TMUX`, which tmux sets to `<socket>,<pid>,<session>`.
/// The socket path can contain commas, so the split runs from the right.
pub fn socket_from_tmux_env(value: &str) -> Option<SocketPath> {
    let mut parts = value.rsplitn(3, ',');
    let (_session_index, _pid, socket) = (parts.next()?, parts.next()?, parts.next()?);
    SocketPath::parse(Path::new(socket)).ok()
}

/// The window name is the last field, so a separator inside a name stays in the name.
fn pane_format() -> String {
    let separator = FIELD_SEPARATOR;
    format!("#{{pid}}{separator}#{{start_time}}{separator}#{{window_index}}{separator}#{{{SESSION_TAG_OPTION}}}{separator}#{{{PROMPT_TAG_OPTION}}}{separator}#{{window_name}}")
}

/// Parses the output of [`read_pane`].
///
/// # Errors
///
/// Returns a message naming the field that is missing or not a number.
pub fn parse_pane_line(line: &str, socket_path: SocketPath, pane: PaneId) -> Result<PaneFacts, String> {
    let line = line.strip_suffix('\n').unwrap_or(line);
    let mut fields = line.splitn(6, FIELD_SEPARATOR);
    let mut next = |name: &str| fields.next().ok_or_else(|| format!("tmux printed no {name} field"));
    let pid = next("pid")?.parse::<u32>().map_err(|error| format!("pid: {error}"))?;
    let start_time = next("start_time")?.parse::<i64>().map_err(|error| format!("start_time: {error}"))?;
    let index = next("window_index")?.parse::<u32>().map_err(|error| format!("window_index: {error}"))?;
    let session_tag = SessionId::parse(next("session tag")?).ok();
    let prompt_tag = PromptId::parse(next("prompt tag")?).ok();
    let name = DisplayText::new(next("window_name")?);
    Ok(PaneFacts {
        key: PaneKey { server: ServerId { socket_path, pid, start_time }, pane },
        window: WindowLabel { index, name },
        session_tag,
        prompt_tag,
    })
}

/// Reads the pane's server identity, window and tags in one tmux call.
///
/// # Errors
///
/// Returns the spawn error, or tmux's stderr when tmux fails.
pub fn read_pane(socket: &SocketPath, pane: &PaneId) -> io::Result<String> {
    run_tmux(socket, &["display-message", "-p", "-t", pane.as_str(), &pane_format()])
}

/// Applies one tag write to `pane`.
///
/// # Errors
///
/// Returns the first tmux failure.
pub fn apply_tag(socket: &SocketPath, pane: &PaneId, tag: &TagWrite) -> io::Result<()> {
    let target = pane.as_str();
    let unset_prompt = ["set-option", "-p", "-q", "-u", "-t", target, PROMPT_TAG_OPTION];
    match tag {
        TagWrite::BindSession(session) => {
            run_tmux(socket, &["set-option", "-p", "-t", target, SESSION_TAG_OPTION, session.as_str()])?;
            run_tmux(socket, &unset_prompt)?;
        }
        TagWrite::BindPrompt(prompt) => {
            run_tmux(socket, &["set-option", "-p", "-t", target, PROMPT_TAG_OPTION, prompt.as_str()])?;
        }
        TagWrite::UnbindSession => {
            run_tmux(socket, &["set-option", "-p", "-q", "-u", "-t", target, SESSION_TAG_OPTION])?;
            run_tmux(socket, &unset_prompt)?;
        }
    }
    Ok(())
}

fn run_tmux(socket: &SocketPath, arguments: &[&str]) -> io::Result<String> {
    let output = Command::new("tmux")
        .arg("-S")
        .arg(socket.as_path())
        .args(arguments)
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() {
        let command = arguments.first().copied().unwrap_or("");
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::other(format!("tmux {command} failed: {}", stderr.trim())));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
```

- [ ] **Step 4: Implement `context.rs`**

```rust
//! Gathers [`Inputs`] from the environment, stdin, tmux and git. The only IO
//! before the planner runs.

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_notify_core::paths::{PathInputs, Platform};
use agent_notify_core::wire::{RepoHead, RepoInfo};
use agent_notify_core::{DisplayText, EpochMs, EventId, PaneId};
use tmux_core::{HeadState, RepositoryFacts};

use crate::cli::Command;
use crate::invocation::{Inputs, PaneLookup};
use crate::tmux;

/// A hook payload larger than this is cut. Claude Code payloads are a few KB.
const MAX_STDIN_BYTES: u64 = 4 * 1024 * 1024;

/// The environment variables the CLI reads, read once.
#[derive(Debug, Clone, Default)]
pub struct EnvSnapshot {
    /// `$TMUX`.
    pub tmux: Option<String>,
    /// `$TMUX_PANE`.
    pub tmux_pane: Option<String>,
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
    /// `$AGENT_NOTIFY_LOG`: `error`, `warn`, `info`, `debug` or `trace`.
    pub log_level: Option<String>,
    /// `$AGENT_NOTIFY_TRACE=1`: log the raw hook payload.
    pub trace: bool,
}

impl EnvSnapshot {
    /// Reads the variables. Empty values count as unset.
    pub fn read() -> Self {
        let text = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
        let path = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty()).map(PathBuf::from);
        EnvSnapshot {
            tmux: text("TMUX"),
            tmux_pane: text("TMUX_PANE"),
            home: path("HOME"),
            xdg_runtime_dir: path("XDG_RUNTIME_DIR"),
            xdg_state_home: path("XDG_STATE_HOME"),
            socket_override: path("AGENT_NOTIFY_SOCKET"),
            log_dir_override: path("AGENT_NOTIFY_LOG_DIR"),
            log_level: text("AGENT_NOTIFY_LOG"),
            trace: text("AGENT_NOTIFY_TRACE").as_deref() == Some("1"),
        }
    }

    /// The facts the core's path rules read.
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

/// Reads everything the planner needs.
pub fn gather(command: &Command, env: &EnvSnapshot, now: EpochMs, random: [u8; 10]) -> Inputs {
    let stdin = command.is_hook().then(read_stdin);
    let pane = lookup_pane(env);
    let cwd = std::env::current_dir().unwrap_or_default();
    let repo = match pane {
        PaneLookup::Found(_) => None,
        PaneLookup::NotInTmux | PaneLookup::Unreadable { .. } => tmux_tools::repo::inspect(&cwd).map(repo_info),
    };
    Inputs { now, event_id: EventId::from_parts(now, random), cwd, pane, repo, stdin }
}

/// The wall clock, read once per invocation.
pub fn clock() -> EpochMs {
    let millis = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis());
    EpochMs(i64::try_from(millis).unwrap_or(i64::MAX))
}

/// 80 random bits for the event id. Falls back to the pid, which keeps ids
/// unique across concurrent hooks in the same millisecond.
pub fn random_bytes() -> [u8; 10] {
    let mut bytes = [0_u8; 10];
    if File::open("/dev/urandom").and_then(|mut source| source.read_exact(&mut bytes)).is_err() {
        bytes[..4].copy_from_slice(&std::process::id().to_be_bytes());
    }
    bytes
}

fn read_stdin() -> String {
    let mut bytes = Vec::new();
    match std::io::stdin().take(MAX_STDIN_BYTES).read_to_end(&mut bytes) {
        Ok(_) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(error) => {
            tracing::warn!(error = %error, "stdin read failed");
            String::new()
        }
    }
}

fn lookup_pane(env: &EnvSnapshot) -> PaneLookup {
    let (Some(tmux_value), Some(pane_value)) = (&env.tmux, &env.tmux_pane) else {
        return PaneLookup::NotInTmux;
    };
    let Some(socket) = tmux::socket_from_tmux_env(tmux_value) else {
        return PaneLookup::Unreadable { reason: format!("TMUX is not <socket>,<pid>,<session>: {tmux_value:?}") };
    };
    let pane = match PaneId::parse(pane_value) {
        Ok(pane) => pane,
        Err(error) => return PaneLookup::Unreadable { reason: error.to_string() },
    };
    match tmux::read_pane(&socket, &pane) {
        Ok(line) => tmux::parse_pane_line(&line, socket, pane).map_or_else(|reason| PaneLookup::Unreadable { reason }, PaneLookup::Found),
        Err(error) => PaneLookup::Unreadable { reason: error.to_string() },
    }
}

fn repo_info(facts: RepositoryFacts) -> RepoInfo {
    let head = match facts.head {
        HeadState::Branch(name) => RepoHead::Branch { name: DisplayText::new(&name) },
        HeadState::Detached { short_sha } => RepoHead::Detached { short_sha: DisplayText::new(&short_sha) },
    };
    RepoInfo { main_repo_name: DisplayText::new(&facts.main_repo_name), head }
}
```

Add `mod context;` and `mod tmux;` to `main.rs`.

- [ ] **Step 5: Run the unit tests**

Run: `cd ~/crates && cargo test -p agent-notify`
Expected: all pass.

- [ ] **Step 6: Write the process-test support**

`crates/agent-notify/tests/support/mod.rs`:

```rust
//! A sandbox for running the real `agent-notify` binary: a private socket
//! path, log directory, and recording stubs for both fallback notifiers, so
//! no test shows a real notification.

// Each test binary uses a different part of this module.
#![allow(dead_code)]

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread::JoinHandle;
use std::time::Duration;

use tempfile::TempDir;

const RECORDING_STUB: &str = "#!/bin/sh\ncall=$(mktemp \"$CALLS_DIR/call.XXXXXX\")\nfor argument in \"$@\"; do printf '%s\\0' \"$argument\" >> \"$call\"; done\n";

pub struct Sandbox {
    root: TempDir,
    stubs: PathBuf,
    calls: PathBuf,
    pub socket: PathBuf,
    pub logs: PathBuf,
}

impl Sandbox {
    pub fn new() -> Self {
        // Under /tmp: a Unix socket path is capped at 104 bytes on macOS.
        let root = tempfile::Builder::new().prefix("an-").tempdir_in("/tmp").expect("a directory under /tmp");
        let stubs = root.path().join("bin");
        let calls = root.path().join("calls");
        fs::create_dir_all(&stubs).expect("stub dir");
        fs::create_dir_all(&calls).expect("calls dir");
        for name in ["terminal-notifier", "notify-send"] {
            dotfiles_test_support::stub::write(&stubs.join(name), RECORDING_STUB).expect("a stub");
        }
        Sandbox { socket: root.path().join("agent-notify.sock"), logs: root.path().join("logs"), stubs, calls, root }
    }

    pub fn command(&self, arguments: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_agent-notify"));
        let path = format!("{}:{}", self.stubs.display(), std::env::var("PATH").unwrap_or_default());
        command
            .args(arguments)
            .current_dir(self.root.path())
            .env_clear()
            .env("PATH", path)
            .env("HOME", self.root.path())
            .env("AGENT_NOTIFY_SOCKET", &self.socket)
            .env("AGENT_NOTIFY_LOG_DIR", &self.logs)
            .env("CALLS_DIR", &self.calls);
        command
    }

    pub fn run(&self, command: &mut Command, stdin: &str) -> Output {
        let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("agent-notify starts");
        if let Some(mut input) = child.stdin.take() {
            input.write_all(stdin.as_bytes()).expect("stdin write");
        }
        child.wait_with_output().expect("agent-notify exits")
    }

    /// One entry per notifier call, each the call's argv after the program name.
    pub fn fallback_calls(&self) -> Vec<Vec<String>> {
        let mut files: Vec<PathBuf> = fs::read_dir(&self.calls).expect("calls dir").map(|entry| entry.expect("entry").path()).collect();
        files.sort();
        files
            .iter()
            .map(|file| {
                let text = fs::read_to_string(file).expect("call file");
                text.split('\0').filter(|argument| !argument.is_empty()).map(str::to_string).collect()
            })
            .collect()
    }

    pub fn log_text(&self) -> String {
        fs::read_to_string(self.logs.join("cli.log")).unwrap_or_default()
    }
}

pub enum ReplyMode {
    Ack,
    Reject,
    Never,
}

/// Accepts one connection on `path`. Returns the request line it read.
pub fn listen(path: &Path, mode: ReplyMode) -> JoinHandle<Option<String>> {
    let listener = UnixListener::bind(path).expect("bind the test socket");
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().ok()?;
        let answer = match mode {
            ReplyMode::Never => {
                std::thread::sleep(Duration::from_secs(2));
                return None;
            }
            ReplyMode::Ack => r#"{"ok":true,"daemon":"test"}"#,
            ReplyMode::Reject => r#"{"ok":false,"code":"unknown_kind","accepts":[1,1]}"#,
        };
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line).ok()?;
        (&stream).write_all(format!("{answer}\n").as_bytes()).ok()?;
        Some(line)
    })
}
```

- [ ] **Step 7: Write the failing tmux integration tests**

`crates/agent-notify/tests/tmux_tags.rs`:

```rust
//! The CLI against a throwaway tmux server: tags land on the pane, and
//! `send` reads them back. Asserts tmux state, never command strings.

mod support;

use agent_notify_core::wire::{Request, decode_request};
use dotfiles_test_support::tmux_server::Server;
use support::{ReplyMode, Sandbox, listen};

struct Pane {
    server: Server,
    id: String,
    _directory: tempfile::TempDir,
}

impl Pane {
    fn new(label: &str) -> Self {
        let server = Server::new(label);
        let directory = tempfile::tempdir().expect("a directory");
        server.new_session("main", directory.path());
        server.tmux(&["rename-window", "-t", "main", "code/feature-x"]);
        let id = server.stdout(&["display-message", "-p", "-t", "main", "#{pane_id}"]);
        Pane { server, id, _directory: directory }
    }

    fn option(&self, name: &str) -> String {
        self.server.stdout(&["show-options", "-p", "-q", "-v", "-t", &self.id, name])
    }

    fn command(&self, sandbox: &Sandbox, arguments: &[&str]) -> std::process::Command {
        let mut command = sandbox.command(arguments);
        command
            .env("TMUX", format!("{},{},0", self.server.socket_path().display(), self.server.pid()))
            .env("TMUX_PANE", &self.id);
        command
    }
}

fn payload(event: &str, session: &str) -> String {
    format!(r#"{{"session_id":"{session}","prompt_id":"prompt-1","cwd":"/tmp","hook_event_name":"{event}","source":"startup"}}"#)
}

#[test]
fn hooks_bind_and_unbind_the_pane_tags() {
    let pane = Pane::new("an-tags");
    let sandbox = Sandbox::new();
    let hook = |event: &str, session: &str| {
        let output = sandbox.run(&mut pane.command(&sandbox, &["hook", "claude-code"]), &payload(event, session));
        assert!(output.status.success(), "{event}: {output:?}");
    };

    hook("SessionStart", "session-a");
    assert_eq!(pane.option("@agent_notify_session"), "session-a");
    assert_eq!(pane.option("@agent_notify_prompt"), "");

    hook("UserPromptSubmit", "session-a");
    assert_eq!(pane.option("@agent_notify_prompt"), "prompt-1");

    hook("SessionEnd", "session-b");
    assert_eq!(pane.option("@agent_notify_session"), "session-a", "another session's end leaves the tag");

    hook("SessionEnd", "session-a");
    assert_eq!(pane.option("@agent_notify_session"), "");
    assert_eq!(pane.option("@agent_notify_prompt"), "");
}

#[test]
fn send_carries_the_pane_tags_and_the_window() {
    let pane = Pane::new("an-send");
    let sandbox = Sandbox::new();
    sandbox.run(&mut pane.command(&sandbox, &["hook", "claude-code"]), &payload("SessionStart", "session-a"));
    sandbox.run(&mut pane.command(&sandbox, &["hook", "claude-code"]), &payload("UserPromptSubmit", "session-a"));

    let received = listen(&sandbox.socket, ReplyMode::Ack);
    let output = sandbox.run(&mut pane.command(&sandbox, &["send", "--kind", "info", "--body", "Report ready"]), "");
    assert!(output.status.success(), "{output:?}");
    let line = received.join().expect("listener thread").expect("a request line");

    let Ok(Request::Agent { origin, .. }) = decode_request(line.trim_end()) else {
        panic!("expected an agent request: {line}");
    };
    let window_index: u32 = pane.server.stdout(&["display-message", "-p", "-t", &pane.id, "#{window_index}"]).parse().expect("an index");
    assert_eq!(origin.session.map(|session| session.as_str().to_string()), Some("session-a".to_string()));
    assert_eq!(origin.prompt.map(|prompt| prompt.as_str().to_string()), Some("prompt-1".to_string()));
    let window = origin.window.expect("a window");
    assert_eq!((window.index, window.name.as_str()), (window_index, "code/feature-x"));
    assert!(sandbox.fallback_calls().is_empty(), "an acknowledged send shows no fallback");
}
```

- [ ] **Step 8: Run the integration tests**

Run: `cd ~/crates && cargo test -p agent-notify --test tmux_tags`
Expected: both tests fail until Task 17 adds `main`, because the stub `main` does nothing. Confirm they fail on the first tag assertion, not on setup. Then continue to Task 17 without committing the failing integration test file; commit the unit-tested modules now:

```bash
cd ~ && config add crates/agent-notify/src/tmux.rs crates/agent-notify/src/context.rs crates/agent-notify/src/main.rs
config commit -m "Add the agent-notify tmux adapter and input gathering" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/agent-notify/src/tmux.rs crates/agent-notify/src/context.rs crates/agent-notify/src/main.rs
```

---

### Task 17: Socket client, fallback, logging, and `main`

**Files:**
- Create: `crates/agent-notify/src/client.rs`, `src/fallback.rs`, `src/log.rs`, `tests/process.rs`
- Modify: `crates/agent-notify/src/main.rs`

**Interfaces:**
- Consumes: Tasks 15 and 16; core `paths`, `wire::{Reply, ReplyCode, encode_line}`, `fallback::FallbackNote`.
- Produces:
  - `client::BUDGET: Duration = 300 ms`; `client::Delivery::{Acked { daemon }, Rejected { code }, NoSocket(&'static str), ConnectFailed(io::ErrorKind), TimedOut, BadReply(String)}` with `acknowledged(&self) -> bool`; `client::deliver(&Path, &str, Duration) -> Delivery`.
  - `fallback::notify(Platform, &FallbackNote) -> io::Result<()>`, `fallback::guard_leading_dash(&str) -> String`, `fallback::neutralize_markup(&str) -> String`.
  - `log::init(Option<&Path>, Option<&str>)`.

- [ ] **Step 1: Write the failing unit tests**

`src/client.rs` tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::UnixListener;

    fn socket_dir() -> tempfile::TempDir {
        tempfile::Builder::new().prefix("an-client-").tempdir_in("/tmp").expect("a directory")
    }

    fn serve(path: &Path, answer: Option<&'static str>) {
        let listener = UnixListener::bind(path).expect("bind");
        std::thread::spawn(move || {
            let Ok((stream, _)) = listener.accept() else { return };
            let Some(answer) = answer else {
                std::thread::sleep(Duration::from_secs(2));
                return;
            };
            let mut line = String::new();
            if BufReader::new(&stream).read_line(&mut line).is_ok() {
                let _sent = (&stream).write_all(format!("{answer}\n").as_bytes());
            }
        });
    }

    #[test]
    fn an_ok_reply_is_an_acknowledgement() {
        let directory = socket_dir();
        let path = directory.path().join("s.sock");
        serve(&path, Some(r#"{"ok":true,"daemon":"stamp"}"#));
        assert!(matches!(deliver(&path, "{}\n", BUDGET), Delivery::Acked { daemon: Some(daemon) } if daemon == "stamp"));
    }

    #[test]
    fn a_refusal_is_not_an_acknowledgement() {
        let directory = socket_dir();
        let path = directory.path().join("s.sock");
        serve(&path, Some(r#"{"ok":false,"code":"unsupported_version","accepts":[1,1]}"#));
        let delivery = deliver(&path, "{}\n", BUDGET);
        assert!(matches!(delivery, Delivery::Rejected { code: Some(ReplyCode::UnsupportedVersion) }));
        assert!(!delivery.acknowledged());
    }

    #[test]
    fn a_silent_daemon_times_out_within_the_budget() {
        let directory = socket_dir();
        let path = directory.path().join("s.sock");
        serve(&path, None);
        let started = Instant::now();
        assert!(matches!(deliver(&path, "{}\n", BUDGET), Delivery::TimedOut));
        assert!(started.elapsed() < BUDGET + Duration::from_millis(200), "took {:?}", started.elapsed());
    }

    #[test]
    fn a_missing_or_stale_socket_fails_to_connect() {
        let directory = socket_dir();
        let missing = directory.path().join("missing.sock");
        assert!(matches!(deliver(&missing, "{}\n", BUDGET), Delivery::ConnectFailed(_)));
        let stale = directory.path().join("stale.sock");
        std::fs::write(&stale, "").expect("a regular file");
        assert!(matches!(deliver(&stale, "{}\n", BUDGET), Delivery::ConnectFailed(_)));
    }

    #[test]
    fn a_non_json_reply_is_a_bad_reply() {
        let directory = socket_dir();
        let path = directory.path().join("s.sock");
        serve(&path, Some("hello"));
        assert!(matches!(deliver(&path, "{}\n", BUDGET), Delivery::BadReply(_)));
    }
}
```

`src/fallback.rs` tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leading_dash_is_guarded() {
        assert_eq!(guard_leading_dash("-execute rm -rf ~"), " -execute rm -rf ~");
        assert_eq!(guard_leading_dash("ok -execute"), "ok -execute");
    }

    #[test]
    fn markup_characters_become_look_alikes() {
        assert_eq!(neutralize_markup(r#"<b>"a" & 'b'</b> a < b"#), "\u{ff1c}b\u{ff1e}\"a\" \u{ff06} 'b'\u{ff1c}/b\u{ff1e} a \u{ff1c} b");
        assert_eq!(neutralize_markup("plain text"), "plain text");
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd ~/crates && cargo test -p agent-notify client fallback`
Expected: compile errors. (Run `cargo test -p agent-notify` if the two-filter form errors.)

- [ ] **Step 3: Implement `client.rs`**

```rust
//! One request, one reply, inside a fixed time budget.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use agent_notify_core::wire::{Reply, ReplyCode};

/// Connect, write and reply together. A hook must not hold up the agent.
pub const BUDGET: Duration = Duration::from_millis(300);
const MAX_REPLY_BYTES: u64 = 64 * 1024;

/// What happened to one request.
#[derive(Debug)]
pub enum Delivery {
    /// The daemon accepted it.
    Acked {
        /// The daemon's build stamp.
        daemon: Option<String>,
    },
    /// The daemon refused it.
    Rejected {
        /// Why.
        code: Option<ReplyCode>,
    },
    /// No socket path on this machine.
    NoSocket(&'static str),
    /// The connect or write failed.
    ConnectFailed(io::ErrorKind),
    /// The budget ran out.
    TimedOut,
    /// The reply was missing or not a reply.
    BadReply(String),
}

impl Delivery {
    /// True only when the daemon accepted the request.
    pub fn acknowledged(&self) -> bool {
        matches!(self, Delivery::Acked { .. })
    }
}

/// Sends `line` to the socket at `path` and reads one reply line. A daemon
/// that accepts after the deadline makes a duplicate; that is the accepted
/// failure, since a silent drop is worse.
pub fn deliver(path: &Path, line: &str, budget: Duration) -> Delivery {
    let started = Instant::now();
    let remaining = || budget.checked_sub(started.elapsed()).filter(|left| !left.is_zero());
    let mut stream = match UnixStream::connect(path) {
        Ok(stream) => stream,
        Err(error) => return Delivery::ConnectFailed(error.kind()),
    };
    let Some(left) = remaining() else { return Delivery::TimedOut };
    if let Err(error) = stream.set_write_timeout(Some(left)).and_then(|()| stream.write_all(line.as_bytes())) {
        return classify(&error);
    }
    let Some(left) = remaining() else { return Delivery::TimedOut };
    if let Err(error) = stream.set_read_timeout(Some(left)) {
        return classify(&error);
    }
    let mut reply = String::new();
    match BufReader::new((&stream).take(MAX_REPLY_BYTES)).read_line(&mut reply) {
        Ok(0) => Delivery::BadReply("the daemon closed the connection without a reply".to_string()),
        Ok(_) => match serde_json::from_str::<Reply>(&reply) {
            Ok(reply) if reply.ok => Delivery::Acked { daemon: reply.daemon },
            Ok(reply) => Delivery::Rejected { code: reply.code },
            Err(error) => Delivery::BadReply(error.to_string()),
        },
        Err(error) => classify(&error),
    }
}

fn classify(error: &io::Error) -> Delivery {
    match error.kind() {
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => Delivery::TimedOut,
        kind => Delivery::ConnectFailed(kind),
    }
}
```

- [ ] **Step 4: Implement `fallback.rs`**

```rust
//! The plain OS notification shown when the daemon does not acknowledge.
//! Text goes in as separate argv elements, never through a shell.

use std::io;
use std::process::{Command, Stdio};

use agent_notify_core::fallback::FallbackNote;
use agent_notify_core::paths::Platform;

const ALACRITTY_BUNDLE_ID: &str = "org.alacritty";

/// Shows `note` with the platform's notifier and waits for it to exit.
///
/// # Errors
///
/// Returns the spawn error, or an error naming the notifier when it fails.
pub fn notify(platform: Platform, note: &FallbackNote) -> io::Result<()> {
    let mut command = match platform {
        Platform::MacOs => {
            let mut command = Command::new("terminal-notifier");
            command
                .arg("-title")
                .arg(guard_leading_dash(&note.title))
                .arg("-subtitle")
                .arg(note.subtitle)
                .arg("-message")
                .arg(guard_leading_dash(&note.message))
                .arg("-activate")
                .arg(ALACRITTY_BUNDLE_ID);
            command
        }
        Platform::Linux => {
            let mut command = Command::new("notify-send");
            command
                .arg("--app-name=agent-notify")
                .arg(if note.urgent { "--urgency=critical" } else { "--urgency=normal" })
                .arg("--")
                .arg(&note.title)
                .arg(neutralize_markup(&format!("{}\n{}", note.subtitle, note.message)));
            command
        }
        Platform::Other => return Err(io::Error::other("no fallback notifier on this platform")),
    };
    let status = command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("{} exited with {status}", command.get_program().to_string_lossy())))
    }
}

/// terminal-notifier reads options through the Cocoa argument domain, which
/// takes a value that starts with `-` as the next option's name. A leading
/// space keeps agent text a value.
pub fn guard_leading_dash(text: &str) -> String {
    if text.starts_with('-') { format!(" {text}") } else { text.to_string() }
}

/// Notification servers disagree on body markup. COSMIC parses tags, cuts
/// the body at a bare `<`, and shows entities literally, so no escaping works
/// on every server. Fullwidth look-alikes render the same everywhere.
pub fn neutralize_markup(text: &str) -> String {
    text.chars()
        .map(|character| match character {
            '<' => '\u{ff1c}',
            '>' => '\u{ff1e}',
            '&' => '\u{ff06}',
            other => other,
        })
        .collect()
}
```

- [ ] **Step 5: Implement `log.rs`**

```rust
//! Best-effort file logging. A failure here has nowhere to be reported, and
//! it must never stop a hook, so every error ends logging quietly.

use std::fs::{DirBuilder, OpenOptions};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;
use std::sync::Mutex;

use tracing::level_filters::LevelFilter;

/// Appends to `<directory>/cli.log` at `level` (default `info`). The CLI
/// never rotates: concurrent hooks would race on a rename.
pub fn init(directory: Option<&Path>, level: Option<&str>) {
    let Some(directory) = directory else { return };
    let level = level.and_then(|text| text.parse::<LevelFilter>().ok()).unwrap_or(LevelFilter::INFO);
    if DirBuilder::new().recursive(true).mode(0o700).create(directory).is_err() {
        return;
    }
    let Ok(file) = OpenOptions::new().create(true).append(true).mode(0o600).open(directory.join("cli.log")) else {
        return;
    };
    let subscriber = tracing_subscriber::fmt().with_writer(Mutex::new(file)).with_ansi(false).with_max_level(level).finish();
    let _already_set = tracing::subscriber::set_global_default(subscriber);
}
```

- [ ] **Step 6: Replace `main.rs`**

```rust
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
//! The agent-notify CLI. Hooks and agents call it. It tags tmux panes, sends
//! one request to the daemon, and falls back to a plain OS notification.
//! `hook` exits 0 whatever happens: a `Stop` hook that exits 2 blocks the stop.

mod cli;
mod client;
mod context;
mod fallback;
mod invocation;
mod log;
mod tmux;

use std::ffi::OsString;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::ExitCode;

use agent_notify_core::paths::{Platform, SocketLocation, log_directory, socket_location};
use agent_notify_core::wire::encode_line;
use clap::Parser;

use crate::cli::{Cli, Command};
use crate::context::EnvSnapshot;
use crate::invocation::{Exit, Inputs, Invocation, PaneLookup};

fn main() -> ExitCode {
    let arguments: Vec<OsString> = std::env::args_os().collect();
    if arguments.get(1).is_some_and(|argument| argument == "--stamp") {
        // config-build embeds the stamp and config doctor probes it.
        println!("{}", option_env!("CONFIG_MANIFEST_STAMP").unwrap_or("unstamped"));
        return ExitCode::SUCCESS;
    }
    let is_hook = arguments.get(1).is_some_and(|argument| argument == "hook");
    let env = EnvSnapshot::read();
    log::init(log_directory(&env.path_inputs()).as_deref(), env.log_level.as_deref());
    std::panic::set_hook(Box::new(|info| {
        tracing::error!(panic = %info, backtrace = %std::backtrace::Backtrace::force_capture(), "panic");
        eprintln!("agent-notify: internal error: {info}");
    }));
    match catch_unwind(AssertUnwindSafe(|| run(&arguments, &env, is_hook))) {
        Ok(code) => code,
        Err(_panic) if is_hook => ExitCode::SUCCESS,
        Err(_panic) => ExitCode::from(1),
    }
}

fn run(arguments: &[OsString], env: &EnvSnapshot, is_hook: bool) -> ExitCode {
    let cli = match Cli::try_parse_from(arguments) {
        Ok(cli) => cli,
        Err(error) if is_hook => {
            tracing::warn!(error = %error, "hook arguments did not parse");
            return ExitCode::SUCCESS;
        }
        Err(error) => error.exit(),
    };
    let inputs = context::gather(&cli.command, env, context::clock(), context::random_bytes());
    if env.trace {
        tracing::info!(event_id = %inputs.event_id, stdin = ?inputs.stdin, "raw hook input");
    }
    let invocation = invocation::plan_invocation(&cli.command, &inputs);
    execute(&cli.command, &inputs, &invocation, env)
}

fn execute(command: &Command, inputs: &Inputs, invocation: &Invocation, env: &EnvSnapshot) -> ExitCode {
    let event_id = inputs.event_id.as_str();
    if let Some(reason) = &invocation.log {
        tracing::info!(event_id, reason = %reason, "nothing sent");
    }
    if let Some(message) = &invocation.stderr {
        eprintln!("agent-notify: {message}");
    }
    if command.is_dry_run() {
        for tag in &invocation.tags {
            println!("tag: {tag:?}");
        }
        if let Some(request) = &invocation.request
            && let Ok(line) = encode_line(request)
        {
            print!("{line}");
        }
        return exit_code(invocation.exit);
    }
    if let PaneLookup::Found(facts) = &inputs.pane {
        for tag in &invocation.tags {
            if let Err(error) = tmux::apply_tag(&facts.key.server.socket_path, &facts.key.pane, tag) {
                tracing::warn!(event_id, error = %error, tag = ?tag, "tag write failed");
            }
        }
    }
    if let PaneLookup::Unreadable { reason } = &inputs.pane {
        tracing::warn!(event_id, reason = %reason, "tmux pane unreadable");
    }
    let Some(request) = &invocation.request else { return exit_code(invocation.exit) };
    let Ok(line) = encode_line(request) else {
        tracing::error!(event_id, "request did not encode");
        return exit_code(invocation.exit);
    };
    let delivery = match socket_location(&env.path_inputs()) {
        SocketLocation::At(path) => client::deliver(&path, &line, client::BUDGET),
        SocketLocation::Unavailable(reason) => client::Delivery::NoSocket(reason),
    };
    tracing::info!(event_id, request = ?request, delivery = ?delivery, "sent");
    if !delivery.acknowledged() {
        fall_back(command, invocation, event_id);
    }
    exit_code(invocation.exit)
}

fn fall_back(command: &Command, invocation: &Invocation, event_id: &str) {
    match &invocation.fallback {
        Some(note) => match fallback::notify(Platform::current(), note) {
            Ok(()) => {
                tracing::info!(event_id, "fallback shown");
                if !command.is_hook() {
                    eprintln!("agent-notify: the daemon did not answer; showed a plain notification");
                }
            }
            Err(error) => tracing::warn!(event_id, error = %error, "fallback failed"),
        },
        None if !command.is_hook() => eprintln!("agent-notify: the daemon did not answer; nothing was recorded"),
        None => {}
    }
}

fn exit_code(exit: Exit) -> ExitCode {
    match exit {
        Exit::Success => ExitCode::SUCCESS,
        Exit::Usage => ExitCode::from(2),
    }
}
```

- [ ] **Step 7: Write the process tests**

`crates/agent-notify/tests/process.rs`:

```rust
//! The real binary, with stub notifiers and a private socket path. Each test
//! asserts the exit status and whether the fallback fired.

mod support;

use std::os::unix::net::UnixListener;
use std::time::{Duration, Instant};

use agent_notify_core::wire::{AgentEvent, Request, decode_request};
use support::{ReplyMode, Sandbox, listen};

const PERMISSION: &str = r#"{"session_id":"session-a","prompt_id":"prompt-1","cwd":"/tmp","hook_event_name":"Notification","notification_type":"permission_prompt","message":"Allow Bash?"}"#;
const STOP: &str = r#"{"session_id":"session-a","prompt_id":"prompt-1","cwd":"/tmp","hook_event_name":"Stop","last_assistant_message":"Done."}"#;

fn hook(sandbox: &Sandbox, stdin: &str) -> std::process::Output {
    sandbox.run(&mut sandbox.command(&["hook", "claude-code"]), stdin)
}

#[test]
fn no_daemon_permission_prompt_falls_back() {
    let sandbox = Sandbox::new();
    let output = hook(&sandbox, PERMISSION);
    assert!(output.status.success());
    assert_eq!(sandbox.fallback_calls().len(), 1);
    assert!(sandbox.log_text().contains("fallback shown"));
}

#[test]
fn no_daemon_stop_shows_nothing() {
    let sandbox = Sandbox::new();
    assert!(hook(&sandbox, STOP).status.success());
    assert!(sandbox.fallback_calls().is_empty());
}

#[test]
fn a_stale_socket_file_falls_back() {
    let sandbox = Sandbox::new();
    std::fs::write(&sandbox.socket, "").expect("a stale file");
    assert!(hook(&sandbox, PERMISSION).status.success());
    assert_eq!(sandbox.fallback_calls().len(), 1);
}

#[test]
fn a_silent_daemon_times_out_and_falls_back() {
    let sandbox = Sandbox::new();
    let _listener = listen(&sandbox.socket, ReplyMode::Never);
    let started = Instant::now();
    assert!(hook(&sandbox, PERMISSION).status.success());
    assert!(started.elapsed() < Duration::from_millis(1_500), "took {:?}", started.elapsed());
    assert_eq!(sandbox.fallback_calls().len(), 1);
}

#[test]
fn a_rejecting_daemon_falls_back() {
    let sandbox = Sandbox::new();
    let _listener = listen(&sandbox.socket, ReplyMode::Reject);
    assert!(hook(&sandbox, PERMISSION).status.success());
    assert_eq!(sandbox.fallback_calls().len(), 1);
}

#[test]
fn an_acknowledging_daemon_gets_the_request_and_no_fallback() {
    let sandbox = Sandbox::new();
    let received = listen(&sandbox.socket, ReplyMode::Ack);
    assert!(hook(&sandbox, PERMISSION).status.success());
    let line = received.join().expect("listener").expect("a line");
    assert!(matches!(decode_request(line.trim_end()), Ok(Request::Agent { event: AgentEvent::NeedsInput { .. }, .. })));
    assert!(sandbox.fallback_calls().is_empty());
}

#[test]
fn empty_and_garbage_stdin_exit_zero_without_a_notification() {
    for stdin in ["", "not json", "\u{0}\u{1}\u{2}", "{\"hook_event_name\":\"Stop\"}"] {
        let sandbox = Sandbox::new();
        let output = hook(&sandbox, stdin);
        assert!(output.status.success(), "{stdin:?}");
        assert!(sandbox.fallback_calls().is_empty());
    }
}

#[test]
fn an_unknown_hook_event_exits_zero_and_logs_it() {
    let sandbox = Sandbox::new();
    let stdin = r#"{"session_id":"s","cwd":"/tmp","hook_event_name":"PreCompact"}"#;
    assert!(hook(&sandbox, stdin).status.success());
    assert!(sandbox.log_text().contains("PreCompact"));
}

#[test]
fn bad_hook_arguments_still_exit_zero() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&mut sandbox.command(&["hook", "codex"]), STOP);
    assert!(output.status.success());
}

#[test]
fn send_usage_errors_exit_two_with_a_message() {
    let sandbox = Sandbox::new();
    for arguments in [&["send", "--kind", "info", "--body", "  "][..], &["send", "--kind", "loud", "--body", "x"]] {
        let output = sandbox.run(&mut sandbox.command(arguments), "");
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(!output.stderr.is_empty());
    }
    assert!(sandbox.fallback_calls().is_empty());
}

#[test]
fn quiet_outside_tmux_exits_two() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&mut sandbox.command(&["quiet", "--until", "next-send"]), "");
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("tmux"));
}

#[test]
fn send_without_a_daemon_falls_back_and_says_so() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&mut sandbox.command(&["send", "--kind", "attention", "--body", "CI is red"]), "");
    assert!(output.status.success());
    assert_eq!(sandbox.fallback_calls().len(), 1);
    assert!(String::from_utf8_lossy(&output.stderr).contains("did not answer"));
}

#[test]
fn an_unwritable_log_directory_does_not_block_the_hook() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(sandbox.command(&["hook", "claude-code"]).env("AGENT_NOTIFY_LOG_DIR", "/dev/null/logs"), PERMISSION);
    assert!(output.status.success());
    assert_eq!(sandbox.fallback_calls().len(), 1);
}

#[test]
fn dry_run_prints_the_request_and_contacts_nothing() {
    let sandbox = Sandbox::new();
    let listener = UnixListener::bind(&sandbox.socket).expect("bind");
    listener.set_nonblocking(true).expect("nonblocking");
    let output = sandbox.run(&mut sandbox.command(&["hook", "claude-code", "--dry-run"]), PERMISSION);
    assert!(output.status.success());
    let printed = String::from_utf8_lossy(&output.stdout);
    assert!(decode_request(printed.trim_end()).is_ok(), "{printed}");
    assert!(listener.accept().is_err(), "dry run must not connect");
    assert!(sandbox.fallback_calls().is_empty());
}

#[test]
fn dead_tmux_server_still_falls_back() {
    let sandbox = Sandbox::new();
    let started = Instant::now();
    let output = sandbox.run(
        sandbox.command(&["hook", "claude-code"]).env("TMUX", "/tmp/an-no-such-dir/sock,1,0").env("TMUX_PANE", "%1"),
        PERMISSION,
    );
    assert!(output.status.success());
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(sandbox.fallback_calls().len(), 1);
}

#[test]
fn fallback_passes_text_as_single_arguments() {
    let sandbox = Sandbox::new();
    let stdin = r#"{"session_id":"s","prompt_id":"p","cwd":"/tmp","hook_event_name":"Notification","notification_type":"permission_prompt","message":"-execute touch /tmp/pwned; echo $(id)"}"#;
    assert!(hook(&sandbox, stdin).status.success());
    let calls = sandbox.fallback_calls();
    assert_eq!(calls.len(), 1);
    let call = &calls[0];
    if cfg!(target_os = "macos") {
        let message_at = call.iter().position(|argument| argument == "-message").expect("a -message flag");
        assert_eq!(call[message_at + 1], " -execute touch /tmp/pwned; echo $(id)");
        assert!(!call.iter().any(|argument| argument == "-execute"));
    } else {
        assert!(call.iter().any(|argument| argument.ends_with("-execute touch /tmp/pwned; echo $(id)")));
    }
}

#[test]
fn stamp_answers_without_a_subcommand() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&mut sandbox.command(&["--stamp"]), "");
    assert!(output.status.success());
    assert!(!output.stdout.is_empty());
}
```

- [ ] **Step 8: Run all CLI tests**

Run: `cd ~/crates && cargo test -p agent-notify`
Expected: unit, `process` and `tmux_tags` tests all pass. If `a_silent_daemon_times_out_and_falls_back` is slow, check that `deliver` sets the read timeout from the remaining budget, not from `BUDGET`.

- [ ] **Step 9: Run the workspace gate and clippy**

Run: `cd ~/crates && cargo test --workspace --locked && cargo clippy --workspace --all-targets -- -D warnings`
Expected: everything passes. `--locked` is fine now: Tasks 1, 13 and 15 committed the lockfile changes.

- [ ] **Step 10: Commit**

```bash
cd ~ && config add crates/agent-notify
config commit -m "Add the agent-notify socket client, fallback, logging and entry point" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/agent-notify
```

---

### Task 18: Claude plugin, dependencies, `/monitor-ci`, and install

**Files:**
- Create: `tools/agent-notify/.claude-plugin/marketplace.json`, `tools/agent-notify/claude-plugin/.claude-plugin/plugin.json`, `tools/agent-notify/claude-plugin/hooks/hooks.json`, `tools/agent-notify/claude-plugin/skills/agent-notify/SKILL.md`, `tools/agent-notify/tests/plugin.test.ts`
- Modify: `.claude/skills/monitor-ci/SKILL.md` (Step 1 and Step 5), `deps/deps-mac.toml`, `deps/deps-linux.toml`

**Interfaces:**
- Consumes: the installed `~/.local/bin/agent-notify` binary (Task 17), its `hook claude-code`, `send --kind {info|attention} --body`, and `quiet --until {next-send|next-prompt} --max-minutes N | --off` commands.
- Produces: the `agent-notify@dotfiles-tools` plugin. Plan 2 reads nothing from this task.

- [ ] **Step 1: Write the failing manifest test**

The hook wiring is configuration, so the test checks the contract the spec names: which events run, sync or async, and the Notification matcher. `tools/agent-notify/tests/plugin.test.ts`:

```ts
import { describe, expect, test } from "bun:test";

const pluginRoot = `${import.meta.dir}/../claude-plugin`;

type HookEntry = { matcher?: string; hooks: { type: string; command: string; timeout?: number; async?: boolean }[] };
type HooksFile = { hooks: Record<string, HookEntry[]> };

const hooksFile: HooksFile = await Bun.file(`${pluginRoot}/hooks/hooks.json`).json();
const command = '"$HOME/.local/bin/agent-notify" hook claude-code';

describe("hooks.json", () => {
  test("binds exactly the events the CLI maps", () => {
    expect(Object.keys(hooksFile.hooks).sort()).toEqual(
      ["Notification", "SessionEnd", "SessionStart", "Stop", "StopFailure", "UserPromptSubmit"],
    );
  });

  test.each(["SessionStart", "UserPromptSubmit", "SessionEnd"])("%s runs sync so the pane tag lands first", (event) => {
    const [entry] = hooksFile.hooks[event] ?? [];
    expect(entry?.hooks).toEqual([{ type: "command", command, timeout: 5 }]);
  });

  test.each(["Stop", "StopFailure", "Notification"])("%s runs async", (event) => {
    const [entry] = hooksFile.hooks[event] ?? [];
    expect(entry?.hooks).toEqual([{ type: "command", command, async: true }]);
  });

  test("the Notification matcher keeps idle_prompt out", () => {
    const matcher = new RegExp(`^(${hooksFile.hooks.Notification?.[0]?.matcher ?? ""})$`);
    for (const notificationType of ["permission_prompt", "elicitation_dialog", "elicitation_url_dialog", "elicitation_complete"]) {
      expect(matcher.test(notificationType)).toBe(true);
    }
    for (const notificationType of ["idle_prompt", "agent_needs_input", "agent_completed"]) {
      expect(matcher.test(notificationType)).toBe(false);
    }
  });
});

describe("marketplace", () => {
  test("points at the plugin directory", async () => {
    const marketplace = await Bun.file(`${import.meta.dir}/../.claude-plugin/marketplace.json`).json();
    expect(marketplace.name).toBe("dotfiles-tools");
    expect(marketplace.plugins).toEqual([expect.objectContaining({ name: "agent-notify", source: "./claude-plugin" })]);
    const plugin = await Bun.file(`${pluginRoot}/.claude-plugin/plugin.json`).json();
    expect(plugin.name).toBe("agent-notify");
  });
});
```

- [ ] **Step 2: Run it to see it fail**

Run: `cd ~/tools/agent-notify && bun test`
Expected: FAIL, `hooks.json` does not exist.

- [ ] **Step 3: Write the plugin files**

`tools/agent-notify/.claude-plugin/marketplace.json`:

```json
{
  "name": "dotfiles-tools",
  "owner": { "name": "dotfiles" },
  "plugins": [
    {
      "name": "agent-notify",
      "source": "./claude-plugin",
      "description": "Contextual desktop notifications for Claude Code sessions in tmux"
    }
  ]
}
```

`tools/agent-notify/claude-plugin/.claude-plugin/plugin.json`:

```json
{
  "name": "agent-notify",
  "version": "0.1.0",
  "description": "Sends Claude Code session events to agent-notify and teaches the agent when to notify"
}
```

`tools/agent-notify/claude-plugin/hooks/hooks.json`:

```json
{
  "hooks": {
    "SessionStart": [
      { "hooks": [{ "type": "command", "command": "\"$HOME/.local/bin/agent-notify\" hook claude-code", "timeout": 5 }] }
    ],
    "UserPromptSubmit": [
      { "hooks": [{ "type": "command", "command": "\"$HOME/.local/bin/agent-notify\" hook claude-code", "timeout": 5 }] }
    ],
    "SessionEnd": [
      { "hooks": [{ "type": "command", "command": "\"$HOME/.local/bin/agent-notify\" hook claude-code", "timeout": 5 }] }
    ],
    "Stop": [
      { "hooks": [{ "type": "command", "command": "\"$HOME/.local/bin/agent-notify\" hook claude-code", "async": true }] }
    ],
    "StopFailure": [
      { "hooks": [{ "type": "command", "command": "\"$HOME/.local/bin/agent-notify\" hook claude-code", "async": true }] }
    ],
    "Notification": [
      {
        "matcher": "permission_prompt|elicitation_dialog|elicitation_url_dialog|elicitation_complete",
        "hooks": [{ "type": "command", "command": "\"$HOME/.local/bin/agent-notify\" hook claude-code", "async": true }]
      }
    ]
  }
}
```

- [ ] **Step 4: Run the manifest test**

Run: `cd ~/tools/agent-notify && bun test`
Expected: PASS.

- [ ] **Step 5: Write the agent skill**

`tools/agent-notify/claude-plugin/skills/agent-notify/SKILL.md`:

````markdown
---
name: agent-notify
description: Use when a task will run long, loops on a wait (CI, deploys, polling), or reaches a result the user must see. Tells you when to call `agent-notify send` and `agent-notify quiet`, and when to stay silent.
---

# agent-notify

The user gets a desktop notification each time you stop. The hooks send it. You do not need to do anything for an ordinary turn.

Use the CLI only to make a notification more useful, or to stop noise.

## `agent-notify send`

```bash
agent-notify send --kind attention --body "CI failed on lint: 3 errors in src/api.ts"
agent-notify send --kind info --body "Benchmarks done: p99 down 18%"
```

- `attention`: the user must act (a decision, a failure, a review). It shows as an amber card and stays until the user clicks it.
- `info`: a result the user wants but does not have to act on. It fades.
- The body is one short sentence. Name what finished and what the user must do. No preamble, no markdown.
- Send at most once per result. Do not send progress updates.

## `agent-notify quiet`

Before a loop that stops and starts many times (a CI watch, a poll, a retry loop), turn off the per-stop notification:

```bash
agent-notify quiet --until next-send --max-minutes 180
```

Then call `agent-notify send` once, with the outcome. That send ends the quiet period. `--until next-prompt` ends it when the user types instead. `agent-notify quiet --off` ends it now.

`quiet` needs a tmux pane that a SessionStart hook tagged. Outside tmux, it exits 2. When that happens, continue without it.

## Do not

- Do not send when the turn ends normally. The Stop hook already notifies.
- Do not send a permission request. The Notification hook already does.
- Do not put secrets, tokens, or file contents in `--body`. The text goes to the OS notification center and a log file.
````

- [ ] **Step 6: Edit `/monitor-ci`**

In `.claude/skills/monitor-ci/SKILL.md`, at the end of "## Step 1: Resolve PR" (after the "Record:" list), add:

````markdown
Silence the per-stop notification for the length of the watch, when `agent-notify` is installed:

```bash
command -v agent-notify >/dev/null && agent-notify quiet --until next-send --max-minutes 180 || true
```

A non-zero exit (for example, outside tmux) is not an error. Continue.
````

In "## Step 5: Final Report", after the bullet list and before "## Important", add:

````markdown
Then send one notification with the outcome. This ends the quiet period from Step 1. Use `attention` when the user must act (a red workflow, a manual gate, review comments) and `info` when everything is green:

```bash
command -v agent-notify >/dev/null && agent-notify send --kind attention --body "PR 123: lint failed, 1 manual gate pending" || true
```

Send it only at a terminal state, never on an intermediate wake-up.
````

The example text names no project. The public-repo rule in the skill's "Important" section still applies.

- [ ] **Step 7: Add the fallback notifiers to the deps manifests**

Append to `deps/deps-mac.toml`:

```toml

# terminal-notifier shows the agent-notify fallback notification when the
# daemon is not running.
[terminal-notifier]
command = "terminal-notifier"
docs = "https://github.com/julienXX/terminal-notifier"
```

Append to `deps/deps-linux.toml`:

```toml

# notify-send shows the agent-notify fallback notification when the daemon
# is not running. Ubuntu and Pop!_OS ship it in libnotify-bin.
[notify-send]
command = "notify-send"
docs = "https://gitlab.gnome.org/GNOME/libnotify"
```

Run: `config test`
Expected: PASS. If a deps test pins the entry count or order, update that test with the new entry and say so in the commit body.

- [ ] **Step 8: Commit**

```bash
cd ~ && config add tools/agent-notify .claude/skills/monitor-ci/SKILL.md deps/deps-mac.toml deps/deps-linux.toml
config commit -m "Add the agent-notify Claude plugin and quiet /monitor-ci" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- tools/agent-notify .claude/skills/monitor-ci/SKILL.md deps/deps-mac.toml deps/deps-linux.toml
```

The leak check runs on this commit. A match in the skill text is a defect in the text: reword it. Do not set `SKIP_LEAK_CHECK`.

- [ ] **Step 9: Install (ask the user first)**

These steps change the user's live Claude Code setup. Show the commands and ask before running them.

```bash
config build
agent-notify --stamp
claude plugin marketplace add ~/tools/agent-notify
claude plugin install agent-notify@dotfiles-tools
```

Then add these two entries to `permissions.allow` in `~/.claude/settings.json` (untracked; show the diff and ask before editing):

```json
"Bash(agent-notify send:*)",
"Bash(agent-notify quiet:*)"
```

`notify.sh` stays installed until plan 2. Both fire on Stop during this period. That duplicate is expected.

- [ ] **Step 10: Manual smoke test (with the user)**

Without the daemon, in a tmux pane:

1. Start `claude` in a new tmux window. Run `tmux show-options -p @agent_notify_session`. Expected: the session id.
2. Ask the agent for a Bash command that needs permission. Expected: one terminal-notifier notification whose title names the window, plus `notify.sh`'s notification.
3. Run `agent-notify send --kind info --body "smoke test"` in the pane. Expected: a fallback notification and the stderr line "the daemon did not answer; showed a plain notification".
4. Run `agent-notify hook claude-code --dry-run < /dev/null`. Expected: exit 0, no output.
5. Read `~/Library/Logs/agent-notify/cli.log` (Linux: `~/.local/state/agent-notify/cli.log`). Expected: one line per hook, each with an `event_id`.
6. Exit the session. Expected: `@agent_notify_session` is unset.

Report each result to the user. A failed step is a finding for plan 2's review, not a reason to edit plan 1's code without a new red test.
