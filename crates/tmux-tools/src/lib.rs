#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
//! The parts of tmux-tools that other crates reuse. Today that is the git
//! inspector, which thumper uses to label a notification from outside
//! tmux with the same repository name a window would show.

pub mod repo;
