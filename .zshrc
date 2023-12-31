# Lines configured by zsh-newuser-install
HISTFILE=~/.histfile
HISTSIZE=1000
SAVEHIST=1000
setopt beep extendedglob nomatch notify
unsetopt autocd
# End of lines configured by zsh-newuser-install
# The following lines were added by compinstall
zstyle :compinstall filename '/home/austin/.zshrc'

autoload -Uz compinit
compinit
# End of lines added by compinstall

# SOURCE BRANCH-SPECIFIC CONFIG ####################################################################
FILE=.zshrc-home
if test -f "$FILE"; then
   source $FILE
fi
FILE=.zshrc-work
if test -f "$FILE"; then
   source $FILE
fi

# CONFIG BY ME #####################################################################################
alias start='source ~/.my-scripts/tmux-start.sh'
alias s='start'
alias split='source ~/.my-scripts/tmux-split.sh'
alias sp='split' 

# PLUGINS ##########################################################################################
# git - comes with zsh
plugin=(git)

# CUSTOMIZING PROMPT ################################################################################
# creates color formatting string based on current staged status
parse_git_dirty() {
  git_status="$(git status 2> /dev/null)"
  [[ "$git_status" =~ "Changes to be committed:" ]] && echo -n "%F{green}"
  [[ "$git_status" =~ "Changes not staged for commit:" ]] && echo -n "%F{yellow}"
  [[ "$git_status" =~ "Untracked files:" ]] && echo -n "%F{red}"
}

setopt prompt_subst

autoload -Uz vcs_info # enable vcs_info
precmd () { vcs_info } # always load before displaying the prompt
zstyle ':vcs_info:git*' formats ' %b' # format $vcs_info_msg_0_

PS1='%F{254}%n%F{245} %F{153}%(5~|%-1~/⋯/%3~|%4~)%f$(parse_git_dirty)${vcs_info_msg_0_} %F{254}λ%f '
