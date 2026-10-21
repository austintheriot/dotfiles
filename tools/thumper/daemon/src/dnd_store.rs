//! Do-not-disturb on disk, so it survives a restart. A corrupt or
//! half-written file reads as off and is logged: a daemon that crashes on
//! it would crash-loop under launchd.

use std::fs;
use std::io;
use std::path::PathBuf;

use thumper_core::EpochMs;
use thumper_core::state::Dnd;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DndFile {
    on: bool,
    until_ms: Option<i64>,
}

/// Parses the file's text.
///
/// # Errors
///
/// The JSON error for text that is not a DND record.
pub fn dnd_from_text(text: &str) -> Result<Dnd, serde_json::Error> {
    let file: DndFile = serde_json::from_str(text)?;
    Ok(if file.on { Dnd::On { until: file.until_ms.map(EpochMs) } } else { Dnd::Off })
}

/// The file's text.
#[must_use]
pub fn dnd_to_text(dnd: &Dnd) -> String {
    let file = match dnd {
        Dnd::Off => DndFile { on: false, until_ms: None },
        Dnd::On { until } => DndFile { on: true, until_ms: until.map(|until| until.0) },
    };
    serde_json::to_string(&file).unwrap_or_else(|_| "{\"on\":false,\"until_ms\":null}".to_string())
}

/// The DND file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DndStore {
    path: PathBuf,
}

impl DndStore {
    /// The store at `path`.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        DndStore { path }
    }

    /// The saved value, or `Off` when the file is missing or unreadable.
    #[must_use]
    pub fn load(&self) -> Dnd {
        match fs::read_to_string(&self.path) {
            Ok(text) => dnd_from_text(&text).unwrap_or_else(|error| {
                tracing::error!(%error, path = %self.path.display(), "the DND file is corrupt; starting with DND off");
                Dnd::Off
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Dnd::Off,
            Err(error) => {
                tracing::error!(%error, path = %self.path.display(), "the DND file is unreadable; starting with DND off");
                Dnd::Off
            }
        }
    }

    /// Writes `dnd` to a temporary file, syncs it to disk, then renames it
    /// over the old one. A failed write or rename removes the temporary file.
    ///
    /// # Errors
    ///
    /// Any filesystem error.
    pub fn save(&self, dnd: &Dnd) -> io::Result<()> {
        if let Some(directory) = self.path.parent() {
            fs::create_dir_all(directory)?;
        }
        let temporary = self.path.with_extension("json.tmp");
        let written = write_synced(&temporary, dnd_to_text(dnd).as_bytes()).and_then(|()| fs::rename(&temporary, &self.path));
        if written.is_err()
            && let Err(error) = fs::remove_file(&temporary)
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(%error, path = %temporary.display(), "the temporary DND file was not removed");
        }
        written
    }
}

// Without the sync, APFS can apply the rename before the data after a power loss, which leaves an empty file.
fn write_synced(path: &std::path::Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::File::create(path)?;
    io::Write::write_all(&mut file, bytes)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use thumper_core::EpochMs;
    use thumper_core::state::Dnd;

    use super::*;

    #[test]
    fn dnd_round_trips_through_text() {
        for dnd in [Dnd::Off, Dnd::On { until: None }, Dnd::On { until: Some(EpochMs(1_760_500_000_000)) }] {
            assert_eq!(dnd_from_text(&dnd_to_text(&dnd)).expect("parses"), dnd);
        }
    }

    #[test]
    fn a_missing_file_reads_as_off() {
        let directory = tempfile::tempdir().expect("a directory");
        assert_eq!(DndStore::new(directory.path().join("dnd.json")).load(), Dnd::Off);
    }

    #[test]
    fn a_corrupt_dnd_file_reads_as_off() {
        let directory = tempfile::tempdir().expect("a directory");
        let path = directory.path().join("dnd.json");
        for text in ["", "{\"on\": tru", "[]", "{\"on\": true, \"until_ms\": \"soon\"}"] {
            fs::write(&path, text).expect("writes");
            assert_eq!(DndStore::new(path.clone()).load(), Dnd::Off, "{text:?}");
        }
    }

    #[test]
    fn save_then_load_keeps_the_value_and_leaves_no_temp_file() {
        let directory = tempfile::tempdir().expect("a directory");
        let store = DndStore::new(directory.path().join("state/dnd.json"));
        let dnd = Dnd::On { until: Some(EpochMs(5)) };
        store.save(&dnd).expect("saves");
        assert_eq!(store.load(), dnd);
        let names: Vec<String> =
            fs::read_dir(directory.path().join("state")).expect("lists").flatten().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["dnd.json".to_string()]);
    }

    #[test]
    fn a_failed_rename_leaves_no_temp_file() {
        let directory = tempfile::tempdir().expect("a directory");
        let blocking = directory.path().join("dnd.json");
        fs::create_dir(&blocking).expect("a directory where the file goes");
        fs::write(blocking.join("keep"), "x").expect("writes");
        assert!(DndStore::new(blocking).save(&Dnd::On { until: None }).is_err(), "a rename over a directory fails");
        let names: Vec<String> = fs::read_dir(directory.path()).expect("lists").flatten().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["dnd.json".to_string()]);
    }
}
