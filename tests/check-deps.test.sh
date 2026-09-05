#!/bin/bash
#
# Tests for check-deps.sh: dependency presence checking and the --fix
# install flow. Package managers and installable tools are stubbed with fake
# executables on a fixture PATH rather than touching the real system --
# this script's whole job is deciding *which* command to run and whether the
# result satisfies the check, and running a real apt-get/brew/pacman here
# would either fail (no root, no such tool) or mutate the machine running
# the tests.
#
# Usage: ~/tests/check-deps.test.sh

. "$(dirname "$0")/lib.sh"

SCRIPT="$DOTFILES_ROOT/.scripts/deps/check-deps.sh"
BIN="$FIXTURES/bin"
mkdir -p "$BIN"

cat > "$BIN/sudo" <<'EOF'
#!/bin/sh
exec "$@"
EOF
chmod +x "$BIN/sudo"

cat > "$BIN/apt-get" <<EOF
#!/bin/sh
printf 'apt-get %s\n' "\$*" >> "$FIXTURES/apt.log"
exit 0
EOF
chmod +x "$BIN/apt-get"

cat > "$BIN/some-tool" <<'EOF'
#!/bin/sh
exit 0
EOF
chmod +x "$BIN/some-tool"

run_check() {
    local conf=$1; shift
    PATH="$BIN:$PATH" DEPS_CONF="$conf" DEPS_LOCAL_CONF="$FIXTURES/no-such-local.conf" \
        "$SCRIPT" "$@"
}

# --- everything present ---------------------------------------------------

conf="$FIXTURES/deps-present.conf"
printf 'some-tool|command -v some-tool|https://example.invalid\n' > "$conf"

output=$(run_check "$conf")
status=$?
assert_equals 'all present exits 0' '0' "$status"
assert_contains 'reports the count' 'all 1 dependencies present' "$output"

# --- a missing dependency, no --fix ---------------------------------------

conf="$FIXTURES/deps-missing.conf"
printf 'nonexistent-tool|command -v nonexistent-tool|https://example.invalid\n' > "$conf"

output=$(run_check "$conf")
status=$?
assert_equals 'a missing dep exits 1 without --fix' '1' "$status"
assert_contains 'names the missing dep' 'missing: nonexistent-tool' "$output"

# --- --fix --yes runs the default package-manager install, verifies it ---

: > "$FIXTURES/apt.log"
marker="$FIXTURES/installed-marker"
rm -f "$marker"
cat > "$BIN/apt-get" <<EOF
#!/bin/sh
printf 'apt-get %s\n' "\$*" >> "$FIXTURES/apt.log"
touch "$marker"
exit 0
EOF

conf="$FIXTURES/deps-fix-success.conf"
printf 'marker-tool|[ -f "%s" ]|https://example.invalid\n' "$marker" > "$conf"

output=$(run_check "$conf" --fix --yes)
status=$?
apt_log=$(cat "$FIXTURES/apt.log")
assert_contains 'runs apt-get install for the default case' 'install -y marker-tool' "$apt_log"
assert_contains 'confirms the install' 'installed marker-tool' "$output"
assert_equals 'a successful --fix exits 0' '0' "$status"

# --- --fix --yes reports (but does not fail on) an install that does not
#     satisfy its own check -----------------------------------------------

: > "$FIXTURES/apt.log"
cat > "$BIN/apt-get" <<EOF
#!/bin/sh
printf 'apt-get %s\n' "\$*" >> "$FIXTURES/apt.log"
exit 0
EOF

conf="$FIXTURES/deps-fix-fail.conf"
printf 'some-tool|command -v some-tool-not-really-installed|https://example.invalid\n' > "$conf"

output=$(run_check "$conf" --fix --yes)
status=$?
assert_contains 'reports the unsatisfied check' 'install did not satisfy the check' "$output"
assert_equals 'an unsatisfied install exits 1' '1' "$status"

# --- --fix --dry-run never executes anything, always exits 0 -------------

: > "$FIXTURES/apt.log"
conf="$FIXTURES/deps-dryrun.conf"
printf 'some-tool|command -v some-tool-not-really-installed|https://example.invalid\n' > "$conf"

output=$(run_check "$conf" --fix --dry-run)
status=$?
apt_log=$(cat "$FIXTURES/apt.log")
assert_equals 'dry-run never invokes apt-get' '' "$apt_log"
assert_contains 'prints what it would run' \
    'would run: sudo apt-get update -qq && sudo apt-get install -y some-tool' "$output"
assert_equals 'dry-run always exits 0' '0' "$status"

# --- a manual-only dependency is reported but never fails --fix ----------

conf="$FIXTURES/deps-manual.conf"
printf 'nvm|[ -f "%s/nonexistent-nvm.sh" ]|https://github.com/nvm-sh/nvm\n' "$FIXTURES" > "$conf"

output=$(run_check "$conf" --fix --yes)
status=$?
assert_contains 'names it as manual-only' 'no automated install' "$output"
assert_contains 'links the docs' 'https://github.com/nvm-sh/nvm' "$output"
assert_equals 'a manual-only dependency does not fail --fix' '0' "$status"

# --- checks see the conventional curl-installer locations -----------------
#
# zoxide installs to ~/.local/bin and rustup to ~/.cargo/bin. Neither is on a
# default non-login PATH, so a bare `command -v` check fails immediately
# after a successful install and --fix can never converge. This masks itself
# on a machine whose shell rc already exports those directories.

pathless_home="$FIXTURES/pathless-home"
mkdir -p "$pathless_home/.local/bin" "$pathless_home/.cargo/bin"
printf '#!/bin/sh\nexit 0\n' > "$pathless_home/.local/bin/fake-local-tool"
printf '#!/bin/sh\nexit 0\n' > "$pathless_home/.cargo/bin/fake-cargo-tool"
chmod +x "$pathless_home/.local/bin/fake-local-tool" "$pathless_home/.cargo/bin/fake-cargo-tool"

conf="$FIXTURES/deps-localbin.conf"
printf 'fake-local-tool|command -v fake-local-tool|https://example.invalid\n' > "$conf"
output=$(env HOME="$pathless_home" PATH="/usr/bin:/bin" DEPS_CONF="$conf" \
    DEPS_LOCAL_CONF="$FIXTURES/no-such-local.conf" "$SCRIPT" 2>&1)
status=$?
assert_equals 'a binary in ~/.local/bin counts as present' '0' "$status"

conf="$FIXTURES/deps-cargobin.conf"
printf 'fake-cargo-tool|command -v fake-cargo-tool|https://example.invalid\n' > "$conf"
output=$(env HOME="$pathless_home" PATH="/usr/bin:/bin" DEPS_CONF="$conf" \
    DEPS_LOCAL_CONF="$FIXTURES/no-such-local.conf" "$SCRIPT" 2>&1)
status=$?
assert_equals 'a binary in ~/.cargo/bin counts as present' '0' "$status"

# The widened PATH must not invent dependencies that are genuinely absent.
conf="$FIXTURES/deps-still-missing.conf"
printf 'definitely-not-installed|command -v definitely-not-installed|https://example.invalid\n' > "$conf"
output=$(env HOME="$pathless_home" PATH="/usr/bin:/bin" DEPS_CONF="$conf" \
    DEPS_LOCAL_CONF="$FIXTURES/no-such-local.conf" "$SCRIPT" 2>&1)
status=$?
assert_equals 'a genuinely absent binary is still missing' '1' "$status"

# --- zsh-autosuggestions installs per package manager ---------------------
#
# The plugin ships as an oh-my-zsh custom clone on Linux and as its own brew
# formula on macOS. Cloning into ~/.oh-my-zsh on a machine with no oh-my-zsh
# leaves a directory nothing ever sources, so this must follow the manager.

cat > "$BIN/brew" <<EOF
#!/bin/sh
printf 'brew %s\n' "\$*" >> "$FIXTURES/brew.log"
exit 0
EOF
chmod +x "$BIN/brew"

# detect_pm prefers pacman, then apt, then brew, so a brew-only machine needs
# a PATH carrying neither of the others. That means the fixture directory has
# to be the ENTIRE PATH: appending /usr/bin would pick up the real apt-get on
# a Debian host (including the test container), and detect_pm would answer
# "apt" before it ever considered brew. The stub set therefore has to carry
# every binary check-deps.sh shells out to.
BREW_ONLY="$FIXTURES/brew-only"
mkdir -p "$BREW_ONLY"
cp "$BIN/brew" "$BIN/sudo" "$BREW_ONLY/"
for passthrough in sh env printf command test grep sed cut mkdir rm cat; do
    real=$(command -v "$passthrough" 2>/dev/null) || continue
    [ -f "$real" ] && ln -sf "$real" "$BREW_ONLY/$passthrough"
done

conf="$FIXTURES/deps-autosuggestions.conf"
printf 'zsh-autosuggestions|[ -f "%s/never" ]|https://example.invalid\n' "$FIXTURES" > "$conf"

: > "$FIXTURES/brew.log"
# HOME is redirected to a fixture: on a real mac the developer's own
# ~/.oh-my-zsh would decide which branch the install logic takes, so the
# assertion would pass or fail based on the host rather than the manager.
output=$(env HOME="$FIXTURES/brew-home" PATH="$BREW_ONLY" \
    DEPS_CONF="$conf" DEPS_LOCAL_CONF="$FIXTURES/no-such-local.conf" \
    "$SCRIPT" --fix --yes --dry-run)
assert_contains 'installs zsh-autosuggestions from brew on a brew machine' \
    'brew install zsh-autosuggestions' "$output"
assert_equals 'does not clone into oh-my-zsh on a brew machine' \
    '0' "$(printf '%s' "$output" | grep -c 'oh-my-zsh')"

# A machine with no oh-my-zsh must never be given the custom-plugin clone.
# The clone's own `git clone` creates the parent directories, so it lands a
# plugin in a ~/.oh-my-zsh that nothing sources, and the check then reports
# success for an install that will never load. Observed for real: a test run
# created ~/.oh-my-zsh/custom/plugins/zsh-autosuggestions on a machine that
# does not use oh-my-zsh.
omz_absent="$FIXTURES/omz-absent-home"
mkdir -p "$omz_absent"
output=$(env HOME="$omz_absent" PATH="$BIN:/usr/bin:/bin" DEPS_CONF="$conf" \
    DEPS_LOCAL_CONF="$FIXTURES/no-such-local.conf" "$SCRIPT" --fix --yes --dry-run)
assert_equals 'never clones into a nonexistent oh-my-zsh' \
    '0' "$(printf '%s' "$output" | grep -c 'oh-my-zsh')"
assert_succeeds 'a dry run creates no oh-my-zsh directory' \
    test ! -d "$omz_absent/.oh-my-zsh"

# With oh-my-zsh genuinely installed, the custom-plugin clone is correct.
omz_present="$FIXTURES/omz-present-home"
mkdir -p "$omz_present/.oh-my-zsh/custom/plugins"
output=$(env HOME="$omz_present" PATH="$BIN:/usr/bin:/bin" DEPS_CONF="$conf" \
    DEPS_LOCAL_CONF="$FIXTURES/no-such-local.conf" "$SCRIPT" --fix --yes --dry-run)
assert_contains 'clones into an existing oh-my-zsh' \
    'zsh-users/zsh-autosuggestions' "$output"

# --- the shared checks accept either platform's install shape -------------
#
# deps.conf is byte-identical on the mac and linux branches, so each check in
# it has to pass against the macOS shape and the Linux shape of the same
# dependency. These read the checks out of the real file rather than
# restating them, so the test fails if the shipped file regresses.

DEPS_CONF_REAL="$DOTFILES_ROOT/.scripts/deps/deps.conf"

check_for() {
    grep "^$1|" "$DEPS_CONF_REAL" | cut -d'|' -f2
}

# A Linux machine: the plugin is an oh-my-zsh custom clone, no brew present.
linux_home="$FIXTURES/linux-home"
mkdir -p "$linux_home/.oh-my-zsh/custom/plugins/zsh-autosuggestions"
touch "$linux_home/.oh-my-zsh/custom/plugins/zsh-autosuggestions/zsh-autosuggestions.zsh"
assert_succeeds 'zsh-autosuggestions resolves via the oh-my-zsh plugin path' \
    env HOME="$linux_home" PATH="/usr/bin:/bin" sh -c "$(check_for zsh-autosuggestions)"

# This machine: the plugin comes from brew, and there is no ~/.oh-my-zsh.
if command -v brew >/dev/null 2>&1; then
    assert_succeeds 'zsh-autosuggestions resolves via the brew share path' \
        env HOME="$FIXTURES/empty-home" sh -c "$(check_for zsh-autosuggestions)"
fi

# A machine with neither shape must still report it missing, or the widened
# check has become a tautology. PATH carries no brew, so the brew operand
# expands to an empty prefix and cannot accidentally match.
bare_home="$FIXTURES/bare-home"
mkdir -p "$bare_home"
if env HOME="$bare_home" PATH="/usr/bin:/bin" sh -c "$(check_for zsh-autosuggestions)" \
        >/dev/null 2>&1; then
    bare_result=present
else
    bare_result=missing
fi
assert_equals 'zsh-autosuggestions is missing on a bare machine' \
    'missing' "$bare_result"

# alacritty is a .app bundle on macOS and a PATH binary on Linux.
alacritty_bin="$FIXTURES/alacritty-bin"
mkdir -p "$alacritty_bin"
printf '#!/bin/sh\nexit 0\n' > "$alacritty_bin/alacritty"
chmod +x "$alacritty_bin/alacritty"
assert_succeeds 'alacritty resolves via a binary on PATH' \
    env PATH="$alacritty_bin:/usr/bin:/bin" sh -c "$(check_for alacritty)"

if [ -d /Applications/Alacritty.app ]; then
    assert_succeeds 'alacritty resolves via the macOS app bundle' \
        env PATH="/usr/bin:/bin" sh -c "$(check_for alacritty)"
fi

# --- no check command may contain the field delimiter --------------------
#
# read_entries splits on `|`, so a pipe or `||` in a check silently truncates
# it and leaks the rest into the docs URL. The truncated check still evals,
# so this fails as a wrong answer rather than an error.

bad_delimiter=$(awk -F'|' '!/^#/ && NF > 3 { print $1 }' "$DEPS_CONF_REAL")
assert_equals 'no check command contains a pipe' '' "$bad_delimiter"

# A leaked pipe shows up as a docs field that is no longer a bare URL, which
# is the visible symptom of the truncation described above.
malformed_docs=$(grep -v '^#' "$DEPS_CONF_REAL" | grep -v '^$' \
    | awk -F'|' '$3 !~ /^http/ { printf "%s ", $1 }')
assert_equals 'every docs url survives parsing' '' "$malformed_docs"

# --- oh-my-zsh must be tracked on a branch whose shell sources it --------
#
# zsh-autosuggestions is installed two different ways. On this repo's mac
# branch .zshrc-mac sources it from Homebrew's share directory; on the linux
# branch .zshrc-linux sources it from $HOME/.oh-my-zsh/custom/plugins. The
# shared deps.conf check accepts either path, so the check alone cannot say
# whether this machine needs oh-my-zsh -- the branch's own zshrc can.
#
# Moving oh-my-zsh out of the shared deps.conf into deps-linux.conf is
# correct, because the mac machine does not use it. The move is only safe
# while the variant whose zshrc sources from the oh-my-zsh path still lists
# it: drop it there and the shell sources a plugin nothing installs, silently
# losing autosuggestions with every check still reporting success.
#
# Every manifest counts. Both platform variants ship on both branches now, so
# this reads all of them rather than only the one this machine selects --
# which is what lets the mac branch's suite catch a dependency dropped from
# the linux variant.
all_tracked=$(cat "$DEPS_CONF_REAL" \
        "$DOTFILES_ROOT"/.scripts/deps/deps-mac.conf \
        "$DOTFILES_ROOT"/.scripts/deps/deps-linux.conf 2>/dev/null \
    | sed -e 's/#.*//' | cut -d'|' -f1 | grep -E '^[a-z]' | sort -u)

sources_from_oh_my_zsh=0
for zshrc in "$DOTFILES_ROOT"/.zshrc "$DOTFILES_ROOT"/.zshrc-*; do
    [ -f "$zshrc" ] || continue
    grep -q '^[^#]*source.*\.oh-my-zsh' "$zshrc" && sources_from_oh_my_zsh=1
done

if [ "$sources_from_oh_my_zsh" -eq 1 ]; then
    oh_my_zsh_tracked=$(printf '%s\n' "$all_tracked" | grep -cx 'oh-my-zsh')
    assert_equals 'oh-my-zsh is tracked on a branch whose shell sources it' \
        '1' "$oh_my_zsh_tracked"
else
    assert_equals 'no zshrc on this branch sources from oh-my-zsh' \
        '0' "$sources_from_oh_my_zsh"
fi

# --- a cask from a third-party tap taps first ----------------------------
#
# aerospace is a cask, not a formula, and it lives in a third-party tap
# (nikitabobko/homebrew-tap). The default brew branch emits
# `brew install <name>`, which fails twice over: "No available formula with
# the name aerospace" because it is a cask, and an untapped third-party cask
# is not findable even with --cask. The first real deps-check run failed on
# exactly this on macOS.
#
# Asserted through --dry-run so no tap or cask is touched here.

# detect_pm prefers pacman, then apt, then brew, so a brew-only PATH is the
# only way to reach the brew branch. /usr/bin must stay off it: the test
# image is Debian and carries a real apt-get, which would otherwise win and
# make this assertion pass on macOS while failing in the container.
brew_bin="$FIXTURES/brew-only-bin"
mkdir -p "$brew_bin"
printf '#!/bin/sh\nexit 0\n' > "$brew_bin/brew"
chmod +x "$brew_bin/brew"
for passthrough in sh dirname cd pwd command printf; do
    real=$(command -v "$passthrough" 2>/dev/null) || continue
    ln -sf "$real" "$brew_bin/$passthrough"
done

conf="$FIXTURES/deps-cask.conf"
printf 'aerospace|command -v aerospace-not-installed|https://github.com/nikitabobko/AeroSpace\n' > "$conf"

output=$(PATH="$brew_bin" DEPS_CONF="$conf" \
    DEPS_LOCAL_CONF="$FIXTURES/no-such-local.conf" \
    "$SCRIPT" --fix --dry-run 2>&1)

assert_contains 'aerospace installs as a cask' '--cask aerospace' "$output"
assert_contains 'aerospace taps its third-party tap first' \
    'nikitabobko' "$output"

# Tapping is not enough on its own. Current Homebrew refuses to load a cask
# from a tap that has not been trusted -- "Refusing to load cask
# nikitabobko/tap/aerospace from untrusted tap" -- which is what the second
# deps-check run hit after the tap itself succeeded.
assert_contains 'aerospace trusts the tap before installing' \
    'brew trust' "$output"

# --- a macOS-only dependency never fails --fix on Linux -----------------
#
# The deps-check workflow checks out one branch and runs every platform job
# against it, and deps-mac.conf ships on both branches, so it is present in
# the Ubuntu and Arch jobs.
# aerospace has no Linux build at all. Emitting the default
# `apt-get install aerospace` there produced "E: Unable to locate package
# aerospace" and failed the whole job, which is what the first real
# deps-check run hit on Ubuntu.
#
# Manual-only is the honest answer for a dependency this system cannot
# install: --fix reports it and still exits 0, which is the same contract
# nvm already relies on.

apt_only_bin="$FIXTURES/apt-only-bin"
mkdir -p "$apt_only_bin"
printf '#!/bin/sh\nexit 0\n' > "$apt_only_bin/apt-get"
printf '#!/bin/sh\nexec "$@"\n' > "$apt_only_bin/sudo"
chmod +x "$apt_only_bin/apt-get" "$apt_only_bin/sudo"

output=$(PATH="$apt_only_bin:/usr/bin:/bin" DEPS_CONF="$conf" \
    DEPS_LOCAL_CONF="$FIXTURES/no-such-local.conf" \
    "$SCRIPT" --fix --yes 2>&1)
status=$?

assert_contains 'aerospace is manual-only on a non-brew system' \
    'no automated install for aerospace' "$output"
assert_equals 'a macOS-only dependency does not fail --fix on Linux' \
    '0' "$status"

# --- zoxide installs from the package manager, not the GitHub API --------
#
# zoxide's install.sh resolves the latest release through
# api.github.com/repos/.../releases/latest, unauthenticated. That quota is 60
# requests an hour per IP, shared across every GitHub Actions runner on that
# IP, so the deps-check workflow failed with "you have exceeded GitHub's API
# rate limit" on a run that changed nothing about zoxide.
#
# Caching does not fix that: a cache miss still calls the API, and the
# installer offers no way to pass a token. zoxide is packaged by both apt and
# brew, so on the platforms CI actually runs, the curl-to-shell installer was
# never needed. The script keeps it only for a manager that has no package.

conf="$FIXTURES/deps-zoxide.conf"
printf 'zoxide|command -v zoxide-not-installed|https://github.com/ajeetdsouza/zoxide\n' > "$conf"

# PATH is the stub directory alone, with no /usr/bin appended. detect_pm
# picks pacman, then apt, then brew, so a real apt-get left on PATH shadows
# the brew stub -- which is exactly what happens inside the Debian test
# container. Each stub directory already symlinks the utilities the script
# needs.
for manager_bin in "$brew_bin" "$apt_only_bin"; do
    manager_name=brew
    [ "$manager_bin" = "$apt_only_bin" ] && manager_name=apt

    output=$(PATH="$manager_bin" DEPS_CONF="$conf" \
        DEPS_LOCAL_CONF="$FIXTURES/no-such-local.conf" \
        "$SCRIPT" --fix --yes --dry-run 2>&1)

    # Matched on the package name plus the manager, not on an exact command
    # string: apt spells it `install -y zoxide` and brew `install zoxide`.
    assert_contains "zoxide installs through $manager_name" \
        "$manager_name" "$output"
    assert_contains "the $manager_name command names the zoxide package" \
        "zoxide" "$output"

    # The specific failure being prevented. An installer reached through the
    # unauthenticated API is what the rate limit hits.
    assert_equals "zoxide does not curl an installer on $manager_name" \
        '' "$(printf '%s' "$output" | grep -o 'install\.sh')"
    assert_equals "zoxide does not reach the GitHub API on $manager_name" \
        '' "$(printf '%s' "$output" | grep -o 'api\.github\.com')"
done

# A manager with no zoxide package still gets the installer: pacman does not
# ship one everywhere, and losing the fallback would turn a working install
# into a manual step on machines that never had the rate-limit problem.
pacman_bin="$FIXTURES/pacman-only"
mkdir -p "$pacman_bin"
printf '#!/bin/sh\nexit 0\n' > "$pacman_bin/pacman"
chmod +x "$pacman_bin/pacman"
for passthrough in sh dirname cd pwd command printf uname; do
    real=$(command -v "$passthrough" 2>/dev/null) || continue
    ln -sf "$real" "$pacman_bin/$passthrough"
done

output=$(PATH="$pacman_bin" DEPS_CONF="$conf" \
    DEPS_LOCAL_CONF="$FIXTURES/no-such-local.conf" \
    "$SCRIPT" --fix --yes --dry-run 2>&1)
assert_contains 'zoxide keeps the installer where there is no package' \
    'install.sh' "$output"

# --- a cask disabled upstream is reported, not attempted -----------------
#
# Homebrew disabled the alacritty cask on 2026-09-01: "does not pass the
# macOS Gatekeeper check". `brew install --cask alacritty` now fails on every
# Mac, which is what failed the macOS deps-check job. Alacritty's own website
# ships a .dmg and its INSTALL.md documents only a source build, so there is
# no automated brew path to fall back to.
#
# Manual-only is the honest answer, and it is the same contract nvm and
# aerospace-on-Linux already use: --fix reports it with the docs URL and
# still exits 0, so one upstream disablement does not fail the whole
# bootstrap.

conf="$FIXTURES/deps-disabled-cask.conf"
printf 'alacritty|command -v alacritty-not-installed|https://alacritty.org/\n' > "$conf"

output=$(PATH="$brew_bin" DEPS_CONF="$conf" \
    DEPS_LOCAL_CONF="$FIXTURES/no-such-local.conf" \
    "$SCRIPT" --fix --yes 2>&1)
status=$?

assert_contains 'alacritty is manual-only on brew' \
    'no automated install for alacritty' "$output"
assert_contains 'alacritty points at its own docs' 'alacritty.org' "$output"
assert_equals 'a cask disabled upstream does not fail --fix' '0' "$status"

# --- --only <set> -----------------------------------------------------------
#
# The CI-subset selector. .github/workflows/test-suite.yml used to carry a
# hand-written apt list and a hand-written brew list naming tmux, zsh, git,
# fzf, ripgrep and shellcheck -- every one of them already a deps.conf entry.
# That second copy is the one that drifts, so the workflow now calls this
# engine and names the set it wants instead.

only_conf="$FIXTURES/only.conf"
cat > "$only_conf" <<'CONF'
present-tool|command -v some-tool|https://example.com/present
wanted-tool|command -v definitely-not-installed-wanted|https://example.com/wanted
other-tool|command -v definitely-not-installed-other|https://example.com/other
CONF

# A named set restricts the run to its members. The unnamed missing entry
# must not be reported at all: a CI step that installs the suite's set should
# not fail because the developer environment wants neovim.
output=$(run_check "$only_conf" --only wanted-tool 2>&1)
status=$?
assert_equals '--only exits non-zero when a named entry is missing' '1' "$status"
assert_contains '--only reports the named missing entry' 'wanted-tool' "$output"
assert_equals '--only ignores entries outside the set' '' \
    "$(printf '%s\n' "$output" | grep 'other-tool' || true)"

# A comma-separated list, because a workflow step names several at once.
output=$(run_check "$only_conf" --only wanted-tool,other-tool 2>&1)
assert_contains '--only takes a comma-separated list (first)' 'wanted-tool' "$output"
assert_contains '--only takes a comma-separated list (second)' 'other-tool' "$output"

# A set whose members are all present is a pass, and says so with the count
# of what it actually checked rather than the whole manifest.
output=$(run_check "$only_conf" --only present-tool 2>&1)
status=$?
assert_equals '--only exits 0 when every named entry is present' '0' "$status"
assert_contains '--only counts only the selected entries' 'all 1 dependencies present' "$output"

# A name that matches no entry is a typo in a workflow file, and a typo that
# silently installs nothing would make the step pass while installing none of
# what it promised. That is the failure mode this guard exists for.
output=$(run_check "$only_conf" --only no-such-dependency 2>&1)
status=$?
assert_equals '--only with an unknown name exits 2' '2' "$status"
assert_contains '--only names the unmatched selector' 'no-such-dependency' "$output"

# --only composes with --fix, which is the whole point: the workflow runs
# `--fix --yes --only <set>`.
rm -f "$FIXTURES/apt.log"
cat > "$BIN/apt" <<'STUB'
#!/bin/sh
exit 0
STUB
chmod +x "$BIN/apt"
output=$(run_check "$only_conf" --fix --yes --dry-run --only wanted-tool 2>&1)
assert_contains '--only composes with --fix --dry-run' 'would run' "$output"
assert_equals '--only --fix touches nothing outside the set' '' \
    "$(printf '%s\n' "$output" | grep 'other-tool' || true)"

# --only needs a value. Without this guard `--only` followed by nothing
# would select the empty set and report a vacuous success.
output=$(run_check "$only_conf" --only 2>&1)
status=$?
assert_equals '--only with no value exits 2' '2' "$status"


# --- the CI-only manifest ---------------------------------------------------
#
# deps-ci.conf holds what the test suite needs and the working environment
# does not: python3, pyyaml, dash. It is never selected by platform detection,
# so `depcheck` on a developer machine does not ask for them.

CI_CONF="$DOTFILES_ROOT/.scripts/deps/deps-ci.conf"
assert_succeeds 'deps-ci.conf exists' test -f "$CI_CONF"

# No literal pipe in a check field. read_entries splits on `|`, so one there
# truncates the check and leaks the rest into docs_url -- silently, with the
# truncated check still returning an answer. This is the manifest format's
# sharpest edge, and deps.conf has its own version of this assertion.
bad_pipe=''
while IFS= read -r line; do
    case $line in
        ''|'#'*) continue ;;
    esac
    field_count=$(printf '%s\n' "$line" | awk -F'|' '{print NF}')
    [ "$field_count" -eq 3 ] || bad_pipe="$bad_pipe $line"
done < "$CI_CONF"
assert_equals 'every deps-ci.conf line has exactly three fields' '' "$bad_pipe"

# pyyaml is the entry that needed a case in install_cmd_for: it is a pip
# package, not a `<pkg-manager> install pyyaml`, so the default case would
# have produced a command that fails on every platform.
#
# Driven against a manifest whose check always fails rather than against
# deps-ci.conf, because pyyaml is installed on any machine that can run this
# suite -- a dry run there reports nothing missing and would assert nothing.
pyyaml_conf="$FIXTURES/pyyaml-missing.conf"
printf 'pyyaml|false|https://pyyaml.org/\n' > "$pyyaml_conf"

output=$(DEPS_CONF="$pyyaml_conf" DEPS_LOCAL_CONF="$FIXTURES/no-such-local.conf" \
    PATH="$BIN:$PATH" "$SCRIPT" --fix --dry-run --only pyyaml 2>&1)
assert_contains 'pyyaml has an automated install' 'would run' "$output"

# The bug this case exists to prevent: the default branch would emit
# `<manager> install pyyaml`, and no package manager has a package by that
# name. apt calls it python3-yaml, pacman calls it python-yaml, and brew has
# no formula at all, so any of those three spellings is correct and the bare
# name never is.
assert_equals 'pyyaml never installs under its pip name as a system package' '' \
    "$(printf '%s\n' "$output" | grep -E 'install -y pyyaml|brew install pyyaml|noconfirm pyyaml' || true)"
assert_succeeds 'pyyaml resolves to a real package name or to pip' \
    grep -qE 'python3-yaml|python-yaml|pip install' <<<"$output"

# The suite's own dependencies must be checkable on this machine, since this
# machine runs the suite. A failure here means deps-ci.conf disagrees with
# what run-all.sh actually needs.
output=$(DEPS_CONF="$CI_CONF" DEPS_LOCAL_CONF="$FIXTURES/no-such-local.conf" "$SCRIPT" 2>&1)
status=$?
assert_equals 'every deps-ci.conf entry is satisfied on this machine' '0' "$status"


# --- the gh keyring write must be privileged --------------------------------
#
# Found by the bootstrap container, which is the first environment that runs
# as a genuine non-root user with gh absent. The install command ran
# `sudo mkdir -p -m 755 /etc/apt/keyrings` and then a BARE
# `wget -O /etc/apt/keyrings/...`, so the write into the root-owned directory
# it had just created failed with "Permission denied" and the check for gh
# failed right after its own install reported success.
#
# Every write into /etc must carry its own privilege. `sudo` on the mkdir does
# not extend to the next command in the chain, which is the whole trap.
gh_cmd=$(sed -n '/^        gh)/,/^            ;;/p' "$SCRIPT" | grep 'apt-get' || true)
assert_succeeds 'the gh apt install command is still there to check' test -n "$gh_cmd"

# Any redirect or -O into /etc must be preceded by sudo. Checked as a
# property rather than by matching one spelling, so switching between
# `sudo tee`, `sudo dd` and `sudo install` keeps passing while an unprivileged
# write fails.
assert_equals 'no unprivileged -O write into /etc' '' \
    "$(printf '%s\n' "$gh_cmd" | grep -oE '[^ ]* -O /etc/[^ ]*' | grep -v 'sudo' || true)"
assert_equals 'no unprivileged shell redirect into /etc' '' \
    "$(printf '%s\n' "$gh_cmd" | grep -oE '> */etc/[^ ]*' | grep -v 'tee\|dd\|sudo' || true)"

# The keyring still has to end up somewhere apt reads, and the repo line must
# still point at it, or the install silently adds an unverifiable source.
assert_contains 'the keyring still lands in /etc/apt/keyrings' \
    '/etc/apt/keyrings/githubcli-archive-keyring.gpg' "$gh_cmd"
assert_contains 'the sources line still signs by that keyring' \
    'signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg' "$gh_cmd"


finish
