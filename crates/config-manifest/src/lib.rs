#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
//! The git-sync and build-stamp half of the `config` command.
//!
//! Split pure from effectful along module lines rather than by convention:
//! `doctor`, `path`, and `stamp` are functions of their arguments, and `git`
//! is the one module that runs a subprocess. That split is what lets the
//! interesting cases ("one crate stale, one current, one orphaned") be table
//! tests instead of fixture repositories.

/// Diagnoses installed binaries against the stamps their sources would
/// produce. Pure; the caller supplies both sides of the comparison.
pub mod doctor;
/// The one module that shells out. Gathers the stamps the pure modules
/// compare, and is therefore the only place a process is spawned.
pub mod git;
/// Validated newtypes for the git object ids a stamp is built from.
pub mod path;
/// The pre-push stamp comparison, as a pure function over two lists.
pub mod stamp;
