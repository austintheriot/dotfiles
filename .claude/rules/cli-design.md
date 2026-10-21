---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-10-21
---

# CLI design

A reference for designing and reviewing command-line programs. Used by the `cli-design` subagent. The scope is the program as a contract and as a process: argument grammar and parsing, the three standard streams and what goes on each, buffering and broken pipes, exit status, interactivity decisions (TTY detection, prompts, color, pager, editor), configuration and its precedence, output as a machine contract, help and version and completions as surfaces, signals and process lifetime, cross-platform process and path behavior, testing, and the functional-core shape of a CLI. It is not terminal rendering, escape sequences, raw mode, or TUI architecture (`terminal-tui`), not shell code or the completion scripts and init hooks a tool ships into shells (`shell-scripting`), not general API evolution theory for HTTP and libraries (`api-design`), not the wording of messages (`content-design`), and not packaging, signing, and auto-update (`platform-release`).

The unifying model: **a CLI has two audiences on three channels, and every observable behavior is a contract with one of them.** The audiences are a person at a terminal and a program (a script, a pipeline, a CI job, and now a coding agent). The channels are stdin, stdout, and stderr, plus the exit status, argv, and the environment. Most CLI defects are one audience's convenience leaking onto the other audience's channel: a progress message on stdout corrupts a pipe, a prompt hangs a CI job, a color code lands in a log, a reworded table breaks a script, a status count used as the exit code wraps to success. Hyrum's Law (Hyrum Wright: "all observable behaviors of your system will be depended on by somebody") applies to a CLI more strongly than to most APIs, because a CLI has no type system and its consumers scrape text.

The operational question, for every behavior: **who reads this channel, is a person or a program on the other end, and what breaks for the other kind when this changes?**

Empirical priority order, by how often each bites:

1. **Stream discipline.** Diagnostics, progress, or prompts on stdout. Data on stderr. Color or carriage returns emitted to a non-terminal. This is the most common defect, and it is invisible during development, because the developer's terminal shows both streams together.
2. **Exit status that lies.** Partial failure that exits 0, "no results" conflated with errors, an error count used as the status (256 failures exits 0), a wrapper that swallows its child's status, `exit(130)` in place of dying by the signal.
3. **Process-lifetime bugs at the edges.** Output truncated by an early exit (Node `process.exit`), a broken-pipe panic or traceback (`tool | head`), signals not forwarded to a child, a prompt that hangs without a TTY.
4. **Parse grammar that cannot evolve.** Prefix abbreviation, a catch-all subcommand, optional option-arguments, inferred subcommands. Each one turns a future additive change into a breaking change.
5. **Output-as-contract drift.** Human output parsed by scripts with no stable machine mode, locale-dependent machine output, newline-delimited filenames.
6. **Cross-platform process semantics.** Windows argv is one string the callee parses, batch files are parsed again by `cmd.exe`, the current directory is searched before PATH, paths over 260 characters fail without a manifest.

## Volatile surface

`last-verified` (see frontmatter, do not restate the date here). These rot. The rest of this file is comparatively durable.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Parser library versions and defaults (clap, cobra, argparse, commander) | Fast (months) | crates.io, proxy.golang.org `@latest`, registry.npmjs.org, pypi.org JSON. **Not** `gh api .../tags` (returned stale, unsorted tags) or the GitHub "latest release" (lags tags) |
| Python argparse defaults (`color`, `suggest_on_error`) | Medium (changed in 3.14 and 3.15) | docs.python.org/3/library/argparse.html |
| Rust broken-pipe API and stdout buffering | Fast (unstable, redesigned in 2026) | rust-lang/rust issues #97889, #150588, #60673, and `library/std/src/io/stdio.rs` |
| clap_complete dynamic completion | Fast (`unstable-dynamic`) | docs.rs/clap_complete, `env` module |
| Node process I/O, exit codes, compile cache, SEA | Medium (per major) | nodejs.org/api/process.html, module.md, single-executable-applications.md |
| PowerShell native argument passing | Medium | Microsoft Learn `about_Parsing` |
| BatBadBut fix status per runtime | Medium | Each runtime's advisory, Flatt Security's write-up |
| Agent-as-consumer guidance | Very fast (weeks to months, all sources 2026) | The named posts, plus a fresh search |
| Telemetry practice of named tools | Medium | Each tool's analytics or telemetry docs page |
| POSIX XBD 12, XDG basedir, GNU Coding Standards, clig.dev | Slow (years) | pubs.opengroup.org, specifications.freedesktop.org/basedir/latest, gnu.org/prep/standards, clig.dev |

## Lineage: what the originators meant, and what changed

Knowing the original intent lets a reviewer tell a convention that still pays from one that was a 1970s workaround.

- **Doug McIlroy, BSTJ foreword, 1978** (quoted in TAOUP ch. 1): "(i) Make each program do one thing well... (ii) Expect the output of every program to become the input to another, as yet unknown, program. Don't clutter output with extraneous information. Avoid stringently columnar or binary input formats. Don't insist on interactive input." Every modern rule about stdout purity and "never require a prompt" is in that paragraph. The later summary via Peter Salus: "Write programs to handle text streams, because that is a universal interface."
- **Rob Pike and Brian Kernighan, "Program design in the UNIX environment", BSTJ 1984** (the paper behind the "cat -v considered harmful" talk). `cat -v` "confuses what cat's job is... with what it happens to do in a common special case." On BSD `ls` columnating only when output is a terminal: "This ad hoc adjustment of the output format depending on the destination is not only distasteful, it is unique -- no standard UNIX command has this property." "Programs should not attempt special solutions to general problems." This is the founding argument *against* TTY-dependent behavior, which almost every modern CLI now does (see Schools of thought).
- **Eric Raymond, *The Art of Unix Programming*, 2003.** Rule of Silence: "When a program has nothing surprising to say, it should say nothing." Rule of Repair: "fail noisily and as soon as possible", with McIlroy's caveat on Postel's law, "It is the specifications that should be generous, not their interpretation", which is the classic argument against lenient parsing and prefix abbreviation. Rule of Separation: "separate interfaces from engines", which is the library-core-plus-thin-CLI shape. Ch. 11 on confirmations: "Constantly asking for confirmation where the answer is almost always 'yes' conditions the user to press 'yes' without thinking." Ch. 10 calls GNU long options "a patch for the symptom, not a cure".
- **GNU Coding Standards ch. 4.** Every program supports `--help` and `--version`, both to stdout with exit 0. In `--version`, "the version number proper starts after the last space" on the first line. Do not compute the program name from `argv[0]`. Errors use the form `program: message`. 4.5: "don't make the behavior of a utility depend on the name used to invoke it", and do not make it depend on the output device, with one exception: refusing binary output to a terminal. GNU getopt permutes argv by default, which "is not what POSIX specifies".
- **POSIX XBD 12.2, Utility Syntax Guidelines** (Issue 8, IEEE 1003.1-2024). Fourteen guidelines. The ones reviews hit: options are single alphanumerics (G3, `-W` reserved for vendors), options can be grouped behind one `-` with at most one argument-taking option placed last (G5), option-arguments should not be optional (G7), options come before operands (G9), the first `--` ends options (G10), `-` means stdin or stdout (G13). XBD 12.1: an optional option-argument must be attached, so a bare `-f` does **not** consume the next word.
- **Command Line Interface Guidelines, clig.dev (Aanand Prasad, Ben Firshman, Carl Tashian, Eva Parish, 2020).** The modern human-first restatement. Points a general model under-weights: print something within 100 ms; `-d` for debug output because `-v` is ambiguous between verbose and version; for severe operations, make the user type the resource name and accept `--confirm=<name>` for scripts; use explicit words such as `none` for optional values; no secrets in flags *and* no secrets in env vars; no catch-all subcommand; no arbitrary prefix abbreviations; page with `less -FIRX`; put the most important error information last, where the eye lands; prefer opt-in analytics.
- **Jeff Dickey, "12 Factor CLI Apps", 2018 (Heroku, oclif).** On positionals: "1 type of argument is fine, 2 very suspect, 3 never good." `help` as a word argument is ambiguous (Heroku had an app named "help"). "Never output table borders." Startup budget: under 100 ms "not feasible for scripting languages", aim for 100 to 500 ms, 2 s or more is "languid". Heroku uses `topic:command` because a space-separated topic cannot also take an argument. On man pages: "I wouldn't bother... they don't work on Windows."
- **What changed after 2010.** Subcommand CLIs (git, docker, kubectl, cargo, gh) became the dominant shape. Color and TTY-adaptive output became the default in every major library. Structured output (`--json`, git `--porcelain`, FreeBSD's libxo, Kelly Brazil's `jc` for tools that lack it) became expected. Single static binaries (Go, Rust) took over new CLIs, largely on startup time. Since 2025, coding agents became a third consumer class with their own needs (see "Agents as consumers").

## Argument grammar and parsing

### Parse, do not validate

Treat argv, the environment, and config files as untrusted text at the boundary, and parse them once into a typed command value: an enum of subcommands, each with typed, validated fields. After that point no code reads `argv`, `env`, or raw strings. Structurally:

```rust
enum Command {
    Sync { source: SourceSpec, dry_run: bool, jobs: NonZeroUsize },
    Status { format: OutputFormat },
}
fn parse(args: &[OsString], env: &EnvSnapshot, config: &ConfigFile) -> Result<Command, UsageError>
```

A parse failure is a usage error: message to stderr, exit 2. Everything downstream takes `Command` and never fails for reasons of syntax. This is where clap derive, cobra with typed flag structs, or argparse with `type=` callbacks pay off, and where a hand-written `if args[1] == "--foo"` chain fails.

### Grammar decisions that are hard to reverse

- **Prefix abbreviation of long options is a forward-compatibility trap.** glibc `getopt_long`: "Long option names may be abbreviated if the abbreviation is unique." **Python argparse has `allow_abbrev=True` by default.** A script that writes `--verb` for `--verbose` breaks with "ambiguous option" the day `--verbatim` ships. clap's `infer_long_args` is opt-in, which is the right default. Set `allow_abbrev=False` in argparse. Offer explicit aliases.
- **Inferred subcommands** (clap `infer_subcommands`) collide with positional values: `tool te` dispatches to `test`. Keep it off, or set `args_conflicts_with_subcommands`.
- **A catch-all or default subcommand** (`tool <anything>` meaning `tool run <anything>`) means no new subcommand name can ever be added without breaking someone (clig.dev).
- **Optional option-arguments** (`-o [file]`, `--color[=WHEN]`). POSIX G7 says do not. Under XBD 12.1 and getopt `::`, `-o file` treats `file` as an operand. Use a required argument with an explicit value word (`--color=never`), or two flags.
- **Negative numbers.** `--offset -5` parses as an option. argparse accepts a positional `-5` only if no defined option looks like a negative number. Support `--offset=-5`, `--`, or clap's `allow_negative_numbers`.
- **Argument permutation.** GNU getopt permutes, so `tool file --help` shows help. That is convenient for a leaf command and wrong for a **wrapper** (`env`, `sudo`, `timeout`, `nice`, `xargs`, `kubectl exec`, `docker run IMAGE CMD`, `mytool run -- cmd -x`): the wrapper eats `-x` meant for the child. A wrapper must stop parsing at the first operand (`POSIXLY_CORRECT`, a leading `+` in the getopt optstring, clap `trailing_var_arg` or `last`) and must honor `--`.
- **Repeated flags.** Define whether the last wins (Node `parseArgs` with `multiple: false`), values accumulate, or repetition is an error. Then keep the rule.
- **Negation.** `--no-foo` (argparse `BooleanOptionalAction`, Node `allowNegative`, off by default) lets a script override a config-file default. A boolean flag with no negation cannot be turned off from the command line once config sets it.
- **`-` means stdin or stdout** (POSIX G13). Support it wherever a file operand is accepted.
- **`--` ends options** (G10). Always honor it, so a user can pass a filename that starts with `-`.
- **Subcommand style.** Spaces (git, kubectl, gh) or colons (Heroku). Noun-verb (`gh pr create`, which clig.dev calls "more common") or verb-noun (`kubectl get pods`). Pick one and apply it everywhere. git itself is migrating flag-modes to subcommands (`git config --get` to `git config get`), with the old forms listed under "DEPRECATED MODES".
- **Positionals.** Dickey's rule: one kind is fine, two is suspect, three is never good. Past one kind, use flags, because flags are self-describing in scripts and can be reordered and added.

### Plugins

`kubectl-foo-bar` on PATH becomes `kubectl foo bar` (longest match first, underscore in the filename becomes a dash). git (`git-foo`), cargo (`cargo-foo`), and gh extensions use the same pattern. clap's `allow_external_subcommands` enables it. Consequences to review: any executable named `tool-*` on PATH runs with the user's credentials; a built-in added in a later release silently shadows a user's plugin of the same name, or the reverse; plugins must not be able to override built-ins (kubectl refuses).

### argv size limits

Linux `execve(2)`: each string is at most `MAX_ARG_STRLEN`, 32 pages (128 KiB on 4 KiB pages), and the total is capped at a quarter of `RLIMIT_STACK`. Over the limit, `exec` fails with `E2BIG`. Windows `CreateProcessW`: the whole command line is at most 32,767 characters. So `--data "$(cat big.json)"` works in testing and fails in production. Accept `@file` response files, `-` for stdin, or `--input-file`.

### Parser libraries: what each defaults to

**VOLATILE** (versions as of `last-verified`): clap 4.6 (a 5.0 milestone exists, not imminent), cobra 1.10, kong 1.16, urfave/cli v3, commander 15, yargs 18, oclif core 5, click 8.5, typer 0.27, Node `util.parseArgs` (built in).

- **clap (Rust).** Derive or builder. Exit 2 on usage error (stderr), 0 for help and version (stdout), from `Error::exit_code`. Abbreviation and inference are opt-in. `clap_mangen` generates man pages. `clap_complete` generates completions.
- **lexopt, bpaf, argh (Rust).** lexopt is a minimal correct tokenizer for hand-written parsers. argh follows Fuchsia's conventions and is not POSIX-compliant by design.
- **cobra + pflag (Go).** Subcommand trees, generated completions with a hidden `__complete` command, "did you mean" suggestions on by default. Go's standard `flag` package uses single-dash long names (`-verbose`), the X-toolkit style, not POSIX.
- **argparse (Python).** `allow_abbrev=True` (change it). Errors to stderr, exit 2. `color=True` is the default since 3.14. `suggest_on_error` was added in 3.14, with a default of True since 3.15. `fromfile_prefix_chars` (response files) is off by default.
- **click, typer (Python).** Decorators and type hints. Their `CliRunner` gives in-process testing.
- **Node `util.parseArgs`.** `strict: true` by default (unknown options throw), `allowNegative` off, last value wins.

## Streams, buffering, and broken pipes

### What goes where

- **stdout: the product.** Exactly the data the user asked for, in a form the next program can read. Nothing else.
- **stderr: everything addressed to a person.** Errors, warnings, progress, spinners, prompts, "wrote 3 files", deprecation notices, update banners.
- **The test:** `tool args > out.txt` must leave a file that contains only the data, and `tool args 2>/dev/null | next` must still work. A developer never sees a violation in a terminal, because both streams render together.
- **Help** is requested output, so `--help` goes to stdout with exit 0 (GNU 4.8.2, argparse, clap). A *usage error* goes to stderr with exit 2. A tool that prints `--help` to stderr and exits 1 breaks `tool --help | less` and makes scripts treat help as failure.

### Buffering by runtime

- **C stdio:** line-buffered on a TTY, block-buffered on a pipe. So a C program piped through `tee` or `grep` appears stuck until 4 KiB accumulates. Hence `stdbuf -oL`, `grep --line-buffered`, `python -u` and `PYTHONUNBUFFERED=1`. A long-running tool whose output a user watches through a pipe needs an explicit line-buffer or flush option.
- **Rust:** the docs say stdout "is line-buffered when connected to a terminal". **That is misleading.** `stdio.rs` always wraps stdout in a `LineWriter`, with a FIXME saying it should depend on whether stdout is a TTY. Issue #60673 has been open since 2019. Consequence: `println!` in a loop to a pipe performs one `write(2)` and one lock acquisition per line, often 10x or more slower than C (magnitude not measured here). Fix: `let mut out = BufWriter::new(io::stdout().lock());`, write to `out`, and flush explicitly at the end.
- **Node:** on POSIX, writes to pipes and files are **asynchronous**, and writes to TTYs are synchronous. On Windows, pipes are synchronous and TTYs asynchronous. So `process.exit()` after `process.stdout.write(bigJson)` **truncates the output when stdout is a pipe**: `tool --json | jq` gets partial JSON, while the same command in a terminal looks fine. Set `process.exitCode = n` and return from main, so the event loop drains.
- **Python:** block-buffered on a pipe, line-buffered on a TTY. The interpreter flushes stdout again at shutdown, which matters for broken pipes (below).

### SIGPIPE and EPIPE

When the reader of a pipe exits (`tool | head -1`), the next write gets `SIGPIPE`. The default disposition kills the writer silently, which is the right behavior for a filter. Runtimes change this:

- **Rust** sets `SIGPIPE` to ignored before `main`, so a write returns `EPIPE`, and `println!` **panics** with "failed printing to stdout: Broken pipe (os error 32)", exit 101, and a panic message on stderr. The unstable fixes are moving: `#[unix_sigpipe]` and `-Zon-broken-pipe` (tracking issue #97889) are being replaced by an externally implementable item, `#[std::io::on_broken_pipe]` returning `OnBrokenPipe::Inherit` (#150588, opened 2026-01, nightly only). **VOLATILE.** Stable fixes today: write through a `BufWriter` and treat `ErrorKind::BrokenPipe` as a clean exit, or for a pure filter reset `SIGPIPE` to `SIG_DFL` with `libc` at the start of `main`.
- **Python** ignores `SIGPIPE` at startup, so writes raise `BrokenPipeError`, and the shutdown flush raises it *again* after you caught it. The documented recipe in the `signal` module docs: on `BrokenPipeError`, `dup2` a `/dev/null` descriptor onto stdout's fd, then exit. The same docs say: "Do not set SIGPIPE's disposition to SIG_DFL", because a network program then dies on an interrupted socket.
- **Node** emits `EPIPE` as an error event on `process.stdout`. Unhandled, it becomes an uncaught exception.
- The Rust-filter and Python-network advice conflict, and both are right for their program type. A pure filter wants to die quietly. A program that writes to sockets must not die on `SIGPIPE`.

### Reading stdin

- If a command expects data on stdin and stdin is a TTY, it waits for typed input, which looks like a hang. Either print a hint to stderr ("reading from stdin, Ctrl-D to end") or refuse with a usage error when no file operand is given and stdin is a terminal.
- Distinguish "no input" (stdin is a TTY, or closed) from "empty input" (a pipe that delivered zero bytes). They often need different results.
- Stream when the data can be large. Slurping all of stdin before producing output breaks `tail -f | tool` and uses unbounded memory.

### Delimiters

Filenames can contain newlines. Any output that lists paths for another program needs a NUL-delimited mode (`-0`, `--null`, `-z`, matching `find -print0`, `xargs -0`, `git status -z`, all in POSIX since Issue 8 for `find` and `xargs`). git's `-z` also turns off path quoting. JSON Lines (one JSON object per line, with strings escaped) serves both line tools and `jq`.

## Exit status

- **0 success, 1 general failure, 2 usage error.** bash builtins, clap, and argparse all use 2 for usage.
- **Stay within 1 to 125.** 126 means found but not executable, 127 means not found, and 128+N means killed by signal N. The bash manual warns the shell "may use values above 125 specially." An application that exits 130 tells every caller "interrupted".
- **The parent sees only the low 8 bits** (POSIX `exit`: `status & 0377`). An error count used as the exit status exits **0 after 256 failures**. Rust's docs give the example: `process::exit(0x100)` is 0 on Linux and 256 on Windows, where exit codes are 32-bit.
- **"No results" is its own outcome.** The POSIX `grep` model: 0 selected, 1 none selected, greater than 1 for an error. `diff`, `cmp`, `test`, and `git diff --exit-code` use the same shape. A search tool that exits 1 on both "nothing found" and "could not connect" makes scripting impossible. Document which applies.
- **Partial failure is failure.** A batch where 3 of 100 items fail must exit non-zero, with per-item results in the machine output. Exiting 0 turns CI green while work is missing.
- **A wrapper propagates its child's status.** Exit with the child's code, or, if the child died by a signal, die by the same signal (below).
- **`sysexits.h`** (`EX_USAGE` 64, `EX_DATAERR` 65, `EX_TEMPFAIL` 75, and more) is disputed (see Schools of thought).
- **Agent-oriented tools are inventing their own tables.** Arcjet's CLI uses 0 ok, 1 general, 2 auth, 3 validation, 4 confirmation required. Its 2 collides with the usage-error convention. A reviewer must flag a new table that reuses 2 for something other than usage.

## Interactivity decisions

### Detect per stream

`isatty` answers a different question for each fd:

- **stdin is a TTY:** a person can answer a prompt.
- **stdout is a TTY:** a person sees the data, so color, column layout, and a pager are acceptable for data output.
- **stderr is a TTY:** a person sees diagnostics, so progress bars, spinners, and colored errors are acceptable.

The common bug is deciding everything from stdout. **Node's `util.styleText` defaults its `stream` option to `process.stdout`**, so `console.error(styleText('red', msg))` strips color from errors when the user redirects stdout to a file, and writes ANSI codes into `err.log` when the user redirects only stderr. Pass `{ stream: process.stderr }`. Decide color per stream in every runtime. Rust: `std::io::IsTerminal` (stable since 1.70) replaces the unmaintained `atty` crate.

### Color environment

- **`NO_COLOR`** (no-color.org, Joshua Stein, 2017): when present **and non-empty**, disable color. `NO_COLOR=""` means unset. User config and explicit flags override it.
- **`FORCE_COLOR`** (force-color.org, 2023): any non-empty value forces color on. **Conflict:** Chalk and `supports-color` treat `FORCE_COLOR=0` as "off" and 1 to 3 as levels. A tool must pick one reading and document it.
- **`CLICOLOR` / `CLICOLOR_FORCE`:** the bixense.com page now marks itself deprecated. `NO_COLOR` beats `CLICOLOR_FORCE`.
- **`TERM=dumb`:** no color and no cursor movement.
- A `--color=auto|always|never` flag beats every environment variable.
- **CI:** many CI systems set `CI=true` and render ANSI color but are not TTYs. `ci-info` (watson) detects vendors, and `CI=false` disables its detection.
- The full escape-sequence and color-depth story (`COLORTERM`, 256 and truecolor, querying the background) belongs to `terminal-tui`.

### Prompts

- **Never prompt when stdin is not a TTY.** In CI, in an agent, under `yes | tool`, or with `tool < file`, a prompt either hangs forever or consumes piped data as the answer. Fail with a message naming the flag to pass (`--yes`, `--no-input`, `--force`, `--confirm=<name>`).
- **Every prompt has a flag equivalent.** clig.dev and Dickey: "Prompt if you can", never require it.
- **Prompts that read `/dev/tty` directly** (passwords, git's HTTP credential prompt) still hang where a TTY exists but nobody is watching, and fail with `ENXIO` where none exists. git added `GIT_TERMINAL_PROMPT=0` for exactly this. Provide an equivalent environment switch.
- **Destructive operations:** confirmation for severe actions, with the resource name typed (GitHub's repo deletion), and `--confirm=<name>` or `--force` for scripts. Offer `--dry-run`, which prints the plan and changes nothing. A dry run is free when the core returns a plan as data (see "Functional core").
- **Passwords:** no echo, read from the TTY, and never accept them as flag values (below).

### Progress output

Progress, spinners, and live counters go to stderr, only when stderr is a TTY. In CI logs a carriage-return progress bar becomes megabytes of redraw lines. Spinner redraws are re-read by screen readers (GitHub CLI accessibility work, 2025). Offer static text ("Working...") when not on a TTY and an option to disable animation when on one. Rendering mechanics belong to `terminal-tui`.

### Pager and editor

- **Pager:** page only when stdout is a TTY. Precedence, from git: `TOOL_PAGER` > config > `PAGER` > `less`. Set `LESS=FRX` only if `LESS` is unset (git does this): `F` quits if the output fits on one screen, `R` passes color through, and `X` keeps the output on the screen after exit. clig.dev suggests `less -FIRX`.
- **Editor:** git's precedence is `GIT_EDITOR` > `core.editor` > `VISUAL` > `EDITOR` > `vi`, and the value "is meant to be interpreted by the shell". So `EDITOR="code --wait"` works in git. A tool that calls `execvp(getenv("EDITOR"))` gets `ENOENT` for that value. Run the editor through the shell with the file as a positional parameter (`sh -c '$EDITOR "$1"' sh "$file"`), or split with a shell-words parser.
- **Opening a browser:** `BROWSER`, then `open` (macOS), `xdg-open` (Linux), `start` via `cmd` (Windows, with batch-quoting hazards). Print the URL as well, because SSH sessions and containers have no browser.

## Configuration

### Precedence

clig.dev and essentially every mature tool: **flags > environment variables > project config > user config > system config > built-in defaults.** Two refinements:

- **Show where a value came from.** git's `git config --list --show-origin --show-scope` reads system, then global (XDG and `~/.gitconfig`), then local, then worktree, then `GIT_CONFIG_COUNT` variables, then `git -c`. A tool without an origin display generates support tickets ("why is this set?").
- **Project config is untrusted input.** A config file inside a cloned repository can point a tool at a hook or a binary. That is code execution on clone-and-run. Gate anything executable in project config behind an explicit trust step.

### Locations

- **XDG Base Directory spec 0.8:** `XDG_CONFIG_HOME` (default `~/.config`), `XDG_DATA_HOME` (`~/.local/share`), `XDG_STATE_HOME` (`~/.local/state`, for logs and history), `XDG_CACHE_HOME` (`~/.cache`), `XDG_RUNTIME_DIR` (must be mode 0700). **Programs must ignore relative paths** in these variables. Go's `os.UserConfigDir` returns an error for one. A tool that honors a relative value writes config into the current directory.
- **macOS** is disputed (see Schools of thought). Go `os.UserConfigDir` and the Rust `dirs` crate (7.0) return `~/Library/Application Support`.
- **Windows:** `%APPDATA%` (roaming) for config, `%LOCALAPPDATA%` for caches and state. `HOME` is usually unset. Use `USERPROFILE` or the known-folder API.

### Environment variables

Prefix with the tool name (`TOOL_*`). Document each one. Treat them as part of the public interface: renaming one is a breaking change. Loading `.env` files implicitly from the current directory is project config and carries the trust problem above.

### Writing state safely

- **Atomic replace:** write a temp file **in the same directory** as the target, `fsync` it, `rename` over the target, then `fsync` the directory. POSIX `rename` guarantees the name refers to either the old file or the new one. A temp file in `$TMPDIR` fails with `EXDEV` across filesystems, and the copy fallback is not atomic. On Windows, use `MoveFileEx` with `MOVEFILE_REPLACE_EXISTING` or `ReplaceFile`.
- **Secrets files:** create with mode 0600 at `open`. Creating with the umask default and then calling `chmod` leaves a window where the file is world-readable.
- **Concurrent invocations:** two runs of the tool at once will corrupt shared state. Take an advisory lock (`flock`) on a lockfile in the state directory.

## Output as a contract

- **Human output is not stable, and say so.** clig.dev recommends documenting that the human format can change. Then ship a machine format that is stable.
- **Machine formats:** `--json` (or `--format=json`), JSON Lines for streams, NUL-delimited for path lists. git's porcelain v1 is "guaranteed not to change in a backwards-incompatible way" and ignores the user's color and `relativePaths` settings. Porcelain v2's rule for evolution: "Parsers should ignore headers they don't recognize." Additive changes only. Version the format if it must break.
- **Machine output ignores locale.** ISO 8601 timestamps in UTC, `.` as the decimal separator, untranslated keys. Kelly Brazil's `jc`, which converts other tools' human output to JSON, advises setting the locale to `C` for best results, which shows that a script parsing human output depends on locale. Test machine output under `LC_ALL=de_DE.UTF-8`.
- **Stable ordering.** Sort machine output by a documented key. Hash-map iteration order is not a contract, and consumers will depend on it anyway.
- **Tables:** no borders (Dickey). Box-drawing characters read badly in screen readers, break `grep` and `cut`, and break in non-UTF-8 terminals. Never encode meaning only in color. Pair color with a text marker (`error:`, `+`, `-`).
- **`--version`:** the first line is `name version`, with the version after the last space (GNU 4.8.1). Use a constant name, not `argv[0]`. Packagers and scripts parse it.
- **Errors:** `tool: message` (GNU 4.4). Put the most important information last (clig.dev). Name the input, the operation, and the remediation. Wording belongs to `content-design`.
- **Deprecation:** warn on stderr (never stdout), keep the old form working for a stated period, name the replacement, and remove on a major version. A deprecation warning on stdout breaks the very scripts it warns.
- **Typos:** "did you mean" suggestions (git `help.autoCorrect`, whose default only shows the suggestion; cobra; argparse in 3.15) help people. **Auto-running the guess** (`help.autoCorrect=immediate`) is dangerous for any destructive subcommand.

## Help, documentation, and completions

- **`-h` for a short summary, `--help` for full help** (clig.dev). Both to stdout, exit 0. Lead with examples. Show the most common usage first, not an alphabetical flag dump.
- **Help when called with no arguments**, if the command cannot do anything useful without them. Print a short usage to stderr and exit 2. Do not block on stdin.
- **Man pages:** generate them from the parser definition (`clap_mangen`, cobra's doc generator) so they cannot drift. Disputed whether they are worth it (Schools of thought).
- **Completions:** ship them, generated from the parser. Static scripts (clap_complete `aot`, `cobra completion bash` written to disk) drift after an upgrade. Dynamic completion (cobra's hidden `__complete` command returning candidates plus a `ShellCompDirective` bitfield; clap's `COMPLETE=$SHELL tool` protocol, still behind `unstable-dynamic`, **VOLATILE**) calls the binary on every TAB, so completions never drift and can complete runtime values. clap's docs say the script-to-binary interface is unstable, so the registration should be regenerated at each shell startup (`source <(COMPLETE=bash tool)`). **Consequence for the core:** on the completion path, the binary must start fast, print nothing to stdout except candidates (an update banner corrupts the candidate list), never prompt, and never touch the network without a short timeout. Where each shell looks for completion files, and how the scripts behave, belongs to `shell-scripting`.

## Functional core, imperative shell, for a CLI

This is the user's most universal code rule, and a CLI is its cleanest case. The read-decide-write shape:

1. **Shell (edge, impure):** read argv, the environment, config files, stdin, the clock, the relevant filesystem listing, and terminal facts (is each stream a TTY, the width, color support). Parse all of it into plain values.
2. **Core (pure, total):** `fn decide(cmd: Command, world: WorldSnapshot) -> Outcome`. No IO, no clock, no environment reads, no `exit`. It returns a **plan** (what to change, at the domain level) or a **report** (what is true), as data.
3. **Shell (edge, impure):** apply the plan, render the report for the audience (human text if stderr or stdout is a TTY, JSON if `--json`), and map the outcome to an exit status in exactly one place, `main`.

What this buys in a CLI specifically:

- `--dry-run` is "render the plan, skip the apply". It is free and cannot diverge from the real run.
- `--json` and human output are two renderers of one report value, so they cannot disagree.
- The exit status is a pure function of the outcome (`Outcome -> ExitCode`), tested as a table.
- The core is testable with no process spawn, no temp directory, and no terminal.

Anti-patterns that break the shape:

- **`std::process::exit`, `os.Exit`, `sys.exit`, or `process.exit` deep in the core.** Rust's docs: "no destructors on the current stack or any other thread's stack will be run", so `BufWriter`s are not flushed, temp files stay, and lock guards are not released. Node truncates piped output. The function cannot be called from a test. Return an error value up to `main`. In Rust, return `ExitCode` or `Result` from `main`.
- **Printing from the core.** A core that calls `println!` or `console.log` decides the stream and the format. Return the report.
- **`isatty` or `env::var` calls in the middle of logic.** Pass a `Terminal { stdout_tty, stderr_tty, width, color }` value in.
- **Reading the clock in the core.** `Date.now()` inside a "pure" function blocks reproducible tests. Pass `now`.
- **Tests that assert command strings or call order when the result is observable.** Test the plan value and the end-to-end transcript, not the steps. Record calls only when the call itself is the contract: a send, a charge, a negative "never calls X", or an order that an atomicity guarantee depends on.

Lineage: Gary Bernhardt, "Boundaries" (SCNA 2012) and the "Functional Core, Imperative Shell" screencast; Thomas Eizinger, "sans-IO" (Firezone, 2024), with time passed in as input; Raymond's Rule of Separation.

### Testing a CLI

Two layers: unit tests on the pure core (most of the logic), and **transcript tests** on the real binary (the contract: argv in, stdout, stderr, and exit status out).

- **trycmd** (Rust, built on snapbox): `.trycmd` or Markdown files with `$ cmd`, expected output, `? <status>`, and wildcards (`[..]`, `...`, `[CWD]`, `[ROOT]`, `[EXE]`). A README can be the test suite, so the docs never drift. `TRYCMD=overwrite` updates snapshots. **VOLATILE** 1.2.x.
- **assert_cmd** (Rust): `Command::cargo_bin`, assert stdout, stderr, and status separately. **insta** with `insta-cmd` for snapshots.
- **testscript** (Go, `rogpeppe/go-internal`, factored out of the Go toolchain's own `cmd/go` script tests): txtar scripts, `!` to expect failure, `testscript.Main` so the test binary is the CLI, `UpdateScripts` to rewrite expectations.
- **Click / Typer `CliRunner`** (Python): in-process invocation. Whether stderr is separated from stdout by default changed across Click versions. Not verified.
- **bats-core** for shell-level transcripts.
- **Harness hygiene:** in every CLI test, set `NO_COLOR=1`, `COLUMNS=80`, `TERM=dumb`, `LC_ALL=C.UTF-8`, `TZ=UTC`, and point `HOME` and every `XDG_*` variable at a temp directory. Otherwise golden files capture the developer's machine and fail in CI. Assert stdout and stderr **separately**: a test that merges them cannot catch the number one defect in this domain.
- **Test the pipe cases explicitly:** `tool | head -1` (no panic, no traceback), `tool > file` (no ANSI, no progress), `tool < /dev/null` (no hang), and a non-TTY stdin with a destructive command (refuses).

## Signals and process lifetime

- **SIGINT (Ctrl-C) goes to the whole foreground process group**, so a wrapper and its children receive it at the same time.
- **Die by the signal, do not `exit(130)`.** Martin Cracauer, "Proper handling of SIGINT/SIGQUIT": "you have to end the program by killing yourself, not by calling exit." After cleanup, reset the handler to `SIG_DFL` and `kill(getpid(), SIGINT)`. bash's wait-and-cooperative-exit logic stops a loop (`for f in *; do tool "$f"; done`) only when the child died *by* SIGINT. With `exit(130)`, the loop continues to the next file, and the user has to press Ctrl-C once per file.
- **First Ctrl-C: graceful, second: immediate.** On the first, stop starting work, finish or roll back the current unit, clean up, and say so on stderr. On the second, exit now.
- **Wrappers:** prefer `exec` (Unix `execvp`, Rust `CommandExt::exec`) so the child inherits the PID, signals, and status for free. Windows has no `exec`, so spawn, wait, and propagate the code. When spawning, forward `SIGTERM`, `SIGHUP`, and `SIGINT` (if not already delivered by the group), wait for the child, and propagate its status or its signal.
- **Grandchildren that hold pipes.** A wrapper that reads its child's stdout to EOF hangs after the child exits if the child started a daemon that inherited the pipe. Go's `Cmd.WaitDelay` bounds this. Elsewhere, close inherited descriptors and use process groups.
- **PID 1 in a container.** The kernel applies no default signal disposition to PID 1, so `SIGTERM` is ignored unless the program installs a handler, and `docker stop` waits 10 seconds and sends `SIGKILL`. PID 1 must also reap zombies. Use `tini` (`docker run --init`). `tini -g` signals the whole process group, and `-e 143` maps a 143 exit to 0.
- **SIGHUP** arrives when the terminal closes. A long-running tool should either exit cleanly or document `nohup`.
- **Windows:** `SetConsoleCtrlHandler`. Ctrl-C and Ctrl-Break go to every process attached to the console. `SetConsoleCtrlHandler(NULL, TRUE)` ignores Ctrl-C and is **inherited by children**. `CREATE_NEW_PROCESS_GROUP` disables Ctrl-C in the child. Close, logoff, and shutdown events allow a short cleanup, and console and CRT functions may not work during it.

## Cross-platform

### Windows process creation

- **argv is one string.** The caller joins it, and the callee splits it, usually with the MSVCRT rules (`CommandLineToArgvW`): 2n backslashes before a quote become n backslashes and the quote toggles; 2n+1 become n plus a literal quote; `""` inside a quoted region is a literal quote. Caret (`^`) is **not** an escape for MSVCRT. It is a `cmd.exe` escape.
- **Batch files are parsed twice.** `CreateProcess` on a `.bat` or `.cmd` runs `cmd.exe`, which applies its own rules (`%` expansion even inside quotes, `^`, `&`, `|`). **BatBadBut** (RyotaK, Flatt Security, CVE-2024-24576 for Rust, April 2024): untrusted arguments to a batch file were command injection in several language runtimes. Rust 1.77.2 returns `InvalidInput` when it cannot escape safely, and `CommandExt::raw_arg` is the unescaped hatch. Java declined to fix. Go and Python changed documentation only. Node and PHP patched. **VOLATILE.** Mitigation: always spawn an explicit `.exe`, never a batch shim, with untrusted input.
- **Search order:** `CreateProcessW` searches the parent's current directory **before** PATH. A repository that contains `git.exe` runs it. Go 1.19 and later refuse to resolve a program through a relative PATH entry or the current directory (`exec.ErrDot`).
- **Unquoted paths with spaces:** an application name `C:\Program Files\MyApp\app.exe` passed unquoted in the command line can run `C:\Program.exe`. Pass `lpApplicationName`, or quote.
- **Go on Windows:** `os/exec` quotes per `CommandLineToArgvW`. `cmd.exe`, `msiexec.exe`, and batch files use different rules, so set `SysProcAttr.CmdLine` by hand.
- **PowerShell 7.3** changed how it builds native command lines (`$PSNativeCommandArgumentPassing`: `Legacy`, `Standard`, `Windows`). Legacy dropped empty-string arguments and mangled embedded quotes. Windows mode keeps Legacy for `cmd.exe` and script files. `--` is passed through literally to native commands. So the same documented example produces different argv in PowerShell 5.1 and 7.3+. JSON arguments are the worst case. **VOLATILE.**

### Paths and files

- **MAX_PATH is 260** characters including the terminator. The `\\?\` prefix allows about 32,767 for wide-character APIs, disables normalization (`/`, `.`, `..` are not interpreted), and cannot be relative. Opt-in long paths (Windows 10 1607 and later) need **both** the `LongPathsEnabled` registry value **and** a `longPathAware` entry in the application manifest. A CLI without the manifest fails on deep `node_modules` or git checkouts even when the user set the registry value.
- Accept both `/` and `\` on Windows input. Print native separators in human output. Pick one, and document it, for machine output.
- **Case-insensitive filesystems:** the macOS default (APFS case-insensitive) and Windows (NTFS by default). Two files that differ only by case collide. Renames that change only case need a two-step rename on some systems.
- **Line endings:** read both `\n` and `\r\n`. Write `\n` unless the format demands otherwise.
- **Executable bit and symlinks:** Windows has neither in the Unix sense. Creating a symlink needs Developer Mode or a privilege (not verified this pass).
- **Encoding:** Python 3.15 enables UTF-8 mode by default (PEP 686), which "mostly affects Windows users" and can cause mojibake or "even silent data corruption" in code that relied on the locale encoding. Pass `encoding=` explicitly. Rust writes to the Windows console with `WriteConsoleW`, but a pipe gets raw bytes. A console code page that is not UTF-8 still exists on older setups.

### Unix userland

A CLI that shells out to `sed`, `date`, `stat`, `readlink`, or `grep` inherits the GNU-versus-BSD split (macOS ships BSD tools). The matrix is in `shell-scripting`. The better fix for a CLI is not to shell out for these at all: do it in-process.

### Binaries

Static musl builds run on any Linux, with slower `malloc` and DNS resolver differences. glibc builds need a glibc at least as new as the build machine's. macOS quarantine and Gatekeeper, signing, and notarization belong to `platform-release`.

## Credentials and secrets

- **Never accept a secret as a flag value.** It is visible to every local user in `ps` and `/proc/<pid>/cmdline`, stored in shell history, printed in CI logs, and now recorded in coding-agent transcripts. docker's `--password-stdin` docs: it "prevents the password from ending up in the shell's history, or log-files."
- **Channels in order of safety:** OS keychain or credential helper (git credential helpers, `docker-credential-osxkeychain`), a file with mode 0600, stdin, environment variable. The environment variable is disputed (Schools of thought).
- **Do not print secrets in debug output**, including `--verbose` request dumps and environment dumps.

## Startup latency

- Dickey's thresholds (2018, one author's rule of thumb): under 100 ms feels instant, 100 to 500 ms is the target, 2 s or more and users avoid the tool. clig.dev: print *something* within 100 ms.
- Go and Rust binaries start in single-digit milliseconds (not measured here). Node and Python pay interpreter startup plus import cost.
- **Python:** `python -X importtime` shows per-module cost. PEP 810 explicit lazy imports (`lazy import json`) were accepted for 3.15. Import heavy modules inside the subcommand that needs them.
- **Node:** `module.enableCompileCache()` (stable since v25.4 and v24.15) caches compiled code. Single executable applications (`--build-sea`) bundle the runtime. Lazy-load each subcommand's module (oclif's approach reaches about 150 ms).
- **Startup side effects** run on every invocation, including every completion TAB: update checks, telemetry pings, config migrations, plugin scans. Make them asynchronous, time-bounded, and skipped on the completion path.

## Telemetry

- **The tool must never block on telemetry.** Offline, behind a captive portal, or on a slow network, a synchronous ping adds seconds or hangs (clig.dev calls these "time bombs"). Fire and forget with a short timeout, or batch and upload later.
- **Disclose before the first event**, name what is collected, and provide one switch to disable it (`TOOL_NO_TELEMETRY=1` or a config value).
- Go's toolchain telemetry defaults to `local` (collected, never uploaded). `go telemetry on` uploads. Homebrew is opt-out (`HOMEBREW_NO_ANALYTICS=1`), with a notice before the first event, 365-day retention, and no option values recorded.
- `DO_NOT_TRACK=1` was proposed as a cross-tool switch. The original site now serves unrelated content, so do not cite its URL. Dagger honors the variable.

## Agents as consumers

**VOLATILE, VERY FAST.** Every source here is from 2026 and most are vendor posts. Expect reversals.

Coding agents now run CLIs more often than people do for some tools. They read stdout and stderr into a context window, pay per token, cannot answer prompts, and hallucinate arguments. What practitioners have published:

- **Justin Poehnelt (Google Workspace CLI, 2026-03):** accept raw JSON payloads and offer runtime schema introspection; support field masks because "Agents pay per token"; page with NDJSON; "Agents hallucinate. Build like it.": reject control characters and `?`, `#`, `%` in IDs, and sandbox output paths.
- **Waldek Mastykarz (Microsoft, 2026-07), the only empirical study found:** with flat flag arguments, all five models tested were correct 5 of 5 times. With a JSON-payload argument, one small model scored 2 of 5 and another 3 of 5, and JSON cost 4 to 11 times more tokens (9x on PowerShell, 1.5x on bash). Conclusion: "Keep your args." The sample is small and synthetic, and it measures JSON *input*, not JSON output.
- **Arcjet CLI (Norvell and Mytton, 2026-06):** JSON output by default when stdout is not a TTY; a confirmation envelope with a `confirmCommand` field, "a protocol, not a prompt", with exit 4 meaning confirmation required; typo suggestions disabled (a hard failure is safer than a guess for an agent); the interface frozen at 1.0 with additive changes only.
- **Elastic CLI (Mock and Ryan, 2026-09):** `--help --json` prints the input schema; without `--yes`, a non-interactive run refuses with a structured error; API keys in the OS keychain so they stay out of agent transcripts; administrator allow and block lists of commands.

What follows for a reviewer, durable even if the details move: everything in this file about non-TTY behavior, stable machine output, no prompts without a TTY, exit statuses that mean one thing, and no secrets in argv matters *more* when an agent is the consumer, not less. The genuinely new demands are machine-readable help (a schema) and output economy (field selection, no decorative text).

## Schools of thought

### Should behavior depend on whether output is a terminal?

**Yes (clig.dev "use TTY detection"; Dickey; Arcjet's JSON-when-not-a-TTY; every color library; BSD and GNU `ls`).** A person and a program want different things. Color, columns, progress, and pagers help a person and break a program. Detecting the TTY gives each audience the right default with no flags. The alternative is making every human type `--color --columns` forever, or making every script type `--plain`.

**No (Pike and Kernighan 1984: "not only distasteful, it is unique"; GNU 4.5: "don't make the behavior... depend on the type of output device... do not compromise it merely to save someone from typing an option").** A program whose output changes depending on where it goes is harder to reason about and to debug: the output you see in the terminal is not the output the script got. Special cases belong in separate tools (columnation in `column`, paging in `less`), not inside every command.

Narrow agreement: never send ANSI codes or carriage-return progress to a non-TTY, and refusing binary output to a terminal is fine. The live question is *content and format* switching (columns, JSON). Arcjet's auto-JSON is the strongest current form of "yes" and makes both formats a contract.

### `sysexits.h`

**For (Eric Allman, who created it in 1980 for delivermail and sendmail; mail transfer agents that act on 75 `EX_TEMPFAIL` by retrying; Python `os.EX_*`; the Rust `exitcode` crate).** Exit codes are the only structured error channel a process has. A shared table of classes (usage, data, unavailable, temporary failure, config) lets a caller decide to retry or give up without parsing text.

**Against (FreeBSD's man page: "deprecated... Its use is discouraged"; OpenBSD: "Do not use them"; clap, argparse, and bash builtins all use 2 for usage, which conflicts with `EX_USAGE` 64).** The table was designed for mail delivery, almost no caller checks for its values, and its classes do not fit most tools. Using it adds a second usage-error code to an ecosystem that already agreed on 2.

### Color by default

**For:** git `color.ui=auto` (since 1.8.4), argparse `color=True` (3.14), Dickey ("Be fancy"). Color carries information quickly to a person on a TTY.

**Against or limited:** the Rule of Silence and McIlroy's "Don't clutter output"; the GitHub CLI accessibility findings ("a terminal's background color is not set by the application", so chosen colors can be unreadable, hence their restriction to the 16 ANSI colors that the user's theme controls); `NO_COLOR`. Sub-dispute: whether `FORCE_COLOR=0` means off (Chalk) or on (force-color.org).

### Rule of Silence or report state changes

**Silence (McIlroy, Raymond):** "When a program has nothing surprising to say, it should say nothing." Output that is always the same carries no information and trains people to ignore output.

**Report (clig.dev: "Report state changes", print something within 100 ms, suggest the next command):** a silent success after a state-changing command leaves the person unsure that anything happened. Both camps agree that any such report goes to stderr.

### Prompts at all

**Against (McIlroy 1978 "Don't insist on interactive input"; Raymond on confirmations training "yes"; Arcjet's "a protocol, not a prompt").** A prompt is an interface only a person at a TTY can use. Every prompt is a place where automation hangs.

**For, with conditions (clig.dev and Dickey: "Prompt if you can", never require it; Elastic prompts only on a TTY).** For a person learning a tool, a prompt for a missing value is friendlier than an error, provided every prompt has a flag and non-TTY runs never prompt.

### Prefix abbreviation

**For (GNU `getopt_long` default; argparse `allow_abbrev=True` default):** convenient for people typing long options interactively.

**Against (clig.dev "Don't allow arbitrary prefix abbreviations"; clap's opt-in default; McIlroy's "specifications should be generous, not their interpretation"):** every abbreviation a script uses becomes a reason the tool cannot add an option. The convenience is for the person, and the breakage is for the script.

### "Did you mean"

**For (git, cobra, argparse 3.15, clig.dev "suggest, ask before running"):** cheap help for a person. **Against (Arcjet disables suggestions for agents):** an agent will act on the suggestion text, and a hard error is safer. Everyone agrees that auto-running the guess for a destructive command is dangerous.

### Argument permutation

**Permute (GNU):** `tool file --verbose` works, which matches how people edit command lines. **Do not (POSIX G9):** options before operands, which is the only grammar where a wrapper can pass the rest of the line to a child unchanged. Leaf commands can permute. Wrappers must not.

### XDG on macOS

**`~/.config` (git, Dickey for config, dotfile-sync users, many open-source CLI authors):** one location across Linux and macOS, easy to version in a dotfiles repository, and the same path in every doc. **`~/Library/Application Support` (Go `os.UserConfigDir`, the Rust `dirs` crate, Apple's platform guidance):** the platform convention, backed up by Time Machine, and where macOS tools expect application data. The complaint thread is xdg-rs/dirs#45. Dickey splits it: config in XDG paths, cache in `~/Library/Caches`. Whatever a tool picks, it must honor `XDG_CONFIG_HOME` when the variable is set.

### Subcommand separators and word order

Spaces (git, kubectl, gh) against colons (Heroku: a topic separated by a space cannot also take an argument). Noun-verb (gh, which clig.dev calls more common) against verb-noun (kubectl). These do not have a right answer. Consistency inside a tool is the requirement.

### JSON or text streams

**Structured (Jeffrey Snover's Monad Manifesto and PowerShell objects; nushell values; FreeBSD libxo; `jc`; Poehnelt: "Rewrite your CLI" for agents; Elastic's schema-in-help).** Text parsing is the largest source of brittle integration. Structured output ends a whole class of breakage and makes evolution additive.

**Text (McIlroy: "text streams... universal interface").** Every Unix tool can consume lines. JSON needs `jq` and a schema, and pretty-printed JSON is hostile to `grep`. The Mastykarz evidence favors flat flags over JSON *input* for agents. No comparable evidence exists for JSON *output*.

Middle path used in practice: human text by default, `--json` on request, JSON Lines for streams.

### Secrets in environment variables

**For (the twelve-factor app methodology, Adam Wiggins; agent CLIs such as Arcjet's `ARCJET_TOKEN` and Google Workspace's env auth; every CI system's masked secrets).** It is the only non-interactive channel many CI systems and orchestrators offer, it does not appear in `ps` output for other users, and it is easy to scope to one process.

**Against (clig.dev: "Don't read secrets from environment variables").** Environment variables leak to every child process, appear in `/proc/<pid>/environ` (same user), crash dumps, and debug output that prints the environment, and they persist in shells.

### Telemetry opt-in or opt-out

**Opt-in (clig.dev "Prefer opt-in"; the Go toolchain's final design, after Russ Cox reversed himself on 2023-02-24).** Users did not agree to it, and a CLI runs on machines with sensitive data.

**Opt-out with disclosure (Homebrew, the .NET CLI, Next.js; Russ Cox's own earlier argument on golang-dev, 2023-02-08).** Opt-in rates are tiny and skewed toward power users, so the data misleads the maintainers about real usage, and opt-in checkboxes tend to justify collecting more. Both positions were argued at full strength by the same person two weeks apart.

### Single static binary or a runtime; self-update or package manager

**Binary (clig.dev "Distribute a single binary where possible"):** no runtime version conflicts, instant startup. **Runtime (Dickey: Node is fast enough at about 150 ms with lazy loading; Node SEA, Python lazy imports):** faster development, a shared ecosystem. **Self-update (rustup and similar):** users get fixes. **Package manager ownership (Homebrew, distributions):** a binary that rewrites itself conflicts with the package manager's file database (this last point is inferred). Detail belongs to `platform-release`.

### Man pages

**Yes (GNU, BSD; generated with mandoc or `clap_mangen`):** offline, searchable with `apropos`, the expected place on Unix. **No (Dickey: "I wouldn't bother... they don't work on Windows"):** `--help` and web docs cover it. clig.dev: consider both.

### Behavior by `argv[0]`

**Against (GNU 4.5):** a symlink or rename changes behavior, which surprises users and packagers. **For (BusyBox and uutils multicall binaries, `gunzip`, `vi` as vim):** one binary, many tools, smaller installs.

## Anti-pattern catalog

Each entry: pattern, trigger, consequence, fix.

1. **`process.exit()` after stdout writes (Node).** stdout is a pipe on POSIX. Output truncated, `tool --json | jq` gets partial JSON, and it works in a terminal. Set `process.exitCode` and return.
2. **`println!` to a pipe (Rust).** `tool | head`. Panic "failed printing to stdout: Broken pipe", exit 101. Handle `BrokenPipe` as clean exit, or reset `SIGPIPE` in a pure filter.
3. **Default Rust stdout for bulk output.** Many short lines to a pipe. One `write(2)` per line, much slower than C. `BufWriter::new(stdout().lock())` and an explicit flush.
4. **Python `BrokenPipeError` at shutdown.** `tool | head`. Traceback on stderr even after catching. The `dup2`-devnull recipe from the `signal` docs.
5. **Status or progress on stdout.** stdout piped or redirected. Consumers parse "Downloading..." as data. All diagnostics to stderr.
6. **Color decided by stdout for stderr text.** `tool > out.json` or `tool 2> err.log`. No color on screen, or ANSI in the log. Decide per stream (`styleText(..., { stream: process.stderr })`).
7. **`NO_COLOR=""` treated as set.** A template exports it empty. Color disabled against the spec. Check present and non-empty.
8. **Prompting when stdin is not a TTY.** CI, agents, `yes | tool`. Hang, or piped data consumed as the answer. Check `isatty(stdin)` and fail with the flag to pass.
9. **`/dev/tty` prompts with no opt-out.** An agent or a detached job. Hang or `ENXIO`. Provide `--password-stdin`, a credential helper, and a `TOOL_TERMINAL_PROMPT=0` switch.
10. **Secret as a flag value.** `tool login --token abc`. In `ps`, history, CI logs, agent transcripts. `--token-stdin`, file, keychain.
11. **Long-option prefix abbreviation on.** A script uses `--verb`. "ambiguous option" after `--verbatim` ships. `allow_abbrev=False`, explicit aliases.
12. **Inferred subcommands with positionals.** `tool te`. Dispatches to `test`. Keep inference off.
13. **Catch-all subcommand.** `tool <anything>` means `tool run`. Can never add a subcommand. No catch-all.
14. **`help` as a word argument.** A resource named "help". Shows help instead of acting. Reserve only `-h` and `--help`.
15. **Optional option-arguments.** `-o file`. `file` becomes an operand. Required arguments with value words.
16. **Negative numbers as values.** `--offset -5`. Parsed as an option. Accept `--offset=-5` and `--`.
17. **Permuting parser in a wrapper.** `mywrap cmd -x`. The wrapper eats `-x`. Stop at the first operand and honor `--`.
18. **Error count as exit status.** 256 failures. Exits 0. Cap at 1 to 125, put counts in output.
19. **Exit codes above 125 for application errors.** Using 126, 127, or 130. Callers read "not executable", "not found", "interrupted". Stay in 1 to 125.
20. **"No results" and "error" share a code.** An empty search and a network failure both exit 1. Scripts cannot tell them apart. The `grep` model, documented.
21. **Partial failure exits 0.** 3 of 100 items fail. CI green with missing work. Non-zero unless all succeeded, per-item results in `--json`.
22. **`exit(130)` on SIGINT.** Ctrl-C in a shell loop. The loop continues to the next item. Reset to `SIG_DFL` and re-raise.
23. **Wrapper that does not forward signals.** `SIGTERM` to the wrapper. Child orphaned or killed without cleanup, status lost. `exec`, or forward, wait, propagate.
24. **CLI as PID 1 with no handlers.** `docker stop`. 10-second hang, then `SIGKILL`, plus zombies. `tini` or `--init`.
25. **Grandchild holds the pipe.** The child started a daemon. Wrapper hangs after the child exits. `WaitDelay`, close inherited fds, process groups.
26. **`exit` deep in the core.** An early-return path in library code. Destructors skipped, buffers unflushed, untestable. Return to `main`.
27. **Non-atomic state write.** Crash or Ctrl-C mid-write. Truncated config, tool fails on next start. Temp file in the same directory, `fsync`, `rename`.
28. **Temp file in `$TMPDIR`, then rename.** Different filesystem. `EXDEV`, non-atomic copy fallback. Create the temp beside the target.
29. **Secrets file created, then `chmod`.** umask 022. World-readable window. Create with 0600.
30. **`execvp($EDITOR)`.** `EDITOR="code --wait"`. `ENOENT`. Run through the shell, as git does.
31. **Pager when not interactive.** `tool log > file`, or an agent. Pager blocks or writes escapes. Page only when stdout is a TTY.
32. **Carriage-return progress in CI.** stderr not a TTY. Megabytes of redraw lines. Progress only on a TTY, static text otherwise.
33. **Box-drawing tables, color-only meaning.** Screen reader, `grep`, `NO_COLOR`. Unreadable or meaning lost. Plain rows, text markers.
34. **Scripts parse human output with no machine mode.** Column order changes in a minor release. Downstream breakage. `--json` or `--porcelain` with a stability promise.
35. **Format switches silently when not a TTY.** A user pipes to `grep` expecting text. Surprise JSON, and now both formats are contracts. Prefer explicit `--json`. If automatic, document it as a contract.
36. **Locale-dependent machine output.** `LANG=de_DE`. Decimal commas, translated words, local dates break parsers. ISO 8601 UTC, `.` decimals, test under a non-English locale.
37. **Newline-delimited filenames.** A name contains `\n`. Wrong splits downstream, injection. `-0` / `--null` / `-z`.
38. **Huge single argv value.** `--data "$(cat big.json)"`. `E2BIG` past 128 KiB on Linux, 32,767 chars on Windows. `@file`, `-`, `--input-file`.
39. **Spawning `.bat` / `.cmd` with untrusted arguments.** Windows. Command injection (BatBadBut). Spawn an explicit `.exe`.
40. **Resolving executables from the current directory.** Windows search order, or old Go. A planted `git.exe` runs. PATH-only resolution, absolute paths.
41. **Unquoted program path with spaces.** `C:\Program Files\...`. Runs `C:\Program.exe`. Quote, or pass `lpApplicationName`.
42. **No `longPathAware` manifest.** Deep paths on Windows. Failures even with the registry key set. Ship the manifest.
43. **Side effects on the completion path.** TAB. Slow completion, or an update banner corrupts candidates. Keep that path pure and quiet.
44. **Static completion script never regenerated.** Upgrade adds flags. Stale completions. Regenerate at shell startup or on install.
45. **Telemetry that blocks.** Offline or captive portal. Command hangs or adds seconds. Asynchronous, time-bounded, opt-in.
46. **Unparsable `--version`, or name from `argv[0]`.** Packagers parse it. Wrong name via symlink, broken parsing. Constant name, version after the last space.
47. **`--help` to stderr with non-zero status.** `tool --help | less`. Empty pager, scripts treat help as failure. stdout, exit 0.
48. **No config origin display.** "Why is this set?" Support burden. `config list --show-origin`.
49. **Relative XDG variable honored.** `XDG_CONFIG_HOME=cfg`. Config written into the current directory. Ignore relative values.
50. **Tests capture the developer's environment.** Golden files under the developer's color, width, locale, `HOME`. Flaky CI. Set `NO_COLOR`, `COLUMNS`, `LC_ALL`, `TZ`, `HOME`, `XDG_*` in the harness.
51. **Tests that merge stdout and stderr.** Any test. Cannot catch stream-discipline defects. Assert each stream separately.
52. **Deprecation warning on stdout.** A script uses the old flag. The warning corrupts the script's data. stderr only.
53. **Project config that can name an executable, with no trust step.** Clone a hostile repo, run the tool. Code execution. Gate executable config behind explicit trust.
54. **Destructive command with no `--dry-run` and no confirmation path for scripts.** Automation needs to run it. Users either cannot automate or must bypass all safety. `--dry-run`, `--confirm=<name>`.

## Authorities

- **POSIX XBD 12, Utility Conventions** (Issue 8, https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/V1_chap12.html). Normative option syntax. The POSIX `grep`, `exit`, and `rename` pages for exit-status and atomicity contracts.
- **GNU Coding Standards ch. 4** (4.4 errors, 4.5 interfaces, 4.8 command-line interfaces). `--help` and `--version` format, error format, the device-independence argument. gnu.org blocks automated fetches.
- **glibc `getopt(3)` and `getopt_long(3)`** (man7.org). Permutation, abbreviation, `POSIXLY_CORRECT`.
- **clig.dev.** The modern human-first checklist.
- **Jeff Dickey, "12 Factor CLI Apps"** (2018). Help, tables, speed budget, topics.
- **McIlroy 1978, Pike and Kernighan 1984, Raymond's *The Art of Unix Programming*.** The original intent, and the arguments against TTY-dependence, chatter, and lenient parsing. Kernighan and Pike, *The Unix Programming Environment* (1984), and Mike Gancarz, *The UNIX Philosophy* (1995), were not read for this file.
- **git documentation and source.** The best worked example of a CLI with a stable machine contract: porcelain formats, pager and editor precedence, config origin, autocorrect, deprecation by subcommand migration, `GIT_TERMINAL_PROMPT`.
- **Runtime docs:** Node `process` (stream sync and async table, exit codes), Python `signal` and `argparse`, Rust `std::process` and `std::io` plus rust-lang/rust issues #97889, #150588, #60673, Go `os/exec` and https://go.dev/blog/path-security.
- **Microsoft Learn:** `CreateProcessW`, C runtime argv parsing, `SetConsoleCtrlHandler`, maximum path length, PowerShell `about_Parsing`.
- **Rust advisory for CVE-2024-24576 and Flatt Security's BatBadBut write-up (RyotaK).**
- **Martin Cracauer, "Proper handling of SIGINT/SIGQUIT"** (https://www.cons.org/cracauer/sigint.html).
- **no-color.org, force-color.org, the XDG basedir spec, tini's README.**
- **Gary Bernhardt "Boundaries", Thomas Eizinger "sans-IO", Hyrum Wright (hyrumslaw.com).**
- **Test harness docs:** trycmd and snapbox, assert_cmd, testscript, clap_complete, cobra completions.
- **Agent-era posts:** Poehnelt (2026-03), Mastykarz (2026-07, the only data), Arcjet (2026-06), Elastic (2026-09).
- **GitHub CLI accessibility post** (Ryan Hecht and Andrew Feller, 2025-05-02).
- **Russ Cox, "Opting In to Transparent Telemetry"** (research.swtch.com), go.dev/doc/telemetry, docs.brew.sh/Analytics.

## Severity rubric

What the levels mean in this domain specifically.

- **blocker**: a secret accepted as a flag value or printed in output; spawning a batch file or a shell with untrusted arguments; executing code named in project config without a trust step; a destructive command that prompts with no non-TTY path, or that runs without confirmation when it can destroy unrecoverable data; partial failure or an error count that exits 0 in a tool used in CI.
- **major**: data and diagnostics mixed on stdout; a prompt reachable when stdin is not a TTY; Node `process.exit` after output, or a Rust or Python broken-pipe crash in a tool meant for pipelines; a wrapper that loses its child's exit status or signals; `exit(130)` in a tool users loop over; prefix abbreviation or a catch-all subcommand in a tool with script users; human output that scripts parse with no machine mode; non-atomic writes of config or state; machine output that varies with locale.
- **minor**: color decided by the wrong stream; `NO_COLOR` empty-string handling; `--help` on stderr or with a non-zero status; an unparsable `--version`; default Rust stdout for bulk output; missing `--` handling; a pager or spinner when not interactive; static completions that drift; no config-origin display.
- **nit**: help ordered alphabetically instead of by frequency; `-v` used for verbose when it collides with version expectations; inconsistent subcommand word order; table borders.
- **insight**: structural observations. "Every flag here is read from `env::args()` in the function that uses it; there is no parse step, so there is no single place to validate input, generate help, or test the grammar." "The core prints and exits in nine places; returning a report value would give `--json`, `--dry-run`, and the exit-status table for free." "Three of these defects are one mistake: the tool decides human-versus-program once, from stdout, at startup."

## Changelog

**Source research**: `~/.claude/local/research-notes/cli-design-research.md`. Claims tagged VERIFIED (primary source fetched 2026-10-21), FOUND-UNVERIFIED, or INFERRED, with a gaps list. Read it before a refresh: it records that gnu.org and Medium refuse plain fetching but serve Playwright, that catb.org's HTTPS certificate is broken (use http), that `gh api .../tags` returned stale versions (use the package registries), and that consoledonottrack.com now serves spam.

- **2026-10-21**: Initial version. Library versions verified against crates.io, proxy.golang.org, npm, PyPI. Rust broken-pipe and buffering state verified against rust-lang/rust issues and `stdio.rs`. Known gaps: Click `CliRunner` stderr separation, Windows console-handler threading and timeouts, whether Rust `Command` resets `SIGPIPE` in children, Python's SIGINT re-raise on uncaught `KeyboardInterrupt`, Chalk `FORCE_COLOR` levels, macOS config directories of `gh` and `platformdirs`, telemetry variable names of Next.js, Gatsby, and .NET, Bun `--compile` and self-upgrade commands, Windows symlink privilege and console code pages, startup benchmarks (none measured), response-file support across parsers, four unread agent-CLI sources.
