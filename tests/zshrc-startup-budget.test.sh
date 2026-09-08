#!/bin/bash
#
# A regression bar on interactive zsh startup cost.
#
# Startup used to run 7-9s: nvm and pyenv each paid their full init at every
# shell. zshrc-node-startup.test.sh and zshrc-python-startup.test.sh pin down
# the two fixes by asserting on source text, which is exact and cheap but only
# covers the causes already known. This suite covers the general case: it
# measures what a shell actually costs, so the NEXT thing that gets sourced
# eagerly is caught without anyone having to predict it first.
#
# `se` builds ~107 panes, so the cost is multiplied by roughly a hundred before
# the terminal is usable. 100ms of new startup work is ten seconds there.
#
# What makes a timing test survivable rather than flaky:
#
#   1. It measures against a same-machine baseline, not a wall-clock constant.
#      `zsh -f` skips every startup file, so subtracting it removes the process
#      spawn, the loader, and the machine's general speed. What remains is what
#      this repo's config costs, which is the only thing a budget can fairly
#      hold still across a fast laptop and a loaded CI runner.
#
#   2. It takes the MINIMUM of several runs, not the mean. Startup cost has a
#      hard floor and an unbounded tail: a scheduler preemption or a competing
#      build can only ever make a sample slower. The minimum is the closest
#      estimate of the true cost, and it is the statistic that does not drift
#      when the machine is busy. A mean over the same samples is a measure of
#      the machine's load.
#
#   3. The budget is generous. It is set well above the measured cost, so it
#      catches a regression of a scale that matters (a newly eager tool init)
#      and stays quiet about noise. A tight budget on a timing test is a
#      standing false alarm, and a standing false alarm gets disabled.
#
# The budget is deliberately NOT a target to optimise toward. Lowering it as
# startup improves is how a suite like this becomes brittle; the number moves
# only when a measurement shows the headroom is gone.
#
# Usage: ~/tests/zshrc-startup-budget.test.sh

. "$(dirname "$0")/lib.sh"

ZSHRC="$DOTFILES_ROOT/.zshrc"

# The budget, in milliseconds, for this repo's config above a bare shell.
#
# Measured on the development machine at ~180ms for an interactive shell
# against a ~10ms bare shell, so ~170ms of config. 400ms leaves better than
# 2x headroom for a slower or busier machine while still failing on the
# regression this exists to catch: a tool init that is sourced eagerly again
# costs 380ms (pyenv) to 2.4s (nvm) on its own.
BUDGET_MS=400

# Runs per measurement. The minimum of 5 is stable in practice; more runs cost
# suite time for a floor that has already stopped moving.
RUNS=5

# This suite measures the developer's own shell: `zsh -i` loads $HOME/.zshrc.
# That is only the config under test when HOME is the repo. Elsewhere it would
# time somebody else's shell and report it as this repo's cost.
if [ "$HOME" != "$DOTFILES_ROOT" ] || [ ! -f "$ZSHRC" ]; then
    skip 'startup budget: HOME is not the repo'
    finish
    exit
fi

if ! command -v zsh >/dev/null 2>&1; then
    skip 'startup budget: no zsh on this machine'
    finish
    exit
fi

# Milliseconds of wall clock for one `zsh` invocation with the given flags.
#
# The clock is read with `zmodload zsh/datetime` and $EPOCHREALTIME rather than
# date(1), because macOS ships a date with no sub-second format and this suite
# has to measure in milliseconds. The zmodload is required: without it
# $EPOCHREALTIME expands to the empty string, every difference computes as
# zero, and the budget assertion passes while measuring nothing. The floor
# assertion below the budget is what now catches that.
#
# Both clock reads happen in one bare shell that brackets the timed run, so the
# two `zsh -f` spawns are outside the measured window rather than inside it.
elapsed_ms() {
    local target="$*"
    zsh -f -c "
        zmodload zsh/datetime
        start=\$EPOCHREALTIME
        zsh $target -c exit >/dev/null 2>&1
        end=\$EPOCHREALTIME
        printf '%d' \$(( (end - start) * 1000 ))
    "
}

# The floor of RUNS measurements. See note 2 in the header for why the minimum
# is the right statistic here.
best_of() {
    local best='' sample remaining=$RUNS
    while [ "$remaining" -gt 0 ]; do
        sample=$(elapsed_ms "$@")
        [ -n "$best" ] && [ "$sample" -ge "$best" ] || best=$sample
        remaining=$((remaining - 1))
    done
    printf '%s' "$best"
}

# One untimed run first. A cold start pays for filesystem cache misses on every
# file the config touches, which is a property of the machine's recent history
# rather than of the config, and it lands entirely in the first sample.
zsh -i -c exit >/dev/null 2>&1

bare_ms=$(best_of -f -i)
full_ms=$(best_of -i)
config_ms=$((full_ms - bare_ms))

# A negative difference means the two measurements overlapped in noise, which
# only happens when the config costs almost nothing. Report it as zero rather
# than as a negative cost.
[ "$config_ms" -ge 0 ] || config_ms=0

printf 'startup: bare %dms, full %dms, config %dms (budget %dms)\n' \
    "$bare_ms" "$full_ms" "$config_ms" "$BUDGET_MS"

# The floor, asserted BEFORE the budget: a broken clock reports every sample as
# zero, and zero passes a budget. Drop the `zmodload` from elapsed_ms and both
# $EPOCHREALTIME reads expand to the empty string, `(end - start) * 1000`
# evaluates to 0 with no error, and this suite -- the repo's only performance
# gate -- reports PASS having measured nothing. Verified by running the
# arithmetic both ways: 7ms with the zmodload, 0ms without it.
#
# The floor is on `full_ms` ALONE, and the two exclusions are both mistakes
# this assertion already made:
#
#   config_ms is excluded because zero is a legitimate result there. The
#   negative clamp above says why: the two measurements can overlap in noise
#   when the config costs almost nothing, so a floor would fail on the good
#   outcome.
#
#   bare_ms is excluded because zero is legitimate there too, which the first
#   version of this floor got wrong. A bare `zsh -f` inside the test
#   container measures 0-1ms, so the minimum of five samples is genuinely 0,
#   and the container run reported "the clock read as zero" while printing
#   `full 19ms` -- a number that could only have come from a working clock.
#   Measured in the image: bare samples of 1 1 0 0 0 against full samples of
#   288 21 21.
#
# What remains is the real signal. A full interactive shell that spawns a
# process and sources this entire config cannot cost zero milliseconds, so a
# zero there means every sample came back empty -- which is exactly what
# happens when the `zmodload` in elapsed_ms goes missing.
[ "$full_ms" -gt 0 ] && measured=yes \
    || measured="no (full ${full_ms}ms -- the clock read as zero)"

assert_equals 'the harness measured a non-zero startup time' 'yes' "$measured"

[ "$config_ms" -le "$BUDGET_MS" ] && within=yes \
    || within="no (config costs ${config_ms}ms, budget is ${BUDGET_MS}ms)"

assert_equals 'interactive startup stays within the config budget' 'yes' "$within"

finish
