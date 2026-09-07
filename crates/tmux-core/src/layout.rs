//! Layout-name resolution, ported from `.scripts/tmux-split.sh`'s `case`.
//!
//! The script's `case $LAYOUT_TYPE in ... *) show_usage ;; esac` did two
//! jobs with one branch. It chose an arrangement for a recognized name, and
//! for every other name it printed usage and returned 1. `tmux-start.sh`
//! relied on that second job as a silent no-op: it sourced the script with
//! no arguments so `${1:-}` read the *caller's* `$1`, the session name, and
//! a session name that is not a layout name fell into the usage branch.
//!
//! Those two jobs are separated here. `layout_for` answers only "is this a
//! layout name, and which arrangement," and `None` carries no judgement
//! about the caller. Deciding that `None` is an ordinary outcome rather
//! than a usage error is the binary's job, and it is why the binary exits 3
//! rather than 2.

/// How a recognized layout name arranges the panes it creates.
///
/// One variant per arrangement function in `tmux-split.sh`, not one per
/// name: the script's `case` maps six names onto three arrangements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrangement {
    /// `create_vertical_terminals`: `vertical_splits` panes stacked
    /// vertically, focus returned to the top pane.
    VerticalTerminals,

    /// `create_editor_with_terminals`: one horizontal split for the editor,
    /// then `vertical_splits` terminals stacked in the right-hand pane,
    /// focus returned to the editor on the left.
    EditorWithTerminals,

    /// `create_main_above_two_below`: `vertical_splits` panes stacked
    /// vertically, then the bottom one split into `horizontal_splits` panes
    /// side by side, focus returned to the main area above.
    MainAboveTwoBelow,
}

/// A resolved layout: its arrangement and the two counts that shape it.
///
/// The counts live here rather than being re-derived by the caller because
/// the script's defaults are per-script constants, not per-layout ones, and
/// a caller that had to remember them could disagree with this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// Which arrangement function the name selected.
    pub arrangement: Arrangement,

    /// `tmux-split.sh`'s `VERTICAL_SPLITS`, the script's second positional
    /// argument. Counts total panes in the vertical stack, so 1 means "make
    /// no vertical split at all".
    pub vertical_splits: u32,

    /// `tmux-split.sh`'s `HORIZONTAL_SPLITS`, the script's third positional
    /// argument. Read only by [`Arrangement::MainAboveTwoBelow`]; the other
    /// two arrangements ignore it, exactly as the shell functions did.
    pub horizontal_splits: u32,
}

/// `tmux-split.sh`'s `DEFAULT_VERTICAL_SPLITS=2`.
pub const DEFAULT_VERTICAL_SPLITS: u32 = 2;

/// `tmux-split.sh`'s `DEFAULT_HORIZONTAL_SPLITS=2`.
pub const DEFAULT_HORIZONTAL_SPLITS: u32 = 2;

/// Resolves a layout name to its arrangement, or `None` when the name is
/// not a layout name at all.
///
/// `None` is not an error. `tmux-start.sh` passes a session name here, and
/// most session names are not layout names; the old shell version reached
/// that same outcome by printing usage, which told a user who typed a
/// perfectly good session name that they had used the command wrong.
///
/// The recognized names are exactly `tmux-split.sh`'s `case` arms:
/// `terms`, `\`, `|`, `-`, `_` and `code`.
#[must_use]
pub fn layout_for(name: &str) -> Option<Layout> {
    layout_with_counts(name, DEFAULT_VERTICAL_SPLITS, DEFAULT_HORIZONTAL_SPLITS)
}

/// [`layout_for`] with the two counts the script took as its second and
/// third positional arguments.
///
/// Separate from `layout_for` because the name-recognition question and the
/// count question have different callers: `tmux-start.sh` asks only the
/// first, and never passes counts.
#[must_use]
pub fn layout_with_counts(
    name: &str,
    vertical_splits: u32,
    horizontal_splits: u32,
) -> Option<Layout> {
    let arrangement = match name {
        "terms" => Arrangement::VerticalTerminals,
        "\\" | "|" | "code" => Arrangement::EditorWithTerminals,
        "-" | "_" => Arrangement::MainAboveTwoBelow,
        _ => return None,
    };
    Some(Layout {
        arrangement,
        vertical_splits,
        horizontal_splits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every name `tmux-split.sh`'s `case` recognizes resolves, and to the
    /// arrangement that script's matching arm called.
    ///
    /// The names come from the shell, which is the contract. The plan's
    /// example used "dev", which no arm of that `case` names.
    #[test]
    fn every_shell_layout_name_resolves_to_its_arrangement() {
        let expected = [
            ("terms", Arrangement::VerticalTerminals),
            ("\\", Arrangement::EditorWithTerminals),
            ("|", Arrangement::EditorWithTerminals),
            ("code", Arrangement::EditorWithTerminals),
            ("-", Arrangement::MainAboveTwoBelow),
            ("_", Arrangement::MainAboveTwoBelow),
        ];
        for (name, arrangement) in expected {
            let layout = layout_for(name);
            assert_eq!(
                layout.map(|resolved| resolved.arrangement),
                Some(arrangement),
                "{name} is a layout name in tmux-split.sh"
            );
        }
    }

    /// A recognized name carries the script's own default counts.
    #[test]
    fn a_known_layout_carries_the_shell_defaults() {
        let layout = layout_for("terms").expect("terms is a layout name");

        assert_eq!(layout.vertical_splits, 2, "DEFAULT_VERTICAL_SPLITS is 2");
        assert_eq!(layout.horizontal_splits, 2, "DEFAULT_HORIZONTAL_SPLITS is 2");
    }

    /// Explicit counts override the defaults, as the script's `${2:-}` and
    /// `${3:-}` did.
    #[test]
    fn explicit_counts_override_the_defaults() {
        let layout = layout_with_counts("-", 3, 4).expect("- is a layout name");

        assert_eq!(layout.vertical_splits, 3);
        assert_eq!(layout.horizontal_splits, 4);
    }

    /// A session name that is not a layout resolves to `None`, which is the
    /// silent no-op the old positional-parameter inheritance produced by
    /// printing usage.
    #[test]
    fn a_session_name_that_is_not_a_layout_resolves_to_none() {
        assert!(
            layout_for("my-feature-branch").is_none(),
            "an unrecognized name is not a layout, and not an error either"
        );
    }

    /// The empty name, which is what `${1:-}` produced when the script was
    /// sourced with no arguments at all, is not a layout name either.
    #[test]
    fn the_empty_name_is_not_a_layout() {
        assert!(layout_for("").is_none(), "the shell's `*)` arm caught the empty case too");
    }

    /// Layout names are matched exactly, not by prefix or case.
    ///
    /// The shell `case` used literal patterns with no globbing, so `Terms`
    /// and `terms-extra` fell to `*)` there and must fall to `None` here.
    #[test]
    fn layout_names_match_exactly() {
        for near_miss in ["Terms", "terms-extra", "CODE", " terms", "||"] {
            assert!(
                layout_for(near_miss).is_none(),
                "{near_miss} is not one of tmux-split.sh's case arms"
            );
        }
    }
}
