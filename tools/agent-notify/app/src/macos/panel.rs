//! The non-activating toast panel (spike 1). `tauri_panel!` expands to
//! Objective-C class definitions, which need `unsafe`.

#![allow(unsafe_code)] // tauri_panel! defines Objective-C classes; see spike S1.
#![expect(dead_code, reason = "wired in Task 16")]

use std::sync::atomic::{AtomicU32, Ordering};

use agent_notify_core::update::UiIntent;
use agent_notify_core::view::PanelViewModel;
use agent_notify_daemon::actor::DaemonHandle;
use agent_notify_daemon::bridge::{HOVER_EVENT, HoverPayload, VIEW_MODEL_EVENT};
use agent_notify_daemon::services::ToastPresenter;
use objc2_app_kit::{NSScreen, NSWindowSharingType};
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

/// Records the page's content height and resizes the panel. The caller must
/// run this on the main thread: Tauri commands run on the async runtime, so
/// hop with `run_on_main_thread` first.
pub fn resize(app: &AppHandle, content_height_px: u32) {
    CONTENT_HEIGHT.store(content_height_px, Ordering::Relaxed);
    place(app, content_height_px);
}

fn place(app: &AppHandle, content_height_px: u32) {
    let Some(marker) = MainThreadMarker::new() else {
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

#[cfg(test)]
mod tests {
    use agent_notify_core::view::{CardView, FinishedEntry};
    use agent_notify_core::wire::Kind;
    use agent_notify_core::{CardId, EpochMs};

    use super::*;

    fn card_id() -> CardId {
        CardId::parse("card-1").expect("a valid card id")
    }

    fn view(cards: Vec<CardView>, finished_row: Vec<FinishedEntry>) -> PanelViewModel {
        PanelViewModel { revision: 1, cards, finished_row, dnd: false }
    }

    fn card() -> CardView {
        CardView { card_id: card_id(), kind: Kind::Info, location: "work".to_string(), title: None, body: None, created_ms: EpochMs(0), note: None }
    }

    fn finished() -> FinishedEntry {
        FinishedEntry { card_id: card_id(), label: "2".to_string() }
    }

    #[test]
    fn the_panel_shows_for_a_card_or_a_finished_row_only() {
        let cases = [
            ("no cards and no finished row", view(vec![], vec![]), false),
            ("one card", view(vec![card()], vec![]), true),
            ("a finished row only", view(vec![], vec![finished()]), true),
        ];
        for (name, model, expected) in cases {
            assert_eq!(has_content(&model), expected, "{name}");
        }
    }
}

