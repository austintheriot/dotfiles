//! Everything inside this module knows `deps_core`. Nothing outside it does.

use std::process::ExitCode;

use crate::DepsVerb;

/// Run one `deps` verb.
///
/// Returns the exit code `deps_core::exit_status` decided, never one computed
/// here.
pub(crate) fn run(_verb: DepsVerb) -> ExitCode {
    eprintln!("config-cli: deps is not implemented yet");
    ExitCode::from(2)
}
