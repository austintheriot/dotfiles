# agent-notify Plan 3: Linux Daemon Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Run `agent-notifyd` on Linux: a layer-shell toast card on Pop!_OS COSMIC, a notification-type window on X11, D-Bus notifications on other Wayland desktops, tmux focus and terminal raise on click, a systemd user unit, and `config build --tools` on Linux.

**Architecture:** Plan 2 built the daemon library with every platform service behind a trait in one `Services` record. This plan adds Linux implementations of those traits and a Linux composition root. The daemon library gains Tauri-free adapters: X11 (`xprop`, `xdotool`), COSMIC Wayland (one long-lived Wayland connection for toplevels and idle), the sound player, the terminal opener, and a D-Bus presenter. Each adapter has a pure core (parsers, tables, diffs) with unit tests that run on macOS too. The app crate gains `linux/` beside `macos/`: the GTK panel and the start-up. The UI does not change: on Linux it receives `native_hover: false` and uses its DOM hover path.

**Tech Stack:** As plan 2, plus gtk 0.18, gtk-layer-shell 0.8.2 (feature `v0_6`), wayland-client 0.31.15, wayland-protocols 0.32.13 (features `client`, `staging`), cosmic-protocols 0.2.0 (optional feature `cosmic`), notify-rust 4.18.2 (feature `z`, pure-Rust D-Bus). System tools: tmux, alacritty, pw-play or paplay, xdotool and xprop (X11 only). Docker for the Linux compile and test gate on macOS. A Tart VM with Pop!_OS 24.04 COSMIC for manual checks.

**Spec:** `~/docs/superpowers/specs/2026-10-15-agent-notify-design.md` (approved, commit `534dcfad`), sections 4, 7.2, 9 and 11. **Spike report:** `~/docs/superpowers/spikes/2026-10-15-agent-notify-spikes.md`, sections S2, S3 and S4 (S4 holds the checks run for this plan). Where the spike report differs from the spec, the spike report wins. Plans 1 and 2 must be complete before Task 1.

## Global Constraints

- Every rule in plan 2's Global Constraints still holds. The ones repeated here are the ones Linux code breaks most easily.
- Linux code compiles only under `#[cfg(target_os = "linux")]`. Pure modules (parsers, tables, diffs, unit planners) are not gated, so their tests run on macOS too.
- No `unsafe` in Linux code. The workspace denies `unsafe_code`; no Linux file gets an `allow`.
- Pinned versions, exact (`=`): gtk `=0.18.2`, gtk-layer-shell `=0.8.2` with feature `v0_6`, wayland-client `=0.31.15`, wayland-protocols `=0.32.13` with features `client` and `staging`, cosmic-protocols `=0.2.0` with `default-features = false, features = ["client"]`, notify-rust `=4.18.2` with `default-features = false, features = ["z"]`.
- The adapter is chosen by `XDG_SESSION_TYPE` (spec 7.2): `wayland` or `x11`. COSMIC is detected by `XDG_CURRENT_DESKTOP` containing `COSMIC`, then confirmed by the Wayland globals the compositor offers.
- The daemon spawns every external tool by absolute path, resolved once at start-up: tmux, alacritty, xdotool, xprop, pw-play, paplay. The systemd unit sets `AGENT_NOTIFY_TMUX` and `PATH`.
- Terminal: Alacritty, matched by Wayland app id `Alacritty` and X11 class `Alacritty`.
- Linux socket: `$XDG_RUNTIME_DIR/agent-notify/agent-notify.sock` (plan 1). No `XDG_RUNTIME_DIR` means no socket: the daemon exits 1 and logs why.
- Linux logs go to stderr, which systemd sends to journald (`journalctl --user -u agent-notifyd`). No log files on Linux.
- D-Bus and `notify-send` text replaces `<`, `>` and `&` with U+FF1C, U+FF1E and U+FF06 (spike D8). Never XML-escape.
- systemd unit: `PartOf=`, `After=` and `WantedBy=graphical-session.target`, `Restart=on-failure` (spike D11).
- Do not run `systemctl --user enable` or `restart` against the user's real machine without the user's approval in the session. The Tart VM is disposable and needs no approval.

## Review Focus

These inputs are implied by the spec, but no rule table names them. Each has a test in the task named.

1. **Alacritty is not running when a card is clicked on COSMIC.** The toplevel table has no `Alacritty` entry. The raise must report failure (the core then shows "window not raised"), and it must not panic or block. Test: Task 6, `activation_without_a_terminal_is_none`.
2. **The Wayland connection dies (compositor restart, session logout) while the daemon runs.** Frontmost must read `Unknown`, idle must read `None`, and the raise must fail fast, so cards always show. Test: Task 6, `a_closed_table_reads_as_unknown`.
3. **The user was already idle when the daemon started.** The idle notifier sends `idled` only after a full timeout from registration. Until then idle must read `None` ("show"), not "active". Test: Task 6, `idle_is_unknown_until_the_first_timeout_passes`.
4. **A D-Bus notification is closed by the user, or replaced, and the action arrives late.** An action for a card that is no longer shown must do nothing, and a replaced card must close its old notification. Test: Task 7, `a_replaced_card_closes_its_old_notification` and the daemon's existing unknown-card rule.
5. **`XDG_SESSION_TYPE` is missing (a service started before the session exported its environment).** The daemon must pick the headless adapter set and show cards through D-Bus or not at all, and `doctor` must say why. Test: Task 3, `a_missing_session_type_is_headless`.

## Deviations from the spec

Tell the user about these at handoff.

- **cosmic-protocols is GPL-3.0-only.** It is the only way to read the active window and to raise Alacritty on COSMIC (spike S2-4, S2-5). The `config` repo has no license file and the binary is built only for personal use, so linking it is allowed. The crate stays behind the `cosmic` cargo feature, on by default, with a `cargo deny` exception for that one crate. Without the feature, COSMIC still shows cards; the click switches tmux but does not raise Alacritty, and frontmost reads `Unknown`. The user decides at plan review.
- **Sound.** `pw-play` (or `paplay`) with `/usr/share/sounds/freedesktop/stereo/message-new-instant.oga`, not libcanberra. `canberra-gtk-play` is not installed on Pop!_OS 24.04 (spike S4-4).
- **X11 frontmost.** `xprop -root _NET_ACTIVE_WINDOW`, then `xprop -id <id> WM_CLASS`. Pop!_OS ships xdotool 3.20160805, which has no `getwindowclassname` (spike S4-3).
- **X11 idle and lock.** `None` in v1. The core treats `None` as "show". X11 is the low-priority Arch path.
- **COSMIC lock.** `None` (spike D10).
- **COSMIC idle.** One `ext_idle_notifier_v1` notification with a 30 s timeout (spike S4-2). Idle reads 0 or 30 seconds, not an exact count, which is all the core's 30 s rule needs.
- **D-Bus presenter.** Used only on a Wayland desktop without layer shell (spec 7.2, "Arch on Wayland"). It has no sound of its own and no persistence (spike D9).
- **Logs.** journald only, as the spec says. `doctor` prints the `journalctl` command instead of a log path.

## File map

Create:

- `tools/agent-notify/docker/linux-check.Dockerfile`, `tools/agent-notify/scripts/linux-check.sh`
- `tools/agent-notify/daemon/src/desktop.rs`: session kind from the environment (pure)
- `tools/agent-notify/daemon/src/x11.rs`: `xprop` parsers (pure) and the X11 frontmost reader and raiser
- `tools/agent-notify/daemon/src/linux_tools.rs`: the terminal opener and the sound player
- `tools/agent-notify/daemon/src/cosmic/mod.rs`, `cosmic/table.rs` (pure), `cosmic/idle.rs` (pure), `cosmic/client.rs` (Wayland thread)
- `tools/agent-notify/daemon/src/dbus_presenter.rs`: notification diff (pure) and the notify-rust presenter
- `tools/agent-notify/daemon/tests/x11_adapter.rs`
- `tools/agent-notify/app/src/startup.rs`: start-up shared by macOS and Linux
- `tools/agent-notify/app/src/linux/mod.rs`, `linux/panel.rs`, `linux/geometry.rs` (pure)
- `tools/agent-notify/scripts/systemd-unit.ts` (pure), `tools/agent-notify/tests/systemd-unit.test.ts`

Modify:

- `tools/agent-notify/deny.toml` (Linux targets, two licence exceptions, three advisory ignores)
- `tools/agent-notify/daemon/Cargo.toml`, `src/lib.rs`, `src/environment.rs`, `src/tools.rs`
- `tools/agent-notify/app/Cargo.toml`, `src/main.rs`, `src/logging.rs`, `src/shell.rs`, `src/macos/mod.rs`
- `tools/agent-notify/scripts/install.ts`
- `crates/agent-notify-core/src/text.rs` (`neutralize_markup` moves here), `crates/agent-notify/src/fallback.rs`
- `crates/agent-notify/src/doctor.rs` (systemd check)
- `README-LINUX.md`, `deps/deps-linux.toml`

---

### Task 0: Preconditions

- [ ] **Step 1: Check the earlier plans landed**

Run: `cd ~/tools/agent-notify && cargo test --workspace && (cd ui && bun test) && agent-notify --version`
Expected: all pass, and the CLI prints a version. Stop if not: this plan extends plan 2's code and does not recreate it.

- [ ] **Step 2: Check the Linux test machines**

Run: `docker info --format '{{.ServerVersion}}'` (start Docker with `open -a Docker` if it is down, then poll `docker info`). Then `tart list | grep pop-cosmic-spike`.
Expected: a server version, and the VM listed. If the VM is gone, recreate it from the spike report's "Test setups" section before Task 10. Tasks 1 to 9 need only Docker.

The VM logs in to COSMIC through VNC: `tart run pop-cosmic-spike --no-graphics --vnc-experimental`, then the password `spikepass` for user `spike` (set during spike S4). The VM is disposable; its password is not a secret.

---

### Task 1: The Linux gate: deny targets and a Docker check

**Files:**
- Create: `tools/agent-notify/docker/linux-check.Dockerfile`, `tools/agent-notify/scripts/linux-check.sh`
- Modify: `tools/agent-notify/deny.toml`

**Interfaces:**
- Produces: `scripts/linux-check.sh [check|test|clippy]`, which every later task runs. It builds the image `agent-notify-linux` once and runs cargo in a throwaway container against a read-only copy of the repo.

- [ ] **Step 1: Write the image**

`tools/agent-notify/docker/linux-check.Dockerfile`:

```dockerfile
# The Linux compile and test gate for tools/agent-notify, run from macOS.
# Debian bookworm carries webkit2gtk-4.1 and gtk-layer-shell 0.8, which
# Tauri 2 and gtk-layer-shell's v0_6 feature need. Xvfb, Openbox, xdotool
# and x11-utils serve the X11 adapter tests; tmux serves the tmux tests.
FROM rust:1.94-slim-bookworm@sha256:cf9dd0ec73e75f827fe59123fff9dc65af1a1c8363c3c31ee8d7f8ad0b6a5fb2
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential pkg-config libssl-dev curl ca-certificates unzip \
        libwebkit2gtk-4.1-dev libgtk-layer-shell-dev \
        tmux xvfb openbox xdotool x11-utils xterm \
    && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL https://bun.sh/install | BUN_INSTALL=/usr/local bash -s "bun-v1.3.14"
RUN rustup component add clippy
WORKDIR /work
```

The digest is the one `tests/docker/Dockerfile` already pins for `rust:1.94-slim-bookworm`; copy it from that file if it changed. The slim image has no `curl`, and the Bun installer needs `curl` and `unzip`, so both are on the install line (checked in spike S4-5).

- [ ] **Step 2: Write the runner**

`tools/agent-notify/scripts/linux-check.sh`:

```sh
#!/bin/sh
# Runs cargo for tools/agent-notify inside a Linux container.
#
# usage: linux-check.sh [check|test|clippy] [cargo args...]
#
# The repo is mounted read-only and copied inside the container, so the
# build never writes into the tracked tree. The target directory is a named
# volume, so a second run reuses the first run's build.

set -eu

mode=${1:-check}
[ "$#" -gt 0 ] && shift

root=${DOTFILES_ROOT:-$HOME}
image=agent-notify-linux
dockerfile="$root/tools/agent-notify/docker/linux-check.Dockerfile"

docker build -q -t "$image" -f "$dockerfile" "$root/tools/agent-notify/docker" >/dev/null

case $mode in
    check) command='cargo check --workspace --all-targets --locked' ;;
    clippy) command='cargo clippy --workspace --all-targets --locked -- -D warnings' ;;
    test) command='xvfb-run -a cargo test --workspace --locked' ;;
    *) printf 'linux-check: unknown mode %s\n' "$mode" >&2; exit 2 ;;
esac

docker run --rm \
    -v "$root/crates:/src/crates:ro" \
    -v "$root/tools/agent-notify:/src/tools/agent-notify:ro" \
    -v agent-notify-linux-target:/target \
    -e CARGO_TARGET_DIR=/target \
    "$image" sh -c "
        set -e
        mkdir -p /work/tools
        cp -R /src/crates /work/crates
        cp -R /src/tools/agent-notify /work/tools/agent-notify
        rm -rf /work/tools/agent-notify/.cargo
        cd /work/tools/agent-notify/ui && bun install --frozen-lockfile >/dev/null && bun run build >/dev/null
        cd /work/tools/agent-notify && $command $*
    "
```

`.cargo/config.toml` points the target directory at `../../.cache`, which does not exist in the container; removing the copy's `.cargo` lets `CARGO_TARGET_DIR` win. `xvfb-run` gives the X11 tests a display and leaves the other tests alone.

Run: `chmod +x ~/tools/agent-notify/scripts/linux-check.sh && ~/tools/agent-notify/scripts/linux-check.sh check`
Expected: success. The first run takes several minutes (it downloads and compiles Tauri for Linux). Spike S4-5 ran the same image against the full Linux dependency set; if this run fails on a missing system library, add its `-dev` package to the Dockerfile.

- [ ] **Step 3: Add the Linux targets to `deny.toml`**

In `tools/agent-notify/deny.toml`, extend `[graph] targets`, add one license, and add the exceptions and ignores. Each needs its reason beside it:

```toml
[graph]
targets = ["aarch64-apple-darwin", "x86_64-apple-darwin", "aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"]

[advisories]
version = 2
yanked = "deny"
unmaintained = "all"
ignore = [
    # gtk-layer-shell and its -sys crate are archived with the rest of the
    # GTK3 bindings. Tauri 2 on Linux is GTK3 itself, and there is no GTK3
    # layer-shell binding that is maintained. Revisit when Tauri moves to GTK4.
    { id = "RUSTSEC-2024-0422", reason = "GTK3 layer-shell binding; Tauri 2 is GTK3" },
    { id = "RUSTSEC-2024-0423", reason = "GTK3 layer-shell binding; Tauri 2 is GTK3" },
    # proc-macro-error reaches the build only through the GTK3 macro crates.
    { id = "RUSTSEC-2024-0370", reason = "build-time only, through gtk3-macros and glib-macros" },
]
```

In `[licenses]`, add `"Apache-2.0 WITH LLVM-exception"` to `allow` (target-lexicon, a build dependency of the GTK system crates), and add:

```toml
# The COSMIC toplevel protocols. GPL-3.0-only; linked only into a personal
# build, behind the `cosmic` feature. See the plan 3 deviation note.
[[licenses.exceptions]]
crate = "cosmic-protocols"
allow = ["GPL-3.0-only"]
```

The exception has no effect until Task 6 adds the crate. `cargo deny` warns about an unused exception or ignore; that warning is expected until Task 6 and Task 8, and those tasks run `cargo deny check` with no warnings allowed.

Spike S4-1 found exactly these five items when it ran `cargo deny check` on a lockfile with the full Linux dependency set: two licences and three advisories. If the check finds a sixth, stop and report it; do not add an ignore without a reason the user can read.

- [ ] **Step 4: Run both gates**

Run: `cd ~/tools/agent-notify && cargo deny check 2>&1 | tail -3 && scripts/linux-check.sh clippy`
Expected: `advisories ok, bans ok, licenses ok, sources ok` (warnings about unused entries are allowed now), and clippy passes inside the container.

- [ ] **Step 5: Commit**

```bash
cd ~ && config add tools/agent-notify/docker tools/agent-notify/scripts/linux-check.sh tools/agent-notify/deny.toml
config commit -m "Add a Docker Linux gate and Linux deny targets for agent-notify" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- tools/agent-notify/docker tools/agent-notify/scripts/linux-check.sh tools/agent-notify/deny.toml
```

---

### Task 2: `neutralize_markup` moves into the core

The D-Bus presenter (Task 7) needs the same text rule the CLI fallback uses. The rule is pure, so it moves to the core.

**Files:**
- Modify: `crates/agent-notify-core/src/text.rs`, `crates/agent-notify/src/fallback.rs`

**Interfaces:**
- Produces: `agent_notify_core::text::neutralize_markup(&str) -> String`. The CLI keeps calling it under its old path through a `pub use`.

- [ ] **Step 1: Move the test first**

Cut the `neutralize_markup` test from `crates/agent-notify/src/fallback.rs` (plan 1 Task 17) and paste it into the tests module of `crates/agent-notify-core/src/text.rs`, unchanged:

```rust
#[test]
fn markup_characters_become_look_alikes() {
    assert_eq!(neutralize_markup(r#"<b>"a" & 'b'</b> a < b"#), "\u{ff1c}b\u{ff1e}\"a\" \u{ff06} 'b'\u{ff1c}/b\u{ff1e} a \u{ff1c} b");
    assert_eq!(neutralize_markup("plain text"), "plain text");
}
```

Keep the test's original name if plan 1 gave it another one.

Run: `cd ~/crates && cargo test -p agent-notify-core text`
Expected: compile error, `cannot find function neutralize_markup`.

- [ ] **Step 2: Move the function**

Cut the function and its doc comment from `crates/agent-notify/src/fallback.rs` and paste it into `crates/agent-notify-core/src/text.rs`. In `fallback.rs`, add `pub use agent_notify_core::text::neutralize_markup;` where the function was, so `fallback::neutralize_markup` still resolves for its callers.

Run: `cd ~/crates && cargo test -p agent-notify-core -p agent-notify && cargo clippy -p agent-notify-core -p agent-notify --all-targets -- -D warnings`
Expected: pass.

- [ ] **Step 3: Commit**

```bash
cd ~ && config add crates/agent-notify-core/src/text.rs crates/agent-notify/src/fallback.rs
config commit -m "Move neutralize_markup into agent-notify-core" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- crates/agent-notify-core/src/text.rs crates/agent-notify/src/fallback.rs
```

---

### Task 3: The desktop kind and the Linux tools

**Files:**
- Create: `tools/agent-notify/daemon/src/desktop.rs`
- Modify: `tools/agent-notify/daemon/src/environment.rs`, `src/tools.rs`, `src/lib.rs`

**Interfaces:**
- Consumes: plan 2 `DaemonEnvironment`, `ToolPaths`, `resolve`, `is_executable`.
- Produces:
  - `desktop::{Desktop::{CosmicWayland, OtherWayland, X11, Headless}, desktop_from(session_type: Option<&str>, current_desktop: Option<&str>, wayland_display: bool, display: bool) -> Desktop, Desktop::label(self) -> &'static str}`.
  - `DaemonEnvironment` gains `session_type`, `current_desktop` (both `Option<String>`), `wayland_display`, `display` (both `bool`), and `desktop() -> Desktop`.
  - `ToolPaths` gains `alacritty`, `xdotool`, `xprop`, `pw_play`, `paplay` (all `Option<PathBuf>`). `statuses()` lists only the tools that matter on the current platform.

- [ ] **Step 1: Write the failing tests**

`daemon/src/desktop.rs`, tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosmic_on_wayland_is_cosmic() {
        assert_eq!(desktop_from(Some("wayland"), Some("COSMIC"), true, false), Desktop::CosmicWayland);
        assert_eq!(desktop_from(Some("wayland"), Some("pop:COSMIC"), true, true), Desktop::CosmicWayland);
    }

    #[test]
    fn other_wayland_desktops_are_other() {
        assert_eq!(desktop_from(Some("wayland"), Some("GNOME"), true, true), Desktop::OtherWayland);
        assert_eq!(desktop_from(Some("wayland"), None, true, false), Desktop::OtherWayland);
    }

    #[test]
    fn the_session_type_wins_over_a_reachable_x_server() {
        assert_eq!(desktop_from(Some("x11"), Some("COSMIC"), false, true), Desktop::X11);
        assert_eq!(desktop_from(Some("wayland"), Some("GNOME"), true, true), Desktop::OtherWayland);
    }

    #[test]
    fn a_missing_session_type_is_headless() {
        assert_eq!(desktop_from(None, Some("COSMIC"), true, true), Desktop::Headless);
        assert_eq!(desktop_from(Some("tty"), None, false, false), Desktop::Headless);
    }

    #[test]
    fn a_session_type_without_its_display_is_headless() {
        assert_eq!(desktop_from(Some("wayland"), Some("COSMIC"), false, true), Desktop::Headless);
        assert_eq!(desktop_from(Some("x11"), None, true, false), Desktop::Headless);
    }
}
```

Add to the tests in `daemon/src/tools.rs`:

```rust
#[test]
fn linux_statuses_list_the_linux_tools() {
    let tools = ToolPaths { tmux: Some(PathBuf::from("/usr/bin/tmux")), pw_play: Some(PathBuf::from("/usr/bin/pw-play")), ..ToolPaths::default() };
    let names: Vec<String> = tools.statuses_for(Platform::Linux).into_iter().map(|status| status.name).collect();
    assert_eq!(names, ["tmux", "alacritty", "xdotool", "xprop", "pw-play", "paplay"]);
    let mac: Vec<String> = tools.statuses_for(Platform::MacOs).into_iter().map(|status| status.name).collect();
    assert_eq!(mac, ["tmux", "aerospace"]);
}
```

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon desktop tools`
Expected: compile errors.

- [ ] **Step 2: Implement `desktop.rs`**

Above the tests:

```rust
//! Which Linux desktop adapters to use. The session type decides, not
//! whether an X server happens to answer (spec 7.2).

/// The adapter set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desktop {
    /// COSMIC: layer-shell card, COSMIC toplevel raise and frontmost, Wayland idle.
    CosmicWayland,
    /// Another Wayland compositor: D-Bus notifications, no raise, no frontmost.
    OtherWayland,
    /// X11: a notification-type window, xdotool raise, xprop frontmost.
    X11,
    /// No graphical session known. No card, no raise; tmux focus still works.
    Headless,
}

impl Desktop {
    /// The name `status` and `doctor` print.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Desktop::CosmicWayland => "cosmic-wayland",
            Desktop::OtherWayland => "wayland",
            Desktop::X11 => "x11",
            Desktop::Headless => "headless",
        }
    }
}

/// Picks the adapter set from `$XDG_SESSION_TYPE`, `$XDG_CURRENT_DESKTOP`,
/// and whether `$WAYLAND_DISPLAY` and `$DISPLAY` are set.
#[must_use]
pub fn desktop_from(session_type: Option<&str>, current_desktop: Option<&str>, wayland_display: bool, display: bool) -> Desktop {
    match session_type {
        Some("wayland") if wayland_display => {
            let cosmic = current_desktop.is_some_and(|desktops| desktops.split(':').any(|name| name.eq_ignore_ascii_case("COSMIC")));
            if cosmic { Desktop::CosmicWayland } else { Desktop::OtherWayland }
        }
        Some("x11") if display => Desktop::X11,
        _ => Desktop::Headless,
    }
}
```

- [ ] **Step 3: Extend the environment and the tools**

In `environment.rs`, add the fields and read them in `read()`:

```rust
    /// `$XDG_SESSION_TYPE`.
    pub session_type: Option<String>,
    /// `$XDG_CURRENT_DESKTOP`.
    pub current_desktop: Option<String>,
    /// `$WAYLAND_DISPLAY` is set.
    pub wayland_display: bool,
    /// `$DISPLAY` is set.
    pub display: bool,
```

```rust
            session_type: text("XDG_SESSION_TYPE"),
            current_desktop: text("XDG_CURRENT_DESKTOP"),
            wayland_display: text("WAYLAND_DISPLAY").is_some(),
            display: text("DISPLAY").is_some(),
```

with `let text = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());` beside the existing `path` closure. Add:

```rust
    /// The Linux adapter set for this session.
    #[must_use]
    pub fn desktop(&self) -> crate::desktop::Desktop {
        crate::desktop::desktop_from(self.session_type.as_deref(), self.current_desktop.as_deref(), self.wayland_display, self.display)
    }
```

In `tools.rs`, add the five fields to `ToolPaths`, each with a doc line ("Alacritty, for a new window when no tmux client is attached.", "xdotool, the X11 raise.", "xprop, the X11 frontmost reader.", "pw-play, the Linux sound.", "paplay, the Linux sound when pw-play is missing."). Resolve them in `discover` with `resolve(name, None, path, is_executable)`; tmux keeps its override. Replace `statuses()` with:

```rust
    /// One row per tool that matters on this platform, for `status` and `doctor`.
    #[must_use]
    pub fn statuses(&self) -> Vec<ToolStatus> {
        self.statuses_for(Platform::current())
    }

    /// The rows for `platform`.
    #[must_use]
    pub fn statuses_for(&self, platform: Platform) -> Vec<ToolStatus> {
        let row = |name: &str, path: &Option<PathBuf>| ToolStatus { name: name.to_string(), path: path.clone() };
        match platform {
            Platform::MacOs => vec![row("tmux", &self.tmux), row("aerospace", &self.aerospace)],
            Platform::Linux | Platform::Other => vec![
                row("tmux", &self.tmux),
                row("alacritty", &self.alacritty),
                row("xdotool", &self.xdotool),
                row("xprop", &self.xprop),
                row("pw-play", &self.pw_play),
                row("paplay", &self.paplay),
            ],
        }
    }
```

Import `agent_notify_core::paths::Platform`. Add `pub mod desktop;` to `lib.rs`.

Plan 2's macOS tests on `statuses()` still pass, because they run on macOS.

- [ ] **Step 4: Run the tests on both platforms**

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon && scripts/linux-check.sh test -p agent-notify-daemon desktop tools`
Expected: pass on both.

- [ ] **Step 5: Commit**

```bash
cd ~ && config add tools/agent-notify/daemon/src/desktop.rs tools/agent-notify/daemon/src/environment.rs tools/agent-notify/daemon/src/tools.rs tools/agent-notify/daemon/src/lib.rs
config commit -m "Choose the Linux desktop adapters from the session type" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- tools/agent-notify/daemon/src/desktop.rs tools/agent-notify/daemon/src/environment.rs tools/agent-notify/daemon/src/tools.rs tools/agent-notify/daemon/src/lib.rs
```

---

### Task 4: The X11 adapters

**Files:**
- Create: `tools/agent-notify/daemon/src/x11.rs`, `tools/agent-notify/daemon/tests/x11_adapter.rs`
- Modify: `tools/agent-notify/daemon/src/lib.rs`

**Interfaces:**
- Consumes: plan 2 `FrontmostReader`, `WindowRaiser`, `AttachRequest`; Task 5's `TerminalOpener` is not needed here (the X11 raiser takes it in Task 5).
- Produces:
  - Pure: `parse_active_window(&str) -> Option<String>` (the hex id), `parse_wm_class(&str) -> Option<String>` (the class, the second string), `TERMINAL_CLASS = "Alacritty"`, `frontmost_from_class(Option<&str>) -> Frontmost`.
  - Shell (Linux only): `XpropFrontmost::new(xprop: PathBuf)`, implementing `FrontmostReader`; `xdotool_raise(xdotool: &Path) -> bool`.

- [ ] **Step 1: Write the failing parser tests**

`daemon/src/x11.rs`, tests first. The strings are the exact output spike S4-3 recorded:

```rust
#[cfg(test)]
mod tests {
    use agent_notify_core::update::Frontmost;

    use super::*;

    #[test]
    fn the_active_window_id_is_read() {
        assert_eq!(parse_active_window("_NET_ACTIVE_WINDOW(WINDOW): window id # 0x400004\n").as_deref(), Some("0x400004"));
    }

    #[test]
    fn no_active_window_is_none() {
        assert_eq!(parse_active_window("_NET_ACTIVE_WINDOW:  not found.\n"), None);
        assert_eq!(parse_active_window("_NET_ACTIVE_WINDOW(WINDOW): window id # 0x0\n"), None);
        assert_eq!(parse_active_window(""), None);
        assert_eq!(parse_active_window("_NET_ACTIVE_WINDOW(WINDOW): window id # 0x4; rm -rf\n"), None);
    }

    #[test]
    fn the_class_is_the_second_string() {
        assert_eq!(parse_wm_class("WM_CLASS(STRING) = \"Alacritty\", \"Alacritty\"\n").as_deref(), Some("Alacritty"));
        assert_eq!(parse_wm_class("WM_CLASS(STRING) = \"xterm\", \"XTerm\"\n").as_deref(), Some("XTerm"));
    }

    #[test]
    fn a_missing_class_is_none() {
        assert_eq!(parse_wm_class("WM_CLASS:  not found.\n"), None);
        assert_eq!(parse_wm_class("WM_CLASS(STRING) = \"only-one\"\n"), None);
    }

    #[test]
    fn the_terminal_class_reads_as_terminal() {
        assert_eq!(frontmost_from_class(Some("Alacritty")), Frontmost::Terminal);
        assert_eq!(frontmost_from_class(Some("XTerm")), Frontmost::Other);
        assert_eq!(frontmost_from_class(None), Frontmost::Unknown);
    }
}
```

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon x11`
Expected: compile errors.

- [ ] **Step 2: Implement the parsers and the shell**

Above the tests:

```rust
//! X11: the frontmost window through `xprop`, the raise through `xdotool`.
//! Pop!_OS's xdotool (3.20160805) has no `getwindowclassname`, so the class
//! comes from `xprop` (spike S4-3).

use agent_notify_core::update::Frontmost;

/// Alacritty's `WM_CLASS` class.
pub const TERMINAL_CLASS: &str = "Alacritty";

/// The hex id in `xprop -root _NET_ACTIVE_WINDOW` output. `None` when
/// no window is active, or the id is not plain hex.
#[must_use]
pub fn parse_active_window(output: &str) -> Option<String> {
    let id = output.lines().next()?.split_once("window id # ")?.1.trim();
    let digits = id.strip_prefix("0x")?;
    let valid = !digits.is_empty() && digits.chars().all(|character| character.is_ascii_hexdigit());
    (valid && digits.trim_start_matches('0') != "").then(|| id.to_string())
}

/// The class (second string) in `xprop -id <id> WM_CLASS` output.
#[must_use]
pub fn parse_wm_class(output: &str) -> Option<String> {
    let (_, values) = output.lines().next()?.split_once(" = ")?;
    let mut strings = values.split(", ").map(|value| value.trim().trim_matches('"'));
    let _instance = strings.next()?;
    strings.next().map(str::to_string)
}

/// Terminal when the class is Alacritty's.
#[must_use]
pub fn frontmost_from_class(class: Option<&str>) -> Frontmost {
    match class {
        Some(TERMINAL_CLASS) => Frontmost::Terminal,
        Some(_) => Frontmost::Other,
        None => Frontmost::Unknown,
    }
}

#[cfg(target_os = "linux")]
pub use shell::{XpropFrontmost, xdotool_raise};

#[cfg(target_os = "linux")]
mod shell {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};

    use agent_notify_core::update::Frontmost;

    use super::{TERMINAL_CLASS, frontmost_from_class, parse_active_window, parse_wm_class};
    use crate::services::FrontmostReader;

    fn stdout(program: &Path, arguments: &[&str]) -> Option<String> {
        let output = Command::new(program).args(arguments).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
        output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// The frontmost window's class through `xprop`.
    pub struct XpropFrontmost {
        xprop: PathBuf,
    }

    impl XpropFrontmost {
        /// A reader that runs `xprop` at `xprop`.
        #[must_use]
        pub fn new(xprop: PathBuf) -> Self {
            XpropFrontmost { xprop }
        }
    }

    impl FrontmostReader for XpropFrontmost {
        fn frontmost(&self) -> Frontmost {
            let Some(window) = stdout(&self.xprop, &["-root", "_NET_ACTIVE_WINDOW"]).as_deref().and_then(parse_active_window) else {
                return Frontmost::Unknown;
            };
            frontmost_from_class(stdout(&self.xprop, &["-id", &window, "WM_CLASS"]).as_deref().and_then(parse_wm_class).as_deref())
        }
    }

    /// Activates the first Alacritty window (spike S3-5). False when there is none.
    #[must_use]
    pub fn xdotool_raise(xdotool: &Path) -> bool {
        Command::new(xdotool)
            .args(["search", "--class", TERMINAL_CLASS, "windowactivate", "--sync"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
}
```

`xdotool search ... windowactivate` exits non-zero when the search finds nothing, so a missing Alacritty reads as a failed raise. `--sync` waits until the window is active; xdotool's own timeout bounds the wait.

Add `pub mod x11;` to `lib.rs`.

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon x11`
Expected: 5 pass.

- [ ] **Step 3: Write the X11 integration test**

`daemon/tests/x11_adapter.rs` runs only on Linux, inside `xvfb-run` (Task 1's `test` mode). It needs a window manager for `_NET_ACTIVE_WINDOW`, so it starts Openbox:

```rust
//! The X11 adapter against a real X server (Xvfb, from xvfb-run) and Openbox.
#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use agent_notify_core::update::Frontmost;
use agent_notify_daemon::services::FrontmostReader;
use agent_notify_daemon::x11::{XpropFrontmost, xdotool_raise};

struct Killed(Child);

impl Drop for Killed {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn(program: &str, arguments: &[&str]) -> Killed {
    Killed(Command::new(program).args(arguments).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap_or_else(|error| panic!("{program} did not start: {error}")))
}

fn until(limit: Duration, check: impl Fn() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if check() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

#[test]
fn frontmost_follows_the_active_window_class() {
    assert!(std::env::var_os("DISPLAY").is_some(), "run under xvfb-run (scripts/linux-check.sh test)");
    let _manager = spawn("openbox", &[]);
    std::thread::sleep(Duration::from_millis(500));
    let reader = XpropFrontmost::new(PathBuf::from("/usr/bin/xprop"));
    let _xterm = spawn("xterm", &["-class", "XTerm"]);
    assert!(until(Duration::from_secs(5), || reader.frontmost() == Frontmost::Other), "xterm never became active");
    let _terminal = spawn("xterm", &["-class", "Alacritty", "-title", "fake alacritty"]);
    assert!(until(Duration::from_secs(5), || reader.frontmost() == Frontmost::Terminal), "the Alacritty-class window never became active");
}

#[test]
fn the_raise_activates_the_terminal_class_and_fails_without_it() {
    assert!(std::env::var_os("DISPLAY").is_some(), "run under xvfb-run (scripts/linux-check.sh test)");
    let _manager = spawn("openbox", &[]);
    std::thread::sleep(Duration::from_millis(500));
    let xdotool = PathBuf::from("/usr/bin/xdotool");
    assert!(!xdotool_raise(&xdotool), "no Alacritty-class window exists yet");
    let _terminal = spawn("xterm", &["-class", "Alacritty"]);
    let _other = spawn("xterm", &["-class", "XTerm"]);
    let reader = XpropFrontmost::new(PathBuf::from("/usr/bin/xprop"));
    assert!(until(Duration::from_secs(5), || reader.frontmost() == Frontmost::Other), "the second xterm never became active");
    assert!(xdotool_raise(&xdotool));
    assert!(until(Duration::from_secs(5), || reader.frontmost() == Frontmost::Terminal), "the raise did not activate the terminal class");
}
```

The fake terminal is an xterm with class `Alacritty`: Alacritty itself needs OpenGL, which Xvfb in Docker does not offer. The two tests start their own Openbox; `xvfb-run -a` gives each `cargo test` run a fresh server, and the tests run in one process, so run this file with `--test-threads=1` (see the command below) to keep the two Openbox instances apart.

Run: `cd ~/tools/agent-notify && scripts/linux-check.sh test -p agent-notify-daemon --test x11_adapter -- --test-threads=1`
Expected: 2 pass.

- [ ] **Step 4: Prove the class check is load-bearing**

Back up `x11.rs`, then change `frontmost_from_class` so `Some(_)` also returns `Frontmost::Terminal`. Run the unit tests and the integration test. Expected: `the_terminal_class_reads_as_terminal` and `frontmost_follows_the_active_window_class` fail. Restore and `diff` against the backup.

- [ ] **Step 5: Commit**

```bash
cd ~ && config add tools/agent-notify/daemon/src/x11.rs tools/agent-notify/daemon/tests/x11_adapter.rs tools/agent-notify/daemon/src/lib.rs
config commit -m "Add the X11 frontmost reader and raise" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- tools/agent-notify/daemon/src/x11.rs tools/agent-notify/daemon/tests/x11_adapter.rs tools/agent-notify/daemon/src/lib.rs
```

---

### Task 5: The Linux terminal opener, raiser and sound

**Files:**
- Create: `tools/agent-notify/daemon/src/linux_tools.rs`
- Modify: `tools/agent-notify/daemon/src/lib.rs`

**Interfaces:**
- Consumes: plan 2 `WindowRaiser`, `AttachRequest`, `SoundPlayer`; Task 3 `ToolPaths`; Task 4 `xdotool_raise`.
- Produces:
  - Pure: `attach_arguments(&AttachRequest) -> Vec<OsString>`, `SOUND_FILE = "/usr/share/sounds/freedesktop/stereo/message-new-instant.oga"`, `sound_player(pw_play: Option<&Path>, paplay: Option<&Path>) -> Option<PathBuf>`.
  - Linux only: `TerminalOpener::new(alacritty: Option<PathBuf>)` with `open(&AttachRequest) -> bool`; `RaiseMethod::{Xdotool(PathBuf), Cosmic(CosmicDesktop) (feature cosmic), None}`; `LinuxRaiser::new(RaiseMethod, TerminalOpener)` implementing `WindowRaiser`; `LinuxSound::new(Option<PathBuf>)` implementing `SoundPlayer`.

Task 6 adds the `Cosmic` variant; write the enum here without it.

- [ ] **Step 1: Write the failing tests**

`daemon/src/linux_tools.rs`, tests first:

```rust
#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::services::AttachRequest;

    #[test]
    fn the_terminal_attaches_by_exact_session_name() {
        let request = AttachRequest { tmux: PathBuf::from("/usr/bin/tmux"), socket: PathBuf::from("/tmp/tmux-1000/default"), session: "code".to_string() };
        let expected: Vec<OsString> = ["-e", "/usr/bin/tmux", "-S", "/tmp/tmux-1000/default", "attach-session", "-t", "=code"].into_iter().map(OsString::from).collect();
        assert_eq!(attach_arguments(&request), expected);
    }

    #[test]
    fn a_session_name_with_spaces_stays_one_argument() {
        let request = AttachRequest { tmux: PathBuf::from("/usr/bin/tmux"), socket: PathBuf::from("/tmp/s"), session: "my notes; rm -rf ~".to_string() };
        assert_eq!(attach_arguments(&request).last(), Some(&OsString::from("=my notes; rm -rf ~")));
    }

    #[test]
    fn pw_play_is_preferred_and_paplay_is_the_fallback() {
        let pw = Path::new("/usr/bin/pw-play");
        let pa = Path::new("/usr/bin/paplay");
        assert_eq!(sound_player(Some(pw), Some(pa)), Some(pw.to_path_buf()));
        assert_eq!(sound_player(None, Some(pa)), Some(pa.to_path_buf()));
        assert_eq!(sound_player(None, None), None);
    }
}
```

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon linux_tools`
Expected: compile errors.

- [ ] **Step 2: Implement**

Above the tests:

```rust
//! Linux helpers every desktop shares: a new Alacritty window attached to a
//! tmux session, the raise, and the attention sound.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::services::AttachRequest;

/// The freedesktop sound for a new message (spike S4-4).
pub const SOUND_FILE: &str = "/usr/share/sounds/freedesktop/stereo/message-new-instant.oga";

/// Alacritty's arguments for a window attached to `request`'s session.
/// `=` makes tmux match the session name exactly.
#[must_use]
pub fn attach_arguments(request: &AttachRequest) -> Vec<OsString> {
    vec![
        OsString::from("-e"),
        request.tmux.clone().into_os_string(),
        OsString::from("-S"),
        request.socket.clone().into_os_string(),
        OsString::from("attach-session"),
        OsString::from("-t"),
        OsString::from(format!("={}", request.session)),
    ]
}

/// The player to run: pw-play, else paplay.
#[must_use]
pub fn sound_player(pw_play: Option<&Path>, paplay: Option<&Path>) -> Option<PathBuf> {
    pw_play.or(paplay).map(Path::to_path_buf)
}

#[cfg(target_os = "linux")]
pub use shell::{LinuxRaiser, LinuxSound, RaiseMethod, TerminalOpener};

#[cfg(target_os = "linux")]
mod shell {
    use std::os::unix::process::CommandExt;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    use super::{SOUND_FILE, attach_arguments};
    use crate::services::{AttachRequest, SoundPlayer, WindowRaiser};

    fn spawn_detached(command: &mut Command) -> bool {
        match command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).process_group(0).spawn() {
            Ok(mut child) => {
                std::thread::spawn(move || child.wait());
                true
            }
            Err(error) => {
                tracing::warn!(%error, program = ?command.get_program(), "did not start");
                false
            }
        }
    }

    /// Starts Alacritty attached to a tmux session.
    #[derive(Debug, Clone)]
    pub struct TerminalOpener {
        alacritty: Option<PathBuf>,
    }

    impl TerminalOpener {
        /// An opener for Alacritty at `alacritty`, or one that always fails.
        #[must_use]
        pub fn new(alacritty: Option<PathBuf>) -> Self {
            TerminalOpener { alacritty }
        }

        /// False when Alacritty is missing or did not start.
        #[must_use]
        pub fn open(&self, request: &AttachRequest) -> bool {
            let Some(alacritty) = &self.alacritty else { return false };
            spawn_detached(Command::new(alacritty).args(attach_arguments(request)))
        }
    }

    /// How this desktop raises the terminal.
    pub enum RaiseMethod {
        /// X11: `xdotool search --class Alacritty windowactivate`.
        Xdotool(PathBuf),
        /// No way to raise on this desktop.
        None,
    }

    /// The Linux raiser.
    pub struct LinuxRaiser {
        method: RaiseMethod,
        opener: TerminalOpener,
    }

    impl LinuxRaiser {
        /// A raiser that uses `method` and opens new windows with `opener`.
        #[must_use]
        pub fn new(method: RaiseMethod, opener: TerminalOpener) -> Self {
            LinuxRaiser { method, opener }
        }
    }

    impl WindowRaiser for LinuxRaiser {
        fn raise(&self) -> bool {
            match &self.method {
                RaiseMethod::Xdotool(xdotool) => crate::x11::xdotool_raise(xdotool),
                RaiseMethod::None => false,
            }
        }

        fn open_terminal(&self, request: &AttachRequest) -> bool {
            self.opener.open(request)
        }
    }

    /// The attention sound through pw-play or paplay.
    pub struct LinuxSound {
        player: Option<PathBuf>,
    }

    impl LinuxSound {
        /// A player that runs `player`, or stays silent without one.
        #[must_use]
        pub fn new(player: Option<PathBuf>) -> Self {
            LinuxSound { player }
        }
    }

    impl SoundPlayer for LinuxSound {
        fn play(&self) {
            if let Some(player) = &self.player {
                spawn_detached(Command::new(player).arg(SOUND_FILE));
            }
        }
    }
}
```

`process_group(0)` puts the new terminal in its own process group, so a `systemctl --user restart agent-notifyd` does not take the user's new terminal with it. systemd still tracks it in the unit's cgroup; Task 9's unit sets `KillMode=process` for that reason.

Add `pub mod linux_tools;` to `lib.rs`.

- [ ] **Step 3: Run the tests on both platforms**

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon linux_tools && scripts/linux-check.sh clippy`
Expected: 3 pass on macOS; clippy passes on Linux.

- [ ] **Step 4: Commit**

```bash
cd ~ && config add tools/agent-notify/daemon/src/linux_tools.rs tools/agent-notify/daemon/src/lib.rs
config commit -m "Add the Linux terminal opener, raiser and sound" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- tools/agent-notify/daemon/src/linux_tools.rs tools/agent-notify/daemon/src/lib.rs
```

---

### Task 6: The COSMIC Wayland client

One thread owns one Wayland connection (spike D7). It keeps a table of toplevels (app id and activated state) and one idle notification. The table and the idle tracker are pure and generic, so their tests run on macOS. The thread and the protocol code are Linux-only and behind the `cosmic` feature.

**Files:**
- Create: `tools/agent-notify/daemon/src/cosmic/mod.rs`, `cosmic/table.rs`, `cosmic/idle.rs`, `cosmic/client.rs`
- Modify: `tools/agent-notify/daemon/Cargo.toml`, `src/lib.rs`, `src/linux_tools.rs` (the `Cosmic` raise variant)

**Interfaces:**
- Consumes: plan 1 `Frontmost`, `IDLE_LIMIT_SECONDS`, `EpochMs`; plan 2 `Clock`, `FrontmostReader`, `PresenceReader`, `Presence`.
- Produces:
  - `cosmic::table::{ToplevelTable<Handle>, activated_in(&[u8]) -> bool, TERMINAL_APP_ID = "Alacritty"}`. Methods: `new()`, `added(key: u32)`, `app_id(key, String)`, `attached(key, Handle)`, `state(key, activated: bool)`, `closed(key)`, `disconnect()`, `frontmost() -> Frontmost`, `terminal() -> Option<&Handle>`, `terminal_activated() -> bool`.
  - `cosmic::idle::IdleTracker` with `new(registered: EpochMs)`, `idled()`, `resumed()`, `lost()`, `idle_seconds(now: EpochMs) -> Option<u32>`.
  - Linux and feature `cosmic`: `cosmic::client::CosmicDesktop::connect(clock: Clock) -> Option<CosmicDesktop>` (cheap `Clone`), implementing `FrontmostReader` and `PresenceReader`, plus `activate_terminal() -> bool`.
  - `RaiseMethod::Cosmic(CosmicDesktop)` under the same cfg.

- [ ] **Step 1: Add the dependencies**

In `daemon/Cargo.toml`:

```toml
[features]
default = ["cosmic"]
cosmic = ["dep:cosmic-protocols"]

[target.'cfg(target_os = "linux")'.dependencies]
wayland-client = "=0.31.15"
wayland-protocols = { version = "=0.32.13", features = ["client", "staging"] }
cosmic-protocols = { version = "=0.2.0", default-features = false, features = ["client"], optional = true }
```

- [ ] **Step 2: Write the failing pure tests**

`daemon/src/cosmic/table.rs`, tests first:

```rust
#[cfg(test)]
mod tests {
    use agent_notify_core::update::Frontmost;

    use super::*;

    fn table_with(entries: &[(u32, &str, bool)]) -> ToplevelTable<u32> {
        let mut table = ToplevelTable::new();
        for (key, app_id, activated) in entries {
            table.added(*key);
            table.app_id(*key, (*app_id).to_string());
            table.attached(*key, *key + 100);
            table.state(*key, *activated);
        }
        table
    }

    #[test]
    fn the_activated_terminal_is_frontmost() {
        assert_eq!(table_with(&[(1, "firefox", false), (2, "Alacritty", true)]).frontmost(), Frontmost::Terminal);
        assert_eq!(table_with(&[(1, "firefox", true), (2, "Alacritty", false)]).frontmost(), Frontmost::Other);
    }

    #[test]
    fn no_activated_window_is_unknown() {
        assert_eq!(table_with(&[(1, "firefox", false)]).frontmost(), Frontmost::Unknown);
        assert_eq!(ToplevelTable::<u32>::new().frontmost(), Frontmost::Unknown);
    }

    #[test]
    fn activation_without_a_terminal_is_none() {
        assert_eq!(table_with(&[(1, "firefox", true)]).terminal(), None);
        assert_eq!(table_with(&[(1, "firefox", true), (2, "Alacritty", false)]).terminal(), Some(&102));
    }

    #[test]
    fn a_closed_window_leaves_the_table() {
        let mut table = table_with(&[(1, "firefox", false), (2, "Alacritty", true)]);
        table.closed(2);
        assert_eq!(table.terminal(), None);
        assert_eq!(table.frontmost(), Frontmost::Unknown);
    }

    #[test]
    fn a_closed_table_reads_as_unknown() {
        let mut table = table_with(&[(2, "Alacritty", true)]);
        table.disconnect();
        assert_eq!(table.frontmost(), Frontmost::Unknown);
        assert_eq!(table.terminal(), None);
    }

    #[test]
    fn a_window_without_a_cosmic_handle_cannot_be_raised() {
        let mut table = ToplevelTable::<u32>::new();
        table.added(5);
        table.app_id(5, "Alacritty".to_string());
        assert_eq!(table.terminal(), None);
    }

    #[test]
    fn the_activated_state_is_value_two_in_native_byte_order() {
        let encode = |values: &[u32]| values.iter().flat_map(|value| value.to_ne_bytes()).collect::<Vec<u8>>();
        assert!(activated_in(&encode(&[0, 2])));
        assert!(!activated_in(&encode(&[0, 1, 3])));
        assert!(!activated_in(&[]));
        assert!(!activated_in(&[2, 0, 0]));
    }
}
```

`daemon/src/cosmic/idle.rs`, tests first:

```rust
#[cfg(test)]
mod tests {
    use agent_notify_core::EpochMs;
    use agent_notify_core::update::IDLE_LIMIT_SECONDS;

    use super::*;

    const START: EpochMs = EpochMs(1_760_500_000_000);

    #[test]
    fn idle_is_unknown_until_the_first_timeout_passes() {
        let tracker = IdleTracker::new(START);
        assert_eq!(tracker.idle_seconds(START.plus_ms(1_000)), None);
        assert_eq!(tracker.idle_seconds(START.plus_ms(29_999)), None);
        assert_eq!(tracker.idle_seconds(START.plus_ms(30_000)), Some(0));
    }

    #[test]
    fn idled_reads_as_the_limit_and_resumed_as_zero() {
        let mut tracker = IdleTracker::new(START);
        tracker.idled();
        assert_eq!(tracker.idle_seconds(START.plus_ms(1_000)), Some(IDLE_LIMIT_SECONDS));
        tracker.resumed();
        assert_eq!(tracker.idle_seconds(START.plus_ms(2_000)), Some(0));
    }

    #[test]
    fn a_lost_connection_is_unknown_for_good() {
        let mut tracker = IdleTracker::new(START);
        tracker.resumed();
        tracker.lost();
        assert_eq!(tracker.idle_seconds(START.plus_ms(60_000)), None);
        tracker.resumed();
        assert_eq!(tracker.idle_seconds(START.plus_ms(61_000)), None);
    }
}
```

If plan 1 put `IDLE_LIMIT_SECONDS` somewhere other than `agent_notify_core::update`, fix the import (`grep -rn "pub const IDLE_LIMIT_SECONDS" ~/crates/agent-notify-core/src`).

`daemon/src/cosmic/mod.rs`:

```rust
//! COSMIC: toplevels and idle over one Wayland connection (spike D7).

pub mod idle;
pub mod table;

#[cfg(all(target_os = "linux", feature = "cosmic"))]
pub mod client;
```

Add `pub mod cosmic;` to `lib.rs`.

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon cosmic`
Expected: compile errors.

- [ ] **Step 3: Implement the table and the tracker**

Above the tests in `table.rs`:

```rust
//! The toplevel windows COSMIC reports. Generic over the protocol handle,
//! so the rules test without a compositor.

use agent_notify_core::update::Frontmost;

/// Alacritty's Wayland app id.
pub const TERMINAL_APP_ID: &str = "Alacritty";

const ACTIVATED: u32 = 2;

/// True when a `zcosmic_toplevel_handle_v1.state` array holds `activated`.
/// The array is native-endian `u32` values; a trailing partial value is ignored.
#[must_use]
pub fn activated_in(state: &[u8]) -> bool {
    state.chunks_exact(4).any(|chunk| <[u8; 4]>::try_from(chunk).is_ok_and(|bytes| u32::from_ne_bytes(bytes) == ACTIVATED))
}

#[derive(Debug, Clone)]
struct Entry<Handle> {
    key: u32,
    app_id: String,
    activated: bool,
    handle: Option<Handle>,
}

/// Every toplevel, keyed by its protocol object id.
#[derive(Debug, Clone)]
pub struct ToplevelTable<Handle> {
    entries: Vec<Entry<Handle>>,
    connected: bool,
}

impl<Handle> Default for ToplevelTable<Handle> {
    fn default() -> Self {
        ToplevelTable { entries: Vec::new(), connected: true }
    }
}

impl<Handle> ToplevelTable<Handle> {
    /// An empty, connected table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn entry(&mut self, key: u32) -> Option<&mut Entry<Handle>> {
        self.entries.iter_mut().find(|entry| entry.key == key)
    }

    /// A new toplevel appeared.
    pub fn added(&mut self, key: u32) {
        if self.entry(key).is_none() {
            self.entries.push(Entry { key, app_id: String::new(), activated: false, handle: None });
        }
    }

    /// The toplevel's app id.
    pub fn app_id(&mut self, key: u32, app_id: String) {
        if let Some(entry) = self.entry(key) {
            entry.app_id = app_id;
        }
    }

    /// The COSMIC handle that can activate the toplevel.
    pub fn attached(&mut self, key: u32, handle: Handle) {
        if let Some(entry) = self.entry(key) {
            entry.handle = Some(handle);
        }
    }

    /// The toplevel's activated state.
    pub fn state(&mut self, key: u32, activated: bool) {
        if let Some(entry) = self.entry(key) {
            entry.activated = activated;
        }
    }

    /// The toplevel closed.
    pub fn closed(&mut self, key: u32) {
        self.entries.retain(|entry| entry.key != key);
    }

    /// The connection is gone. Everything reads as unknown from now on.
    pub fn disconnect(&mut self) {
        self.connected = false;
        self.entries.clear();
    }

    /// Which app is active.
    #[must_use]
    pub fn frontmost(&self) -> Frontmost {
        match self.entries.iter().find(|entry| entry.activated) {
            Some(entry) if entry.app_id == TERMINAL_APP_ID => Frontmost::Terminal,
            Some(_) => Frontmost::Other,
            None => Frontmost::Unknown,
        }
    }

    /// The first Alacritty window's handle.
    #[must_use]
    pub fn terminal(&self) -> Option<&Handle> {
        self.entries.iter().find(|entry| entry.app_id == TERMINAL_APP_ID).and_then(|entry| entry.handle.as_ref())
    }

    /// True when an Alacritty window is active.
    #[must_use]
    pub fn terminal_activated(&self) -> bool {
        self.frontmost() == Frontmost::Terminal
    }
}
```

`connected` is kept so a later reader can tell "no windows" from "no connection" in logs; `disconnect` clears the entries, so the read methods need no extra check. If clippy flags `connected` as never read, add a `connected()` getter used by `client.rs` in its `tracing` line, not an `allow`.

Above the tests in `idle.rs`:

```rust
//! The user's idle state from one `ext_idle_notifier_v1` notification with
//! a timeout of `IDLE_LIMIT_SECONDS` (spike S4-2). The compositor sends
//! `idled` after that much quiet and `resumed` on the next input.

use agent_notify_core::EpochMs;
use agent_notify_core::update::IDLE_LIMIT_SECONDS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Waiting { registered: EpochMs },
    Active,
    Idle,
    Lost,
}

/// What the idle notification has said so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdleTracker {
    state: State,
}

impl IdleTracker {
    /// A notification registered at `registered`.
    #[must_use]
    pub fn new(registered: EpochMs) -> Self {
        IdleTracker { state: State::Waiting { registered } }
    }

    /// The compositor sent `idled`.
    pub fn idled(&mut self) {
        if self.state != State::Lost {
            self.state = State::Idle;
        }
    }

    /// The compositor sent `resumed`.
    pub fn resumed(&mut self) {
        if self.state != State::Lost {
            self.state = State::Active;
        }
    }

    /// The connection is gone.
    pub fn lost(&mut self) {
        self.state = State::Lost;
    }

    /// Seconds idle, as the core reads it: 0 while active, the limit while
    /// idle. `None` before the first timeout has had time to fire, and after
    /// the connection is lost.
    #[must_use]
    pub fn idle_seconds(&self, now: EpochMs) -> Option<u32> {
        match self.state {
            State::Waiting { registered } => (now.since(registered) >= i64::from(IDLE_LIMIT_SECONDS) * 1_000).then_some(0),
            State::Active => Some(0),
            State::Idle => Some(IDLE_LIMIT_SECONDS),
            State::Lost => None,
        }
    }
}
```

`EpochMs::since` is plan 1's method (`self.0 - earlier.0`, in milliseconds). If it has another signature, adapt the one line.

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon cosmic`
Expected: 10 pass.

- [ ] **Step 4: Implement the Wayland client**

`daemon/src/cosmic/client.rs` follows the protocol code spike S2 ran (`cosmic-raise`) and spike S4-2 ran (idle):

```rust
//! The Wayland side: one connection, one thread, `blocking_dispatch` until
//! the compositor goes away. Readers take the lock briefly; nothing waits
//! on the Wayland thread.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use agent_notify_core::update::{Frontmost, IDLE_LIMIT_SECONDS};
use cosmic_protocols::toplevel_info::v1::client::{zcosmic_toplevel_handle_v1, zcosmic_toplevel_info_v1};
use cosmic_protocols::toplevel_management::v1::client::zcosmic_toplevel_manager_v1;
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, event_created_child};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{ext_foreign_toplevel_handle_v1, ext_foreign_toplevel_list_v1};
use wayland_protocols::ext::idle_notify::v1::client::{ext_idle_notification_v1, ext_idle_notifier_v1};

use super::idle::IdleTracker;
use super::table::ToplevelTable;
use crate::services::{Clock, FrontmostReader, Presence, PresenceReader};

type CosmicHandle = zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1;

struct Shared {
    table: ToplevelTable<CosmicHandle>,
    idle: IdleTracker,
}

/// A live COSMIC connection. Clones share it.
#[derive(Clone)]
pub struct CosmicDesktop {
    shared: Arc<Mutex<Shared>>,
    connection: Connection,
    manager: zcosmic_toplevel_manager_v1::ZcosmicToplevelManagerV1,
    seat: wl_seat::WlSeat,
    clock: Clock,
}

struct Globals {
    list: Option<ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1>,
    info: Option<zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1>,
    manager: Option<zcosmic_toplevel_manager_v1::ZcosmicToplevelManagerV1>,
    notifier: Option<ext_idle_notifier_v1::ExtIdleNotifierV1>,
    seat: Option<wl_seat::WlSeat>,
}

struct Dispatcher {
    globals: Globals,
    shared: Arc<Mutex<Shared>>,
}

impl Dispatcher {
    fn with<Output>(&self, work: impl FnOnce(&mut Shared) -> Output) -> Output {
        work(&mut self.shared.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

impl CosmicDesktop {
    /// Connects and starts the thread. `None` when there is no Wayland
    /// display, or the compositor lacks a protocol this needs.
    #[must_use]
    pub fn connect(clock: Clock) -> Option<CosmicDesktop> {
        let connection = Connection::connect_to_env().inspect_err(|error| tracing::info!(%error, "no Wayland connection")).ok()?;
        let mut queue = connection.new_event_queue();
        let handle = queue.handle();
        connection.display().get_registry(&handle, ());
        let shared = Arc::new(Mutex::new(Shared { table: ToplevelTable::new(), idle: IdleTracker::new(clock()) }));
        let mut dispatcher = Dispatcher {
            globals: Globals { list: None, info: None, manager: None, notifier: None, seat: None },
            shared: Arc::clone(&shared),
        };
        queue.roundtrip(&mut dispatcher).ok()?;
        let (Some(manager), Some(seat), Some(notifier)) = (dispatcher.globals.manager.clone(), dispatcher.globals.seat.clone(), dispatcher.globals.notifier.clone()) else {
            tracing::warn!("COSMIC toplevel management, a seat or the idle notifier is missing");
            return None;
        };
        if dispatcher.globals.list.is_none() || dispatcher.globals.info.is_none() {
            tracing::warn!("COSMIC toplevel info is missing");
            return None;
        }
        let limit_ms = IDLE_LIMIT_SECONDS * 1_000;
        let _notification = notifier.get_idle_notification(limit_ms, &seat, &handle, ());
        dispatcher.with(|shared| shared.idle = IdleTracker::new(clock()));
        let thread_shared = Arc::clone(&shared);
        let spawned = std::thread::Builder::new().name("cosmic-wayland".to_string()).spawn(move || {
            let _keep = _notification;
            loop {
                if let Err(error) = queue.blocking_dispatch(&mut dispatcher) {
                    tracing::warn!(%error, "the COSMIC connection closed");
                    let mut shared = thread_shared.lock().unwrap_or_else(PoisonError::into_inner);
                    shared.table.disconnect();
                    shared.idle.lost();
                    return;
                }
            }
        });
        if let Err(error) = spawned {
            tracing::warn!(%error, "the COSMIC thread did not start");
            return None;
        }
        Some(CosmicDesktop { shared, connection, manager, seat, clock })
    }

    fn with<Output>(&self, work: impl FnOnce(&mut Shared) -> Output) -> Output {
        work(&mut self.shared.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// Activates the first Alacritty window and waits up to 500 ms for COSMIC
    /// to report it active. False when there is none or it did not activate.
    #[must_use]
    pub fn activate_terminal(&self) -> bool {
        let Some(terminal) = self.with(|shared| shared.table.terminal().cloned()) else { return false };
        self.manager.activate(&terminal, &self.seat);
        if let Err(error) = self.connection.flush() {
            tracing::warn!(%error, "the activate request was not sent");
            return false;
        }
        let deadline = Instant::now() + Duration::from_millis(500);
        while Instant::now() < deadline {
            if self.with(|shared| shared.table.terminal_activated()) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        false
    }
}

impl FrontmostReader for CosmicDesktop {
    fn frontmost(&self) -> Frontmost {
        self.with(|shared| shared.table.frontmost())
    }
}

impl PresenceReader for CosmicDesktop {
    fn presence(&self) -> Presence {
        let now = (self.clock)();
        Presence { idle_seconds: self.with(|shared| shared.idle.idle_seconds(now)), screen_locked: None }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for Dispatcher {
    fn event(state: &mut Self, registry: &wl_registry::WlRegistry, event: wl_registry::Event, _: &(), _: &Connection, handle: &QueueHandle<Self>) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            let globals = &mut state.globals;
            match interface.as_str() {
                "ext_foreign_toplevel_list_v1" => globals.list = Some(registry.bind(name, 1, handle, ())),
                "zcosmic_toplevel_info_v1" => globals.info = Some(registry.bind(name, version.min(3), handle, ())),
                "zcosmic_toplevel_manager_v1" => globals.manager = Some(registry.bind(name, version.min(4), handle, ())),
                "ext_idle_notifier_v1" => globals.notifier = Some(registry.bind(name, 1, handle, ())),
                "wl_seat" if globals.seat.is_none() => globals.seat = Some(registry.bind(name, 1, handle, ())),
                _ => {}
            }
        }
    }
}

impl Dispatch<ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1, ()> for Dispatcher {
    fn event(state: &mut Self, _: &ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1, event: ext_foreign_toplevel_list_v1::Event, _: &(), _: &Connection, handle: &QueueHandle<Self>) {
        if let ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } = event {
            let key = toplevel.id().protocol_id();
            state.with(|shared| shared.table.added(key));
            if let Some(info) = &state.globals.info {
                let cosmic = info.get_cosmic_toplevel(&toplevel, handle, key);
                state.with(|shared| shared.table.attached(key, cosmic));
            }
        }
    }

    event_created_child!(Dispatcher, ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1, ())
    ]);
}

impl Dispatch<ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1, ()> for Dispatcher {
    fn event(state: &mut Self, toplevel: &ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1, event: ext_foreign_toplevel_handle_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        let key = toplevel.id().protocol_id();
        match event {
            ext_foreign_toplevel_handle_v1::Event::AppId { app_id } => state.with(|shared| shared.table.app_id(key, app_id)),
            ext_foreign_toplevel_handle_v1::Event::Closed => state.with(|shared| shared.table.closed(key)),
            _ => {}
        }
    }
}

impl Dispatch<CosmicHandle, u32> for Dispatcher {
    fn event(state: &mut Self, _: &CosmicHandle, event: zcosmic_toplevel_handle_v1::Event, key: &u32, _: &Connection, _: &QueueHandle<Self>) {
        if let zcosmic_toplevel_handle_v1::Event::State { state: bytes } = event {
            let activated = super::table::activated_in(&bytes);
            state.with(|shared| shared.table.state(*key, activated));
        }
    }
}

impl Dispatch<ext_idle_notification_v1::ExtIdleNotificationV1, ()> for Dispatcher {
    fn event(state: &mut Self, _: &ext_idle_notification_v1::ExtIdleNotificationV1, event: ext_idle_notification_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match event {
            ext_idle_notification_v1::Event::Idled => state.with(|shared| shared.idle.idled()),
            ext_idle_notification_v1::Event::Resumed => state.with(|shared| shared.idle.resumed()),
            _ => {}
        }
    }
}

wayland_client::delegate_noop!(Dispatcher: ignore zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1);
wayland_client::delegate_noop!(Dispatcher: ignore zcosmic_toplevel_manager_v1::ZcosmicToplevelManagerV1);
wayland_client::delegate_noop!(Dispatcher: ignore ext_idle_notifier_v1::ExtIdleNotifierV1);
wayland_client::delegate_noop!(Dispatcher: ignore wl_seat::WlSeat);
```

Notes:

- Spike S2-5 found the initial `state` event about 100 ms after `get_cosmic_toplevel`. The thread handles it like any later event; nothing waits for it.
- The `_notification` proxy must outlive the loop: dropping the last proxy does not destroy the object, but keeping it in the thread makes the lifetime visible. Clippy may flag `_keep`; rename it to `notification` and pass it to a `tracing::debug!` at start-up if so.
- `blocking_dispatch` returns an error when the compositor closes the socket. The table then reads `Unknown` and idle `None` for the rest of the process (Review Focus 2). systemd restarts the daemon with the session.
- Proxies in wayland-client 0.31 are `Clone + Send + Sync`, so `activate` from the effects worker thread is allowed; `flush` sends it.

In `linux_tools.rs`, add the variant and its arm:

```rust
        /// COSMIC: activate the Alacritty toplevel.
        #[cfg(feature = "cosmic")]
        Cosmic(crate::cosmic::client::CosmicDesktop),
```

```rust
                #[cfg(feature = "cosmic")]
                RaiseMethod::Cosmic(desktop) => desktop.activate_terminal(),
```

- [ ] **Step 5: Run the gates**

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon && scripts/linux-check.sh clippy && scripts/linux-check.sh check --no-default-features && cargo deny check 2>&1 | tail -3`
Expected: all pass. `--no-default-features` proves the crate builds without the GPL dependency. `cargo deny` no longer warns about the `cosmic-protocols` exception.

The Wayland client has no automated test: it needs a COSMIC session. Task 10 checks it in the VM.

- [ ] **Step 6: Prove the idle guard is load-bearing**

Back up `idle.rs`, then make `State::Waiting` return `Some(0)` at once. Run `cargo test -p agent-notify-daemon cosmic`. Expected: `idle_is_unknown_until_the_first_timeout_passes` fails. Restore and `diff`.

- [ ] **Step 7: Commit**

```bash
cd ~ && config add tools/agent-notify/daemon/Cargo.toml tools/agent-notify/Cargo.lock tools/agent-notify/daemon/src/cosmic tools/agent-notify/daemon/src/lib.rs tools/agent-notify/daemon/src/linux_tools.rs
config commit -m "Add the COSMIC toplevel and idle client" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- tools/agent-notify/daemon/Cargo.toml tools/agent-notify/Cargo.lock tools/agent-notify/daemon/src/cosmic tools/agent-notify/daemon/src/lib.rs tools/agent-notify/daemon/src/linux_tools.rs
```

---

### Task 7: The D-Bus presenter

For a Wayland desktop without layer shell. Each card becomes one notification with a `default` action; a click sends `Clicked` for that card; a card that leaves the view closes its notification.

**Files:**
- Create: `tools/agent-notify/daemon/src/dbus_presenter.rs`
- Modify: `tools/agent-notify/daemon/Cargo.toml`, `src/lib.rs`

**Interfaces:**
- Consumes: plan 1 `PanelViewModel`, `CardView`, `CardId`, `Kind`, `UiIntent`; Task 2 `neutralize_markup`; plan 2 `ToastPresenter`, `DaemonHandle`.
- Produces:
  - Pure: `NotificationOp::{Show(NotificationText), Close(CardId)}`, `NotificationText { card_id, summary, body, urgency: u8 }`, `notification_ops(shown: &[CardId], view: &PanelViewModel) -> Vec<NotificationOp>`, `notification_text(&CardView) -> NotificationText`.
  - Linux only: `DbusPresenter::connect(on_click: impl Fn(CardId) + Send + 'static) -> Option<DbusPresenter>`, implementing `ToastPresenter`.

The presenter cannot hold a `DaemonHandle` at construction, because the daemon starts after the presenter. `on_click` is a closure the composition root fills in through a `OnceLock` (Task 8).

- [ ] **Step 1: Add the dependency**

In `daemon/Cargo.toml`, under the Linux target dependencies: `zbus = "=5.19.0"` (default features `async-io` and `blocking-api`; spike S4-6 used exactly this).

- [ ] **Step 2: Write the failing tests**

`daemon/src/dbus_presenter.rs`, tests first:

```rust
#[cfg(test)]
mod tests {
    use agent_notify_core::ids::{CardId, EpochMs, EventId};
    use agent_notify_core::state::Kind;
    use agent_notify_core::view::{CardView, PanelViewModel};

    use super::*;

    fn card(number: u8, kind: Kind) -> CardView {
        CardView {
            card_id: CardId::for_event(&EventId::from_parts(EpochMs(1_760_500_000_000), [number; 10])),
            kind,
            location: format!("{number} · code/feature-x"),
            title: Some("Fix <sync>".to_string()),
            body: Some("Tests & lint pass".to_string()),
            created_ms: EpochMs(1_760_500_000_000),
            note: None,
        }
    }

    fn view(cards: Vec<CardView>) -> PanelViewModel {
        PanelViewModel { revision: 1, cards, finished_row: Vec::new(), dnd: false }
    }

    #[test]
    fn new_cards_are_shown_and_known_cards_are_left_alone() {
        let first = card(1, Kind::Attention);
        let second = card(2, Kind::Info);
        let ops = notification_ops(&[first.card_id.clone()], &view(vec![first.clone(), second.clone()]));
        assert_eq!(ops, vec![NotificationOp::Show(notification_text(&second))]);
    }

    #[test]
    fn a_replaced_card_closes_its_old_notification() {
        let old = card(1, Kind::Info);
        let new = card(2, Kind::Attention);
        let ops = notification_ops(&[old.card_id.clone()], &view(vec![new.clone()]));
        assert_eq!(ops, vec![NotificationOp::Close(old.card_id), NotificationOp::Show(notification_text(&new))]);
    }

    #[test]
    fn an_empty_view_closes_everything() {
        let shown = card(1, Kind::Info);
        assert_eq!(notification_ops(&[shown.card_id.clone()], &view(Vec::new())), vec![NotificationOp::Close(shown.card_id)]);
    }

    #[test]
    fn text_is_neutralized_and_attention_is_critical() {
        let text = notification_text(&card(1, Kind::Attention));
        assert_eq!(text.summary, "1 · code/feature-x");
        assert_eq!(text.body, "Fix \u{ff1c}sync\u{ff1e}\nTests \u{ff06} lint pass");
        assert_eq!(text.urgency, 2);
        assert_eq!(notification_text(&card(2, Kind::Info)).urgency, 1);
    }

    #[test]
    fn a_note_is_the_last_body_line() {
        let mut noted = card(3, Kind::Attention);
        noted.title = None;
        noted.body = None;
        noted.note = Some("Session ended".to_string());
        assert_eq!(notification_text(&noted).body, "Session ended");
    }
}
```

Fix the `use` paths to wherever plan 1 put `CardId`, `EventId`, `EpochMs` and `Kind`, as earlier tasks did. `CardId::for_event` is the constructor Task 10's fixtures in plan 2 use.

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon dbus_presenter`
Expected: compile errors.

- [ ] **Step 3: Implement the pure part and the presenter**

Above the tests:

```rust
//! Cards as freedesktop notifications, for a Wayland desktop without layer
//! shell. Text uses look-alike characters (spike D8). Nothing relies on
//! popups staying up (spike D9) or on `NotificationClosed`, which COSMIC
//! sends twice for one close (spike S4-6).

use agent_notify_core::ids::CardId;
use agent_notify_core::state::Kind;
use agent_notify_core::text::neutralize_markup;
use agent_notify_core::view::{CardView, PanelViewModel};

/// One notification's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationText {
    /// The card it shows.
    pub card_id: CardId,
    /// The location line.
    pub summary: String,
    /// Title, body and note, one per line.
    pub body: String,
    /// 1 for info, 2 (critical) for attention.
    pub urgency: u8,
}

/// A change to the shown notifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationOp {
    /// Close the card's notification.
    Close(CardId),
    /// Show a new notification.
    Show(NotificationText),
}

/// The notification for `card`.
#[must_use]
pub fn notification_text(card: &CardView) -> NotificationText {
    let lines: Vec<&str> = [card.title.as_deref(), card.body.as_deref(), card.note.as_deref()].into_iter().flatten().collect();
    NotificationText {
        card_id: card.card_id.clone(),
        summary: neutralize_markup(&card.location),
        body: neutralize_markup(&lines.join("\n")),
        urgency: if card.kind == Kind::Attention { 2 } else { 1 },
    }
}

/// The closes, then the shows, that take `shown` to `view`.
#[must_use]
pub fn notification_ops(shown: &[CardId], view: &PanelViewModel) -> Vec<NotificationOp> {
    let closes = shown.iter().filter(|card_id| !view.cards.iter().any(|card| &card.card_id == *card_id)).cloned().map(NotificationOp::Close);
    let shows = view.cards.iter().filter(|card| !shown.contains(&card.card_id)).map(|card| NotificationOp::Show(notification_text(card)));
    closes.chain(shows).collect()
}

#[cfg(target_os = "linux")]
pub use shell::DbusPresenter;

#[cfg(target_os = "linux")]
mod shell {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, PoisonError};

    use agent_notify_core::ids::CardId;
    use agent_notify_core::view::PanelViewModel;
    use zbus::blocking::{Connection, Proxy};
    use zbus::zvariant::Value;

    use super::{NotificationOp, notification_ops};
    use crate::services::ToastPresenter;

    const DESTINATION: &str = "org.freedesktop.Notifications";
    const PATH: &str = "/org/freedesktop/Notifications";

    type Shown = Arc<Mutex<Vec<(CardId, u32)>>>;

    /// Shows cards through the session bus.
    pub struct DbusPresenter {
        proxy: Proxy<'static>,
        shown: Shown,
    }

    impl DbusPresenter {
        /// Connects to the session bus and starts the action listener.
        /// `None` when there is no session bus or no notification server.
        pub fn connect(on_click: impl Fn(CardId) + Send + 'static) -> Option<DbusPresenter> {
            let connection = Connection::session().inspect_err(|error| tracing::warn!(%error, "no session bus")).ok()?;
            let proxy = Proxy::new(&connection, DESTINATION, PATH, DESTINATION).inspect_err(|error| tracing::warn!(%error, "no notification proxy")).ok()?;
            let shown: Shown = Arc::new(Mutex::new(Vec::new()));
            let mut actions = proxy.receive_signal("ActionInvoked").inspect_err(|error| tracing::warn!(%error, "no action signal")).ok()?;
            let listener_shown = Arc::clone(&shown);
            std::thread::Builder::new()
                .name("dbus-actions".to_string())
                .spawn(move || {
                    for message in actions.by_ref() {
                        let Ok((id, action)) = message.body().deserialize::<(u32, String)>() else { continue };
                        if action != "default" {
                            continue;
                        }
                        let card = listener_shown.lock().unwrap_or_else(PoisonError::into_inner).iter().find(|(_, shown_id)| *shown_id == id).map(|(card_id, _)| card_id.clone());
                        if let Some(card_id) = card {
                            on_click(card_id);
                        }
                    }
                    tracing::warn!("the notification action stream ended");
                })
                .inspect_err(|error| tracing::warn!(%error, "the action listener did not start"))
                .ok()?;
            Some(DbusPresenter { proxy, shown })
        }

        fn apply(&self, op: NotificationOp) {
            match op {
                NotificationOp::Close(card_id) => {
                    let id = {
                        let mut shown = self.shown.lock().unwrap_or_else(PoisonError::into_inner);
                        let id = shown.iter().find(|(shown_id, _)| *shown_id == card_id).map(|(_, id)| *id);
                        shown.retain(|(shown_id, _)| *shown_id != card_id);
                        id
                    };
                    if let Some(id) = id
                        && let Err(error) = self.proxy.call::<_, _, ()>("CloseNotification", &(id,))
                    {
                        tracing::debug!(%error, id, "close failed");
                    }
                }
                NotificationOp::Show(text) => {
                    let mut hints: HashMap<&str, Value<'_>> = HashMap::new();
                    hints.insert("urgency", Value::U8(text.urgency));
                    let sent: zbus::Result<u32> =
                        self.proxy.call("Notify", &("agent-notify", 0_u32, "", text.summary.as_str(), text.body.as_str(), vec!["default", "Open"], hints, 0_i32));
                    match sent {
                        Ok(id) => self.shown.lock().unwrap_or_else(PoisonError::into_inner).push((text.card_id, id)),
                        Err(error) => tracing::warn!(%error, "Notify failed"),
                    }
                }
            }
        }
    }

    impl ToastPresenter for DbusPresenter {
        fn present(&self, view: &PanelViewModel) {
            let shown: Vec<CardId> = self.shown.lock().unwrap_or_else(PoisonError::into_inner).iter().map(|(card_id, _)| card_id.clone()).collect();
            for op in notification_ops(&shown, view) {
                self.apply(op);
            }
        }
    }
}
```

`present` makes D-Bus calls, and plan 2's `ToastPresenter` says it must return at once. Each call is a local socket round trip (spike S4-6: under a few milliseconds), and the actor calls `present` only when the view changed. If the log shows a slow `present`, move `apply` onto a worker thread with a channel; do not do that before a measurement says so.

Add `pub mod dbus_presenter;` to `lib.rs`.

- [ ] **Step 4: Run the gates**

Run: `cd ~/tools/agent-notify && cargo test -p agent-notify-daemon dbus_presenter && scripts/linux-check.sh clippy && cargo deny check 2>&1 | tail -3`
Expected: 5 pass, clippy and deny pass.

- [ ] **Step 5: Commit**

```bash
cd ~ && config add tools/agent-notify/daemon/Cargo.toml tools/agent-notify/Cargo.lock tools/agent-notify/daemon/src/dbus_presenter.rs tools/agent-notify/daemon/src/lib.rs
config commit -m "Add the D-Bus notification presenter" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- tools/agent-notify/daemon/Cargo.toml tools/agent-notify/Cargo.lock tools/agent-notify/daemon/src/dbus_presenter.rs tools/agent-notify/daemon/src/lib.rs
```

---

### Task 8: The Linux panel and composition root

**Files:**
- Create: `tools/agent-notify/app/src/startup.rs`, `app/src/linux/mod.rs`, `app/src/linux/panel.rs`, `app/src/linux/geometry.rs`
- Modify: `tools/agent-notify/app/Cargo.toml`, `app/src/main.rs`, `app/src/logging.rs`, `app/src/shell.rs`, `app/src/macos/mod.rs`

**Interfaces:**
- Consumes: plan 2 Tasks 12 to 16; Tasks 3 to 7.
- Produces:
  - `startup::{Prepared { environment, inputs-derived socket, listener, state_dir, tools, runtime, facts, clock }, prepare(stamp: &str) -> Result<Prepared, ExitCode>}`. Both `macos::run` and `linux::run` call it.
  - `logging::init_stderr(LevelFilter)` for Linux (journald reads stderr).
  - `linux::geometry::{x11_origin(work_area: (i32, i32, i32, i32), width: i32) -> (i32, i32)}` (pure).
  - `linux::panel::{LABEL = "toast", build(&AppHandle, Desktop) -> tauri::Result<PanelMode>, PanelMode::{LayerShell, X11Window}, GtkPresenter::new(AppHandle)`, `resize(&AppHandle, u32)}`.
  - `linux::run() -> ExitCode`.

- [ ] **Step 1: Extract the shared start-up, test first**

The start-up in plan 2's `macos::run` (environment, socket, bind, state directory, tools, runtime, facts) is the same on Linux. Move it to `startup.rs` without changing behavior. The pure part is the order of checks; the test pins the one decision that differs, the exit code when another daemon runs:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn another_daemon_is_a_clean_exit() {
        assert_eq!(exit_for(BindProblem::AlreadyRunning), ExitCode::SUCCESS);
        assert_eq!(exit_for(BindProblem::NoSocketPath), ExitCode::from(1));
        assert_eq!(exit_for(BindProblem::Bind), ExitCode::from(1));
        assert_eq!(exit_for(BindProblem::NoStateDirectory), ExitCode::from(1));
        assert_eq!(exit_for(BindProblem::Runtime), ExitCode::from(1));
    }
}
```

`ExitCode` has no `PartialEq` in std. If the comparison does not compile, make `exit_for` return `u8` and convert with `ExitCode::from` at the call site; keep the test on the `u8`.

Then write `startup.rs`:

```rust
//! Start-up shared by every platform: environment, socket, state, tools,
//! runtime. Each failure is logged where it happens and maps to an exit code.

use std::path::PathBuf;
use std::process::ExitCode;

use agent_notify_core::paths::{SocketLocation, socket_location, state_directory};
use agent_notify_core::status::DaemonFacts;
use agent_notify_daemon::environment::DaemonEnvironment;
use agent_notify_daemon::services::{Clock, system_clock};
use agent_notify_daemon::socket::{BindOutcome, Listener, bind};
use agent_notify_daemon::tools::ToolPaths;

/// Why start-up stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindProblem {
    /// No socket path on this machine.
    NoSocketPath,
    /// Another daemon holds the socket.
    AlreadyRunning,
    /// The socket did not bind.
    Bind,
    /// No state directory on this machine.
    NoStateDirectory,
    /// The Tokio runtime did not start.
    Runtime,
}

/// The exit code for `problem`. A second daemon exits 0 so launchd and
/// systemd do not restart it in a loop.
#[must_use]
pub fn exit_for(problem: BindProblem) -> ExitCode {
    match problem {
        BindProblem::AlreadyRunning => ExitCode::SUCCESS,
        _ => ExitCode::from(1),
    }
}

/// Everything the platform root needs before it builds services.
pub struct Prepared {
    /// The environment snapshot.
    pub environment: DaemonEnvironment,
    /// The socket path.
    pub socket: PathBuf,
    /// The bound socket.
    pub listener: Listener,
    /// The state directory.
    pub state_dir: PathBuf,
    /// The resolved tools.
    pub tools: ToolPaths,
    /// The daemon's runtime.
    pub runtime: tokio::runtime::Runtime,
    /// Facts for `status`.
    pub facts: DaemonFacts,
    /// The wall clock.
    pub clock: Clock,
}

/// Runs the shared start-up.
///
/// # Errors
///
/// The exit code to return, already logged.
pub fn prepare(stamp: &str) -> Result<Prepared, ExitCode> {
    let environment = DaemonEnvironment::read();
    let inputs = environment.path_inputs();
    let SocketLocation::At(socket) = socket_location(&inputs) else {
        tracing::error!("no socket path on this machine");
        return Err(exit_for(BindProblem::NoSocketPath));
    };
    let listener = match bind(&socket) {
        Ok(BindOutcome::Bound(listener)) => listener,
        Ok(BindOutcome::AlreadyRunning) => {
            tracing::info!("another agent-notifyd runs; exiting");
            return Err(exit_for(BindProblem::AlreadyRunning));
        }
        Err(error) => {
            tracing::error!(%error, socket = %socket.display(), "the socket did not bind");
            return Err(exit_for(BindProblem::Bind));
        }
    };
    let Some(state_dir) = state_directory(&inputs) else {
        tracing::error!("no state directory on this machine");
        return Err(exit_for(BindProblem::NoStateDirectory));
    };
    let tools = ToolPaths::discover(&environment);
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).thread_name("agent-notifyd").enable_all().build().map_err(|error| {
        tracing::error!(%error, "the runtime did not start");
        exit_for(BindProblem::Runtime)
    })?;
    let clock = system_clock();
    let facts = DaemonFacts { stamp: stamp.to_string(), started_ms: clock(), tools: tools.statuses() };
    tracing::info!(stamp, ?tools, socket = %socket.display(), "starting");
    Ok(Prepared { environment, socket, listener, state_dir, tools, runtime, facts, clock })
}
```

Replace the matching lines in `macos::run` with `let Prepared { listener, state_dir: state, tools, runtime, facts, clock, .. } = match startup::prepare(STAMP) { Ok(prepared) => prepared, Err(code) => return code };`. Logging set-up stays before it, in each platform's `run`, because the platforms log to different places.

Run: `cd ~/tools/agent-notify && cargo test -p agent-notifyd && cargo build -p agent-notifyd`
Then repeat plan 2 Task 16 Step 5 checks 1, 2 and 7 by hand (status, a card, a second instance exits 0). Expected: unchanged behavior.

Commit this step alone, so a reviewer sees the refactor apart from the Linux code:

```bash
cd ~ && config add tools/agent-notify/app/src/startup.rs tools/agent-notify/app/src/macos/mod.rs tools/agent-notify/app/src/main.rs
config commit -m "Share agent-notifyd start-up between platforms" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- tools/agent-notify/app/src/startup.rs tools/agent-notify/app/src/macos/mod.rs tools/agent-notify/app/src/main.rs
```

- [ ] **Step 2: Add the Linux dependencies and stderr logging**

In `app/Cargo.toml`:

```toml
[features]
default = ["cosmic"]
cosmic = ["agent-notify-daemon/cosmic"]

[target.'cfg(target_os = "linux")'.dependencies]
gtk = "=0.18.2"
gtk-layer-shell = { version = "=0.8.2", features = ["v0_6"] }
```

And make the daemon dependency `agent-notify-daemon = { path = "../daemon", default-features = false }`, so the `cosmic` feature flows only through the app's own feature.

In `logging.rs`, add:

```rust
/// Logs to stderr without ANSI colour, for journald.
pub fn init_stderr(level: LevelFilter) {
    tracing_subscriber::fmt().with_writer(std::io::stderr).with_ansi(false).without_time().with_max_level(level).init();
}
```

`without_time` because journald stamps each line itself. If `without_time` needs a feature tracing-subscriber 0.3.23 does not enable here, drop it and accept the double timestamp.

- [ ] **Step 3: Write the pure geometry test**

`app/src/linux/geometry.rs`:

```rust
//! Where the X11 window goes. Layer shell places itself through anchors and
//! margins; X11 needs an explicit origin.

/// The X11 window's top-left corner for a window `width` wide in the
/// monitor's work area `(x, y, width, height)`, 16 px from its top-right.
#[must_use]
pub fn x11_origin(work_area: (i32, i32, i32, i32), width: i32) -> (i32, i32) {
    let (x, y, area_width, _) = work_area;
    (x + area_width - width - MARGIN, y + MARGIN)
}

/// The gap to the work area's edges.
pub const MARGIN: i32 = 16;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_sits_top_right_inside_the_margin() {
        assert_eq!(x11_origin((0, 0, 1920, 1080), 404), (1920 - 404 - 16, 16));
    }

    #[test]
    fn a_second_monitor_and_a_top_panel_offset_the_origin() {
        assert_eq!(x11_origin((1920, 32, 2560, 1408), 404), (1920 + 2560 - 404 - 16, 32 + 16));
    }
}
```

This module is not gated on Linux, so `cargo test -p agent-notifyd` runs it on macOS. Declare it from `main.rs` as `#[cfg_attr(not(target_os = "linux"), allow(dead_code))] mod linux_geometry;` pointing at the file with `#[path = "linux/geometry.rs"]`, or move the file to `src/x11_geometry.rs`. Pick the second if the first reads awkwardly; the plan does not depend on the name.

Run: `cd ~/tools/agent-notify && cargo test -p agent-notifyd geometry`
Expected: plan 2's geometry tests and these 2 pass.

- [ ] **Step 4: Write the Linux panel**

`app/src/linux/panel.rs`, following spike S2-1 to S2-3 and S4-1 (layer shell before realize, explicit size request, resize by size request, hide and show keep the anchors):

```rust
//! The toast window on Linux: a layer-shell surface on Wayland, a
//! notification-type window on X11.

use std::sync::atomic::{AtomicU32, Ordering};

use agent_notify_core::view::PanelViewModel;
use agent_notify_daemon::bridge::VIEW_MODEL_EVENT;
use agent_notify_daemon::desktop::Desktop;
use agent_notify_daemon::services::ToastPresenter;
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use super::geometry::{MARGIN, x11_origin};

/// The window label.
pub const LABEL: &str = "toast";
const WIDTH: i32 = 404;
const INITIAL_HEIGHT: u32 = 120;

static CONTENT_HEIGHT: AtomicU32 = AtomicU32::new(INITIAL_HEIGHT);

/// How the window was set up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelMode {
    /// A layer-shell surface (COSMIC and other wlroots-style compositors).
    LayerShell,
    /// A notification-type X11 window.
    X11Window,
}

/// Builds the hidden window. Call only for `CosmicWayland` or `X11`.
///
/// # Errors
///
/// When Tauri cannot build the web view, or the GTK window is missing.
pub fn build(app: &AppHandle, desktop: Desktop) -> tauri::Result<PanelMode> {
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .inner_size(f64::from(WIDTH), f64::from(INITIAL_HEIGHT))
        .decorations(false)
        .transparent(true)
        .skip_taskbar(true)
        .focused(false)
        .always_on_top(true)
        .visible(false)
        .build()?;
    let gtk_window = window.gtk_window()?;
    if desktop != Desktop::X11 && gtk_layer_shell::is_supported() {
        if gtk_window.is_realized() {
            gtk_window.unrealize();
        }
        gtk_window.init_layer_shell();
        gtk_window.set_layer(Layer::Overlay);
        gtk_window.set_anchor(Edge::Top, true);
        gtk_window.set_anchor(Edge::Right, true);
        gtk_window.set_layer_shell_margin(Edge::Top, MARGIN);
        gtk_window.set_layer_shell_margin(Edge::Right, MARGIN);
        gtk_window.set_keyboard_mode(KeyboardMode::None);
        gtk_window.set_namespace("agent-notify");
        gtk_window.set_size_request(WIDTH, i32::try_from(INITIAL_HEIGHT).unwrap_or(120));
        return Ok(PanelMode::LayerShell);
    }
    gtk_window.set_type_hint(gtk::gdk::WindowTypeHint::Notification);
    gtk_window.set_accept_focus(false);
    gtk_window.set_focus_on_map(false);
    place_x11(&gtk_window);
    Ok(PanelMode::X11Window)
}

fn place_x11(gtk_window: &gtk::ApplicationWindow) {
    let display = gtk_window.display();
    let Some(monitor) = display.primary_monitor().or_else(|| display.monitor(0)) else { return };
    let area = monitor.workarea();
    let (x, y) = x11_origin((area.x(), area.y(), area.width(), area.height()), WIDTH);
    gtk_window.move_(x, y);
}

/// Records the page's content height and resizes the window. Main thread only.
pub fn resize(app: &AppHandle, content_height_px: u32) {
    CONTENT_HEIGHT.store(content_height_px, Ordering::Relaxed);
    let Some(gtk_window) = app.get_webview_window(LABEL).and_then(|window| window.gtk_window().ok()) else { return };
    let height = i32::try_from(content_height_px.max(1)).unwrap_or(i32::MAX);
    gtk_window.set_size_request(WIDTH, height);
    gtk_window.resize(WIDTH, height);
}

/// Shows the view in the window.
pub struct GtkPresenter {
    app: AppHandle,
}

impl GtkPresenter {
    /// A presenter for `app`'s window.
    #[must_use]
    pub fn new(app: AppHandle) -> Self {
        GtkPresenter { app }
    }
}

impl ToastPresenter for GtkPresenter {
    fn present(&self, view: &PanelViewModel) {
        if let Err(error) = self.app.emit_to(LABEL, VIEW_MODEL_EVENT, view) {
            tracing::warn!(%error, "view model not delivered");
        }
        let visible = !view.cards.is_empty() || !view.finished_row.is_empty();
        let app = self.app.clone();
        let scheduled = self.app.run_on_main_thread(move || {
            let Some(window) = app.get_webview_window(LABEL) else { return };
            let result = if visible {
                resize(&app, CONTENT_HEIGHT.load(Ordering::Relaxed));
                window.show()
            } else {
                window.hide()
            };
            if let Err(error) = result {
                tracing::warn!(%error, "window show or hide failed");
            }
        });
        if let Err(error) = scheduled {
            tracing::warn!(%error, "window update not scheduled");
        }
    }
}
```

Check two names against gtk 0.18 before you build: `gtk_window()` returns `gtk::ApplicationWindow` in Tauri 2.12 (spike S2 used it); `move_` is gtk-rs's name for `gtk_window_move`. If `gtk_layer_shell::is_supported` is not exported with feature `v0_6`, look for it in `~/.cargo/registry/src/*/gtk-layer-shell-0.8.2/src/` and use the name found there.

- [ ] **Step 5: Write `linux::run`**

`app/src/linux/mod.rs`:

```rust
//! The Linux composition root.

mod panel;

pub use panel::resize;

use std::process::ExitCode;
use std::sync::{Arc, Mutex, OnceLock};

use agent_notify_core::update::UiIntent;
use agent_notify_daemon::actor::{Daemon, DaemonConfig, DaemonHandle};
use agent_notify_daemon::dbus_presenter::DbusPresenter;
use agent_notify_daemon::desktop::Desktop;
use agent_notify_daemon::dnd_store::DndStore;
use agent_notify_daemon::linux_tools::{LinuxRaiser, LinuxSound, RaiseMethod, TerminalOpener, sound_player};
use agent_notify_daemon::services::{FrontmostReader, NoPresenter, PresenceReader, Services, ToastPresenter, UnknownFrontmost, UnknownPresence};
use agent_notify_daemon::tmux::Tmux;
use agent_notify_daemon::x11::XpropFrontmost;
use tauri::{Manager, RunEvent};

use crate::shell::{Shell, bootstrap, ui_message};
use crate::startup::{Prepared, prepare};
use crate::{STAMP, logging};

struct Adapters {
    raise: RaiseMethod,
    frontmost: Arc<dyn FrontmostReader>,
    presence: Arc<dyn PresenceReader>,
}

fn adapters(desktop: Desktop, prepared: &Prepared) -> Adapters {
    let unknown = || Adapters { raise: RaiseMethod::None, frontmost: Arc::new(UnknownFrontmost), presence: Arc::new(UnknownPresence) };
    match desktop {
        #[cfg(feature = "cosmic")]
        Desktop::CosmicWayland => match agent_notify_daemon::cosmic::client::CosmicDesktop::connect(prepared.clock.clone()) {
            Some(cosmic) => Adapters { raise: RaiseMethod::Cosmic(cosmic.clone()), frontmost: Arc::new(cosmic.clone()), presence: Arc::new(cosmic) },
            None => unknown(),
        },
        Desktop::X11 => Adapters {
            raise: prepared.tools.xdotool.clone().map_or(RaiseMethod::None, RaiseMethod::Xdotool),
            frontmost: prepared.tools.xprop.clone().map_or_else(|| Arc::new(UnknownFrontmost) as Arc<dyn FrontmostReader>, |xprop| Arc::new(XpropFrontmost::new(xprop))),
            presence: Arc::new(UnknownPresence),
        },
        _ => unknown(),
    }
}

/// Runs the daemon until the app exits.
pub fn run() -> ExitCode {
    logging::init_stderr(logging::level_from(std::env::var("AGENT_NOTIFY_LOG").ok().as_deref()));
    logging::install_panic_hook();
    let prepared = match prepare(STAMP) {
        Ok(prepared) => prepared,
        Err(code) => return code,
    };
    let desktop = prepared.environment.desktop();
    tracing::info!(desktop = desktop.label(), "desktop");
    let adapters = adapters(desktop, &prepared);
    let Prepared { listener, state_dir, tools, runtime, facts, clock, .. } = prepared;
    let daemon_slot: Arc<Mutex<Option<Daemon>>> = Arc::new(Mutex::new(None));
    let setup_slot = Arc::clone(&daemon_slot);
    let runtime_handle = runtime.handle().clone();
    let built = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![bootstrap, ui_message])
        .setup(move |app| {
            let handle = app.handle().clone();
            let daemon_handle: Arc<OnceLock<DaemonHandle>> = Arc::new(OnceLock::new());
            let presenter: Arc<dyn ToastPresenter> = match desktop {
                Desktop::CosmicWayland | Desktop::X11 => {
                    let mode = panel::build(&handle, desktop)?;
                    tracing::info!(?mode, "panel");
                    Arc::new(panel::GtkPresenter::new(handle.clone()))
                }
                Desktop::OtherWayland => {
                    let clicks = Arc::clone(&daemon_handle);
                    match DbusPresenter::connect(move |card_id| {
                        if let Some(daemon) = clicks.get() {
                            daemon.send_ui(UiIntent::Clicked { card_id });
                        }
                    }) {
                        Some(presenter) => Arc::new(presenter),
                        None => Arc::new(NoPresenter),
                    }
                }
                Desktop::Headless => Arc::new(NoPresenter),
            };
            let services = Arc::new(Services {
                tmux: tools.tmux.clone().map(Tmux::new),
                raiser: Arc::new(LinuxRaiser::new(adapters.raise, TerminalOpener::new(tools.alacritty.clone()))),
                presenter,
                frontmost: adapters.frontmost,
                presence: adapters.presence,
                sound: Arc::new(LinuxSound::new(sound_player(tools.pw_play.as_deref(), tools.paplay.as_deref()))),
                dnd_store: DndStore::new(state_dir.join("dnd.json")),
                clock,
                facts,
            });
            let daemon = {
                let _entered = runtime_handle.enter();
                Daemon::start(services, listener, DaemonConfig::default())
            };
            let _ = daemon_handle.set(daemon.handle());
            app.manage(Shell { daemon: daemon.handle(), native_hover: false });
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

- `OnceLock` closes the start-up loop for the D-Bus presenter: the presenter exists before the daemon, and a click before `set` is dropped, which cannot happen because no notification is shown before the daemon runs.
- On `Headless` and `OtherWayland`, no GTK window is built. Tauri still needs the page bundle at compile time; it stays unused.
- The `.setup` closure takes `FnOnce`, so `adapters`, `tools` and `listener` move in once.

Wire it in `main.rs`: `#[cfg(target_os = "linux")] mod linux;` and

```rust
#[cfg(target_os = "linux")]
fn run() -> ExitCode {
    linux::run()
}
```

replacing the non-macOS stub for Linux (keep the stub for other platforms with `#[cfg(not(any(target_os = "macos", target_os = "linux")))]`). In `shell.rs`, add the Linux `resize`:

```rust
#[cfg(target_os = "linux")]
fn resize(app: &AppHandle, content_height_px: u32) {
    crate::linux::resize(app, content_height_px);
}
```

and narrow the existing no-op to `#[cfg(not(any(target_os = "macos", target_os = "linux")))]`.

- [ ] **Step 6: Run the gates**

Run: `cd ~/tools/agent-notify && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && scripts/linux-check.sh clippy && scripts/linux-check.sh clippy --no-default-features && scripts/linux-check.sh test && cargo deny check 2>&1 | tail -3`
Expected: all pass on both platforms, with and without `cosmic`, and `cargo deny` prints no warnings.

- [ ] **Step 7: Commit**

```bash
cd ~ && config add tools/agent-notify/app tools/agent-notify/Cargo.lock
config diff --cached --name-only
config commit -m "Add the Linux panel and composition root" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- tools/agent-notify/app tools/agent-notify/Cargo.lock
```

---

### Task 9: Install on Linux: systemd unit, doctor, docs

**Files:**
- Create: `tools/agent-notify/scripts/systemd-unit.ts`, `tools/agent-notify/tests/systemd-unit.test.ts`
- Modify: `tools/agent-notify/scripts/install.ts`, `crates/agent-notify/src/doctor.rs`, `README-LINUX.md`, `deps/deps-linux.toml`

**Interfaces:**
- Consumes: plan 2 Task 17 `install.ts`, `stampFrom`; plan 2 Task 7 `DoctorFacts`, `evaluate`.
- Produces:
  - `systemd-unit.ts`: `UNIT_NAME = "agent-notifyd.service"`, `type UnitInputs = { home: string; tmux: string | null; binary: string; path: string }`, `renderUnit(inputs) -> string`, `unitPaths(home) -> { binary: string; unit: string; stateDir: string; stampPath: string }`.
  - `install.ts` on Linux: builds, copies the binary to `~/.local/libexec/agent-notifyd`, writes the unit, `systemctl --user daemon-reload`, and with consent `enable --now` plus `restart`.
  - `doctor`: `SystemdState::{Active, Inactive, Failed, Missing}` in `DoctorFacts.systemd: Option<SystemdState>`, and one check row.

- [ ] **Step 1: Write the failing unit tests**

`tools/agent-notify/tests/systemd-unit.test.ts`:

```ts
import { expect, test } from "bun:test";
import { UNIT_NAME, renderUnit, unitPaths } from "../scripts/systemd-unit";

const inputs = { home: "/home/dev", tmux: "/usr/bin/tmux", binary: "/home/dev/.local/libexec/agent-notifyd", path: "/usr/local/bin:/usr/bin:/bin" };

test("the unit follows the graphical session", () => {
  const unit = renderUnit(inputs);
  expect(unit).toContain("PartOf=graphical-session.target");
  expect(unit).toContain("After=graphical-session.target");
  expect(unit).toContain("WantedBy=graphical-session.target");
  expect(unit).toContain("Restart=on-failure");
});

test("a second instance's clean exit does not restart, and new terminals survive a restart", () => {
  const unit = renderUnit(inputs);
  expect(unit).not.toContain("Restart=always");
  expect(unit).toContain("KillMode=process");
});

test("the unit carries absolute paths", () => {
  const unit = renderUnit(inputs);
  expect(unit).toContain("ExecStart=/home/dev/.local/libexec/agent-notifyd");
  expect(unit).toContain('Environment="AGENT_NOTIFY_TMUX=/usr/bin/tmux"');
  expect(unit).toContain('Environment="PATH=/usr/local/bin:/usr/bin:/bin"');
  expect(unit).toContain('Environment="RUST_BACKTRACE=1"');
});

test("a missing tmux leaves its variable out", () => {
  expect(renderUnit({ ...inputs, tmux: null })).not.toContain("AGENT_NOTIFY_TMUX");
});

test("a path with a space or a quote is quoted for systemd", () => {
  const unit = renderUnit({ ...inputs, binary: '/home/a b/.local/libexec/agent-notifyd', tmux: '/opt/t"x/tmux' });
  expect(unit).toContain('ExecStart="/home/a b/.local/libexec/agent-notifyd"');
  expect(unit).toContain('Environment="AGENT_NOTIFY_TMUX=/opt/t\\"x/tmux"');
});

test("paths follow XDG defaults under home", () => {
  expect(unitPaths("/home/dev")).toEqual({
    binary: "/home/dev/.local/libexec/agent-notifyd",
    unit: `/home/dev/.config/systemd/user/${UNIT_NAME}`,
    stateDir: "/home/dev/.local/state/agent-notify",
    stampPath: "/home/dev/.local/state/agent-notify/installed-stamp",
  });
});
```

The state directory must match plan 2 Task 2's `state_directory` on Linux (`$XDG_STATE_HOME/agent-notify`, default `~/.local/state/agent-notify`). The install script uses the default; a user with a custom `XDG_STATE_HOME` gets a `doctor` warning about a missing stamp, which is accepted for v1.

Run: `cd ~/tools/agent-notify && bun test tests/systemd-unit.test.ts`
Expected: failure, module not found.

- [ ] **Step 2: Implement the unit renderer**

`tools/agent-notify/scripts/systemd-unit.ts`:

```ts
export const UNIT_NAME = "agent-notifyd.service";

export type UnitInputs = { home: string; tmux: string | null; binary: string; path: string };

const needsQuotes = /[\s"\\]/;

function quoted(value: string): string {
  return `"${value.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}"`;
}

const execValue = (value: string): string => (needsQuotes.test(value) ? quoted(value) : value);

export function renderUnit(inputs: UnitInputs): string {
  const environment = [`PATH=${inputs.path}`, "RUST_BACKTRACE=1"];
  if (inputs.tmux !== null) environment.push(`AGENT_NOTIFY_TMUX=${inputs.tmux}`);
  return [
    "[Unit]",
    "Description=agent-notify toast daemon",
    "PartOf=graphical-session.target",
    "After=graphical-session.target",
    "",
    "[Service]",
    `ExecStart=${execValue(inputs.binary)}`,
    "Restart=on-failure",
    "KillMode=process",
    ...environment.map((entry) => `Environment=${quoted(entry)}`),
    "",
    "[Install]",
    "WantedBy=graphical-session.target",
    "",
  ].join("\n");
}

export function unitPaths(home: string) {
  const stateDir = `${home}/.local/state/agent-notify`;
  return {
    binary: `${home}/.local/libexec/agent-notifyd`,
    unit: `${home}/.config/systemd/user/${UNIT_NAME}`,
    stateDir,
    stampPath: `${stateDir}/installed-stamp`,
  };
}
```

`KillMode=process` stops only the daemon on `restart`, so an Alacritty window the daemon opened (Task 5) survives. `Restart=on-failure` does not restart a clean exit 0, which is what a second instance returns.

Run: `cd ~/tools/agent-notify && bun test tests/systemd-unit.test.ts`
Expected: 6 pass.

- [ ] **Step 3: Add the Linux branch to `install.ts`**

Replace the `process.platform !== "darwin"` exit with a branch. The macOS path stays as plan 2 wrote it, moved into `installMac()`; add:

```ts
import { renderUnit, unitPaths, UNIT_NAME } from "./systemd-unit";

async function installLinux(stamp: string): Promise<void> {
  const required = ["webkit2gtk-4.1", "gtk-layer-shell-0"];
  const missing = required.filter((name) => !Bun.spawnSync(["pkg-config", "--exists", name]).success);
  if (missing.length > 0) {
    console.error(`agent-notify install: missing development packages: ${missing.join(", ")}. See README-LINUX.md.`);
    process.exit(1);
  }
  const paths = unitPaths(home);
  run(["bun", "install", "--frozen-lockfile"], { cwd: join(toolRoot, "ui") });
  run(["bun", "run", "build"], { cwd: join(toolRoot, "ui") });
  run(["cargo", "deny", "check"]);
  run(["cargo", "build", "--release", "--locked", "-p", "agent-notifyd"], { env: { CONFIG_MANIFEST_STAMP: stamp } });
  const built = join(home, ".cache/agent-notify/target/release/agent-notifyd");
  await mkdir(dirname(paths.binary), { recursive: true });
  const staged = `${paths.binary}.tmp-${process.pid}`;
  await Bun.write(staged, Bun.file(built));
  await chmod(staged, 0o755);
  await rename(staged, paths.binary);
  const reported = run([paths.binary, "--stamp"]).trim();
  if (reported !== stamp) {
    console.error(`agent-notify install: the binary reports stamp ${reported}, expected ${stamp}`);
    process.exit(1);
  }
  await writeAtomic(paths.unit, renderUnit({ home, tmux: Bun.which("tmux"), binary: paths.binary, path: process.env.PATH ?? "/usr/local/bin:/usr/bin:/bin" }));
  await mkdir(paths.stateDir, { recursive: true, mode: 0o700 });
  await writeAtomic(paths.stampPath, `${stamp}\n`);
  run(["systemctl", "--user", "daemon-reload"]);
  if (skipLaunchd) {
    console.log(`agent-notify install: installed ${paths.binary} (stamp ${stamp}); service not started`);
    return;
  }
  run(["systemctl", "--user", "enable", UNIT_NAME]);
  run(["systemctl", "--user", "restart", UNIT_NAME]);
  console.log(`agent-notify install: installed and restarted ${UNIT_NAME} (stamp ${stamp})`);
}
```

Then dispatch: `if (process.platform === "darwin") await installMac(stamp); else if (process.platform === "linux") await installLinux(stamp); else { console.error(...); process.exit(1); }`. `--no-launchd` keeps its name and means "do not start the service" on both platforms; update its help line in `config-build` to say "launch agent or systemd unit".

`daemon-reload` runs even with `--no-launchd`: it only reloads unit files and starts nothing.

- [ ] **Step 4: Add the systemd check to `doctor`, test first**

In `crates/agent-notify/src/doctor.rs`, add beside `LaunchdState`:

```rust
/// The systemd user unit. `None` off Linux.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemdState {
    /// Running.
    Active,
    /// Installed, not running.
    Inactive,
    /// Exited with an error; systemd may be restarting it.
    Failed,
    /// No unit file.
    Missing,
}

/// Reads `systemctl --user is-active` output.
#[must_use]
pub fn parse_systemd_state(is_active: &str, unit_exists: bool) -> SystemdState {
    if !unit_exists {
        return SystemdState::Missing;
    }
    match is_active.trim() {
        "active" | "activating" | "reloading" => SystemdState::Active,
        "failed" => SystemdState::Failed,
        _ => SystemdState::Inactive,
    }
}
```

Tests in the same file:

```rust
#[test]
fn systemd_states_parse() {
    assert_eq!(parse_systemd_state("active\n", true), SystemdState::Active);
    assert_eq!(parse_systemd_state("failed\n", true), SystemdState::Failed);
    assert_eq!(parse_systemd_state("inactive\n", true), SystemdState::Inactive);
    assert_eq!(parse_systemd_state("active\n", false), SystemdState::Missing);
}

#[test]
fn a_failed_unit_fails_the_check_and_names_journalctl() {
    let facts = DoctorFacts { systemd: Some(SystemdState::Failed), ..linux_facts() };
    let check = evaluate(&facts).into_iter().find(|check| check.name == "service").expect("a service row");
    assert_eq!(check.status, CheckStatus::Fail);
    assert!(check.detail.contains("journalctl --user -u agent-notifyd"), "{}", check.detail);
}
```

`linux_facts()` is a test helper beside plan 2's macOS one, with `platform: Platform::Linux`, `launchd: None`, and every other fact healthy. Use the field names plan 2 Task 7 gave `Check` (`name`, `status`, `detail` here); if they differ, follow plan 2.

Add `pub systemd: Option<SystemdState>` to `DoctorFacts`. In `gather`, fill it on Linux: `systemctl --user is-active agent-notifyd.service` (exit status ignored; stdout parsed) and whether `~/.config/systemd/user/agent-notifyd.service` exists. In `evaluate`, add a `service` row for Linux: `Active` passes; `Inactive` warns with `systemctl --user enable --now agent-notifyd.service`; `Failed` fails with `journalctl --user -u agent-notifyd -n 50`; `Missing` fails with `config build --tools`. The macOS `launchd` row keeps its name; if plan 2 named it `service`, reuse that name for both.

Run: `cd ~/crates && cargo test -p agent-notify doctor`
Expected: the new tests pass with plan 2's.

- [ ] **Step 5: Docs and deps**

`README-LINUX.md`, replace the "Claude Code notifications" section (plan 2 Task 19 wrote a placeholder) with:

```markdown
## Claude Code notifications

`agent-notifyd` (`tools/agent-notify`) shows a card at the top right when a
Claude Code session finishes or needs you. A click switches tmux to that
session's pane and raises Alacritty.

- Pop!_OS COSMIC: a layer-shell card, and the raise goes through COSMIC's
  toplevel protocol.
- X11 (Arch): a notification window; the raise uses `xdotool`.
- Other Wayland desktops: desktop notifications with an Open action.

Build dependencies (Debian, Ubuntu, Pop!_OS):

    sudo apt install build-essential pkg-config libssl-dev \
        libwebkit2gtk-4.1-dev libgtk-layer-shell-dev

Arch: `sudo pacman -S base-devel webkit2gtk-4.1 gtk-layer-shell`.

Install or update with `config build --tools`. It runs as the systemd user
unit `agent-notifyd.service`. `agent-notify doctor` checks the setup, and
`journalctl --user -u agent-notifyd` shows its log.

The COSMIC raise links `cosmic-protocols`, which is GPL-3.0-only. Build
without it with `cargo build --no-default-features` in `tools/agent-notify`;
cards still show and tmux still switches, but Alacritty is not raised.
```

Check the Arch package names with `pacman -Si webkit2gtk-4.1 gtk-layer-shell` on an Arch machine or container (`docker run --rm archlinux pacman -Sy --noconfirm >/dev/null && pacman -Si ...`) before you commit; fix them if they differ.

`deps/deps-linux.toml`, append:

```toml
# agent-notifyd plays its attention sound with pw-play (PipeWire), or
# paplay where PipeWire is missing.
[pw-play]
any_of = [
    { command = "pw-play" },
    { command = "paplay" },
]
docs = "https://pipewire.org/"
```

Run: `cd ~/crates && cargo test -p config-cli --test deps_manifest 2>&1 | tail -3` (or whichever test parses `deps-linux.toml`; `rg -l deps-linux crates/*/tests`).
Expected: pass.

- [ ] **Step 6: Install in the VM**

The VM is disposable, so this needs no approval. Copy the repo's agent-notify parts into the VM and run the install there:

```bash
S=~/path/to/scratch  # any scratch directory
cd ~ && tar czf "$S/an.tgz" crates tools/agent-notify .scripts/config deps
scp -i "$KEY" "$S/an.tgz" spike@"$(tart ip pop-cosmic-spike)":
ssh -i "$KEY" spike@"$(tart ip pop-cosmic-spike)" 'mkdir -p ~/an && tar xzf ~/an.tgz -C ~/an && cd ~/an && DOTFILES_ROOT=~/an sh .scripts/config/config-build --tools-only'
```

`$KEY` is the VM's SSH key from the spike set-up. Expected: the install finishes and `systemctl --user is-active agent-notifyd` prints `active`. Install `libwebkit2gtk-4.1-dev libgtk-layer-shell-dev` and Bun in the VM first if the script says they are missing.

- [ ] **Step 7: Commit**

```bash
cd ~ && config add tools/agent-notify/scripts tools/agent-notify/tests crates/agent-notify/src/doctor.rs README-LINUX.md deps/deps-linux.toml .scripts/config/config-build
config diff --cached --name-only
config commit -m "Install agent-notifyd as a systemd user unit on Linux" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- tools/agent-notify/scripts tools/agent-notify/tests crates/agent-notify/src/doctor.rs README-LINUX.md deps/deps-linux.toml .scripts/config/config-build
```

---

### Task 10: Linux smoke test

No code. Run each check and add a "Plan 3 smoke test" table to the spike report (id, result, evidence), then commit it.

- [ ] **Step 1: In the COSMIC VM**

1. `agent-notify doctor` passes, and its desktop line reads `cosmic-wayland`.
2. In a tmux pane in Alacritty, `agent-notify send attention 'Needs you <b>x</b> & y'` shows an amber card at the top right with the text literal. Alacritty keeps keyboard focus (type in it).
3. Hover the card: the close button fades in (CSS hover, spike S2-3). Click ×: the card goes.
4. Switch to another tmux window and to another app (COSMIC Files), send an attention card, click it: tmux switches back and Alacritty comes to the front.
5. Close every Alacritty window, send a card, click it: a new Alacritty window opens attached to the session.
6. `agent-notify send info done` with the card's pane focused in the front Alacritty window, while typing: no card (frontmost Terminal, idle 0, pane active).
7. Stop typing for 35 s, then send the same: a card shows (idle 30).
8. `systemctl --user restart agent-notifyd`: the daemon comes back, a terminal opened in check 5 stays open, and `journalctl --user -u agent-notifyd -n 20` shows the start-up line with `desktop=cosmic-wayland`.
9. Rebuild with `--no-default-features` in the VM, reinstall, and repeat check 4: tmux switches, Alacritty is not raised, and the card does not report an error beyond "window not raised".

- [ ] **Step 2: D-Bus path**

10. In the VM, start the daemon by hand with `XDG_CURRENT_DESKTOP=GNOME` (so it picks `OtherWayland`): `systemctl --user stop agent-notifyd; XDG_CURRENT_DESKTOP=GNOME ~/.local/libexec/agent-notifyd`. Send an attention card: a COSMIC popup shows with look-alike characters. Click it: tmux switches. Send a second card for the same session: the first popup closes.

- [ ] **Step 3: X11 (best effort)**

11. On an Arch X11 machine if one is at hand, or in the VM under `startx`/Openbox if not: checks 2 and 4. Record the focus flicker on click (spike D12).

- [ ] **Step 4: Commit the results**

```bash
cd ~ && config add docs/superpowers/spikes/2026-10-15-agent-notify-spikes.md
config commit -m "Record the agent-notify plan 3 smoke test" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -- docs/superpowers/spikes/2026-10-15-agent-notify-spikes.md
```

Every failed check gets a fix task appended to this plan before it closes.
