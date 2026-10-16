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
    /// Switch the most recently active client. On a tie in activity, the
    /// client with the lowest tty, so the list order tmux printed does not matter.
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
        match world.clients.iter().max_by_key(|client| (client.activity, std::cmp::Reverse(&client.tty))) {
            Some(client) => TmuxSwitch::SwitchClient { tty: client.tty.clone() },
            None => TmuxSwitch::AttachNewTerminal { tmux_session: chosen.tmux_session.clone() },
        }
    };
    FocusPlan::Focus { pane: chosen.pane.clone(), switch }
}

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
    fn a_tie_in_client_activity_picks_the_same_client_in_any_list_order() {
        let first = client("/dev/ttys001", "code", 20);
        let second = client("/dev/ttys002", "code", 20);
        let expected = FocusPlan::Focus { pane: pane(5).pane, switch: TmuxSwitch::SwitchClient { tty: "/dev/ttys001".to_string() } };
        for clients in [vec![first.clone(), second.clone()], vec![second, first]] {
            let world = FocusWorld { panes: vec![world_pane(5, Some("a"), "work")], clients };
            assert_eq!(plan_focus(&target(5), Some(&world)), expected);
        }
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
