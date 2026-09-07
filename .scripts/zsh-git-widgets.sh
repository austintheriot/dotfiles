# zsh ZLE widgets for git, sourced at shell-init time (not run as a command).
# Registers a binding for every prompt, so it must be sourced from .zshrc startup.

# Ctrl+G - fuzzy-pick a git branch and paste it onto the command line.
# Lists local + remote branches by most-recent commit, strips the remote prefix,
# and inserts the chosen name at the cursor.
#
# The listing and formatting (the old git | rg | sed | sed stages) moved into
# tmux-tools list-branches: one spawn instead of five, and the widget's own
# LBUFFER assignment, zle -N registration and bindkey calls stay here because
# assigning into the zsh line editor is impossible from another process. This
# is not a keystroke path: the widget blocks on the interactive fzf picker
# below, so a human is reading the screen while it runs, and the spawn-cost
# argument that keeps .zshrc startup lean does not apply to a Ctrl+G press.
fzf-git-branch-widget() {
  setopt localoptions pipefail no_aliases 2>/dev/null
  local branch
  branch=$(
    tmux-tools list-branches \
      | fzf --ansi --no-sort --reverse --height=40% --min-height=20
  )
  if [[ -n "$branch" ]]; then
    LBUFFER="${LBUFFER}${branch}"
  fi
  zle reset-prompt
}
zle -N fzf-git-branch-widget
bindkey -M emacs '^G' fzf-git-branch-widget
bindkey -M vicmd '^G' fzf-git-branch-widget
bindkey -M viins '^G' fzf-git-branch-widget
