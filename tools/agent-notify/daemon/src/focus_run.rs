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
    let chosen = PaneKey { server: last_seen.server.clone(), pane };
    match tmux.session_tag(&chosen) {
        Ok(Some(tag)) if &tag == session => {}
        Ok(_) | Err(TmuxError::ServerGone) => return FocusOutcome::SessionGone,
        Err(error) => return failed(&error),
    }
    match tmux.switch(&chosen.server, &chosen.pane, &switch) {
        Ok(()) => {}
        Err(TmuxError::ServerGone) => return FocusOutcome::SessionGone,
        Err(error) => return failed(&error),
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
