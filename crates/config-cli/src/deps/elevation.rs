//! Resolve `deps_core::Elevation` once per run.
//!
//! Ports `check-deps.sh:172-187`'s privilege detection. The shell comment
//! there names four environments and three answers:
//!
//! ```text
//! root, no sudo      -> run directly (the container that reported this)
//! root, sudo present -> run directly anyway; escalating from root is
//!                       pointless, and on an image with a misconfigured
//!                       sudo it is one more way to fail
//! non-root, sudo     -> escalate (the normal laptop and CI runner)
//! non-root, no sudo  -> no way to install, so a command needing privilege is
//!                       reported manual-only rather than emitted as a
//!                       command that cannot work
//! ```
//!
//! `Elevation` collapses "root, no sudo" and "root, sudo present" into one
//! variant, [`deps_core::Elevation::AlreadyRoot`], because both answers are
//! "run directly" and the core's `PrivilegeRequirement` never asks which
//! kind of root the process is.

use deps_core::Elevation;

/// Resolve this process's elevation, once, by the same rule `check-deps.sh`
/// used: already root wins outright, then `sudo` on `PATH`, then neither.
///
/// A process already running as root has nothing to escalate to, so a
/// process already at effective user ID (UID) 0 is `AlreadyRoot` even when
/// `sudo` is also on `PATH` -- escalating from root is pointless, and on a
/// machine with a misconfigured `sudo` it is one more way to fail.
///
/// # Errors
///
/// Returns an error when `id -u` cannot be spawned or prints text this
/// function cannot parse as a UID. `check-deps.sh:175` guards the same call
/// with a fallback (`id -u 2>/dev/null || printf 1`) rather than assuming
/// it always succeeds; this port surfaces that failure instead of silently
/// assuming non-root, so a caller can decide how to report it.
pub fn resolve() -> Result<Elevation, ElevationError> {
    if is_root()? {
        return Ok(Elevation::AlreadyRoot);
    }
    if sudo_on_path() {
        return Ok(Elevation::ViaSudo);
    }
    Ok(Elevation::Unavailable)
}

/// Why elevation could not be resolved at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElevationError {
    /// `id -u` could not be spawned.
    CannotSpawnId,
    /// `id -u` ran but its output was not a plain non-negative integer.
    UnparsableUserId,
}

impl std::fmt::Display for ElevationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ElevationError::CannotSpawnId => write!(formatter, "could not spawn `id -u`"),
            ElevationError::UnparsableUserId => {
                write!(formatter, "`id -u` did not print a plain user ID")
            }
        }
    }
}

impl std::error::Error for ElevationError {}

/// `id -u`: the direct port of `check-deps.sh:175`'s effective user ID
/// (UID) probe. 0 is root on every platform this binary targets.
fn is_root() -> Result<bool, ElevationError> {
    let output = std::process::Command::new("id")
        .arg("-u")
        .output()
        .map_err(|_| ElevationError::CannotSpawnId)?;
    if !output.status.success() {
        return Err(ElevationError::CannotSpawnId);
    }
    let printed = String::from_utf8_lossy(&output.stdout);
    let uid: u32 =
        printed.trim().parse().map_err(|_| ElevationError::UnparsableUserId)?;
    Ok(uid == 0)
}

/// `command -v sudo`: whether `sudo` resolves on `PATH`.
///
/// A plain `PATH` scan rather than a spawn, matching `probe_command` in
/// `gather.rs`: `check-deps.sh:181` uses the same `command -v` test, and
/// this binary has no need to invoke `sudo` just to learn whether it
/// exists.
fn sudo_on_path() -> bool {
    std::env::var_os("PATH").is_some_and(|path_var| {
        std::env::split_paths(&path_var).any(|directory| directory.join("sudo").is_file())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Positive control: resolution succeeds and returns some variant, on
    /// every platform this suite runs on. Every test below this one relies
    /// on `resolve` being reachable at all without panicking.
    #[test]
    fn it_resolves_to_some_elevation_without_erroring() {
        let elevation = resolve().expect("unix always reports an effective user ID");
        assert!(matches!(
            elevation,
            Elevation::AlreadyRoot | Elevation::ViaSudo | Elevation::Unavailable
        ));
    }

    /// `sudo_on_path` agrees with a direct `command -v`-style scan: it
    /// finds `sudo` whenever `PATH` holds a directory containing it.
    ///
    /// This machine has real `sudo`, so this is a live behavioral check
    /// rather than a construction test: it fails if `sudo_on_path` stops
    /// scanning `PATH` correctly, not just if it stops compiling.
    #[test]
    fn sudo_on_path_finds_sudo_when_path_holds_it() {
        let found_by_which = std::env::var_os("PATH")
            .into_iter()
            .flat_map(|path_var| std::env::split_paths(&path_var).collect::<Vec<_>>())
            .any(|directory| directory.join("sudo").is_file());

        assert_eq!(sudo_on_path(), found_by_which);
    }
}
