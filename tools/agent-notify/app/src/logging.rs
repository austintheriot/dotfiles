//! Daily log files, seven kept, in a private directory.

use std::fs::DirBuilder;
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::path::Path;

use tracing::level_filters::LevelFilter;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};

/// The level from `$AGENT_NOTIFY_LOG`. Unknown values read as `info`.
#[must_use]
#[cfg_attr(not(test), expect(dead_code, reason = "wired in Task 16"))]
pub fn level_from(value: Option<&str>) -> LevelFilter {
    match value.map(str::to_ascii_lowercase).as_deref() {
        Some("error") => LevelFilter::ERROR,
        Some("warn") => LevelFilter::WARN,
        Some("debug") => LevelFilter::DEBUG,
        Some("trace") => LevelFilter::TRACE,
        _ => LevelFilter::INFO,
    }
}

/// Starts logging to `directory/agent-notifyd.<date>.log`. Keep the guard
/// alive until exit, or the last lines are lost.
/// The caller must drop the guard only after the daemon stops, because
/// `app.run` can exit the process without returning.
///
/// # Errors
///
/// When the directory cannot be created or the appender cannot open a file.
#[expect(dead_code, reason = "wired in Task 16")]
pub fn init(directory: &Path, level: LevelFilter) -> io::Result<WorkerGuard> {
    prepare_directory(directory)?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("agent-notifyd")
        .filename_suffix("log")
        .max_log_files(7)
        .build(directory)
        .map_err(io::Error::other)?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt().with_writer(writer).with_ansi(false).with_max_level(level).init();
    Ok(guard)
}

/// Creates `directory` and its missing parents at 0700, as the CLI does. An
/// existing directory keeps its mode: this code did not create it.
fn prepare_directory(directory: &Path) -> io::Result<()> {
    DirBuilder::new().recursive(true).mode(0o700).create(directory)
}

/// Logs a panic's location before the default hook runs. The payload can
/// hold model text from a request, so it is logged only at `debug`, escaped.
#[expect(dead_code, reason = "wired in Task 16")]
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info.location().map_or_else(|| "unknown".to_owned(), ToString::to_string);
        tracing::error!(%location, "agent-notifyd panicked");
        tracing::debug!(payload = ?info.payload_as_str(), "panic payload");
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use std::fs::{self, Permissions};
    use std::os::unix::fs::PermissionsExt;

    use tracing::level_filters::LevelFilter;

    use super::*;

    #[test]
    fn the_level_reads_the_variable_and_defaults_to_info() {
        assert_eq!(level_from(None), LevelFilter::INFO);
        assert_eq!(level_from(Some("debug")), LevelFilter::DEBUG);
        assert_eq!(level_from(Some("TRACE")), LevelFilter::TRACE);
        assert_eq!(level_from(Some("loud")), LevelFilter::INFO);
    }

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).expect("the directory exists").permissions().mode() & 0o777
    }

    #[test]
    fn a_created_log_directory_is_private() {
        let parent = tempfile::tempdir().expect("a temporary directory");
        let directory = parent.path().join("Logs").join("agent-notify");
        prepare_directory(&directory).expect("the directory is created");
        assert_eq!(mode(&directory), 0o700);
    }

    #[test]
    fn an_existing_log_directory_keeps_its_mode() {
        let parent = tempfile::tempdir().expect("a temporary directory");
        let directory = parent.path().join("agent-notify");
        fs::create_dir(&directory).expect("the directory is created");
        fs::set_permissions(&directory, Permissions::from_mode(0o755)).expect("the mode is set");
        prepare_directory(&directory).expect("an existing directory is accepted");
        assert_eq!(mode(&directory), 0o755);
    }
}
