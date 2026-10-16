//! The Tauri-free shell of `agent-notifyd`. The core crate decides; this
//! crate reads the world, applies effects and serves the socket. Every
//! platform service arrives through [`services`].

pub mod aerospace;
pub mod child;
pub mod environment;
pub mod presence;
pub mod services;
pub mod tools;

#[cfg(test)]
pub(crate) mod test_support;
