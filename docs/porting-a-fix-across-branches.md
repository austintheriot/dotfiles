# Porting a fix across branches

A shared test can reach a branch before the per-branch file it tests. The test
then fails on that branch, and the failure names a defect that was already
fixed somewhere else. This is the workflow for that case.

## How the failure looks

`config push origin linux` runs the suite in Docker and blocks the push:

```
[28/31] zshrc-git-aliases.test.sh ... FAIL (0s)
      FAIL: no per-alias git config --get runs at top level
            expected: [0]
            actual:   [11]
```

The worked example below is that exact failure. `tests/` is a shared path and
`~.zshrc` is a per-branch path, so a sync commit carried
`tests/zshrc-git-aliases.test.sh` from mac to linux while the `.zshrc` rewrite
it asserts stayed on mac.

## Why the branch matters more than the diagnosis

`tests/lib.sh` sets `DOTFILES_ROOT=${DOTFILES_ROOT:-$HOME}`, and the Docker
image copies the pushed ref into `/root`. The suite therefore reads the
**pushed branch's** copy of a per-branch file, not the working tree you are
sitting in. Reading `~/.zshrc` while checked out on mac shows the fixed file
and explains nothing.

Read the pushed branch's copy instead:

```sh
config show linux:.zshrc | awk '/^# GIT ALIASES/{f=1} f && /^# ENVIRONMENT-SPECIFIC/{exit} f'
```

## Steps

1. **Confirm the test is identical on both branches.** If it is, the test is
   not the defect and the per-branch file is.

   ```sh
   diff <(config show mac:tests/<suite>.test.sh) <(config show linux:tests/<suite>.test.sh)
   config rev-parse mac:.zshrc linux:.zshrc
   ```

2. **Diff the per-branch file and find the genuine divergence.** A per-branch
   file differs on purpose. Porting the whole file destroys that. In the
   worked example the deliberate differences were the Calibre PATH, the fzf
   fallback for older distro packages, `~/.zshrc-wsl` sourcing, and lazy pyenv
   init.

   ```sh
   config diff mac:.zshrc linux:.zshrc
   ```

3. **Splice only the section under test.** Find unique boundary lines and cut
   on them, rather than editing by hand.

   ```sh
   config show linux:.zshrc > linux.zshrc
   config show mac:.zshrc | awk '/^# GIT ALIASES/{f=1} f && /^# ENVIRONMENT-SPECIFIC/{exit} f' > block.txt
   grep -n '^# GIT ALIASES\|^# ENVIRONMENT-SPECIFIC' linux.zshrc   # -> 90, 151
   { sed -n '1,89p' linux.zshrc; cat block.txt; sed -n '151,$p' linux.zshrc; } > linux.zshrc.new
   ```

4. **Prove the splice touched nothing else.** Compare the prefix and the
   suffix, then grep for the branch-specific markers in both directions: the
   ones that must survive, and the ones that must not leak across.

   ```sh
   diff <(sed -n '1,89p' linux.zshrc) <(sed -n '1,89p' linux.zshrc.new)
   grep -q 'calibre.app' linux.zshrc.new && echo LEAKED
   zsh -n linux.zshrc.new
   ```

5. **Run the suite against the candidate before committing.** `DOTFILES_ROOT`
   is the override that makes this possible without checking the branch out.

   ```sh
   mkdir -p root && cp linux.zshrc.new root/.zshrc
   DOTFILES_ROOT=$PWD/root bash ~/tests/<suite>.test.sh
   ```

6. **Commit on the target branch in a worktree.** A worktree leaves the
   current branch checked out and its working tree untouched. Branches here
   are per-machine and are not meant to merge wholesale, so switching the
   checkout to commit a one-section port is the wrong tool.

   ```sh
   config worktree add /tmp/linux-wt linux
   ```

## Adding a doc while you are here

`.sync-manifest` requires every tracked file to match a rule, so a new file
under `docs/` that matches no rule fails `check-branch-drift.sh`. Only
`docs/research/` (shared) and `~docs/superpowers/` (per-branch) are covered.
Add the rule in the same commit as the file.

## What not to do

- Do not run the suite against `$HOME` and conclude the pushed branch is fine.
  The failing file is the one on the branch being pushed.
- Do not merge the per-branch file wholesale to make a test pass. The
  divergence is deliberate.
- Do not reach for `--no-verify`. The gate is reporting a real defect on the
  branch being pushed.
