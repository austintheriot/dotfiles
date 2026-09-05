#!/bin/bash
#
# Tests for the Docker harness (.scripts/deps/test-local.sh, the two
# Dockerfiles) and the CI workflow (.github/workflows/deps-check.yml) that
# together run check-deps.sh --fix --yes against throwaway Linux and macOS
# environments.
#
# Deliberately builds and runs nothing. The containers are the slow, network-
# dependent, daemon-dependent part, and run-all.sh gates a pre-commit hook --
# a suite that needs a Docker daemon would fail on every machine that does not
# happen to have one running. Actually exercising the images is
# test-local.sh's job and CI's job.
#
# What is left is the set of static contracts the three artifacts share, each
# of which can rot silently:
#
#   - the ENTRYPOINT/CMD pair both images expose, which is the only reason
#     `docker run depcheck-arch` means "the bootstrap worked"
#   - the DEPS_LOCAL_CONF neutralization, without which every Linux run fails
#     on the mac branch's macOS-exclusive entries
#   - the opposite pinning decisions for the two base images, which a future
#     "make these consistent" cleanup would helpfully break in one direction
#     or the other
#   - the workflow's trigger set, permission scope, and per-job timeouts
#   - the workflow and the local harness agreeing on the command they run
#
# The two genuine behaviors test-local.sh owns before it touches Docker (the
# detached-HEAD guard and the docker-absence preflight) are driven for real,
# against a fixture $HOME carrying its own bare repo at .cfg. test-local.sh
# resolves the repo from $HOME rather than $DOTFILES_ROOT, so a fixture $HOME
# is what isolates it.
#
# YAML assertions parse the file with python3 rather than grepping it, so
# reindenting the workflow cannot fail this suite.
#
# Usage: ~/tests/deps-harness.test.sh

. "$(dirname "$0")/lib.sh"

HARNESS="$DOTFILES_ROOT/.scripts/deps/test-local.sh"
DOCKERFILE_UBUNTU="$DOTFILES_ROOT/.scripts/deps/docker/Dockerfile.ubuntu"
DOCKERFILE_ARCH="$DOTFILES_ROOT/.scripts/deps/docker/Dockerfile.arch"
WORKFLOW="$DOTFILES_ROOT/.github/workflows/deps-check.yml"
MANIFEST="$DOTFILES_ROOT/.sync-manifest"

# `case` inside a command substitution trips bash's parser on the `)` of a
# pattern, so the two predicates that need one live in functions instead.
pin_state() {
    case $1 in
        *@sha256:*) printf pinned ;;
        *) printf unpinned ;;
    esac
}

path_shape() {
    case $1 in
        /*) printf absolute ;;
        *) printf relative ;;
    esac
}

# --- test-local.sh is POSIX sh -------------------------------------------
#
# It carries a #!/bin/sh shebang and is one of the paths .sync-manifest keeps
# identical across branches, so it has to parse under the linux branch's
# /bin/sh (dash) too, not just the bash-flavored /bin/sh on macOS.

assert_succeeds 'test-local.sh parses as POSIX sh' sh -n "$HARNESS"

# `sh -n` alone is weak on macOS, where /bin/sh is bash in POSIX mode and
# happily parses `[[ ]]` and other bashisms that dash rejects. dash is the
# real /bin/sh on the linux branch, so when it is installed here it is the
# assertion that can actually catch a bashism before that branch sees it.
if command -v dash >/dev/null 2>&1; then
    assert_succeeds 'test-local.sh parses under dash' dash -n "$HARNESS"
fi

# --- the preflights, driven for real ------------------------------------
#
# A fixture $HOME with its own bare repo at .cfg. test-local.sh hardcodes
# $HOME/.cfg as the git dir, so this is the seam: the fixture repo is what
# `branch --show-current` and `git archive` see.

fake_home="$FIXTURES/home"
mkdir -p "$fake_home/.scripts/deps"
cp "$DOTFILES_ROOT/.scripts/deps/check-deps.sh" "$fake_home/.scripts/deps/"
git init -q --bare "$fake_home/.cfg"

home_git() { git --git-dir="$fake_home/.cfg" --work-tree="$fake_home" "$@"; }

home_git checkout -q -b mac
home_git add -A
home_git -c user.email=t@t -c user.name=t commit -q -m init

# A PATH with the tools test-local.sh needs but no docker. Symlinks rather
# than a directory prepend: prepending cannot hide a docker that is already
# on the real PATH, and this machine has one.
nodocker="$FIXTURES/nodocker-bin"
mkdir -p "$nodocker"
for tool in git tar mktemp rm cp mkdir; do
    ln -sf "$(command -v "$tool")" "$nodocker/$tool"
done

output=$(env HOME="$fake_home" PATH="$nodocker" "$HARNESS" 2>&1)
status=$?
assert_equals 'a missing docker exits non-zero' '1' "$status"
assert_contains 'a missing docker names docker' 'docker' "$output"

# Detached HEAD is checked before docker, so this asserts the guard rather
# than accidentally re-asserting the docker preflight. The full PATH is used
# on purpose: the message must be the detached-HEAD one even on a machine
# where docker is installed and running.
home_git checkout -q --detach HEAD

output=$(env HOME="$fake_home" "$HARNESS" 2>&1)
status=$?
assert_equals 'a detached HEAD exits non-zero' '1' "$status"
assert_contains 'a detached HEAD says so' 'HEAD is detached' "$output"
assert_contains 'a detached HEAD says what to do about it' 'check out' "$output"

home_git checkout -q mac

# --- the ENTRYPOINT/CMD contract both images share -----------------------
#
# `docker run --rm depcheck-arch` with no arguments has to mean
# `check-deps.sh --fix --yes`. test-local.sh and the workflow's arch job both
# run the image bare and read its exit code as the verdict, so a CMD that
# lost --yes would hang on a prompt and a CMD that lost --fix would report
# missing dependencies it never tried to install.

for dockerfile in "$DOCKERFILE_UBUNTU" "$DOCKERFILE_ARCH"; do
    image=${dockerfile##*Dockerfile.}
    contents=$(cat "$dockerfile")

    entrypoint=$(sed -n 's/^ENTRYPOINT *//p' "$dockerfile")
    assert_contains "$image ENTRYPOINT runs check-deps.sh" \
        'check-deps.sh' "$entrypoint"

    cmd=$(sed -n 's/^CMD *//p' "$dockerfile")
    assert_equals "$image CMD is exactly --fix --yes" \
        '["--fix", "--yes"]' "$cmd"

    # --- DEPS_LOCAL_CONF neutralization ---------------------------------
    #
    # These images verify the shared deps.conf only. check-deps.sh would
    # otherwise select deps-linux.conf here, whose entries (oh-my-zsh, xclip)
    # belong to the linux machine rather than to this container.
    # read_entries() skips a missing file, so pointing the variable at a path
    # that does not exist is the neutralization.

    local_conf=$(sed -n 's/^ENV DEPS_LOCAL_CONF=//p' "$dockerfile")
    assert_succeeds "$image sets DEPS_LOCAL_CONF" test -n "$local_conf"
    assert_equals "$image points DEPS_LOCAL_CONF at a path that does not exist" \
        'absent' "$([ -e "$local_conf" ] && printf present || printf absent)"

    # An absolute path, or check-deps.sh resolves it against its own working
    # directory and could land on a real file.
    assert_equals "$image DEPS_LOCAL_CONF is absolute" \
        'absolute' "$(path_shape "$local_conf")"

    # Every image copies in the deps tree and runs from /dotfiles, so the
    # ENTRYPOINT path has to be the copied one, not a $HOME-relative guess.
    assert_contains "$image copies the deps tree into the image" \
        'COPY .scripts/deps' "$contents"
done

# --- the two base images are pinned in deliberately opposite ways --------
#
# Ubuntu is digest-pinned because `ubuntu:24.04` is republished for every
# point release, so a tag-only reference makes the image change under a
# passing test. Arch is deliberately NOT digest-pinned because archlinux:base
# is rolling-release: a stale snapshot plus check-deps.sh's `pacman -Sy`
# against current mirrors is Arch's documented partial-upgrade breakage.
#
# Both decisions are documented in their respective Dockerfiles, and both are
# what a future "make the two consistent" cleanup would break. Asserting one
# without the other only catches half of that.

ubuntu_from=$(sed -n 's/^FROM *//p' "$DOCKERFILE_UBUNTU")
arch_from=$(sed -n 's/^FROM *//p' "$DOCKERFILE_ARCH")

assert_contains 'ubuntu pins its base image by digest' '@sha256:' "$ubuntu_from"
assert_equals 'arch does not pin its base image by digest' \
    'unpinned' "$(pin_state "$arch_from")"

# The comment is the only place the reasoning lives. Deleting it is how the
# next reader concludes the asymmetry was an oversight.
assert_contains 'arch documents why it is unpinned' \
    'NOT digest-pinned' "$(cat "$DOCKERFILE_ARCH")"

# --- the workflow ------------------------------------------------------

# Every fact the assertions below need is extracted in one interpreter start,
# into a `key=value` file the shell then reads. A python3 process per assertion
# was measured at 10 seconds for this section alone, and run-all.sh gates a
# pre-commit hook.
#
# $PYTHON_BIN rather than `python3`, for the same reason: lib.sh resolves the
# real interpreter once, and the shim on PATH costs 750ms per start.
WF_FACTS="$FIXTURES/workflow-facts"
"$PYTHON_BIN" - "$WORKFLOW" "$WF_FACTS" <<'PY'
import sys, yaml

with open(sys.argv[1]) as handle:
    document = yaml.safe_load(handle)

# GitHub's `on:` parses as the YAML 1.1 boolean True, which is exactly why the
# trigger set is read through the parser rather than by grepping for "on:".
triggers = document[True] if True in document else document.get('on', {})
jobs = document['jobs']


def steps_of(name):
    return jobs.get(name, {}).get('steps', [])


def run_text(name):
    # Flattened to a single line: a `run: |` block is multi-line, and the
    # facts file below is line-oriented, so an embedded newline would silently
    # truncate the value the assertions read back.
    # Line continuations are dropped with them, so the flattened value reads
    # as the command the runner actually executes.
    joined = ' '.join(str(step.get('run', '')) for step in steps_of(name))
    return ' '.join(token for token in joined.split() if token != '\\')


def is_checkout(step):
    return str(step.get('uses', '')).startswith('actions/checkout')


facts = {
    'triggers': ' '.join(sorted(triggers)),
    'has_push': 'yes' if 'push' in triggers else 'no',
    'permission_scopes': ' '.join(sorted(document.get('permissions', {}))),
    'contents_permission': document.get('permissions', {}).get('contents', ''),
    'cancel_in_progress': str(document.get('concurrency', {}).get('cancel-in-progress', '')),
    'jobs_without_timeout': ' '.join(
        sorted(name for name, job in jobs.items() if not job.get('timeout-minutes'))
    ),
    'checkouts_keeping_credentials': ' '.join(sorted(
        name for name, job in jobs.items() for step in job.get('steps', [])
        if is_checkout(step) and step.get('with', {}).get('persist-credentials') is not False
    )),
    'macos_runner': jobs.get('macos', {}).get('runs-on', ''),
    'push_paths': ' '.join(
        (triggers.get('push') or {}).get('paths', []) if isinstance(triggers.get('push'), dict) else []
    ),
    'push_is_filtered': (
        'yes' if isinstance(triggers.get('push'), dict)
        and (triggers.get('push') or {}).get('paths') else 'no'
    ),
    'macos_env': ' '.join(sorted(
        key for step in steps_of('macos') for key in (step.get('env') or {})
    )),
    'ubuntu_run': run_text('ubuntu'),
    'macos_run': run_text('macos'),
    'arch_run': run_text('arch'),
}

with open(sys.argv[2], 'w') as handle:
    for key, value in facts.items():
        handle.write('%s=%s\n' % (key, value))
PY
assert_equals 'the workflow is parseable YAML' '0' "$?"

# Reports a missing key rather than returning an empty string. Two of the
# assertions below expect an empty value (nothing violated the rule), so a
# key that stopped being emitted -- a rename, a typo in the extraction block
# above -- would otherwise make them pass while testing nothing.
wf() {
    if ! grep -q "^$1=" "$WF_FACTS"; then
        printf 'NO SUCH WORKFLOW FACT: %s' "$1"
        return
    fi
    sed -n "s/^$1=//p" "$WF_FACTS"
}

assert_equals 'the workflow triggers on push, schedule and workflow_dispatch' \
    'push schedule workflow_dispatch' "$(wf triggers)"

# An unfiltered push trigger is the specific regression that matters. Every
# job installs packages off the network, so a push trigger that fires for
# every commit would make unrelated work fail on an upstream outage -- a
# GitHub API rate limit already failed the zoxide install on one run. The
# path filter is what keeps that exposure proportional: only about 3% of
# recent commits touched the dependency files at all, and a commit that does
# not touch them cannot break the bootstrap.
assert_equals 'the push trigger is path-filtered' 'yes' "$(wf push_is_filtered)"
assert_contains 'the push filter covers the deps directory' \
    '.scripts/deps/' "$(wf push_paths)"
assert_contains 'the push filter covers the workflow itself' \
    'deps-check.yml' "$(wf push_paths)"

# `contents: read` and nothing else. A bare `permissions:` block is what
# demotes the default write-scoped token for a scheduled run on a public repo,
# so both the scope list and the one value are asserted.
assert_equals 'the workflow permissions are contents only' \
    'contents' "$(wf permission_scopes)"
assert_equals 'the workflow grants contents: read' 'read' "$(wf contents_permission)"

assert_equals 'the workflow cancels overlapping runs' 'True' "$(wf cancel_in_progress)"

# Every job, not a hardcoded list: a fourth leg added without a timeout is the
# regression this catches. A job with no timeout inherits GitHub's 6-hour
# default, and these jobs hang on a network stall rather than failing.
assert_equals 'every job declares a timeout' '' "$(wf jobs_without_timeout)"

# checkout with persist-credentials: false everywhere. The default leaves a
# usable token in .git/config for every later step in the job.
assert_equals 'every checkout drops its credentials' '' \
    "$(wf checkouts_keeping_credentials)"

# --- the workflow and the local harness run the same thing --------------
#
# test-local.sh exists so the CI legs can be iterated on locally. The moment
# the two disagree on the command or the Dockerfile, a green local run stops
# meaning anything about CI.

assert_contains 'the ubuntu job runs check-deps.sh --fix --yes' \
    '.scripts/deps/check-deps.sh --fix --yes' "$(wf ubuntu_run)"

assert_contains 'the macos job runs check-deps.sh --fix --yes' \
    '.scripts/deps/check-deps.sh --fix --yes' "$(wf macos_run)"

assert_equals 'the macos job runs on a macOS runner' 'macos-latest' "$(wf macos_runner)"

assert_contains 'the arch job builds the arch Dockerfile' \
    '.scripts/deps/docker/Dockerfile.arch' "$(wf arch_run)"
assert_contains 'the arch job runs the image it built' \
    'docker run --rm depcheck-arch' "$(wf arch_run)"

# The image tag is the join between the build step and the run step, and
# test-local.sh builds the same `depcheck-$image` name. A rename in one place
# only is a broken job, not a failed check.
assert_contains 'the local harness tags images the way the arch job does' \
    '"depcheck-$image"' "$(cat "$HARNESS")"

# --- the deps platform variants are shared, not excluded ----------------
#
# deps-local.conf used to be excluded from .sync-manifest so each branch could
# carry its own. The deps-mac.conf / deps-linux.conf pair replaced it exactly
# to end that: both ship on both branches, so both sit inside the drift check
# like the rest of .scripts/. An exclusion reappearing would quietly take them
# back out of it.

for platform in mac linux; do
    assert_succeeds "deps-$platform.conf ships on this branch" \
        test -f "$DOTFILES_ROOT/.scripts/deps/deps-$platform.conf"
    assert_equals "the manifest does not exclude deps-$platform.conf" \
        '' "$(grep -nxF "!.scripts/deps/deps-$platform.conf" "$MANIFEST")"
done

assert_equals 'the retired deps-local.conf is gone from the manifest' \
    '' "$(grep -n 'deps-local\.conf' "$MANIFEST")"


finish
