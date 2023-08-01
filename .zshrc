# Lines configured by zsh-newuser-install
HISTFILE=~/.histfile
HISTSIZE=1000
SAVEHIST=1000
setopt beep extendedglob nomatch notify
unsetopt autocd
bindkey -e
# End of lines configured by zsh-newuser-install
# The following lines were added by compinstall
zstyle :compinstall filename '/home/austin/.zshrc'

autoload -Uz compinit
compinit
# End of lines added by compinstall

# CONFIG BY ME #######################################################################
# DOTFILE SETUP (for sharing dotfiles accross envs in git)
alias config='/usr/bin/git --git-dir=/home/austin/.cfg/ --work-tree=/home/austin'

# WORK CONFIG #######################################################################

# PLUGINS
# git - comes with zsh
# zsh-autosuggestions - see https://github.com/zsh-users/zsh-autosuggestions
plugin=(git zsh-autosuggestions)

# SETUP NVM PLUGIN
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"  # This loads nvm
[ -s "$NVM_DIR/bash_completion" ] && \. "$NVM_DIR/bash_completion"  # This loads nvm bash_completion

# INIT PYENV
eval "$(pyenv init -)"

# GOOGLE CLOUD INIT
# The next line updates PATH for the Google Cloud SDK.
if [ -f '/Users/austintheriot/google-cloud-sdk/path.zsh.inc' ]; then . '/Users/austintheriot/google-cloud-sdk/path.zsh.inc'; fi
# The next line enables shell command completion for gcloud.
if [ -f '/Users/austintheriot/google-cloud-sdk/completion.zsh.inc' ]; then . '/Users/austintheriot/google-cloud-sdk/completion.zsh.inc'; fi

# SOURCE BASH_SECURE FILE (IF IT EXISTS)
if [ -f "$HOME/.bash_secure" ]; then 
    source "$HOME/.bash_secure"
fi

export PATH=$(pyenv root)/shims:$PATH
export USE_GKE_GCLOUD_AUTH_PLUGIN=True
alias config='/usr/bin/git --git-dir=$HOME/.cfg/ --work-tree=$HOME'

# CONFIG FOR ALL #####################################################################

# CUSTOMIZING PROMPT 
parse_git_dirty() {
  git_status="$(git status 2> /dev/null)"
  [[ "$git_status" =~ "Changes to be committed:" ]] && echo -n "%F{green}·%f"
  [[ "$git_status" =~ "Changes not staged for commit:" ]] && echo -n "%F{yellow}·%f"
  [[ "$git_status" =~ "Untracked files:" ]] && echo -n "%F{red}·%f"
}

setopt prompt_subst

autoload -Uz vcs_info # enable vcs_info
precmd () { vcs_info } # always load before displaying the prompt
zstyle ':vcs_info:git*' formats ' (%F{254}%b%F{245})' # format $vcs_info_msg_0_

PS1='%F{254}%n%F{245} %F{153}%(5~|%-1~/⋯/%3~|%4~)%F{245}${vcs_info_msg_0_}$(parse_git_dirty) %F{254}λ%f '


