import os
import subprocess
import tempfile
import unittest

import claude_rm_scratch as rs


def make_world():
    base = os.path.realpath(tempfile.mkdtemp())
    code_root = os.path.join(base, "code")
    scratch_root = os.path.join(base, "claude-501")
    repo = os.path.join(code_root, "repo")
    os.makedirs(os.path.join(repo, "src"))
    os.makedirs(scratch_root)
    subprocess.run(["git", "init", "-q", repo], check=True)
    with open(os.path.join(repo, ".gitignore"), "w") as handle:
        handle.write(".superpowers/\n")
    return {"base": base, "code": code_root, "scratch": scratch_root, "repo": repo}


def touch(path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as handle:
        handle.write("x")
    return path


def commit(repo, path):
    subprocess.run(["git", "-C", repo, "add", "-f", path], check=True)
    subprocess.run(
        ["git", "-C", repo, "-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "t"],
        check=True,
    )


def check(world, path):
    return rs.check_target(path, code_roots=[world["code"]], scratch_root=world["scratch"])


class AllowedTargets(unittest.TestCase):
    def test_untracked_probe_file_is_allowed(self):
        world = make_world()
        path = touch(os.path.join(world["repo"], "src", "probe-web962.test.ts"))
        self.assertIsNone(check(world, path))

    def test_untracked_test_file_is_allowed(self):
        world = make_world()
        path = touch(os.path.join(world["repo"], "src", "scratch.spec.ts"))
        self.assertIsNone(check(world, path))

    def test_untracked_temp_file_is_allowed(self):
        world = make_world()
        path = touch(os.path.join(world["repo"], "notes.tmp"))
        self.assertIsNone(check(world, path))

    def test_superpowers_workspace_directory_is_allowed(self):
        world = make_world()
        touch(os.path.join(world["repo"], ".superpowers", "sdd", "plan", "progress.md"))
        self.assertIsNone(check(world, os.path.join(world["repo"], ".superpowers", "sdd", "plan")))

    def test_anything_in_the_scratchpad_is_allowed(self):
        world = make_world()
        touch(os.path.join(world["scratch"], "session", "out", "a.json"))
        self.assertIsNone(check(world, os.path.join(world["scratch"], "session", "out")))


class RefusedTargets(unittest.TestCase):
    def test_tracked_test_file_is_refused(self):
        world = make_world()
        path = touch(os.path.join(world["repo"], "src", "real.test.ts"))
        commit(world["repo"], path)
        self.assertIn("tracked", check(world, path))

    def test_superpowers_directory_holding_a_tracked_file_is_refused(self):
        world = make_world()
        path = touch(os.path.join(world["repo"], ".superpowers", "sdd", "plan", "kept.md"))
        commit(world["repo"], path)
        self.assertIn("tracked", check(world, os.path.dirname(path)))

    def test_ordinary_source_file_is_refused(self):
        world = make_world()
        path = touch(os.path.join(world["repo"], "src", "index.ts"))
        self.assertIn("not a probe", check(world, path))

    def test_ordinary_directory_is_refused(self):
        world = make_world()
        os.makedirs(os.path.join(world["repo"], "tmp-build"))
        self.assertIn("directory", check(world, os.path.join(world["repo"], "tmp-build")))

    def test_superpowers_root_itself_is_refused(self):
        world = make_world()
        touch(os.path.join(world["repo"], ".superpowers", "sdd", "x.md"))
        self.assertIn("root", check(world, os.path.join(world["repo"], ".superpowers")))

    def test_scratch_root_itself_is_refused(self):
        world = make_world()
        self.assertIn("root", check(world, world["scratch"]))

    def test_path_outside_allowed_roots_is_refused(self):
        world = make_world()
        path = touch(os.path.join(world["base"], "elsewhere", "probe-x.ts"))
        self.assertIn("outside", check(world, path))

    def test_symlink_is_refused(self):
        world = make_world()
        target = touch(os.path.join(world["repo"], "src", "index.ts"))
        link = os.path.join(world["repo"], "src", "probe-link.ts")
        os.symlink(target, link)
        self.assertIn("symlink", check(world, link))

    def test_dotdot_escape_is_refused(self):
        world = make_world()
        touch(os.path.join(world["base"], "elsewhere", "probe-x.ts"))
        path = os.path.join(world["code"], "..", "elsewhere", "probe-x.ts")
        self.assertIn("outside", check(world, path))

    def test_missing_path_is_refused(self):
        world = make_world()
        self.assertIn("does not exist", check(world, os.path.join(world["repo"], "probe-gone.ts")))


class RemoveAll(unittest.TestCase):
    def test_one_refusal_deletes_nothing(self):
        world = make_world()
        probe = touch(os.path.join(world["repo"], "src", "probe-a.ts"))
        source = touch(os.path.join(world["repo"], "src", "index.ts"))
        refusals = rs.remove_all([probe, source], code_roots=[world["code"]], scratch_root=world["scratch"], dry_run=False)
        self.assertEqual(len(refusals), 1)
        self.assertTrue(os.path.exists(probe))

    def test_dry_run_deletes_nothing(self):
        world = make_world()
        probe = touch(os.path.join(world["repo"], "src", "probe-a.ts"))
        refusals = rs.remove_all([probe], code_roots=[world["code"]], scratch_root=world["scratch"], dry_run=True)
        self.assertEqual(refusals, [])
        self.assertTrue(os.path.exists(probe))

    def test_allowed_targets_are_deleted(self):
        world = make_world()
        probe = touch(os.path.join(world["repo"], "src", "probe-a.ts"))
        workspace = os.path.dirname(touch(os.path.join(world["repo"], ".superpowers", "sdd", "p", "a.md")))
        refusals = rs.remove_all([probe, workspace], code_roots=[world["code"]], scratch_root=world["scratch"], dry_run=False)
        self.assertEqual(refusals, [])
        self.assertFalse(os.path.exists(probe))
        self.assertFalse(os.path.exists(workspace))


if __name__ == "__main__":
    unittest.main()
