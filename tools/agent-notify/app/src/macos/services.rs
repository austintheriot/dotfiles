//! The macOS service implementations. AppKit calls run on the main thread
//! through Tauri, and every wait for the main thread is bounded.

use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use agent_notify_core::update::Frontmost;
use agent_notify_daemon::aerospace::Aerospace;
use agent_notify_daemon::child::{RunError, run_bounded};
use agent_notify_daemon::presence::{TERMINAL_BUNDLE_ID, frontmost_from_bundle};
use agent_notify_daemon::services::{AttachRequest, FrontmostReader, SoundPlayer, WindowRaiser};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};
use objc2_foundation::NSString;
use tauri::AppHandle;

const MAIN_THREAD_WAIT: Duration = Duration::from_secs(1);
/// The limit for `open`. It returns once LaunchServices accepts the launch,
/// and a cold Alacritty start can take longer than the 1 s child limit.
const OPEN_LIMIT: Duration = Duration::from_secs(3);
const STDERR_LOG_BYTES: usize = 1024;
const AFPLAY: &str = "/usr/bin/afplay";
const SOUND: &str = "/System/Library/Sounds/Glass.aiff";
/// Glass.aiff plays for under 2 s. The limit only reaps a wedged `afplay`.
const AFPLAY_LIMIT: Duration = Duration::from_secs(10);
const OPEN: &str = "/usr/bin/open";

/// Runs `work` on the main thread and waits up to `limit` for its answer.
///
/// Called on the main thread, it runs `work` inline: waiting there would
/// block the thread that has to run `work`, so the wait would always time out.
pub fn on_main<T: Send + 'static>(app: &AppHandle, limit: Duration, work: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    if MainThreadMarker::new().is_some() {
        return Some(work());
    }
    let (sender, receiver) = mpsc::channel();
    if let Err(error) = app.run_on_main_thread(move || {
        let _receiver_gone = sender.send(work());
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
    #[expect(dead_code, reason = "wired in Task 16")]
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
#[expect(dead_code, reason = "wired in Task 16")]
#[derive(Debug, Clone, Copy, Default)]
pub struct AfplaySound;

impl SoundPlayer for AfplaySound {
    fn play(&self) {
        if let Err(error) = std::thread::Builder::new().name("afplay".to_owned()).spawn(play_bounded) {
            tracing::warn!(%error, "the afplay thread did not start");
        }
    }
}

#[expect(dead_code, reason = "wired in Task 16")]
fn play_bounded() {
    match run_bounded(Command::new(AFPLAY).arg(SOUND), AFPLAY_LIMIT) {
        Ok(finished) if finished.status.success() => {}
        Ok(finished) => tracing::warn!(status = ?finished.status, "afplay failed"),
        Err(error @ RunError::TimedOut { .. }) => tracing::warn!(%error, "afplay was stopped"),
        Err(error) => tracing::warn!(%error, "afplay did not finish"),
    }
}

/// AeroSpace focus when present, then app activation (spike D2).
pub struct MacRaiser {
    app: AppHandle,
    aerospace: Option<Aerospace>,
}

impl MacRaiser {
    /// A raiser for `app`, with AeroSpace at `aerospace` when found.
    #[expect(dead_code, reason = "wired in Task 16")]
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
        let mut command = Command::new(OPEN);
        command
            .args(["-n", "-b", TERMINAL_BUNDLE_ID, "--args", "-e"])
            .arg(&request.tmux)
            .args(["-u", "-S"])
            .arg(&request.socket)
            .args(["attach-session", "-t", &request.session]);
        match run_bounded(&mut command, OPEN_LIMIT) {
            Ok(finished) if finished.status.success() => true,
            Ok(finished) => {
                let shown = &finished.stderr[..finished.stderr.len().min(STDERR_LOG_BYTES)];
                tracing::warn!(status = ?finished.status, stderr = %String::from_utf8_lossy(shown).trim(), "open did not start the terminal");
                false
            }
            Err(error) => {
                tracing::warn!(%error, "open did not finish");
                false
            }
        }
    }
}
