#!/bin/bash
#
# Tests ~/.profile, which puts ~/.local/bin on PATH for bash and sh.
#
# Reported from a bare container: `config init` finished, and `config` was
# still not found. install-hooks links it into ~/.local/bin, which no default
# PATH carries, and only .zshrc added that directory -- so a bash or sh login
# never found it, no matter how many shells were started.
#
# The file is deliberately minimal and POSIX: bash reads it at login, sh reads
# it as $ENV in some configurations, and it must not depend on anything zsh
# provides.
#
# Usage: ~/tests/profile-path.test.sh

. "$(dirname "$0")/lib.sh"

PROFILE="$DOTFILES_ROOT/.profile"

assert_succeeds 'the profile exists' test -f "$PROFILE"

# --- it puts ~/.local/bin on PATH -------------------------------------------
#
# Driven through a real shell rather than grepped, so the assertion is about
# the effect and not about the spelling.
for shell in sh bash; do
    if ! command -v "$shell" >/dev/null 2>&1; then
        skip "$shell picks up ~/.local/bin from the profile" "no $shell here"
        continue
    fi

    fixture_home="$FIXTURES/profile-$shell"
    mkdir -p "$fixture_home/.local/bin"
    cp "$PROFILE" "$fixture_home/.profile"
    printf '#!/bin/sh\nprintf found\n' > "$fixture_home/.local/bin/only-in-local-bin"
    chmod +x "$fixture_home/.local/bin/only-in-local-bin"

    actual=$(HOME="$fixture_home" PATH="/usr/bin:/bin" "$shell" -c \
        '. "$HOME/.profile"; command -v only-in-local-bin >/dev/null 2>&1 && printf yes || printf no' 2>&1)
    assert_equals "$shell picks up ~/.local/bin from the profile" 'yes' "$actual"
done

# --- it is POSIX, not zsh or bash specific ----------------------------------
#
# A bashism here fails on a machine whose /bin/sh is dash, which is every
# Debian and Ubuntu box -- exactly where this file matters most.
assert_succeeds 'the profile parses under sh' sh -n "$PROFILE"
if command -v dash >/dev/null 2>&1; then
    assert_succeeds 'the profile parses under dash' dash -n "$PROFILE"
else
    skip 'the profile parses under dash' 'dash is not installed'
fi

# --- it does not duplicate the entry on repeated sourcing -------------------
#
# A login shell can source this more than once (a nested login, `su -`, a
# terminal that re-runs it), and a PATH that grows without bound on every
# source is a slow leak that eventually shows up as a mysterious slowdown.
fixture_home="$FIXTURES/profile-idempotent"
mkdir -p "$fixture_home/.local/bin"
cp "$PROFILE" "$fixture_home/.profile"
count=$(HOME="$fixture_home" PATH="/usr/bin:/bin" sh -c \
    '. "$HOME/.profile"; . "$HOME/.profile"; . "$HOME/.profile"; printf "%s" "$PATH"' 2>&1 \
    | tr ':' '\n' | grep -c "^$fixture_home/.local/bin$")
assert_equals 'sourcing it three times adds the entry once' '1' "$count"

# --- it does not clobber an existing PATH -----------------------------------
#
# The reported session included a typo -- `$PATHD` instead of `$PATH` -- which
# emptied PATH and made even iconv disappear. The profile must never be able
# to do that to someone: every system directory has to survive it.
fixture_home="$FIXTURES/profile-preserve"
mkdir -p "$fixture_home/.local/bin"
cp "$PROFILE" "$fixture_home/.profile"
after=$(HOME="$fixture_home" PATH="/usr/bin:/bin:/sbin" sh -c \
    '. "$HOME/.profile"; printf "%s" "$PATH"' 2>&1)
for required in /usr/bin /bin /sbin; do
    assert_contains "the profile keeps $required on PATH" "$required" "$after"
done

finish
