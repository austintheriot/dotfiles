# shellcheck shell=sh
# Sourced from .zshrc. Nags at most once every 24h if a CLI dependency from
# .scripts/deps/deps.conf is missing. Never blocks startup, never
# prompts -- see .scripts/deps/README.md for the manual `depcheck`
# command this also defines.

alias depcheck='~/.local/bin/config deps install'

_depcheck_cache="$HOME/.cache/depcheck-last-run"
_depcheck_last=0
if [ -f "$_depcheck_cache" ]; then
    _depcheck_last=$(head -c 32 "$_depcheck_cache" 2>/dev/null)
    # A digit string longer than the arithmetic width makes zsh print
    # "number truncated after 19 digits" to stderr mid-startup, so the
    # length cap is part of the validation, not just the digit test.
    case "$_depcheck_last" in
        ''|*[!0-9]*|??????????????????*) _depcheck_last=0 ;;
    esac
fi

if [ $(($(date +%s) - _depcheck_last)) -ge 86400 ]; then
    # The redirect is inside the subshell because a failing redirect is
    # reported by the shell before `date` runs, so `date ... 2>/dev/null`
    # cannot suppress it. An unwritable ~/.cache must not print during
    # startup; it only costs the throttle, which re-checks next shell.
    mkdir -p "$HOME/.cache" 2>/dev/null
    (date +%s > "$_depcheck_cache") 2>/dev/null
    # 127 is "the engine is not here", which is a different problem from
    # "a dependency is missing" and needs a different sentence. Reporting
    # missing dependencies on a machine whose binary was never built sends
    # the reader to `depcheck`, which cannot run either. This suite already
    # shipped that exact conflation once, in deps-docs.test.sh.
    ~/.local/bin/config deps check >/dev/null 2>&1
    _depcheck_status=$?
    if [ "$_depcheck_status" -eq 127 ]; then
        echo "depcheck: the deps engine is not built -- run \`config build\`"
    elif [ "$_depcheck_status" -ne 0 ]; then
        echo "depcheck: missing dependencies detected -- run \`depcheck\` for details"
    fi
    unset _depcheck_status
fi
unset _depcheck_cache _depcheck_last
