//! The parts of tmux-tools that other crates reuse. Today that is the git
//! inspector, which agent-notify uses to label a notification from outside
//! tmux with the same repository name a window would show.

pub mod repo;
