# Login PATH for bash and sh. Read by bash at login and by sh where $ENV
# points here; zsh reads .zshrc instead and sets the same directory there.
#
# ~/.local/bin exists because `config install-hooks` links the `config`
# dispatcher into it, and `config build` installs config-manifest there. No
# default PATH carries it, so without this file a bash or sh login never finds
# either one: a bare container reported `config init` finishing and `config`
# still being "command not found", however many shells were started.
#
# ~/.cargo/bin for the same reason, one layer down: rustup installs cargo
# there and nothing else puts it on PATH.
#
# Guarded against duplicates because a login shell can source this more than
# once -- a nested login, `su -`, a terminal that re-runs it -- and a PATH that
# grows on every source is a slow leak that surfaces later as an unexplained
# slowdown.
for profile_dir in "$HOME/.local/bin" "$HOME/.cargo/bin"; do
    case ":$PATH:" in
        *":$profile_dir:"*) ;;
        *) PATH="$profile_dir:$PATH" ;;
    esac
done
unset profile_dir
export PATH

# Sourced only when it exists. The previous version of this file sourced it
# unconditionally, so on a machine without a Rust toolchain every login
# printed "No such file or directory" and, in a POSIX shell run with -e,
# stopped reading the file there -- taking the PATH lines above with it.
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

# Never let this file fail a login. It is the last thing standing between a
# fresh machine and a usable shell, and a non-zero exit here is reported by
# some shells as a login error.
true
