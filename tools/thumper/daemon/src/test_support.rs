use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const WARM_UP: &str = "THUMPER_STUB_WARM_UP";

/// macOS assesses a freshly written executable on its first exec, about
/// 200 ms each and serialized across processes, which breaks a 1 s limit
/// under a parallel test run. One untimed warm-up exec pays that cost here.
pub(crate) fn stub_executable(directory: &Path, name: &str, body: &str) -> PathBuf {
    let path = directory.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n[ -n \"${WARM_UP}\" ] && exit 0\n{body}\n")).expect("the stub is writable");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("the stub is executable");
    let warmed = Command::new(&path).env(WARM_UP, "1").status().expect("the stub runs");
    assert!(warmed.success(), "the warm-up exec of {} failed", path.display());
    path
}
