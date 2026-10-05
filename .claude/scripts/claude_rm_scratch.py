#!/usr/bin/env python3
"""Delete Claude's own scratch artifacts: probe, test, and temp files, superpowers workspaces,
and session scratchpad contents. Refuses anything git tracks, anything outside the code and
scratchpad roots, symlinks, and every other file. Validates all targets before deleting any.

Usage: claude_rm_scratch.py [--dry-run] PATH...
"""
import fnmatch
import os
import shutil
import subprocess
import sys

CODE_ROOTS = [os.path.realpath(os.path.expanduser("~/Documents/code"))]
SCRATCH_ROOT = os.path.realpath(f"/tmp/claude-{os.getuid()}")
SUPERPOWERS_DIRS = (".superpowers", os.path.join("docs", "superpowers"))
SCRATCH_FILE_PATTERNS = (
    "probe-*", "*.probe.*",
    "*.test.*", "*.spec.*", "*_test.*", "test_*",
    "tmp-*", "temp-*", "*.tmp",
)


def _is_within(path, root):
    return path != root and path.startswith(root + os.sep)


def _superpowers_root(path):
    parts = path.split(os.sep)
    for marker in SUPERPOWERS_DIRS:
        marker_parts = marker.split(os.sep)
        for i in range(len(parts) - len(marker_parts) + 1):
            if parts[i:i + len(marker_parts)] == marker_parts:
                return os.sep.join(parts[:i + len(marker_parts)])
    return None


def _tracked_files(path):
    directory = path if os.path.isdir(path) else os.path.dirname(path)
    toplevel = subprocess.run(
        ["git", "-C", directory, "rev-parse", "--show-toplevel"],
        capture_output=True, text=True,
    )
    if toplevel.returncode != 0:
        return []
    listed = subprocess.run(
        ["git", "-C", toplevel.stdout.strip(), "ls-files", "--", path],
        capture_output=True, text=True, check=True,
    )
    return [line for line in listed.stdout.splitlines() if line]


def check_target(path, code_roots, scratch_root):
    """Return None when `path` is safe to delete, otherwise the reason it is refused."""
    absolute = os.path.abspath(path)
    if os.path.islink(absolute):
        return f"{path}: is a symlink"
    if not os.path.exists(absolute):
        return f"{path}: does not exist"
    real = os.path.realpath(absolute)

    if real == scratch_root or _is_within(real, scratch_root):
        return None if _is_within(real, scratch_root) else f"{path}: is the scratchpad root"

    if not any(_is_within(real, root) for root in code_roots):
        return f"{path}: is outside the code and scratchpad roots"

    tracked = _tracked_files(real)
    if tracked:
        return f"{path}: {len(tracked)} tracked file(s) here; use git rm"

    superpowers_root = _superpowers_root(real)
    if superpowers_root is not None:
        return None if _is_within(real, superpowers_root) else f"{path}: is a superpowers root"

    if os.path.isdir(real):
        return f"{path}: is a directory outside a superpowers workspace or the scratchpad"
    name = os.path.basename(real)
    if not any(fnmatch.fnmatch(name, pattern) for pattern in SCRATCH_FILE_PATTERNS):
        return f"{path}: is not a probe, test, or temp file"
    return None


def remove_all(paths, code_roots, scratch_root, dry_run):
    """Delete every path only when all of them pass `check_target`. Returns the refusals."""
    refusals = [reason for reason in (check_target(path, code_roots, scratch_root) for path in paths) if reason]
    if refusals or dry_run:
        return refusals
    for path in paths:
        real = os.path.realpath(os.path.abspath(path))
        if os.path.isdir(real):
            shutil.rmtree(real)
        else:
            os.remove(real)
    return []


def main(argv):
    dry_run = "--dry-run" in argv
    paths = [arg for arg in argv if arg != "--dry-run"]
    if not paths:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    refusals = remove_all(paths, CODE_ROOTS, SCRATCH_ROOT, dry_run)
    for reason in refusals:
        print(f"refused: {reason}", file=sys.stderr)
    if refusals:
        print("nothing deleted", file=sys.stderr)
        return 1
    for path in paths:
        print(f"{'would delete' if dry_run else 'deleted'}: {os.path.abspath(path)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
