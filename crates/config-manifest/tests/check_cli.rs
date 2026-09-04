use std::path::Path;
use std::process::Command;

use assert_cmd::prelude::*;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.email=t@t", "-c", "user.name=t"])
        .args(args)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} failed");
}

/// A repo with a linux branch and a mac branch, both from the same manifest
/// and the same shared content; the same shape check-branch-drift.test.sh uses.
fn manifest_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "-q", "-b", "linux"]);
    std::fs::write(
        dir.path().join(".sync-manifest"),
        "# comment\n\n.sync-manifest\nshared.txt\n",
    )
    .expect("write");
    std::fs::write(dir.path().join("shared.txt"), "same content\n").expect("write");
    git(dir.path(), &["add", ".sync-manifest", "shared.txt"]);
    git(dir.path(), &["commit", "-q", "-m", "add manifest"]);
    git(dir.path(), &["branch", "mac"]);
    dir
}

fn check(dir: &Path, refs: &[&str]) -> assert_cmd::assert::Assert {
    Command::cargo_bin("config-manifest")
        .expect("binary built")
        .env("DOTFILES_ROOT", dir)
        .arg("check")
        .args(refs)
        .assert()
}

#[test]
fn matching_paths_exit_zero_and_report_the_count() {
    let dir = manifest_repo();
    check(dir.path(), &["mac", "linux"])
        .success()
        .stdout("check-branch-drift: mac and linux match on all 2 shared path(s)\n")
        .stderr("");
}

#[test]
fn a_diverged_path_exits_one_and_names_it() {
    let dir = manifest_repo();
    git(dir.path(), &["checkout", "-q", "mac"]);
    std::fs::write(dir.path().join("shared.txt"), "different content\n").expect("write");
    git(dir.path(), &["add", "shared.txt"]);
    git(dir.path(), &["commit", "-q", "-m", "diverge on mac"]);
    check(dir.path(), &["mac", "linux"])
        .code(1)
        .stdout("diverged: shared.txt\n")
        .stderr("\ncheck-branch-drift: mac and linux diverged on 1 of 2 shared path(s)\n");
}

#[test]
fn a_missing_manifest_exits_one_and_names_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "-q", "-b", "linux"]);
    git(dir.path(), &["commit", "-q", "--allow-empty", "-m", "init"]);
    git(dir.path(), &["branch", "mac"]);
    check(dir.path(), &["mac", "linux"])
        .code(1)
        .stderr("check-branch-drift: could not read .sync-manifest from mac\n");
}

#[test]
fn an_unmatched_file_reports_the_branch_it_is_on() {
    let dir = manifest_repo();
    git(dir.path(), &["checkout", "-q", "mac"]);
    std::fs::write(dir.path().join("brand-new.txt"), "brand new\n").expect("write");
    git(dir.path(), &["add", "brand-new.txt"]);
    git(dir.path(), &["commit", "-q", "-m", "add unlabeled file"]);
    let assert = check(dir.path(), &["mac", "linux"]).code(1);
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8");
    assert!(stderr.contains("brand-new.txt"));
    assert!(stderr.contains("(on mac)"));
    assert!(stderr.contains("match no .sync-manifest rule"));
}

#[test]
fn too_many_arguments_is_a_usage_error() {
    let dir = manifest_repo();
    check(dir.path(), &["a", "b", "c"]).code(2);
}

#[test]
fn works_against_a_bare_repo_at_root_dot_cfg() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = dir.path().join(".cfg");
    assert!(
        Command::new("git")
            .args(["init", "-q", "--bare", "-b", "linux"])
            .arg(&cfg)
            .status()
            .expect("git")
            .success()
    );
    let bare = |args: &[&str]| {
        let status = Command::new("git")
            .arg(format!("--git-dir={}", cfg.display()))
            .arg(format!("--work-tree={}", dir.path().display()))
            .args(["-c", "user.email=t@t", "-c", "user.name=t"])
            .args(args)
            .current_dir(dir.path())
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?} failed");
    };
    std::fs::write(dir.path().join(".sync-manifest"), ".sync-manifest\n").expect("write");
    bare(&["add", ".sync-manifest"]);
    bare(&["commit", "-q", "-m", "init"]);
    bare(&["branch", "mac"]);
    check(dir.path(), &["mac", "linux"])
        .success()
        .stdout("check-branch-drift: mac and linux match on all 1 shared path(s)\n");
}
