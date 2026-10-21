---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-10-21
---

# Shell scripting

A reference for reviewing and advising on shell code, and on the shell-side surface that a command-line tool ships. Used by the `shell-scripting` subagent. The scope is shell as a language (POSIX sh, bash, zsh, fish, PowerShell, BusyBox ash, dash), shell as the host a program runs inside (startup files, PATH, non-interactive environments), the files a CLI author ships into shells (completion scripts, `tool init` hooks, shell integration), install scripts, and the quoting rules at every place where a program hands a string to a shell. It is not the design of the CLI program itself (`cli-design`), not terminal rendering or TTY mechanics (`terminal-tui`), not CI workflow YAML beyond its shell defaults (`ci-pipeline`), and not system-level threat modeling (`security`).

The unifying model: **a shell is a string-rewriting machine whose default is to reinterpret data as code.** Every unquoted expansion is split on `IFS`, then globbed. Every `eval`, `sh -c`, `ssh host cmd`, arithmetic context, and `[[ -eq ]]` parses its input again. The bugs in this domain are almost never logic errors. They are places where a value crossed into a parse step the author did not see. The second half of the model: **the shell's error handling is opt-in, inconsistent across versions, and silently disabled in common contexts.** A script that "works" has usually only met the inputs that do not trigger either half.

The operational question, for every line: **what does this value look like after expansion, how many times is it parsed, and what happens to the script when this command fails?**

Empirical priority order, by how often each bites:

1. **Unquoted expansion and re-parsing.** Word splitting, globbing, filenames with spaces or leading dashes, `eval`, `ssh`, `sh -c`. This is the largest class, and the destructive tail (`rm -rf "$D/"*` with `D` empty) lives here.
2. **Error handling that silently does not run.** `set -e` disabled inside functions called from `if` or `||`, `local x=$(cmd)` masking the status, pipelines returning the last command's status, command substitution not inheriting errexit.
3. **Portability assumptions.** A `#!/bin/sh` script that uses bash features and meets dash. A macOS-tested script that meets GNU userland, or the reverse. A shebang that works on one kernel only.
4. **Host-environment assumptions.** A script or tool that expects the interactive environment (PATH, aliases, rc-file setup) and runs from cron, launchd, systemd, an editor, an agent, or ssh.
5. **The wrong tool.** A script that grew decisions, data structures, or JSON handling and now has no tests and no types. Shell is all imperative shell and no functional core.

## Volatile surface

`last-verified` (see frontmatter, do not restate the date here). These rot. The rest of this file is comparatively durable.

| Claim class | Rots | Re-verify at |
|---|---|---|
| bash release and patch level | Slow (years per minor) | https://ftp.gnu.org/gnu/bash/ and the bug-bash list |
| macOS shell defaults (`/bin/sh` target, bash 3.2, `/bin/dash` features) | Slow, changes at major macOS releases | Run `/bin/sh -c 'echo $BASH_VERSION; set -o'` on the target. https://support.apple.com/en-us/102360 |
| dash feature support for POSIX Issue 8 (`$'...'`, pipefail) | Medium | https://git.kernel.org/pub/scm/utils/dash/dash.git, plus the distro's packaged version |
| POSIX text | Slow (Issue 8 is 2024, next is a technical corrigendum) | https://pubs.opengroup.org/onlinepubs/9799919799/ |
| ShellCheck checks and defaults | Medium (yearly release) | `gh api repos/koalaman/shellcheck/releases`, the wiki page per SC code |
| PowerShell native-command argument passing | Medium | Microsoft Learn `about_Preference_Variables` and `about_Parsing` |
| GitHub Actions default `shell` | Slow but high impact | docs.github.com workflow syntax, `jobs.<job_id>.steps[*].shell` |
| clap_complete dynamic completion stability | Fast | docs.rs/clap_complete, the `unstable-dynamic` feature |
| Bun shell implementation and semantics | Fast | https://bun.com/docs/runtime/shell |
| Maintenance status of prompt frameworks and shell tools | Fast | Each repo's README and commit recency |
| Installer script behavior of named vendors | Fast | Download the script and read it |

## Lineage, and why it explains today's behavior

- **Bourne shell (1979, V7 Unix).** Stephen Bourne, Bell Labs, from 1976. ALGOL 68 control structures. Replaced the Thompson shell. Every POSIX-family shell inherits its expansion model.
- **csh (Bill Joy).** C-like syntax, better interactive features in its day, and a scripting language with famously broken quoting and redirection. Tom Christiansen's "Csh Programming Considered Harmful" (Usenet, 1992, revised 1996) is the reason "do not script in csh" is settled. Nobody defends csh scripting now. tcsh survives as a FreeBSD root shell.
- **ksh (David Korn).** ksh88 is the basis of POSIX sh. ksh93 added associative arrays and floating point long before bash. Many "bash-isms" (`[[`, `$(( ))`, `typeset`) are ksh-isms.
- **bash (Brian Fox, then Chet Ramey as maintainer since the early 1990s).** bash 5.3 was announced 2025-07-05. New in 5.3: `${ cmd; }` command substitution that runs in the current shell without a fork, `source -p PATH`, and an error on `cd ""`. **VOLATILE** (2026-10-21): 5.3 with patches is current. No 5.4 alpha exists.
- **ash (Kenneth Almquist, 1989).** Descendants: NetBSD and FreeBSD `/bin/sh`, dash (Debian Almquist shell, Herbert Xu), and BusyBox ash (from dash 0.3.8). Ubuntu 6.10 (2006) made dash `/bin/sh` to speed boot scripts. Debian followed in 6.0 Squeeze (2011). This switch is the single largest source of "works on my machine" shell bugs: every `#!/bin/sh` script with a bash-ism broke on Debian-family systems.
- **zsh (Paul Falstad, 1990).** Default login shell on macOS since 10.15 Catalina (2019). zsh 5.9 is current. Its defaults differ from POSIX on purpose: no word splitting of unquoted `$x`, 1-indexed arrays, and an unmatched glob is an error.
- **fish.** Non-POSIX by design. fish 4.0 (2025-02-27) is a port of the core from C++ to Rust. **VOLATILE**: 4.9.3 (2026-09-08) at last check.
- **PowerShell.** Jeffrey Snover's Monad Manifesto (2002) argued for a pipeline of .NET objects in place of parsed text. Shipped as Windows PowerShell (2006), then cross-platform PowerShell 7. **VOLATILE**: 7.6.x is current stable, 7.7 in preview.
- **nushell.** "A new type of shell": structured tables through the pipeline, closer to PowerShell's model than to POSIX. Pre-1.0 (0.116 at last check), so syntax still changes between minors.
- **Oils (Andy Chu, oils.pub).** One binary with two modes. OSH runs existing bash scripts with stricter, documented semantics. YSH is a new language "for Python and JavaScript users who avoid shell". Andy Chu's blog (oilshell.org, 2016-2024) is the deepest public analysis of why shell parsing is hard.

### What macOS actually ships

Verified on macOS 26.6.2:

- `/bin/sh` is a shim that execs `/private/var/select/sh`, a link to `/bin/bash`. Inside it, `BASH_VERSION=3.2.57(1)-release`, `POSIXLY_CORRECT=y`, and `set -o posix` is on. **It is not dash.** An admin can repoint the link to `/bin/dash`, which also ships.
- `/bin/bash` is 3.2.57 (2007). The usual explanation is that bash 4.0 and later are GPLv3. Apple never stated this.
- `echo "a\tb"` prints a TAB under `/bin/sh` and prints `a\tb` literally under `/bin/bash` on the same machine, though `shopt xpg_echo` reports off in both. POSIX mode changes `echo`.
- Apple's `/bin/dash` rejects `set -o pipefail`.
- Consequence: "I tested it with sh on my Mac" tests bash 3.2 in POSIX mode. It does not test dash, and bash-isms pass.

### POSIX.1-2024 (Issue 8) changed the baseline

Issue 8 added, in XCU (Shell and Utilities):

- `set -o pipefail`.
- `$'...'` dollar-single-quotes (XCU 2.2.4).
- `read -d delim`.
- `find -print0` and `xargs -0`.
- New utilities `realpath`, `readlink` (with `-n` only, no `-f`), and `timeout`.
- An optional `{location}redir-op` form, implementation-defined.

Still absent: `local`, `mktemp`, arrays, `[[`.

**"POSIX sh" now needs a revision number.** Debian stable's dash 0.5.12 accepts pipefail and does not support `$'...'` (it prints `$a<TAB>b`). Upstream dash added pipefail in 2024 and has dollar-single-quote work in progress. **VOLATILE.** When a review says "this is POSIX now", also ask "is it in the `/bin/sh` the script meets?"

## Expansion: the core bug engine

The POSIX expansion order: tilde, parameter, command substitution, arithmetic (all left to right), then **field splitting** on `IFS` (unquoted results only), then **pathname expansion** (unquoted results only), then quote removal. Field splitting and globbing apply to the *results* of expansion. That is why `$var` holding `* .txt` can expand to every file in the directory.

### Rules that follow

- **Quote every expansion** unless splitting or globbing is the intent, and write a comment when it is. `"$var"`, `"$(cmd)"`, `"${arr[@]}"`.
- **`"$@"` is the only correct way to forward arguments.** `$*` and `$@` unquoted both split and glob each argument. `"$*"` joins with the first char of `IFS`.
- **Intended splitting uses an array** (bash, zsh, ksh) or `set --` (POSIX sh). For an optional flag, `${debug:+"-x"}` expands to nothing or to one word. This is the ShellCheck SC2086 wiki's advice.
- **Command substitution strips every trailing newline.** A file whose content is "a\n\n" becomes "a". Rich Felker's workaround preserves it: `var=$(cmd; echo x); var=${var%x}`.
- **Unmatched globs.** bash and POSIX sh pass the literal pattern (`*.log` stays `*.log`). zsh aborts the command with "no matches found". bash `nullglob` expands to nothing, so `ls *.nomatch` becomes a bare `ls` and lists the current directory, and `grep pat *.log` reads stdin and hangs. bash `failglob` aborts. Test the match count explicitly before acting on a glob.
- **Filenames that begin with `-`.** After `touch -- -rf`, `rm *` passes `-rf` as an option. Use `rm -- *` or `rm ./*` (SC2035).
- **Filenames that contain newlines** break every line-oriented loop. `find -print0 | xargs -0` and `while IFS= read -r -d '' f` are the only safe forms. These are now in POSIX, with the dash caveat above.
- **Never parse `ls`.** `for f in $(ls)` splits on spaces and globs the names. Use `for f in ./*`, then test `[ -e "$f" ]` for the no-match case.
- **`read` without `-r` and `IFS=`** strips leading and trailing whitespace and eats backslashes. `IFS= read -r line` is the only line-faithful form.
- **`echo` is not portable.** Under dash, macOS `/bin/sh`, and BusyBox, `echo` treats `\t`, `\c`, and `-n` differently. `printf '%s\n' "$x"` is portable.
- **Locale-dependent ranges.** `[a-z]` in a glob, `tr`, `grep`, or `sed` can match uppercase letters under some locales and tool versions. bash 5 sets `globasciiranges` on by default for its own globs, but the external tools do not follow it. Use `[[:lower:]]` or `LC_ALL=C` for byte semantics. Collation also reorders `sort` output, so `LC_ALL=C sort` is the stable form for machine comparison.

### Re-parse contexts (each one is an injection point)

- **`eval`.** Every metacharacter in the value is live code.
- **Arithmetic.** In bash and ksh, `$(( ))`, `(( ))`, `let`, array subscripts, and `[[ $x -eq $y ]]` evaluate their operands recursively as expressions. `num='a[$(echo PWNED >&2)]'; echo $((num+1))` runs the command. So `read num; echo $((num+1))` on untrusted input is remote code execution (BashPitfalls #46). Validate with `[[ $num =~ ^[0-9]+$ ]]` or a `case` pattern before any arithmetic.
- **`sh -c "$string"`, `bash -c`, `su -c`, `sudo sh -c`.** The string is parsed by a new shell. Pass data as positional arguments: `sh -c 'printf "%s\n" "$1"' sh "$value"`. The second `sh` becomes `$0`.
- **`find -exec sh -c 'echo {}' \;`.** The filename is pasted into code. Use `find ... -exec sh -c 'for f; do ...; done' sh {} +`.
- **`ssh host cmd args`.** ssh joins its arguments with spaces and the remote login shell parses that string again (man ssh). argv boundaries do not survive. Quote for the remote shell (`printf '%q'` works only when the remote shell is bash), or send the script on stdin: `ssh host 'bash -s' -- "$arg" < script.sh`, still with remote quoting for args.
- **`trap "..."`, `PROMPT_COMMAND`, `alias`.** These strings are code, and they expand when they run, not when they are set, unless double-quoted at definition.

## Error handling

### What `set -e` actually does

POSIX: errexit "shall be ignored when executing the compound list following the while, until, if, or elif reserved word, a pipeline beginning with the ! reserved word, or any command of an AND-OR list other than the last." The consequence most people miss: **the suppression is dynamic, so it applies to every command inside a function called from such a context.**

```bash
set -e
f() { false; echo "after-false"; }
f || true        # prints "after-false": set -e is off inside f
if f; then :; fi # same
```

Other verified holes:

- **`local x=$(cmd)`** returns the status of `local`, which is 0. The same goes for `export x=$(cmd)`, `declare`, and `readonly`. Split declaration and assignment (Google style guide, SC2155).
- **Command substitution does not inherit errexit** in bash: `x=$(false; echo hi)` continues inside the substitution. `shopt -s inherit_errexit` (bash 4.4 and later) fixes it.
- **Pipelines** return the last command's status. `grep x /missing | sort` succeeds. `pipefail` fixes that and introduces the reverse failure: `yes | head -1` exits 141 (128 + SIGPIPE) under pipefail, and so does any producer that a `head`, `grep -q`, or `sed q` stops reading early.
- **`((i++))` with `i=0`** evaluates to 0, which is exit status 1. bash 5.2 exits the script under `set -e`. bash 3.2 does not. The same script behaves differently on macOS and Linux. Use `i=$((i + 1))`.
- **Process substitution** `<(cmd)` status is lost entirely.
- **`set -u` with an empty array**: `"${a[@]}"` with `a=()` is "unbound variable" in bash before 4.4, which includes macOS `/bin/bash`. The portable form is `${a[@]+"${a[@]}"}`.

### `trap`

- `trap cleanup EXIT` is the reliable cleanup hook in bash and zsh. In POSIX sh, EXIT traps run on normal exit and on `exit`, but a signal that kills the shell does not run the EXIT trap in every shell. Trap `INT TERM HUP` too, and have the handler `exit` with the correct status so the EXIT trap runs once.
- To exit with the conventional signal status from a handler, reset the trap and re-raise: `trap - INT; kill -INT $$`. A handler that does `exit 1` tells the parent "failed" when the truth is "interrupted". That matters to a caller that checks for 130.
- `ERR` traps (bash) fire at the call site only, not inside functions, unless `set -E` (errtrace) is on.
- POSIX: "When a subshell is entered, traps that are not being ignored shall be set to the default actions." Cleanup registered in the parent does not run when a `( ... )` subshell or a pipeline element exits.

## Schools of thought

### Strict mode: `set -euo pipefail` or explicit checks

**For (Aaron Maxwell, "Use Bash Strict Mode (Unless You Love Debugging)", redsymbol.net; widely copied into templates, Bun's installer uses it).** Every mainstream language stops on an unhandled error, and bash's default is to continue. "If one line in a script fails, but the last line succeeds, the whole script has a successful exit code." `set -u` turns a typo (`$firstName` against `$firstname`) into an error in place of an empty string. `pipefail` makes `grep x /missing | sort` fail. The known holes are a short list a competent author learns once. A script with strict mode and a few documented workarounds fails loudly far more often than a script without it, and loud failure is the goal. Maxwell also sets `IFS=$'\n\t'`, arguing the default IFS is too eager.

**Against (Greg Wooledge's wiki, BashFAQ/105, and the #bash IRC community; geirha: "don't use set -e. Add your own error checking instead").** "set -e was an attempt to add 'automatic error detection' to the shell... This does not work well in practice." The rules about when it fires are context-dependent, differ across bash versions (the `((i++))` case), and are invisible at the line that fails. So the author thinks errors are handled and they are not, which is worse than knowing they are not. `pipefail` produces false failures on ordinary pipelines. Explicit `cmd || die "msg"` is visible, reviewable, and carries a message. BashPitfalls lists `set -euo pipefail` itself as pitfall #60.

**Neutral (Google Shell Style Guide).** It mandates neither, and warns specifically about `(( ))` "particularly with set -e enabled".

Note when each is right: strict mode is a reasonable default for a short script whose author will not write per-command checks, and the review must then check the known holes. Explicit checks are right for code where a missed failure is expensive and the author will write them. A reviewer must not present either as settled. The same evidence (version-dependent behavior, SIGPIPE) is a "known workaround" to the first camp and "proof the feature is unreliable" to the second.

### POSIX sh or bash

**POSIX sh camp.** rustup's installer (`#!/bin/sh`, about 930 lines, with a `local` shim) and Deno's installer target `/bin/sh` because it exists on every Unix including minimal containers, and an installer must need nothing installed. dylanaraps's pure-sh-bible (archived 2024) is the reference for doing things in pure POSIX sh. The ash family is faster at startup.

**bash camp.** Google mandates bash for all executables. Homebrew's installer refuses to run unless `$BASH_VERSION` is set and refuses POSIX mode ("Bash must not run in POSIX mode"). Arrays, `[[`, `local`, `pipefail` (until Issue 8), and `printf %q` make correct code much easier to write, and the portable alternative is harder to get right than it looks. Portability to a shell nobody tests on is not portability.

When each is right: POSIX sh for installers, container entrypoints on minimal images, and boot or init scripts. bash for scripts that run on known developer and CI machines. Either way the shebang must match the dialect, and ShellCheck must run with the matching `-s`.

### `#!/bin/bash` or `#!/usr/bin/env bash`

**`/bin/bash` (Google, Homebrew).** Predictable, and it cannot be hijacked by a writable directory early in PATH. On macOS it pins bash 3.2, which is a known target.

**`env bash` (Bun's installer, NixOS convention, most of the open-source world).** NixOS has no `/bin/bash`, and neither does Alpine. On macOS it picks up Homebrew's bash 5 when it is on PATH, which is either a feature or a portability hazard, depending on whether the script is tested against 3.2.

Facts that hold for both: `#!/usr/bin/env bash -eu` works on macOS (BSD `env` splits) and fails on Linux with exit 127 ("env: 'bash -eu': No such file or directory"), because Linux passes everything after the interpreter as one argument. `env -S` works on both modern GNU coreutils and macOS. Put `set -eu` in the body instead of the shebang.

### `curl | sh`

**Defense (Kenton Varda, Sandstorm, 2015).** "curl|bash over HTTPS is just as 'secure' as any of these": a downloaded `.deb`, a tarball with a `make install`, an npm `postinstall`. The trust decision is in the HTTPS origin, and reading a script you are about to give root anyway is theater. Mitigate the real risk, partial download, by wrapping the body in a function that the last line calls.

**Critique (idontplaydarts.com, 2016).** The server can detect a pipe: bash executes line by line, so a `sleep` early in the script stalls the TCP stream once buffers fill, and the server sends a malicious body only to clients that stall. So "inspect, then pipe" proves nothing about what the pipe runs. Also: no checksum, no signature, no uninstall record, and installers that edit rc files without asking.

What current installers actually do (downloaded and read 2026-10-21, **VOLATILE**): rustup puts all logic in `main()` called on the last line. Bun runs top-level code and appends PATH lines to `.zshrc`, `.bashrc`, and `config.fish`. Homebrew's documented form `/bin/bash -c "$(curl -fsSL ...)"` reads the whole script before running it, which defeats partial execution (the guard is inferred, not stated by Homebrew). Deno runs top-level code. None of the four checksums the payload it downloads. I did not check whether rustup-init verifies itself.

### When to leave shell

**Leave early (Google Shell Style Guide: "more than 100 lines long, or that uses non-straightforward control flow logic... rewrite it in a more structured language now"; Greg Wooledge's BashWeaknesses page lists 16 weaknesses including data structures, JSON, binary data, error handling, and "hidden landmines"; Rich Felker, despite writing the best POSIX sh tricks page: "Bourne-derived languages are extremely bad, on the same order of badness as Perl, for programming.").** The threshold is not length alone. It is the first decision that needs a data structure, the first JSON parse, the first retry loop, the first place a test would help.

**Shell is the right tool (the installer authors, the POSIX minimalists).** For gluing processes together, shell is shorter and clearer than any alternative, needs no runtime, and starts instantly. A 40-line pipeline rewritten in TypeScript is 120 lines of `spawn` boilerplate with its own quoting bugs.

The user's own position (CLAUDE.md): Rust or TypeScript on Bun for anything non-trivial or persistent, shell for one-off pipelines. In functional-core terms, a shell script has no core: every line is an effect. Once a script makes decisions, the decisions belong in a typed language where they can be a pure function with tests, and shell can stay as the thin outer layer, or vanish.

Replacements and their own traps:

- **Bun `$`** (Bun shell). Its own interpreter, documented as "a small programming language implemented in Rust" (**VOLATILE**: older docs said Zig). Escapes interpolations by default, throws `ShellError` on non-zero exit, `.nothrow()` to opt out, `{ raw: }` is the unsafe escape hatch. It does not stop argument injection (a value `--upload-pack=...` is still an option), and a nested `bash -c` loses all protection.
- **zx (Google).** Spawns bash, and quotes interpolations with `$'...'`, each array item separately. Adding your own quotes around `${x}` "can lead to an unsafe injection".
- **dax (Deno).** Its own cross-platform shell parser. Details not verified.
- **xshell (Rust, matklad).** `cmd!(sh, "git commit -m {msg}")`: `{var}` is exactly one argv element, `{args...}` splats a list, no shell is spawned, non-zero exit is an error. The cleanest model for a Rust project's task scripts.
- **duct (Rust).** Pipelines and redirection as a builder, no shell.
- **just.** Default shell `sh -cu`. `{{arg}}` is textual substitution and "the quotes are not preserved", so `'{{arg}}'` breaks on a value that contains `'`. Use `set positional-arguments` and `"$1"`, `set export`, or `quote()`.
- **make.** Each recipe line runs in its own shell (so `cd` does not persist), `$$` is a literal `$`, `.ONESHELL` and `.SHELLFLAGS` change both.

### Non-POSIX daily shells (fish, nushell, PowerShell)

**For.** fish: sane defaults, no word splitting ("Once a variable has been set to a value, that value stays as it is"), autosuggestions and completions without configuration. nushell and PowerShell: structured data ends the text-parsing class of bug.

**Against.** Every copied snippet from documentation, every `eval "$(tool init bash)"`, and every `source env.sh` fails or needs a translation. Tools must ship a separate integration per shell. The user lives in two languages: the daily shell and the POSIX scripts.

For a reviewer, the consequence is narrow: a tool that ships shell integration must support fish and PowerShell explicitly, not assume POSIX syntax in what it prints for `eval`.

### `eval "$(tool init zsh)"` or static config or shims

**Dynamic init (direnv, mise `activate`, zoxide, starship, `brew shellenv`).** The tool generates the right code for its current version, so the rc file never goes stale.

**Costs.** Every shell start forks the tool, so five of these add measurable startup latency. The rc file runs whatever the binary prints, so a compromised or replaced binary owns every new shell. Prompt hooks run on every prompt or `cd`.

mise documents the split cleanly: `activate` for interactive terminals, shims for "Editors, IDEs, GUI apps, login profiles", and `mise exec`/`run` for scripts and CI. Shims skip `[env]` and cd hooks, and "a loop that calls a shim pays that cost on every call." The middle option: generate the init output once to a file and source the file, regenerating on upgrade.

### Generated or handwritten completions

**Generated, dynamic (cobra `__complete`, clap `COMPLETE=$SHELL`).** The script calls back into the binary on every TAB, so completions never drift from the parser and can complete runtime values (branch names, resource IDs). Cost: a process start per TAB, which is noticeable for slow-starting runtimes.

**Generated, static (clap_complete `aot`, `cobra completion bash` saved to disk).** No runtime cost, but stale after an upgrade unless the package manager regenerates it.

**Handwritten (zsh `_arguments`, fish `complete -c`).** Richest descriptions and grouping, and drift from the real flags the day someone adds an option. This comparison is my inference, not a sourced position.

## Interactive shells as a host

### Startup files

**zsh** (zsh manual, "Files"): `/etc/zshenv` then `~/.zshenv` for every shell, including scripts and `ssh host cmd`. Login shells then read `/etc/zprofile` and `~/.zprofile`. Interactive shells read `/etc/zshrc` and `~/.zshrc`. Login shells then read `/etc/zlogin` and `~/.zlogin`. `ZDOTDIR` defaults to `HOME`.

- `.zshenv` must not produce output or assume a TTY (zsh intro). Output there corrupts scp, rsync, and git-over-ssh, which all run a non-interactive zsh on the remote end.
- **macOS reorders PATH after `.zshenv`.** `/etc/zprofile` runs `` eval `/usr/libexec/path_helper -s` ``, which rebuilds PATH with the system paths first, and it runs after `.zshenv` in a login shell. Every Terminal.app and most iTerm2 windows are login shells. Set PATH in `.zprofile` or `.zshrc` on macOS. macOS `/etc/zprofile` also sets `LANG=C.UTF-8` when LANG is empty.

**bash** (`man bash`, INVOCATION): login shells read `/etc/profile`, then the *first* of `~/.bash_profile`, `~/.bash_login`, `~/.profile`. Interactive non-login shells read `~/.bashrc`. Non-interactive shells read `$BASH_ENV`. Invoked as `sh`, bash reads `/etc/profile` and `~/.profile` at login and `$ENV` when interactive. When bash detects it is run by sshd (`ssh host cmd`), it reads `~/.bashrc` even though it is non-interactive. So a `.bashrc` that prints, or that assumes a TTY, breaks scp. The standard guard at the top of `.bashrc`: `case $- in *i*) ;; *) return;; esac`.

**fish:** `~/.config/fish/config.fish` and `conf.d/*.fish`, every shell. Guard interactive-only code with `status is-interactive`.

### The non-interactive environment

Cron, launchd, systemd units, Git hooks run by a GUI client, editor-spawned processes, `ssh host cmd`, CI runners, and coding agents run commands without the user's interactive setup. Consequences:

- **PATH is minimal.** cron's default is often `/usr/bin:/bin`. A launchd agent gets `/usr/bin:/bin:/usr/sbin:/sbin`. Homebrew, mise, nvm, cargo, and `~/.local/bin` are absent. A script that works from a terminal fails with "command not found" from cron.
- **Aliases do not expand** in non-interactive bash unless `shopt -s expand_aliases`. Functions defined in `.bashrc` do not exist.
- **`$-` lacks `i`.** Tools that detect interactivity by `$-` or by `PS1` behave differently.
- **No TTY.** Anything that prompts hangs or fails (see `cli-design` for the CLI side).
- **Locale may be `C` or `POSIX`.** UTF-8 handling, sort order, and tool messages change.

The fix is in the invoked script, not the environment: set PATH explicitly at the top, call tools by absolute path or through a shim, and never depend on rc files. A version manager's shims exist for this exact case.

### Shipping completions from a CLI

- **bash** (scop/bash-completion v2): install to `$(pkg-config --variable=completionsdir bash-completion)` as `foo.bash`. The unsuffixed file name is deprecated from 2.18. Loading is lazy, by command name. User directory: `$XDG_DATA_HOME/bash-completion/completions`. bash-completion 2 needs bash 4.2 or later, so it does not run under macOS's bash 3.2. Homebrew users need `bash-completion@2` with Homebrew's bash.
- **zsh:** a file named `_foo` in a directory on `fpath`, before `compinit` runs. Homebrew uses `$(brew --prefix)/share/zsh/site-functions`, and the user must `eval "$(brew shellenv)"` before `compinit`. Oh My Zsh calls `compinit` itself, so additions to `fpath` must come before Oh My Zsh is sourced. "Insecure directories" warnings come from group-writable directories on `fpath`. Diagnose with `compaudit`, and do not recursively chmod the Homebrew prefix (Homebrew docs). The `.zcompdump` cache can hold a stale completion after an upgrade until it is deleted.
- **fish:** search order `~/.config/fish/completions`, `/etc/fish/completions`, `~/.local/share/fish/vendor_completions.d`, `/usr/share/fish/vendor_completions.d`, then built-ins, then completions generated from man pages. Packages find the directory with `pkg-config --variable completionsdir fish`. Autoloaded on demand.
- **PowerShell:** `Register-ArgumentCompleter -Native -CommandName foo -ScriptBlock { ... }`, loaded from the user's `$PROFILE`. There is no drop-in directory, so the tool must tell the user what to add.
- **Generators:** cobra (`foo completion bash|zsh|fish|powershell`, dynamic via a hidden `__complete` subcommand returning candidates plus a `ShellCompDirective` bitfield). clap_complete: the `aot` module generates static scripts. The `CompleteEnv` / `COMPLETE=$SHELL foo` dynamic protocol is still behind the `unstable-dynamic` feature (**VOLATILE**). carapace is a multi-shell completion engine with specs for many tools.

### Shell integration and hooks

- **Hook points.** bash: `PROMPT_COMMAND` (an array since 5.1), `PS0`, the `DEBUG` trap. bash-preexec emulates zsh's hooks and "must be the last thing imported in your bash profile". zsh: `precmd`, `preexec`, `chpwd`, via `add-zsh-hook`, never by overwriting the function. fish: `--on-event fish_prompt`, `fish_preexec`, `--on-variable PWD`.
- **Never clobber.** A tool that assigns `PROMPT_COMMAND="mytool_hook"` removes every other tool's hook. Append, or add to the array.
- **OSC 133** (semantic prompt marks A, B, C, D) and **OSC 7** (current directory as `file://HOSTNAME/DIR`, originated in macOS Terminal) are what terminals use to jump between prompts, show exit status, and open new tabs in the same directory. The terminal side is `terminal-tui`'s. The shell side is emitting them from the prompt hooks above.
- **kitty's integration model** is the cleanest: it injects environment and modifies no files. For zsh it sets `ZDOTDIR` to a directory whose `.zshenv` loads the integration and restores the original `ZDOTDIR`. For fish it prepends `XDG_DATA_DIRS`. For bash it starts bash in POSIX mode with `ENV` pointing at its script, which then leaves POSIX mode. This also shows the attack surface: `ENV`, `BASH_ENV`, and `ZDOTDIR` are environment-borne code execution.
- **Prompt frameworks.** starship (one binary, all shells, actively released). powerlevel10k's README now says "THE PROJECT HAS VERY LIMITED SUPPORT / NO NEW FEATURES ARE IN THE WORKS / MOST BUGS WILL GO UNFIXED" (**VOLATILE**, repo not archived). Do not recommend p10k for a new setup without saying so.

## Quoting across the boundary to other programs

The safe default in every language: **spawn the program directly with an argv array, and never build a command string for a shell.**

- **C / POSIX:** `system()` and `popen()` run `/bin/sh -c`. Use `posix_spawn` or `fork`/`execv`.
- **Python:** `subprocess.run([...])` with a list. `shell=True` with interpolated input is injection. `shlex.quote` is "only designed for Unix shells", and quoting for Windows with it "can open up the possibility of a command injection vulnerability".
- **Node:** `child_process.exec()` and `spawn(..., { shell: true })` run a shell. The docs: "Never pass unsanitized user input." Node deprecated `shell: true` with an args array (DEP0190). Use `execFile` or `spawn` without `shell`. `.bat` and `.cmd` files cannot run through `execFile` on Windows, which pushes people back to `shell: true`, which is where the injection comes from.
- **Rust:** `std::process::Command` passes argv directly on Unix. On Windows, argv is one string that the callee parses, and `.bat`/`.cmd` files are parsed again by `cmd.exe` with different rules. CVE-2024-24576 (fixed in Rust 1.77.2, April 2024, found by RyotaK, escaping rules from Simon Sawicki) let untrusted arguments to a batch file run arbitrary commands. Rust now returns `InvalidInput` when it cannot escape safely. `CommandExt::raw_arg` is the unescaped escape hatch. The same bug class affected several other languages' standard libraries in the same disclosure ("BatBadBut"). The cross-language list was not verified.
- **Shell to shell:** `printf '%q'` (bash 3.2 and later) and `${var@Q}` (bash 4.4 and later, a "bad substitution" error on macOS `/bin/bash`) produce bash-re-readable quoting. Neither output is valid POSIX sh in every case.
- **sudo:** `env_reset` and `secure_path` replace the environment and PATH. `sudo cmd > /root/f` opens the redirect as the caller, so it fails. `sudo ls /root/*` expands the glob as the caller. Use `cmd | sudo tee /root/f > /dev/null` and `sudo sh -c 'ls /root/*'` (BashPitfalls #53, #54).
- **PowerShell 7.3 and later** "changed the way it parses the command line for native commands". `$PSNativeCommandArgumentPassing` defaults to `Windows` on Windows (legacy behavior kept for `cmd.exe`, `cscript.exe`, `find.exe`, `sqlcmd.exe`, `wscript.exe`, and `.bat`, `.cmd`, `.js`, `.vbs`, `.wsf` files) and `Standard` elsewhere. `$PSNativeCommandUseErrorActionPreference` (7.4, default `$false`) is PowerShell's `set -e` for native commands. **VOLATILE.**
- **cmd.exe:** `^` escapes, `%` expands even inside quotes, and delayed expansion `!var!` is a second pass. There is no fully general safe quoting for arbitrary data passed through `cmd /c`.
- **GitHub Actions `run:`.** With no `shell` key, a step runs `bash -e {0}`, with no pipefail. With an explicit `shell: bash`, it runs `bash --noprofile --norc -eo pipefail {0}`. So adding `shell: bash` changes pipeline semantics, and without it `curl ... | tar x` can fail and leave the step green. Falls back to `sh -e {0}` when bash is missing. Interpolating `${{ github.event.issue.title }}` directly into `run:` is script injection: pass it through `env:` and reference `"$TITLE"`. CI security depth belongs to `ci-pipeline`.

## Portability matrix

Verified by running each on 2026-10-21:

| Feature | macOS `/bin/sh` (bash 3.2 POSIX mode) | Debian dash 0.5.12 | Alpine BusyBox 1.37 ash | bash 5.2 |
|---|---|---|---|---|
| `set -o pipefail` | yes | yes | yes | yes |
| `$'...'` | yes | **no** | yes | yes |
| `[[ ]]` | yes | `[[: not found` | yes | yes |
| `{1..3}` | yes | literal | literal | yes |
| `local` | yes | in functions | in functions | yes |
| `echo "a\tb"` | TAB | TAB | literal | literal |

GNU versus BSD userland (macOS 26 against Debian and Alpine):

| Operation | GNU / BusyBox | macOS | Portable route |
|---|---|---|---|
| In-place edit | `sed -i 's/a/b/' f` | `sed -i 's/a/b/' f` treats the script as the backup suffix and fails | `sed -i.bak ... && rm f.bak`, or write a temp file and `mv` |
| Parse a date | `date -d '2026-01-01'` | illegal option. `date -j -f %F 2026-01-01` fills missing fields with the current time | Use a real language for date math |
| File size | `stat -c %s` | `stat -f %z` | `wc -c < f` |
| PCRE grep | `grep -P` (GNU only, not BusyBox) | fails | `grep -E`, or a real language |
| Canonical path | `readlink -f`, `realpath` | `readlink -f` exits 1 silently for a missing path. `realpath` exists on macOS 26 | `realpath` where available, or `cd -P` plus `pwd -P` |
| Base64 decode | `base64 -d` | `-d` and `-D` both work | `base64 -d` |
| Temp file | `mktemp` | `mktemp` | `mktemp` (not POSIX, present everywhere tested) |
| `xargs` on empty input | runs once unless `-r` | does not run | `xargs -r` on GNU, test before calling on others |

Other portability facts:

- `command -v foo` is POSIX and is the way to test for a command. `which` is an external tool with varying output and exit codes (ShellCheck SC2230 under `-o all`).
- The script's own directory: in bash, `"${BASH_SOURCE[0]}"`, not `$0` (wrong when sourced). In POSIX sh there is no reliable form when the script is sourced.
- Run `shellcheck -s sh` (or `# shellcheck shell=sh`) on every `#!/bin/sh` script. `checkbashisms` (Debian devscripts) is the second check. Test on dash in a container, not on macOS `/bin/sh`.

## Tooling

- **ShellCheck (Vidar Holen).** v0.11.0 (2025-08-04), **VOLATILE**. Run it on every script, in CI. Defaults worth knowing: SC2086 (unquoted expansion), SC2115 (`rm -rf "$var/"*`, use `"${var:?}"`), SC2155 (declare and assign separately), SC2164 (`cd` without `|| exit`), SC2035 (glob that can start with `-`), SC2045 (iterating over `ls`), SC2046 (unquoted `$(...)`), SC2162 (`read` without `-r`), SC3043 (`local` in sh). 0.11.0 turned **SC2002 (useless cat) off by default** and added SC2329 (function never invoked) and SC2332 (`[ ! -o opt ]` always true). `-o all` enables SC2250 (braces), SC2292 (`[[` over `[`), SC2312 (masked return in command substitution), SC2230. Directives: `# shellcheck disable=SC2086` on the line above, `.shellcheckrc` per project. A disable directive needs a reason in a comment.
- **shfmt** (Daniel Martí, mvdan/sh). Formatter and parser. `shfmt -d` in CI. **VOLATILE** v3.14.x.
- **bats-core** for tests. v1.14.0 (2026-07). shellspec has had no release since 2021, so treat it as unmaintained.
- **Debugging:** `bash -n` parses without running. `set -x` with `PS4='+ ${BASH_SOURCE}:${LINENO}:${FUNCNAME[0]:-main}: '` gives file, line, and function per traced line. **xtrace prints secrets in clear text**: verified, a traced `curl -H "Authorization: $TOKEN"` prints the token. Turn tracing off around secrets, and never enable `set -x` in CI on a script that handles credentials.
- **explainshell.com** explains a command line against man pages. Active.

## Security in shell

- **Injection** through unquoted expansion, `eval`, arithmetic contexts, `sh -c` with interpolation, `find -exec sh -c '{}'`, and `ssh`. Covered above.
- **Option injection** through values that begin with `-`. Put `--` before untrusted operands, or prefix paths with `./`.
- **Environment-borne code execution.** Shellshock (CVE-2014-6271): bash through 4.3 executed trailing strings after function definitions in environment variable values, reachable through OpenSSH `ForceCommand`, Apache `mod_cgi`, and DHCP clients. The first patch was incomplete (CVE-2014-7169). The surviving surface: `BASH_ENV`, `ENV`, `ZDOTDIR`, `PROMPT_COMMAND`, `PS4` (expanded under `set -x`), `IFS`, and `PATH` all change what a shell runs. A privileged script must reset them.
- **PATH hijacking.** `.` or a writable directory early in PATH. A root script that calls `ls` unqualified runs whatever `ls` comes first.
- **Temp files.** `/tmp/foo.$$` is predictable, and a local attacker can pre-create a symlink there. Use `mktemp` and a trap that removes it.
- **Secrets.** In argv they are visible in `ps` and `/proc/<pid>/cmdline`. In `set -x` output they land in logs. In `export` they reach every child. Pass by file descriptor or stdin (`--password-stdin`).
- **The destructive tail.** The Steam for Linux bug (https://github.com/ValveSoftware/steam-for-linux/issues/3671, 2015): after the user moved the Steam directory, the launcher "deleted everything on system owned by user". The press coverage quotes the line as `rm -rf "$STEAMROOT/"*` with a "# Scary!" comment (exact line not verified). `"${STEAMROOT:?}"` would have stopped it. ShellCheck SC2115 flags the pattern.

## Anti-pattern catalog

Each entry: pattern, trigger, consequence, fix.

1. **`rm -rf "$D/"*`.** `D` empty or unset. Deletes `/*` as the user (Steam 2015). Use `"${D:?}"`, `set -u`, and an explicit non-empty check before destructive commands.
2. **`cd "$dir"` with no failure check, then destructive work.** `dir` missing. Work runs in the current directory. Use `cd -- "$dir" || exit 1`.
3. **Unquoted `$var`.** Value contains spaces or glob characters. Wrong argv, or the value expands to file names. Quote it, or use an array.
4. **`for f in $(ls)` or `$(find ...)`.** Names with spaces or newlines. Wrong files. Use `for f in ./*` or `find -print0 | while IFS= read -r -d '' f`.
5. **`local x=$(cmd)`.** `cmd` fails. Status 0, the error is lost even under `set -e`. Declare, then assign.
6. **Trusting `set -e` inside a function.** The function is called from `if`, `||`, `&&`, or `!`. Every error inside is ignored. Check explicitly where failure matters.
7. **`((i++))` under `set -e`.** `i` is 0 on bash 4.1 or later. The script exits on the first increment, on Linux only. Use `i=$((i + 1))`.
8. **`pipefail` with an early-closing reader.** `producer | head -1`. Exit 141, script dies. Accept 141 for that pipeline, or avoid pipefail there.
9. **`"${a[@]}"` with `set -u`.** bash before 4.4, which is macOS. "unbound variable" on an empty array. Use `${a[@]+"${a[@]}"}`.
10. **`cmd | while read ...; do var=...; done`.** bash, dash, ash. `var` is empty after the loop. Use `while ...; done < <(cmd)`, a heredoc, or `shopt -s lastpipe`.
11. **`read line`.** Input with backslashes or leading spaces. Mangled data. Use `IFS= read -r line`.
12. **`echo "$x"` for data.** `x` is `-n`, or contains `\t`, on dash, macOS sh, or BusyBox. Different output per shell. Use `printf '%s\n' "$x"`.
13. **Arithmetic on untrusted input.** `$((input))`, `[[ $in -eq 1 ]]`, array subscripts. `a[$(cmd)]` executes. Validate digits-only first.
14. **`eval "$cmd"` with any external data.** Metacharacters. Injection. Use an array: `cmd=(git log -- "$path"); "${cmd[@]}"`.
15. **Unguarded leading-dash filenames.** `rm *`, `mv $f`, `grep $pattern`. A file or value `-rf`. Option injection. Use `--` and `./*`.
16. **`nullglob` with `ls` or `grep`.** No match. `ls` lists the cwd, `grep` hangs on stdin. Collect into an array and test its length.
17. **`find | xargs`.** Names with spaces, quotes, or newlines. Wrong files. Use `-print0 | xargs -0`.
18. **`find -exec sh -c 'cmd {}' \;`.** Hostile filename. Code injection. Use `-exec sh -c 'cmd "$1"' sh {} \;` or the `for f; do` form with `+`.
19. **`#!/usr/bin/env bash -e`.** Run on Linux. Exit 127. Use `env -S`, or `set -e` in the body.
20. **`#!/bin/bash`.** Run on NixOS or Alpine. "not found". Use `#!/usr/bin/env bash` (and accept PATH dependence) or document the platforms.
21. **Bash-isms under `#!/bin/sh`.** Run on Debian or Ubuntu (dash). `[[: not found`, literal `{1..3}`. Run `shellcheck -s sh` and test on dash, or change the shebang to bash.
22. **`sed -i 's/x/y/' f`.** Run on macOS. Error, or a stray backup file. Use `-i.bak` plus `rm`, or a temp file plus `mv`.
23. **`date -d`, `stat -c`, `grep -P`, `readlink -f`.** macOS or BusyBox. Failure or silent empty output. Feature-detect, or move the logic to a real language.
24. **Output from `.zshenv` or a non-guarded `.bashrc`.** scp, rsync, or git over ssh. Protocol corruption, cryptic transfer errors. Print only in interactive sections.
25. **PATH set in `.zshenv` on macOS.** A login shell. `path_helper` reorders it and system tools win. Set PATH in `.zprofile` or `.zshrc`.
26. **Installer with top-level code, run from a pipe.** Truncated download. A half line runs (`rm -rf ~/.tool` cut to `rm -rf ~`). Wrap all logic in `main "$@"` on the last line.
27. **Installer that appends to rc files.** Re-run or uninstall. Duplicate lines, broken shells, a user who no longer trusts the tool. Print the line for the user to add, or write a marked block idempotently and document removal.
28. **`set -x` near secrets.** CI logs. Credentials in clear text in a log that many people can read. `set +x` around secret handling, and pass secrets by file or stdin.
29. **Secrets in argv.** `ps`, `/proc`, shell history. Leaked to every local user. Use an env var for the child, a file descriptor, or stdin.
30. **`ssh host cmd "$arg"`.** Value with spaces or metacharacters. Remote re-parse, wrong command, or injection on the remote host. Quote for the remote shell, or send data on stdin.
31. **`sudo cmd > /root/f`.** The caller opens the redirect. Permission denied. Use `| sudo tee`.
32. **`child_process.exec`, `shell=True`, `system()` with interpolation.** Metacharacters in input. Remote code execution. Pass an argv array.
33. **Batch files with untrusted arguments on Windows.** `cmd.exe` re-parses. Remote code execution. Avoid batch files as targets, require Rust 1.77.2 or later, and audit other runtimes.
34. **GitHub Actions `run:` with a pipeline and no `shell: bash`.** The left side fails. The step stays green. Set `shell: bash` explicitly.
35. **just `{{arg}}` unquoted or single-quoted.** A value with spaces or `'`. Wrong argv or injection. Use `set positional-arguments` and `"$1"`.
36. **`trap ... ERR` without `set -E`.** A failure inside a function. The handler never runs. Add `set -E`.
37. **`[a-z]` ranges in tools under a non-C locale.** Uppercase letters match. Wrong file or line selection. Use `[[:lower:]]` or `LC_ALL=C`.
38. **`/tmp/foo.$$` temp files.** A local attacker pre-creates a symlink. Overwrites another file with the script's privileges. Use `mktemp` and a cleanup trap.
39. **A tool that overwrites `PROMPT_COMMAND` or `precmd`.** Another tool also hooks the prompt. One of them silently stops working. Append to the array, or use `add-zsh-hook`.
40. **`eval "$(tool init)"` for six tools in `.zshrc`.** Every new shell. 200 ms or more of startup, and each binary can run arbitrary code at every shell start. Cache the generated output to a file and source it, or use shims.
41. **A cron or launchd job that calls `node`, `brew`, or `cargo` by name.** Minimal PATH. "command not found" only in the scheduled run. Use absolute paths or set PATH at the top.
42. **A trap handler that ends with `exit 1` on SIGINT.** Parent checks for 130. The parent cannot distinguish "user interrupted" from "failed". Reset the trap and re-raise the signal.
43. **A shell script that parses JSON with `grep` and `sed`.** Reordered keys, escaped quotes, nested objects. Silent wrong values. Use `jq`, or move to a real language.

## Authorities

- **Greg Wooledge's wiki** (mywiki.wooledge.org): BashFAQ, BashPitfalls (65 entries), BashWeaknesses, BashGuide. The most accurate practical reference. The source of the anti-`set -e` position.
- **POSIX XCU Chapter 2, Shell Command Language** (Issue 8, https://pubs.opengroup.org/onlinepubs/9799919799/). The normative text for expansion order, errexit, traps.
- **`man bash`, the GNU Bash manual, and the bug-bash list.** Chet Ramey's answers on the list are authoritative for bash behavior and history.
- **The zsh manual and intro** (zsh.sourceforge.io). Startup files, options, completion system.
- **ShellCheck and its wiki** (Vidar Holen). One page per SC code with the reasoning and the exceptions.
- **Google Shell Style Guide.** The leave-at-100-lines threshold, `/bin/bash` only, declare-then-assign.
- **Aaron Maxwell, "Use Bash Strict Mode".** The strict-mode position at full strength.
- **Rich Felker, "Rich's sh (POSIX shell) tricks".** Correct POSIX sh idioms (trailing-newline preservation, quoting functions), from someone who argues against shell as a programming language.
- **Andy Chu, the Oils blog.** Why shell parsing is hard, and what a stricter shell looks like.
- **Kenton Varda's and idontplaydarts' posts** on `curl | sh`.
- **scop/bash-completion README, fish docs, cobra and clap_complete docs** for completion installation.
- **kitty shell-integration docs** (Kovid Goyal) for a non-invasive integration model.
- **Tom Christiansen, "Csh Programming Considered Harmful".** Historical, and still the reason for "no csh scripts".
- **Jeffrey Snover, Monad Manifesto.** The case for object pipelines.

## Severity rubric

What the levels mean in this domain specifically.

- **blocker**: a destructive command whose target can expand to empty or to `/` (`rm -rf "$D/"*` without `:?`); `eval`, arithmetic, `sh -c`, or `ssh` on untrusted input; a script that runs as root with unqualified commands and an inherited PATH; secrets printed by `set -x` in CI; an installer that executes top-level code from a pipe and does something destructive early.
- **major**: unquoted expansion of a path or user value; error handling that the author believes runs and does not (`local x=$(cmd)`, `set -e` in a function called from a conditional, a pipeline whose left side can fail silently); bash-isms under `#!/bin/sh` on a script that ships to Debian-family systems; a shebang that fails on a supported platform; a tool that overwrites other tools' prompt hooks; a CLI's printed `eval` code that is POSIX-only while it claims fish or PowerShell support.
- **minor**: `echo` for data; `read` without `-r`; GNU-only flags in a script documented as macOS-compatible; a missing cleanup trap for temp files; completion files installed to deprecated names or paths; a ShellCheck disable without a reason.
- **nit**: backticks in place of `$(...)`; `which` in place of `command -v`; `[` where the script is bash and `[[` reads better; style inconsistencies that shfmt fixes.
- **insight**: structural observations. "This script now holds three decisions and a retry loop; it is a program with no tests, and the user's convention says it belongs in TypeScript on Bun with shell as a thin outer layer or none." "Every failure in this file is the same shape: a value crossed a parse boundary nobody drew."

## Changelog

**Source research**: `~/.claude/local/research-notes/shell-scripting-research.md`. Claims tagged V (verified this session, by fetch or by running), F (found, unverified), I (inferred). Behavior tests ran on macOS 26.6.2, Debian stable (bash 5.2, dash 0.5.12), and Alpine (BusyBox 1.37) in Docker. Read it before a refresh: it records that freedesktop GitLab (OSC 133 spec) blocks fetchers with Anubis, that the Maxwell article's TLS certificate had expired, that oils.pub returns 403 to WebFetch but not to curl, and the full gaps list.

- **2026-10-21**: Initial version. Known gaps: OSC 133 spec text, the Monad Manifesto PDF, the exact Steam code line, NixOS `/bin/bash` absence (F), the coreutils version that added `env -S`, the bash version where `((i++))` began to trigger errexit, CVE-2024-27980 and the cross-language BatBadBut list, dax internals, zsh `.zcompdump` and `compinit -C` details, named adherents for fish and nushell as daily drivers, Drew DeVault's position.
