# zsh ZLE widgets for git, sourced at shell-init time (not run as a command).
# Registers a binding for every prompt, so it must be sourced from .zshrc startup.

# Ctrl+G - fuzzy-pick a git branch and paste it onto the command line.
# Lists local + remote branches by most-recent commit, strips the remote prefix,
# and inserts the chosen name at the cursor.
fzf-git-branch-widget() {
  setopt localoptions pipefail no_aliases 2>/dev/null
  local branch
  branch=$(
    git branch --color=always --format="%(refname:short)%09%(color:yellow)%(committerdate:relative)%(color:reset)" --sort=-committerdate --all 2>/dev/null \
      | rg -v 'HEAD|^origin' \
      | sed 's/^[* ]*//' \
      | sed 's#^remotes/[^/]*/##' \
      | fzf --ansi --no-sort --reverse --height=40% --min-height=20 \
            --delimiter=$'\t' --with-nth=1,2 --nth=1 \
      | cut -f1
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
