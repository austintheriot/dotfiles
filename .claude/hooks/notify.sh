#!/usr/bin/env bash
# Claude Code notification helper.
# Reads the hook JSON payload from stdin, decides whether the user is
# already looking at this pane, and (if not) fires a macOS notification
# via terminal-notifier.
#
# Args:
#   $1 - notification kind: "stop" | "notification"

set -u

kind="${1:-stop}"

# Overridable so the tests can point these at stubs. The defaults are the real
# absolute paths, which is what a hook running outside a login shell needs.
AEROSPACE_BIN=${AEROSPACE_BIN:-/opt/homebrew/bin/aerospace}
OSASCRIPT_BIN=${OSASCRIPT_BIN:-/usr/bin/osascript}

# Drain stdin so the hook returns quickly even though we don't parse it.
# The value is discarded on purpose; only the read matters.
cat >/dev/null 2>&1 || true

# --- Focus check ---------------------------------------------------------
# Suppress the notification when the user is already looking at this pane.
# "Looking at this pane" means: Alacritty is the frontmost app AND this
# tmux pane is the active pane in the active window of its session.

is_focused() {
  # 1. Frontmost app must be Alacritty.
  local front
  front="$("$AEROSPACE_BIN" list-windows --focused --format '%{app-name}' 2>/dev/null | head -n1)"
  [[ "$front" == "Alacritty" ]] || return 1

  # 2. If we're inside tmux, this pane must be the active one in the
  #    active window. Outside tmux, frontmost-Alacritty is enough.
  if [[ -n "${TMUX:-}" && -n "${TMUX_PANE:-}" ]]; then
    local active_pane
    active_pane="$(tmux display -p -t "$TMUX_PANE" \
      '#{?pane_active,#{?window_active,1,0},0}' 2>/dev/null)"
    [[ "$active_pane" == "1" ]] || return 1
  fi

  return 0
}

if is_focused; then
  exit 0
fi

# --- Build notification --------------------------------------------------

cwd_name="$(basename "$(pwd)")"

case "$kind" in
  stop)
    title="Claude finished"
    message="$cwd_name"
    ;;
  notification)
    title="Claude needs input"
    message="$cwd_name"
    ;;
  *)
    title="Claude"
    message="$cwd_name"
    ;;
esac

# Escape double quotes for AppleScript string literals.
esc() { printf '%s' "$1" | sed 's/"/\\"/g'; }

"$OSASCRIPT_BIN" \
  -e "display notification \"$(esc "$message")\" with title \"$(esc "$title")\"" \
  >/dev/null 2>&1 || true

exit 0
