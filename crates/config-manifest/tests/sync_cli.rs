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

fn git_out(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?} failed");
    String::from_utf8(output.stdout)
        .expect("utf8")
        .trim()
        .to_string()
}

/// mac checked out with a changed shared file, a new shared file, a removed
/// shared file, and a changed per-branch file; linux at the base.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "-q", "-b", "linux"]);
    std::fs::write(
        dir.path().join(".sync-manifest"),
        ".sync-manifest\nshared.txt\ndir/\n~local.txt\n",
    )
    .expect("write");
    std::fs::write(dir.path().join("shared.txt"), "same\n").expect("write");
    std::fs::create_dir_all(dir.path().join("dir")).expect("mkdir");
    std::fs::write(dir.path().join("dir/gone.txt"), "only on linux\n").expect("write");
    std::fs::write(dir.path().join("local.txt"), "linux local\n").expect("write");
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "base"]);
    git(dir.path(), &["checkout", "-q", "-b", "mac"]);
    std::fs::write(dir.path().join("shared.txt"), "changed on mac\n").expect("write");
    std::fs::write(dir.path().join("dir/new.txt"), "new on mac\n").expect("write");
    std::fs::remove_file(dir.path().join("dir/gone.txt")).expect("rm");
    std::fs::write(dir.path().join("local.txt"), "mac local\n").expect("write");
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "mac changes"]);
    dir
}

fn sync(dir: &Path, args: &[&str]) -> assert_cmd::assert::Assert {
    Command::cargo_bin("config-manifest")
        .expect("binary built")
        .env("DOTFILES_ROOT", dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .arg("sync")
        .args(args)
        .assert()
}

fn check(dir: &Path, a: &str, b: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("config-manifest")
        .expect("binary built")
        .env("DOTFILES_ROOT", dir)
        .args(["check", a, b])
        .assert()
}

#[test]
fn dry_run_prints_the_plan_and_changes_nothing() {
    let dir = fixture();
    let before = git_out(dir.path(), &["rev-parse", "linux"]);
    let assert = sync(dir.path(), &["--dry-run"]).success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    assert!(stdout.contains("set shared.txt 100644 "), "{stdout}");
    assert!(stdout.contains("set dir/new.txt 100644 "), "{stdout}");
    assert!(stdout.contains("remove dir/gone.txt"), "{stdout}");
    assert!(!stdout.contains("local.txt"), "{stdout}");
    assert_eq!(git_out(dir.path(), &["rev-parse", "linux"]), before);
}

#[test]
fn sync_commits_onto_the_other_branch_and_leaves_the_worktree_alone() {
    let dir = fixture();
    let before = git_out(dir.path(), &["rev-parse", "linux"]);
    let assert = sync(dir.path(), &[]).success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    assert!(stdout.contains("config push origin mac"), "{stdout}");
    assert!(stdout.contains("config push origin linux"), "{stdout}");

    assert_eq!(
        git_out(dir.path(), &["rev-parse", "linux^"]),
        before,
        "one new commit on linux"
    );
    assert_eq!(
        git_out(dir.path(), &["log", "-1", "--format=%s", "linux"]),
        format!(
            "Sync shared paths from mac ({})",
            &git_out(dir.path(), &["rev-parse", "--short", "mac"])
        )
    );
    assert_eq!(
        git_out(dir.path(), &["symbolic-ref", "--short", "HEAD"]),
        "mac"
    );
    assert_eq!(git_out(dir.path(), &["status", "--porcelain"]), "");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("local.txt")).expect("read"),
        "mac local\n"
    );
    assert_eq!(
        git_out(dir.path(), &["show", "linux:local.txt"]),
        "linux local"
    );
    check(dir.path(), "mac", "linux").success();
}

#[test]
fn a_second_sync_has_nothing_to_do() {
    let dir = fixture();
    sync(dir.path(), &[]).success();
    let after_first = git_out(dir.path(), &["rev-parse", "linux"]);
    let assert = sync(dir.path(), &[]).success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    assert!(stdout.contains("nothing to sync"), "{stdout}");
    assert_eq!(git_out(dir.path(), &["rev-parse", "linux"]), after_first);
}

#[test]
fn a_branch_other_than_mac_or_linux_is_refused() {
    let dir = fixture();
    git(dir.path(), &["checkout", "-q", "-b", "feature"]);
    let assert = sync(dir.path(), &[]).code(1);
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8");
    assert!(stderr.contains("feature"), "{stderr}");
}

#[test]
fn a_bad_to_target_is_a_usage_error() {
    let dir = fixture();
    sync(dir.path(), &["--to", "mac"]).code(2);
    sync(dir.path(), &["--to", "feature"]).code(2);
    sync(dir.path(), &["--bogus"]).code(2);
}

#[test]
fn a_target_not_at_its_origin_is_refused() {
    let dir = fixture();
    let origin = tempfile::tempdir().expect("tempdir");
    assert!(
        Command::new("git")
            .args(["init", "-q", "--bare"])
            .arg(origin.path())
            .status()
            .expect("git")
            .success()
    );
    git(
        dir.path(),
        &[
            "remote",
            "add",
            "origin",
            origin.path().to_str().expect("path"),
        ],
    );
    git(dir.path(), &["push", "-q", "origin", "mac", "linux"]);
    git(dir.path(), &["checkout", "-q", "linux"]);
    std::fs::write(dir.path().join("dir/local-only.txt"), "not pushed\n").expect("write");
    git(dir.path(), &["add", "dir/local-only.txt"]);
    git(dir.path(), &["commit", "-q", "-m", "linux moved locally"]);
    git(dir.path(), &["checkout", "-q", "mac"]);
    let before = git_out(dir.path(), &["rev-parse", "linux"]);
    let assert = sync(dir.path(), &[]).code(1);
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8");
    assert!(stderr.contains("origin/linux"), "{stderr}");
    assert_eq!(git_out(dir.path(), &["rev-parse", "linux"]), before);
}

#[test]
fn a_target_checked_out_in_a_worktree_is_refused() {
    let dir = fixture();
    let worktree = tempfile::tempdir().expect("tempdir");
    git(
        dir.path(),
        &[
            "worktree",
            "add",
            "-q",
            worktree.path().to_str().expect("path"),
            "linux",
        ],
    );
    let before = git_out(dir.path(), &["rev-parse", "linux"]);

    let assert = sync(dir.path(), &[]).code(1);
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8");
    assert!(stderr.contains("worktree"), "{stderr}");
    assert!(stderr.contains("linux"), "{stderr}");
    assert_eq!(git_out(dir.path(), &["rev-parse", "linux"]), before);
    assert_eq!(git_out(worktree.path(), &["status", "--porcelain"]), "");
}

#[test]
fn an_unmatched_path_after_sync_exits_one_not_three() {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "-q", "-b", "linux"]);
    std::fs::write(
        dir.path().join(".sync-manifest"),
        ".sync-manifest\nshared.txt\ndir/\n~local.txt\n",
    )
    .expect("write");
    std::fs::write(dir.path().join("shared.txt"), "same\n").expect("write");
    std::fs::create_dir_all(dir.path().join("dir")).expect("mkdir");
    std::fs::write(dir.path().join("dir/gone.txt"), "only on linux\n").expect("write");
    std::fs::write(dir.path().join("local.txt"), "linux local\n").expect("write");
    std::fs::write(dir.path().join("stray.txt"), "not in the manifest\n").expect("write");
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "base"]);
    git(dir.path(), &["checkout", "-q", "-b", "mac"]);
    std::fs::write(dir.path().join("shared.txt"), "changed on mac\n").expect("write");
    std::fs::write(dir.path().join("dir/new.txt"), "new on mac\n").expect("write");
    std::fs::remove_file(dir.path().join("dir/gone.txt")).expect("rm");
    std::fs::write(dir.path().join("local.txt"), "mac local\n").expect("write");
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "mac changes"]);

    let before = git_out(dir.path(), &["rev-parse", "linux"]);
    let assert = sync(dir.path(), &[]).code(1);
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8");
    assert!(stderr.contains("unmatched"), "{stderr}");
    assert!(!stderr.contains("BUG"), "{stderr}");
    assert_eq!(
        git_out(dir.path(), &["rev-parse", "linux^"]),
        before,
        "the sync landed as one new commit"
    );
}

#[test]
fn a_detached_head_is_refused() {
    let dir = fixture();
    git(dir.path(), &["checkout", "-q", "--detach"]);
    let assert = sync(dir.path(), &[]).code(1);
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8");
    assert!(stderr.contains("detached"), "{stderr}");
}

#[test]
fn a_missing_manifest_on_the_source_is_refused() {
    let dir = fixture();
    git(dir.path(), &["rm", "-q", ".sync-manifest"]);
    git(dir.path(), &["commit", "-q", "-m", "drop manifest"]);
    let before = git_out(dir.path(), &["rev-parse", "linux"]);

    let assert = sync(dir.path(), &[]).code(1);
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8");
    assert!(stderr.contains(".sync-manifest"), "{stderr}");
    assert_eq!(git_out(dir.path(), &["rev-parse", "linux"]), before);
}

#[test]
fn a_dirty_shared_path_warns_but_syncs_committed_content() {
    let dir = fixture();
    std::fs::write(dir.path().join("shared.txt"), "uncommitted edit\n").expect("write");
    let assert = sync(dir.path(), &[]).success();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8");
    assert!(stderr.contains("uncommitted"), "{stderr}");
    assert_eq!(
        git_out(dir.path(), &["show", "linux:shared.txt"]),
        "changed on mac"
    );
}
