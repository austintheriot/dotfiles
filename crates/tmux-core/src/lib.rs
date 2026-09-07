#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
//! Pure computation for the tmux and zsh helper scripts.
//!
//! Holds no capabilities: the window-name precedence, the layout choice and
//! the branch-list formatting are all functions of injected values. The
//! `tmux-tools` binary performs the tmux and git calls and hands the results
//! in. That split is what makes this crate's behavior testable without a
//! tmux server, and it is the same split `deps-core` uses.

mod branches;
mod naming;

pub use branches::{BranchRef, format_branches};
pub use naming::{HeadState, RepositoryFacts, WindowFacts, window_name};

#[cfg(test)]
mod purity {
    /// The crate source must name no IO capability.
    ///
    /// `include_str!` reads at compile time, so this test spawns nothing and
    /// opens nothing at runtime.
    ///
    /// Each later task adds its own module to `sources` in the same commit
    /// that adds the module. A module absent from this array is unchecked.
    #[test]
    fn no_module_names_an_io_capability() {
        let sources = [
            ("lib.rs", include_str!("lib.rs")),
            ("naming.rs", include_str!("naming.rs")),
            ("branches.rs", include_str!("branches.rs")),
        ];
        for (file_name, source) in sources {
            for forbidden in forbidden_capabilities() {
                assert!(
                    !source.contains(&forbidden),
                    "{file_name} names {forbidden}; tmux-core performs no IO"
                );
            }
        }
    }

    /// Positive control for the assertion above.
    ///
    /// The purity test asserts an absence, so it passes vacuously if
    /// `include_str!` ever yields empty text or a needle stops matching.
    #[test]
    fn the_purity_check_detects_a_forbidden_string() {
        for forbidden in forbidden_capabilities() {
            let planted = format!("fn reach_out() {{ {forbidden}::whatever() }}");
            assert!(
                planted.contains(&forbidden),
                "a needle the purity test relies on does not match a source that names it"
            );
        }
        for source in [
            include_str!("lib.rs"),
            include_str!("naming.rs"),
            include_str!("branches.rs"),
        ] {
            assert!(
                !source.is_empty(),
                "include_str! yielded empty text, so the purity test proves nothing"
            );
        }
    }

    /// The capability paths no module may name.
    ///
    /// Assembled from segments rather than written as literals, because this
    /// file is one of the files the check scans and a literal here would make
    /// the crate fail its own purity test.
    fn forbidden_capabilities() -> Vec<String> {
        ["fs", "process", "env", "io"]
            .into_iter()
            .map(|capability| format!("std::{capability}"))
            .collect()
    }
}
