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

/// Every message from the page. A sync command, so it runs on the main
/// thread; nothing here may block. Tauri decodes `message` before this runs,
/// so no size cap here could act first: the page is this repo's own code
/// under a strict CSP, and `route` cuts error text to its limits.
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
