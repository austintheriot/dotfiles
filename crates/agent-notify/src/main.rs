#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
//! The agent-notify CLI. Hooks and agents call it. It tags tmux panes, sends
//! one request to the daemon, and falls back to a plain OS notification.

#[allow(dead_code)] // used from main in Task 17
mod cli;
#[allow(dead_code)] // used from main in Task 17
mod invocation;

fn main() {}
