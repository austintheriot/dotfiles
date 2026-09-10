#!/bin/bash
#
# A branch change renames tmux windows on the event, not on the next prompt.
#
# THE LAG THIS CLOSES. Nothing in tmux watches git; the only trigger was
# zsh's precmd, so a checkout in pane A renamed the window when pane A next
# drew a prompt, and a checkout made anywhere else (another pane, an editor,
# a script) never renamed the window showing that repo. A git post-checkout
# hook fires on the event itself.
#
# The hook is a tracked file, .scripts/git-hooks/post-checkout, installed per
# repository by `config install-repo-hooks [dir]` into the repository's
# COMMON git directory, so one install covers every worktree of that repo.
# Not a global core.hooksPath: that would replace ~/.cfg's own hooks, which
# githooks-installed.test.sh asserts against.
#
# tmux-tools is STUBBED on PATH here and records its argv. The hook's whole
# contract is "invoke the renamer with -a"; whether the renamer then names
# windows correctly is tmux-update-window-names.test.sh's question, and it
# would need a tmux server this suite has no business starting.
#
# Usage: ~/tests/git-post-checkout-hook.test.sh

. "$(dirname "$0")/lib.sh"

INSTALLER="$DOTFILES_ROOT/.scripts/config/config-install-repo-hooks"
HOOK="$DOTFILES_ROOT/.scripts/git-hooks/post-checkout"

assert_succeeds 'the hook script exists and is executable' test -x "$HOOK"
assert_succeeds 'the installer exists and is executable' test -x "$INSTALLER"
assert_succeeds 'the hook is POSIX sh, so dash parses it' dash -n "$HOOK"

# A stub renamer first on PATH. The hook backgrounds the real call, so the
# stub writes synchronously and the assertions poll briefly for the record.
stub_dir="$FIXTURES/stub-bin"; mkdir -p "$stub_dir"
record="$FIXTURES/tmux-tools.argv"
cat > "$stub_dir/tmux-tools" <<STUB
#!/bin/sh
printf '%s\n' "\$*" >> "$record"
STUB
chmod +x "$stub_dir/tmux-tools"

wait_for_record() {
    local tries=0
    while [ ! -s "$record" ] && [ "$tries" -lt 50 ]; do sleep 0.1; tries=$((tries + 1)); done
}

# --- install into a repo, then check out a branch ---------------------------
repo=$(make_repo hooked)
assert_succeeds 'the installer accepts a repository' "$INSTALLER" "$repo"
assert_succeeds 'the hook is linked into the common git directory' \
    test -L "$repo/.git/hooks/post-checkout"
assert_equals 'the link points at the tracked hook' "$HOOK" \
    "$(readlink "$repo/.git/hooks/post-checkout")"

PATH="$stub_dir:$PATH" git -C "$repo" checkout -q -b feature
wait_for_record
assert_equals 'a checkout invokes the renamer for every window' \
    'name-windows -a' "$(head -1 "$record" 2>/dev/null)"

# --- idempotent -------------------------------------------------------------
assert_succeeds 'installing again succeeds' "$INSTALLER" "$repo"
assert_equals 'installing again leaves one link, not a copy or an error' "$HOOK" \
    "$(readlink "$repo/.git/hooks/post-checkout")"

# --- a worktree shares the install ------------------------------------------
worktree=$(make_worktree "$repo" wt feature-two 2>/dev/null || true)
if [ -n "$worktree" ] && [ -d "$worktree" ]; then
    : > "$record"
    PATH="$stub_dir:$PATH" git -C "$worktree" checkout -q -b feature-three
    wait_for_record
    assert_equals 'a checkout in a worktree fires the same hook' \
        'name-windows -a' "$(head -1 "$record" 2>/dev/null)"
else
    skip 'a checkout in a worktree fires the same hook (no worktree helper)'
fi

# --- refusals -----------------------------------------------------------------
not_a_repo="$FIXTURES/plain-dir"; mkdir -p "$not_a_repo"
"$INSTALLER" "$not_a_repo" >/dev/null 2>&1
assert_equals 'a directory that is not a repository is refused with the usage code' \
    '2' "$?"

finish
