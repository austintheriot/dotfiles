//! Best-effort file logging. A failure here has nowhere to be reported, and
//! it must never stop a hook, so every error ends logging quietly.

use std::fs::{DirBuilder, OpenOptions};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;
use std::sync::Mutex;

use tracing::level_filters::LevelFilter;

/// The CLI log's file name inside the log directory.
pub const LOG_FILE: &str = "cli.log";

/// Appends to `<directory>/cli.log` at `level` (default `info`). The CLI
/// never rotates: concurrent hooks would race on a rename.
pub fn init(directory: Option<&Path>, level: Option<&str>) {
    let Some(directory) = directory else { return };
    let level = level.and_then(|text| text.parse::<LevelFilter>().ok()).unwrap_or(LevelFilter::INFO);
    if DirBuilder::new().recursive(true).mode(0o700).create(directory).is_err() {
        return;
    }
    let Ok(file) = OpenOptions::new().create(true).append(true).mode(0o600).open(directory.join(LOG_FILE)) else {
        return;
    };
    let subscriber = tracing_subscriber::fmt().with_writer(Mutex::new(file)).with_ansi(false).with_max_level(level).finish();
    let _already_set = tracing::subscriber::set_global_default(subscriber);
}
