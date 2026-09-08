# Rust error handling at the binary edge (2026-09-06)

Research pass on one question: for `crates/config-manifest`, which error-handling
library belongs at the binary edge, where errors collapse to a message and an
exit code? The library crates use `thiserror`. That is decided and not in
question here.

Candidates: `anyhow` (the incumbent), `eyre`, `color-eyre`, `miette`, `snafu`.
Plus the null hypothesis: no library at the edge at all.

**Verdict up front.** Keep `anyhow`, and keep the `fn main() -> ExitCode`
shape the crate already has. No candidate library is better for this binary,
and two are actively worse. The measured reasons are not the ones the brief
expected:

1. **Startup latency does not discriminate.** Every candidate measures
   6.95-8.53 ms against a 7.2-8.0 ms no-dependency baseline, with rep-to-rep
   variance larger than the between-candidate spread. `color_eyre::install()`
   is not measurably expensive. The latency argument I was asked to build does
   not exist, so I am not going to build it.
2. **Dependency count does not discriminate either**, because the Dockerfile
   already pre-builds dependencies in a cached layer
   (`tests/docker/Dockerfile:21-26`). Warm rebuild after touching `main.rs` is
   0.79-0.88 s for every candidate, including a 34-crate one.
3. **The non-TTY output is where candidates fail, and two fail badly.**
   `color-eyre` writes ANSI escapes into a pipe with no TTY detection anywhere
   in its source. `miette` forces a choice between a `Debug` struct dump and a
   34-crate tree that writes ANSI plus Unicode box-drawing into a pipe.
4. **The decisive constraint is the exit code, and it eliminates the library
   question entirely.** `fn main() -> Result<(), E>` exits **1**, always, for
   every library tested. Precise exit codes require `fn main() -> ExitCode`,
   and once `main` returns `ExitCode` the edge library is doing nothing but
   formatting a string.

The honest summary: the incumbent wins, the exciting candidates lose on
verified grounds, and the null hypothesis is a close second that would be a
defensible choice for different reasons.

## Measurement method

macOS 26.5, Darwin 25.5.0, arm64. cargo 1.94.0, rustc 1.94.0. hyperfine is not
installed, so every latency number comes from a zsh loop using `EPOCHREALTIME`
with one warm-up call discarded:

```zsh
#!/bin/zsh
zmodload zsh/datetime
n=$1; shift; label=$1; shift
"$@" >/dev/null 2>&1                       # warm
start=$EPOCHREALTIME
for i in {1..$n}; do "$@" >/dev/null 2>&1; done
end=$EPOCHREALTIME
printf '%-16s %7.3f ms/call (n=%d)\n' "$label" $(( (end-start)*1000.0/n )) $n
```

Every candidate is a real compiled binary that constructs a real wrapped error
and returns it from `main`, built with `[profile.release] strip = true`.
Resolved versions: `anyhow` 1.0.104, `thiserror` 2.0.20, `eyre` 0.6.14,
`color-eyre` 0.6.5, `miette` 7.6.0, `snafu` 0.8.9 and 0.9.2.

Every number in this document is **verified** (I ran it) unless the text says
otherwise. Where I did not verify something, the text says so.

**One methodology warning, because it corrupted my first result set.** Cargo
unifies features across a workspace. My first bench workspace contained both
`miette` and `miette` with the `fancy` feature, so the plain-`miette` binary
silently got `fancy` too and I recorded ANSI escapes for a configuration that
does not emit them. Every miette and color-eyre finding below was re-run in a
**separate single-crate workspace** to defeat unification. Anyone re-running
these numbers in one workspace will get the wrong answer for `miette`.

## 1. What this binary actually is

Four constraints, read from the repo rather than assumed.

**It runs from git hooks.** `tests/pre-push:146` calls
`config-manifest check mac linux` and branches on the exit status.
`tests/pre-push:133-141` calls `config-manifest --stamp` and compares the
output. Neither is a TTY.

**It runs in CI, and the consumer greps combined output.**
`.github/workflows/branch-drift.yml:57-65`:

```yaml
output=$(config-manifest check 2>&1)
...
printf '%s' "$output" | grep -q '^diverged: ' && has_drift=1
printf '%s' "$output" | grep -q 'match no .sync-manifest rule' && has_unlabeled=1
```

`2>&1` merges the streams, so anything a library writes to stderr lands in the
same buffer the workflow greps. One of those greps is **anchored to column 1**.

**The test suite compares stderr byte for byte.**
`tests/check-branch-drift.test.sh:170-180`:

```sh
stderr_only=$(run_check "$repo" mac linux 2>&1 >/dev/null)
expected_stderr=$( ... printf ... )
assert_equals 'the unmatched block matches byte for byte' "$expected_stderr" "$stderr_only"
```

This captures **all** of stderr and compares it exactly. Any library that adds
one byte to stderr on that path breaks this test. Five test files in the suite
use the `2>&1 >/dev/null` stderr-capture idiom. This is the single strictest
consumer of the binary's output, and it is stricter than the brief suggested.

**It runs in Docker on `debian:bookworm-slim`.** `tests/docker/Dockerfile:19`
builds with `rust:1.94-slim-bookworm`, `:28` runs on
`debian:bookworm-slim`, `:57` copies just the binary in. Section 5 shows why
this matters less than expected.

**One correction to the brief.** The brief asks whether miette's source spans
help "a TOML manifest parse error ... a manifest with a bad
`[dependency.package.aptt]` key." No such TOML manifest exists in this repo
today. `.sync-manifest` is a **line-oriented custom format** (one rule per
line, `!` and `~` prefixes), parsed by hand in
`crates/config-manifest/src/manifest.rs:115-148`. The dependency manifest is
`~/deps/deps.toml` (at the time of this research, `deps.conf`, a **pipe-delimited shell-read format**
spelled `name|check_command|docs_url`). Section 6 evaluated spans against
the format that existed then, plus the prospective TOML case -- which is now
the real one, so the prospective column is the one that came true.

## 2. Non-TTY output: the measurement that decides it

Every binary below constructs the same error ("failed to read manifest" wrapping
"no such file") and returns it from `main`. stderr is a **pipe**, stdin is
`/dev/null`, and `RUST_BACKTRACE` / `RUST_LIB_BACKTRACE` / `TERM` are unset.

| Candidate | stderr bytes | ANSI lines | Non-ASCII lines | Exit code |
|---|---|---|---|---|
| no dependency (hand-written) | 47 | 0 | 0 | **4** |
| `thiserror` + hand-written edge | 55 | 0 | 0 | **4** |
| `snafu` 0.8 (bare `main`) | 23 | 0 | 0 | 1 |
| `anyhow` | 60 | 0 | 0 | 1 |
| `eyre` | 99 | 0 | 0 | 1 |
| `miette` (no `fancy`) | 224 | 0 | 0 | 1 |
| `miette` + `fancy` | 111 | **3** | **3** | 1 |
| `color-eyre` | 272 | **3** | **3** | 1 |

The literal bytes, which are the point. `anyhow`:

```
Error: failed to read manifest

Caused by:
    no such file
```

`color-eyre`, into a pipe, `cat -v` to show escapes:

```
Error: 
   0: ^[[91mfailed to read manifest^[[0m
   1: ^[[91mno such file^[[0m

Location:
   ^[[35mc_coloreyre/src/main.rs^[[0m:^[[35m3^[[0m

Backtrace omitted. Run with RUST_BACKTRACE=1 environment variable to display it.
Run with RUST_BACKTRACE=full to include source snippets.
```

`miette` **without** `fancy`, in an isolated workspace:

```
Error: Diagnostic { message: "no such file", code: "cfg::manifest::missing", help: "create the manifest first" }
NOTE: If you're looking for the fancy error reports, install miette with the `fancy` feature, or write your own and hook it up with miette::set_hook().
```

That is a `Debug` struct dump plus an advertisement for the library's own
feature flag, on the error path, in a git hook. It is the worst output of any
candidate.

`miette` **with** `fancy`, into a pipe:

```
Error: ^[[31mcfg::manifest::missing^[[0m

  ^[[31m<U+2717>^[[0m no such file
^[[36m  help: ^[[0mcreate the manifest first
```

### No standard environment variable turns the color off

Verified against isolated single-crate builds of `color-eyre` 0.6.5 and
`miette` 7.6.0 + `fancy`. Count of lines containing an ESC byte, stderr piped:

| Environment | `color-eyre` | `miette` + `fancy` |
|---|---|---|
| (nothing set) | 3 | 3 |
| `NO_COLOR=1` | 3 | 3 |
| `TERM=dumb` | 3 | 3 |
| `CLICOLOR=0` | 3 | not measured |
| `CLICOLOR_FORCE=0` | 3 | not measured |

**The cause is in the source, not inferred.** Grepping `color-eyre` 0.6.5's
`src/*.rs` for `supports_color`, `is_terminal`, `stream::Stream`, and `on(Stream`
returns **zero matches**. There is no TTY detection in the crate. Color is
unconditional. This is verified by absence of code, which is weaker evidence
than a positive test, so I also ran the five-environment table above, and it
agrees.

Note `color-eyre` does depend on `owo-colors` 4.4.0 and `supports-color` 3.0.2,
which can do TTY detection. `color-eyre` 0.6.5 does not call it on this path.

### `color-eyre` can be tamed, in about ten lines

```rust
let theme = if std::io::stderr().is_terminal() { Theme::dark() } else { Theme::new() };
let (_panic_hook, eyre_hook) = HookBuilder::default()
    .theme(theme)
    .display_location_section(false)
    .display_env_section(false)
    .into_hooks();
eyre_hook.install()?;
```

Verified output, stderr piped, 0 ANSI lines, 57 bytes:

```
Error: 
   0: failed to read manifest
   1: no such file
```

This works. It is also the argument against `color-eyre` rather than for it:
after ten lines of configuration whose entire purpose is to switch the library's
distinguishing feature off, the output is no better than `anyhow`'s 60 bytes at
zero configuration, and it still fails the byte-for-byte stderr assertion in
`tests/check-branch-drift.test.sh:180` because the shape changed.

### What the CI greps actually do under color

I tested the real workflow greps against `color-eyre` output rather than
assuming they break. **They survive.** A binary that prints
`diverged: .config/nvim/init.lua` to stdout and then fails through `color-eyre`:

```
grep '^diverged: '                  -> MATCH
grep 'match no .sync-manifest rule' -> MATCH
```

The escapes bracket the message rather than interleaving with it, and neither
grep is anchored to the start of the decorated line. **So the CI-grep hazard is
smaller than the brief implies, and I am recording that against my own
argument.** The byte-for-byte stderr assertion is the assertion that actually
breaks, and it breaks for every candidate that changes the stderr shape,
colored or not.

## 3. The exit code, which settles the question

This is the finding that makes the library choice nearly irrelevant.

`fn main() -> Result<(), E>` returns exit code **1**, for every library, for
every error. Verified:

```
$ ./iso_term >/dev/null 2>&1; echo $?
main()->Result exit code = 1
```

That is `std::process::Termination` for `Result`, not a library decision, and
no candidate overrides it. The measured exit codes in the section 2 table make
it concrete: every library variant exits 1, and only the two variants that
return `ExitCode` from `main` exit 4.

The brief describes the new design as "one `ExitStatus` enum mapped in exactly
one function." That design **requires** `fn main() -> ExitCode`. The crate
already does this (`crates/config-manifest/src/main.rs:63`,
`fn main() -> ExitCode`), and already returns 0, 1, 2, and 3 from distinct
sites.

Once `main` returns `ExitCode`, the edge library's `Termination` impl is unused,
its panic hook is optional, and its report formatter runs only if you call it.
What remains of an edge library is one line: turning an error chain into a
string. `anyhow` does that with `{error:#}`, which the crate already uses at
`main.rs:86` and `main.rs:93`.

**So the library question at the edge reduces to: which crate gives the nicest
`{:#}`, and does its `?` ergonomics help the code between `main` and the
library crates.** That is a much smaller question than the brief assumed, and
it is one `anyhow` already answers.

### Nothing here interferes with controlling the exit code

To be explicit, since the brief asks: no candidate prevents precise exit codes,
because the escape hatch is the same for all of them (return `ExitCode` from
`main`, print the error yourself). What they do is make the *default* wrong.
A future contributor who changes `fn main() -> ExitCode` to
`fn main() -> anyhow::Result<()>` because it is tidier will silently collapse
every exit status to 1 and break `tests/pre-push`. That risk is identical
across `anyhow`, `eyre`, `color-eyre`, and `miette`.

The mitigation is not a library choice. It is a test that asserts the exit
codes, which section 8 recommends.

### The hybrid works, and it is what the crate already does

Verified: keep `anyhow` internally, return `ExitCode` from `main`, recover the
typed error to pick a status.

```rust
fn main() -> ExitCode {
    match work() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("config-manifest: {error:#}");
            let code = if error.downcast_ref::<BadManifest>().is_some() { 4 } else { 1 };
            ExitCode::from(code)
        }
    }
}
```

Measured: `exit=4`, stderr
`config-manifest: reading the manifest: bad manifest`, 0 ANSI escapes.

## 4. Startup latency: the honest negative result

The brief asked me to measure this at n=100 and report per-invocation cost
against a no-dependency baseline. I did. **It does not discriminate.**

Success path, no error constructed, `n=100`, three independent reps:

| Candidate | rep 1 | rep 2 | rep 3 |
|---|---|---|---|
| `/usr/bin/true` | 7.531 | 7.225 | 7.328 |
| no dependency (baseline) | 7.729 | 7.628 | 8.049 |
| `thiserror` + hand-written edge | 7.992 | 7.721 | 8.023 |
| `snafu` | 7.969 | 7.994 | 7.708 |
| `anyhow` | 7.757 | 7.894 | 7.791 |
| `eyre` | 7.816 | 7.889 | 8.112 |
| `miette` | 8.007 | 7.699 | 7.987 |
| `miette` + `fancy` | 8.168 | 8.134 | 7.823 |
| `color-eyre` (with `install()`) | 7.970 | 8.532 | 8.127 |

All values ms/call, n=100.

Read the columns, not the rows. The **within-candidate** spread across reps
(`color-eyre`: 7.970 to 8.532, a 0.56 ms range) is larger than the
**between-candidate** spread within any single rep. `baseline` is slower than
`anyhow` in rep 3 and faster in rep 1. This is noise, not signal.

A second run isolating the null hypothesis confirms it:

| Candidate | rep 1 | rep 2 | rep 3 |
|---|---|---|---|
| no dependency (baseline) | 7.219 | 7.211 | 6.989 |
| `thiserror` + hand-written edge | 7.209 | 7.339 | 6.954 |
| `anyhow` | 7.341 | 7.237 | 7.206 |

**`color_eyre::install()` costs nothing measurable at this resolution.** That
contradicts the assumption in constraint 4 that "installing hooks, resolving
backtrace settings, loading terminal capabilities" is paid per invocation at a
cost worth avoiding. It is paid, and it is below the noise floor of a ~7 ms
process spawn on this machine.

This matches the prior doc's finding (`rust-external-tool-boundaries.md`, "The
floor"): the ~7-8 ms is macOS fork+exec, and library initialization disappears
into it.

**What I did not measure**: latency on Linux, in the Docker container, or on
the pre-push path under load. The macOS spawn floor is high (7.2 ms against
6.66 ms for `/usr/bin/true` in the prior doc); a platform with a cheaper spawn
would give library init a larger share of a smaller total. I do not expect that
to change the ranking, because the absolute init cost is what it is, but I did
not verify it.

### Backtrace capture cost, measured

Constraint 4 also asks when a backtrace is captured and what it costs. Measured
by constructing 20,000 errors in-process and timing construction:

| Candidate | `RUST_BACKTRACE` unset | `=0` | `=1` |
|---|---|---|---|
| `anyhow` | 141 ns | 133 ns | **23,987 ns** |
| `eyre` | 326 ns | 325 ns | **25,283 ns** |
| `thiserror` | 74 ns | 74 ns | 76 ns |

All values ns per error construction, n=20,000.

Three facts fall out:

1. **Capture is opt-in and avoidable.** With `RUST_BACKTRACE` unset or `0`,
   `anyhow` and `eyre` capture nothing. This is the default in a git hook and
   in CI.
2. **When enabled it costs ~24 microseconds per error**, a 170x increase. At
   one error per invocation that is 0.024 ms against a 7,000 ms-scale... against
   a 7 ms process. It is 0.3% of one invocation. Irrelevant here.
3. **`thiserror` never captures**, at any setting. This is why the null
   hypothesis has no backtrace story at all, which is a feature for this binary
   and a limitation for a debuggable service.

`RUST_LIB_BACKTRACE=1` behaves identically to `RUST_BACKTRACE=1` for output
size in my test (`anyhow` 260 bytes, `eyre` 352 bytes, `color-eyre` 1780 bytes).
I did not test the documented `RUST_LIB_BACKTRACE=0` + `RUST_BACKTRACE=1`
combination that suppresses library capture while keeping panic backtraces.

### Output size under `RUST_BACKTRACE=1`, which is the real hazard

| Candidate | unset | `=1` | `=full` |
|---|---|---|---|
| `snafu` | 23 | 23 | 23 |
| `thiserror` + hand-written edge | 55 | 55 | 55 |
| `anyhow` | 60 | **260** | 260 |
| `eyre` | 99 | **352** | 352 |
| `color-eyre` | 272 | **1780** | 1723 |
| `miette` + `fancy` | 111 | 111 | 111 |

All values bytes to stderr, stderr piped.

`color-eyre` reaches 1780 bytes on a single error when `RUST_BACKTRACE=1` is
exported. Constraint 2 says a large report before every shell prompt is
"actively harmful." A developer who exports `RUST_BACKTRACE=1` in their shell
profile (a normal thing to do while debugging Rust) turns that on for
everything, permanently. `anyhow` grows 4.3x under the same condition;
`color-eyre` grows 6.5x from a 4.5x larger base.

## 5. Dependency tree, binary size, compile time

| Candidate | transitive crates | tree depth | stripped binary | cold build | warm rebuild |
|---|---|---|---|---|---|
| no dependency | **0** | 0 | 336,672 B | **0.98 s** | 0.84 s |
| `thiserror` only | 6 | 6 | **336,176 B** | 3.46 s | 0.80 s |
| `anyhow` | **1** | 2 | 353,920 B | **1.60 s** | 0.84 s |
| `eyre` | 3 | 3 | 371,040 B | 1.80 s | 0.82 s |
| `snafu` | 7 | 6 | 336,192 B | 4.31 s | 0.79 s |
| `miette` (no `fancy`) | 11 | 7 | 353,376 B | 3.85 s | 0.85 s |
| `miette` + `fancy` | **34** | 8 | **606,672 B** | **6.12 s** | 0.88 s |
| `color-eyre` | **25** | 9 | **693,008 B** | 4.66 s | 0.86 s |

Crate counts and sizes for `miette` and `color-eyre` come from isolated
single-crate workspaces. Cold build is a fresh `CARGO_TARGET_DIR`. Warm rebuild
is `touch src/main.rs` then rebuild.

Three readings, and the second one defuses constraint 3.

**`anyhow` has the smallest tree of any library: one crate.** It is a
single-crate dependency with no proc macro. `thiserror`, already a dependency
by decision, pulls 6 (`syn`, `quote`, `proc-macro2`, `unicode-ident`,
`thiserror-impl`). So **adding `anyhow` on top of `thiserror` costs exactly one
additional crate**, and the combined tree is 7. That is the cheapest library
option on the table and it is the incumbent.

**Warm rebuild does not discriminate: 0.79-0.88 s for every candidate,
including the 34-crate one.** This is not an accident, it is the Dockerfile's
design. `tests/docker/Dockerfile:21-23` copies only `Cargo.toml` and
`Cargo.lock`, builds a stub `src/main.rs` to compile dependencies, and
`:25-26` then copies real sources and rebuilds. Dependencies live in a cached
layer keyed on the manifest. The header comment at `:17-19` says exactly this:
"edit does not recompile dependencies." So constraint 3's "real cost on
rebuild" is paid **once per manifest change**, not per edit, and the
`color-eyre` cold cost is 4.66 s against `anyhow`'s 1.60 s: a one-time 3 s
on a layer that is cached.

**Binary size is the surviving real cost, and it is small in absolute terms.**
`color-eyre` doubles the binary (693 KB against 354 KB, +339 KB) and `miette`
+ `fancy` nearly doubles it (607 KB, +253 KB). On `debian:bookworm-slim`
(~75 MB) a 339 KB increase is 0.45% of the image. That is a real cost and it is
not a decisive one. I am not going to inflate it into an argument.

`color-eyre`'s 25 crates include `backtrace`, `addr2line`, `gimli`, `object`,
`miniz_oxide`, `adler2`, `rustc-demangle` (the backtrace symbolication stack)
and `tracing`, `tracing-core`, `tracing-subscriber`, `tracing-error`,
`sharded-slab`, `thread_local` (the span-trace stack). Both stacks exist to
support features this binary does not use.

## 6. Does miette's diagnostic model help here?

This is the one place a candidate offers something `anyhow` genuinely cannot,
so it deserves a fair test rather than a dismissal.

I built the real case: a `.sync-manifest` with trailing whitespace on line 3,
which is a live error variant
(`crates/config-manifest/src/manifest.rs:60`, `ManifestError::Whitespace`).
Verified output:

```
sync_manifest::whitespace

  <U+2717> leading or trailing whitespace is not allowed
   <U+256D><U+2500>[.sync-manifest:3:1]
 2 <U+2502> .zshrc
 3 <U+2502> .config/nvim/
   <U+00B7>        <U+2570><U+2500><U+2500> this rule has trailing whitespace
 4 <U+2502> .scripts/
   <U+2570><U+2500><U+2500><U+2500><U+2500>
  help: remove the trailing space, or delete the line
```

**This is good, and it is real.** 438 bytes, points at line 3 column 1, shows
two lines of context, puts a caret under the exact span, and carries actionable
help text. `anyhow` cannot produce this. For a compiler, a linter, or a
config-validation tool a human reads interactively, this is a strong argument.

Four reasons it does not apply to this binary.

**1. The error already names the line, and no consumer wants more.** The
existing `Display` impl (`manifest.rs:64-77`) produces
`.sync-manifest line 3: leading or trailing whitespace is not allowed`. That is
one line, greppable, and it already carries the line number, which is the
information the span would add. The gap miette closes is showing the *source
text*, and the reader of this error has the file open.

**2. The primary reader is a script, not a human.** stderr goes to a
byte-for-byte assertion (`tests/check-branch-drift.test.sh:180`), a
`grep -q` in CI (`branch-drift.yml:57-65`), and `/dev/null` in a precmd. A
438-byte box-drawn diagram is worse than a 60-byte line for all three.

**3. The formats are not span-shaped.** `.sync-manifest` is one rule per line;
the offending unit *is* the line, so a span within it points at what the line
number already identified. `deps.conf` is pipe-delimited, and its documented
failure mode (`deps.conf:9-13`) is a literal `|` in the check field truncating
the record. A span could highlight that character usefully.

When this was written, a shell script read that file rather than this binary,
so miette could not render it without a port that was not then on the table.
The port has since landed: `config deps` reads the manifest in Rust, so the
span is now reachable. Whether it earns its rendering cost is the same open
question the rest of this section asks.

**4. The prospective TOML case does not need miette to get a span.** If a
`[dependency.package.aptt]`-style TOML manifest is added later,
`toml` / `toml_edit` already returns a byte span for a parse error
(`toml::de::Error::span()`), and `basic-toml` returns line and column. Rendering
`config-manifest: deps.toml:14:3: unknown key "aptt"` from that span is a
`format!` call. The Rust compiler's own diagnostic style is available without
adopting a diagnostic framework. **I did not verify this**: I did not add
`toml` to a test crate and confirm `Error::span()` returns what I expect on a
bad-key input. It is a documented API and I am reporting it as unverified.

**Where miette would earn its place**: if this binary grew an interactive
`config lint` command whose primary output is a human reading a config error in
a terminal, and the scripted consumers moved to a `--porcelain` machine format.
That is a real future, and it is not today.

### miette's structural problems for this binary

Two, both verified.

**The feature flag is a trap in both positions.** Without `fancy`, the error
path prints a `Debug` struct dump plus `NOTE: If you're looking for the fancy
error reports, install miette with the fancy feature`. With `fancy`, the tree
goes to 34 crates and the output carries ANSI plus box-drawing into a pipe. The
"install `fancy` only in dev" pattern does not help, because the dependency is
declared in the manifest that the Docker layer and `Cargo.lock` are keyed on.

**`into_diagnostic()` destroys the typed error.** Verified downcast behavior on
a `thiserror` error from a library crate:

| Path | `downcast_ref::<ManifestError>()` |
|---|---|
| `anyhow` + `.context(...)` | `Some(Whitespace { line: 7 })` |
| `eyre` + `.wrap_err(...)` | `Some(Whitespace { line: 7 })` |
| `miette` + `.into_diagnostic()` | **`None`** |
| `miette::Report::new(e)` where `e: Diagnostic` | `Some(Whitespace { line: 7 })` |
| `?` into `miette::Result` where `e: Diagnostic` | `Some(Whitespace { line: 7 })` |
| `miette` `.wrap_err(...)` on a `Diagnostic` | `Some(Whitespace { line: 7 })` |

**I am correcting my own first reading here.** My initial test used
`into_diagnostic()` and I nearly recorded "miette breaks downcast" as a general
flaw. It is not. miette preserves downcast **when the error derives
`Diagnostic`**. Only `into_diagnostic()`, the adapter for foreign errors that
do not, erases the type.

The consequence for this repo is still real but narrower: the exit-code contract
needs `downcast_ref` to select a status, so adopting miette means every library
error type that participates in exit-code selection must derive `Diagnostic`.
That pushes a binary-edge concern down into the library crates, which is the
wrong direction for a boundary. `anyhow` requires nothing of them.

## 7. `snafu`, `eyre`, and interop

### `snafu` is a `thiserror` alternative, and the edge is not where it competes

`snafu`'s pitch is context selectors: `.context(ManifestSnafu)` at the call
site generating a typed variant. That competes with `thiserror` in the library
crates, which the brief puts out of scope. At the edge it has two verified
problems.

**Bare `main` returning a `snafu` error prints `Debug`, not `Display`.**
Measured, stderr piped: `Error: ManifestMissing`. 23 bytes. The
`#[snafu(display("failed to read manifest: no such file"))]` message is
**absent**. A consumer grepping for the message text finds nothing:

```
c_snafu:  plain substring grep: NO MATCH  <-- grep broken
```

That is the only candidate that fails a plain substring grep on its own error
message. It is a `Termination`-for-`Result` behavior (`Debug` is what
`Termination` prints) rather than a snafu bug, and every library shares the
mechanism, but snafu is the one whose `Debug` omits the message.

**`#[snafu::report]` fixes that and introduces a different problem.** snafu's
own recommended edge attribute, on 0.9.2, produces:

```
Error: failed to read manifest *

Caused by this error:
  1: No such file or directory (os error 2)

NOTE: Some redundant information has been removed from the lines marked with *. Set SNAFU_RAW_ERROR_MESSAGES=1 to disable this behavior.
```

The library **rewrites the message text**, marks the rewrite with a `*`, and
appends a `NOTE:` paragraph pointing at an environment variable. For a binary
whose stderr is compared byte for byte and grepped in CI, a library that edits
message text by default and gates the behavior on an environment variable is a
liability. Exit code is still 1.

Version note: `snafu`'s current stable is 0.9.2; my section 2 and 5 numbers are
0.8.9, and the `#[snafu::report]` test is 0.9.2. I did not re-run the size and
tree table on 0.9.

### `eyre` is `anyhow` with a location line and no reason to switch

`eyre` is a fork of `anyhow` whose purpose is the pluggable report handler,
which is what `color-eyre` plugs into. Used bare, it produces `anyhow`'s output
plus a `Location:` section:

```
Error: failed to read manifest

Caused by:
    no such file

Location:
    c_eyre/src/main.rs:3:26
```

99 bytes against `anyhow`'s 60. The extra 39 bytes are a source location that
names a line inside the binary, which helps a developer debugging the binary and
means nothing to `pre-push`. Tree is 3 crates against `anyhow`'s 1. Downcast
works identically. There is no reason to migrate to `eyre` unless the
destination is `color-eyre`, and section 2 argues against that destination.

### Interop with `thiserror` is clean for `anyhow` and `eyre`

Verified against library-crate error types shaped exactly like
`ManifestError` and a `GitError` with a `source`:

- **`?` works with no annotation.** `lib_call().context("reading the manifest")?`
  compiles against `anyhow::Result` because `anyhow::Error: From<E>` for any
  `E: std::error::Error + Send + Sync + 'static`, which `thiserror` derives.
- **Context attaches** and the chain is walkable: `err.chain().count()` = 2 for
  one `.context()` over one typed error.
- **The typed error stays recoverable**: `downcast_ref::<ManifestError>()`
  returns `Some(Whitespace { line: 7 })`, so `status_for()` can match on the
  real variant to pick an exit code.

That last property is what the exit-code contract needs, and `anyhow` and
`eyre` both have it. This is the strongest single technical argument for keeping
`anyhow`: the design the brief describes (typed error selects an `ExitStatus`)
is directly expressible with the incumbent.

## 8. The null hypothesis, taken seriously

The brief asks whether a library is needed at the edge at all, and asks me to
assess it as the null hypothesis rather than assume a library is wanted. I built
it.

The full edge, with the `ExitStatus` design the brief describes:

```rust
#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)] Manifest(#[from] ManifestError),
    #[error(transparent)] Git(#[from] GitError),
    #[error("neither DOTFILES_ROOT nor HOME is set")] NoRoot,
}

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum ExitStatus {
    Ok = 0, Drift = 1, Usage = 2, Bug = 3,
    BadManifest = 4, GitFailed = 5, BadEnvironment = 6,
}

/// The single place an error becomes an exit code.
fn status_for(error: &CliError) -> ExitStatus {
    match error {
        CliError::Manifest(_) => ExitStatus::BadManifest,
        CliError::Git(_) => ExitStatus::GitFailed,
        CliError::NoRoot => ExitStatus::BadEnvironment,
    }
}

/// The single place an error becomes a message. Renders the full source chain,
/// which is what `{:#}` buys from anyhow, in five lines and no dependency.
fn render(error: &CliError) -> String {
    let mut out = format!("config-manifest: {error}");
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        out.push_str(&format!(": {cause}"));
        source = cause.source();
    }
    out
}

fn main() -> ExitCode {
    match work() {
        Ok(()) => ExitCode::from(ExitStatus::Ok as u8),
        Err(error) => {
            eprintln!("{}", render(&error));
            ExitCode::from(status_for(&error) as u8)
        }
    }
}
```

Verified behavior, stderr piped:

| Error | exit | stderr |
|---|---|---|
| `ManifestError::Whitespace` | **4** | `config-manifest: .sync-manifest line 7: leading or trailing whitespace is not allowed` |
| `GitError::Spawn` (with `source`) | **5** | `config-manifest: failed to spawn git rev-parse HEAD: No such file or directory` |
| `CliError::NoRoot` | **6** | `config-manifest: neither DOTFILES_ROOT nor HOME is set` |
| success | **0** | (empty) |

Zero ANSI escapes. 336,864 bytes stripped, which is **192 bytes larger than the
no-dependency baseline** and 17 KB smaller than the `anyhow` build. Startup
6.954-7.339 ms, indistinguishable from baseline. Tree is 6 crates, all
`thiserror`'s, which are already paid for.

**This works completely, and it is a genuinely viable answer.** The exit codes
are exhaustive and compiler-checked (adding a `CliError` variant without
updating `status_for` fails to compile, which is a real safety property
`anyhow`'s `downcast_ref` chain does not have). The source chain renders. The
`render` function is five lines. `#[from]` makes `?` work without annotation.

**Two honest costs, which are why I do not recommend it for this crate today.**

**1. Ad-hoc context requires declaring a variant.** `anyhow` lets any call site
attach a one-off string:

```rust
.with_context(|| format!("failed to spawn git {}", args.join(" ")))
```

`crates/config-manifest/src/git.rs` does this **23 times** with 8 distinct
message shapes, including `bail!` for conditions that have no type
(`git.rs:36`, `git.rs:98`, `git.rs:133`, `git.rs:179`). Under the null
hypothesis each becomes either a `CliError` variant or a
`CliError::Other(String)` catch-all. A catch-all that absorbs 23 sites and maps
to one exit status re-creates the exact defect the prior review found: one
status meaning many things. Declaring 8+ variants is the honest version, and it
is real work in a crate whose `git.rs` is 595 lines.

**2. The migration is a rewrite of `git.rs`, not of `main.rs`.** The edge is
already 30 lines and already correct. The `anyhow` usage is 5 sites in
`main.rs` and 23 in `git.rs`. "Remove the edge library" means touching the
23 sites that are not at the edge. That is a large diff whose measured benefit
is 17 KB of binary and one fewer crate.

**When the null hypothesis becomes right**: if `git.rs`'s error handling is
being restructured anyway (the prior doc's section on the `Git` boundary
suggests it is well-shaped and stable, so I do not expect it soon), or if the crate
ever wants exhaustive compiler-checked exit-status coverage badly enough to pay
for the variants. The second is a legitimate want. It is a different project
from choosing an edge library.

## 9. MSRV and maintenance

| Crate | Version | `rust-version` | Edition | Last push | Open issues | 90-day downloads |
|---|---|---|---|---|---|---|
| `anyhow` | 1.0.104 | **1.68** | 2021 | 2026-08-22 | 44 | 206,028,468 |
| `thiserror` | 2.0.20 | **1.71** | 2021 | 2026-09-05 | 31 | 349,228,281 |
| `eyre` | 0.6.14 | **1.65.0** | 2018 | 2026-08-11 | 56 | 19,749,324 |
| `color-eyre` | 0.6.5 | **1.65.0** | 2018 | 2025-05-30 | (shared repo) | 12,791,809 |
| `miette` | 7.6.0 | **1.70.0** | 2018 | 2026-06-25 | **115** | 16,587,088 |
| `snafu` | 0.9.2 | **1.56** | 2018 | 2026-07-21 | 88 | 16,661,934 |

MSRV read from the vendored `Cargo.toml` in the local registry. Repo activity
and issue counts from the GitHub API on 2026-09-06. Downloads from the crates.io
API.

Every MSRV is far below the toolchain in use (1.94.0) and below the Docker
builder image (`rust:1.94-slim-bookworm`). **MSRV does not discriminate.**

Two things worth flagging, neither alarming enough to be decisive.

**`color-eyre` 0.6.5 was last published 2025-05-30**, about 15 months before
this writing, and the `eyre-rs/eyre` repo (which hosts both) was pushed
2026-08-11. So the workspace is alive and `color-eyre` specifically has not
shipped in over a year. For a crate whose job is terminal output formatting
that is defensible (the job is done) rather than abandonment. The absence of
TTY detection in a 2025 release of a terminal-output crate is a more
interesting signal than the date: `NO_COLOR` has been a de facto standard since
2018, and not honoring it is a design position or an oversight, not a
maintenance gap.

**`miette` has 115 open issues**, the highest of the set, against 2,601 stars,
and was last pushed 2026-06-25. I read the count, not the issues. I did not
review them for severity, so I am not claiming anything is wrong. The count
alone is not evidence of a problem in a crate with an active maintainer.

`anyhow` and `thiserror` are both `dtolnay` crates with the highest download
counts in the Rust ecosystem for their category (`thiserror` at 349 million
90-day downloads) and pushes within the last three weeks. On maintenance
signal alone they are the safest choice available.

## 10. Panic hooks

Constraint 2 asks specifically: is a panic hook installed, and can it be
declined? Verified per candidate, stderr piped.

| Candidate | Installs a panic hook? | Panic output, non-TTY |
|---|---|---|
| `anyhow` | No | std hook, 139-150 B, no color |
| `eyre` | No | std hook |
| `thiserror` / null hypothesis | No | std hook |
| `snafu` | No | std hook |
| `miette` + `fancy` | **No** | std hook, 147 B, no color |
| `color-eyre` via `install()` | **Yes** | **263 B, colorized, 3 ANSI lines** |

`miette` does not install a panic hook even with `fancy`, which I confirmed by
panicking in a `fancy`-enabled binary and getting the standard
`thread 'main' panicked at ...` output. Good behavior, and it removes constraint
2 as an argument against miette specifically.

`color-eyre::install()` **does** replace the panic hook. Its output into a pipe:

```
^[[31mThe application panicked (crashed).^[[0m
Message:  ^[[36mboom^[[0m
Location: ^[[35mp_coloreyre/src/main.rs^[[0m:^[[35m10^[[0m

Backtrace omitted. Run with RUST_BACKTRACE=1 environment variable to display it.
Run with RUST_BACKTRACE=full to include source snippets.
```

**It can be declined, cleanly, in three lines.** `HookBuilder::into_hooks()`
returns the pair, and installing only the eyre hook leaves the std panic hook
in place:

```rust
let (_panic_hook, eyre_hook) = HookBuilder::default().into_hooks();
eyre_hook.install()?;
```

Verified: panicking then produces the standard
`thread 'main' (5299778) panicked at ...` output, no color.

The size difference matters under `RUST_BACKTRACE=1`:

| Configuration | panic output under `RUST_BACKTRACE=1` |
|---|---|
| `color_eyre::install()` (panic hook active) | **2,239 bytes** |
| `eyre_hook.install()` only (std panic hook) | **178 bytes** |

So constraint 2's concern is real and it is fully mitigable. It is not an
argument that eliminates `color-eyre`; section 2's unconditional-color finding
is.

## Recommendation

Answering for this repo's constraints, not in general.

1. **Keep `anyhow`. Do not migrate.** It is the smallest library option
   (**1 transitive crate**, and only 1 *additional* crate on top of the
   `thiserror` the library crates already require), it writes no ANSI escapes
   to a pipe, it installs no panic hook, it captures no backtrace unless
   `RUST_BACKTRACE` is set, and its `downcast_ref` preserves the typed
   `thiserror` error that the `ExitStatus` mapping needs. Every candidate that
   would replace it is larger, and the two that are interesting are worse on
   the non-TTY path that this binary's three consumers all use.

   The migration cost is also asymmetric in `anyhow`'s favor: 23 `.context()` /
   `bail!` sites in `git.rs` depend on ad-hoc string context, which is the one
   thing `anyhow` provides and the null hypothesis does not.

2. **Keep `fn main() -> ExitCode`. This is the load-bearing decision, and it is
   not a library decision.** `fn main() -> Result<(), E>` exits **1** for every
   library tested, verified. The `ExitStatus` design the brief describes is
   impossible under `Result`-returning `main` and trivial under
   `ExitCode`-returning `main`. The crate already does the right thing at
   `crates/config-manifest/src/main.rs:63`.

   **Add a test that asserts the exit codes**, because nothing currently stops a
   future contributor from "tidying" `main` into
   `fn main() -> anyhow::Result<()>` and silently collapsing all nine statuses
   to 1. The test suite has 22 `assert_equals` calls and pins stderr byte for
   byte; it must pin the status numbers with equal force. This is the single
   highest-value change in this document, and it is worth more than the entire
   library question.

3. **Reject `color-eyre`.** Verified: it writes ANSI escapes to a pipe, and
   **no standard environment variable suppresses them** (`NO_COLOR`,
   `TERM=dumb`, `CLICOLOR=0`, `CLICOLOR_FORCE=0` all leave 3 escape-bearing
   lines). Grepping its 0.6.5 source for `is_terminal`, `supports_color`, and
   `Stream` returns zero matches: there is no TTY detection in the crate. It
   also doubles the binary (693 KB against 354 KB) for 25 crates, and reaches
   1,780 bytes of stderr on one error under `RUST_BACKTRACE=1`.

   Its panic hook is declinable in three lines and its color is suppressible in
   ten, and that is the argument against it rather than for it: after
   configuring away the feature it exists to provide, the output is no better
   than `anyhow`'s at zero configuration, and it still breaks the byte-for-byte
   stderr assertion because the shape changed.

   **What I am not claiming**: the CI workflow's two actual greps
   (`branch-drift.yml:64-65`) still match under `color-eyre` output. I tested
   it. The escapes bracket the message rather than interleaving, and neither
   grep is anchored to the decorated line. The grep hazard is smaller than it
   sounds, and the byte-for-byte assertion is the real breakage.

4. **Reject `miette`, while recording that its diagnostic model is genuinely
   good.** Rendered against a real `.sync-manifest` whitespace error it points
   at `line 3:1` with a caret under the span and actionable help, in 438 bytes.
   `anyhow` cannot do that. Four reasons it is wrong here:

   - The feature flag is a trap in both positions. Without `fancy` the error
     path prints a `Debug` struct dump plus `NOTE: If you're looking for the
     fancy error reports, install miette with the fancy feature`. With `fancy`
     it is 34 crates, 607 KB, and ANSI plus box-drawing into a pipe that
     `NO_COLOR=1` does not suppress.
   - The primary reader is a script. A byte-exact stderr assertion, a `grep -q`
     in CI, and `/dev/null` in a precmd all prefer 60 bytes to 438.
   - Neither real format is span-shaped. `.sync-manifest` is one rule per line,
     so the line number the message already carries is the whole answer.
     `deps.conf` is pipe-delimited and is read by a shell script, not this
     binary.
   - Adopting it pushes `#[derive(Diagnostic)]` down into the library crates
     for any error that participates in exit-status selection, because
     `into_diagnostic()` erases the type and breaks `downcast_ref` (verified:
     `None`). That is a binary-edge concern leaking into library contracts,
     which is the wrong direction. `anyhow` requires nothing of them.

   **Revisit if** an interactive `config lint` command appears whose primary
   consumer is a human reading a config error, with the scripted consumers
   moved to a `--porcelain` format. Then the diagnostic model is worth its cost,
   and it must be scoped to that command.

5. **Reject `snafu` at the edge, and note that the edge is not where it
   competes.** Its actual pitch is context selectors as a `thiserror`
   alternative in library crates, which is out of scope. At the edge, verified:
   a bare `main` returning a snafu error prints `Error: ManifestMissing`, the
   `Debug` form, and the `display` message is **absent**. It is the only
   candidate that fails a plain substring grep on its own message text. Its
   recommended `#[snafu::report]` fixes that and instead **rewrites the message,
   marks the rewrite with `*`, and appends a `NOTE:` paragraph** gated on
   `SNAFU_RAW_ERROR_MESSAGES`. For a binary with a byte-exact stderr assertion,
   a library that edits message text by default is a liability.

6. **Reject `eyre` as a destination in itself.** It is `anyhow` plus a
   `Location:` section (99 bytes against 60) and 3 crates against 1, with
   identical downcast behavior. The only reason to adopt it is to reach
   `color-eyre`, and item 3 rejects that destination.

7. **Do not remove the edge library, but keep the null hypothesis on file: it
   works, and it is the right answer to a different question.** I built it and
   verified it end to end: distinct exit codes 4, 5, 6, and 0 per error class;
   full source chain rendered by a five-line `render`; zero ANSI escapes; 336,864
   bytes, which is 192 bytes over a no-dependency baseline and 17 KB under the
   `anyhow` build; startup indistinguishable from baseline.

   It has one property `anyhow` cannot match: `status_for` is an exhaustive
   `match` on `CliError`, so adding a variant without assigning it a status is a
   **compile error**. Against the prior review's finding (status 1 meaning eight
   different things), compiler-checked exhaustiveness is a real safety
   improvement over a chain of `downcast_ref` calls.

   It costs the thing `anyhow` is for. `git.rs` attaches ad-hoc string context
   at **23 sites** with 8 distinct shapes, including four `bail!` conditions
   with no type. Each becomes a declared variant, or a `CliError::Other(String)`
   catch-all that re-creates the one-status-many-meanings defect. So the honest
   migration is 8+ new variants and a rewrite of a 595-line file, to save 17 KB
   and one crate, on a binary whose edge is already 30 lines and already
   correct.

   **Adopt it if** `git.rs`'s error handling is being restructured for other
   reasons, or if compiler-checked exit-status exhaustiveness is wanted badly
   enough to pay for the variants. That is a legitimate want and a separate
   project from choosing an edge library.

8. **Stop treating startup latency and dependency count as inputs to this
   decision.** Both were measured and both came back flat.

   Startup: 6.95-8.53 ms for all eight candidates against a 6.99-8.05 ms
   baseline, n=100, three reps, with within-candidate rep variance exceeding
   between-candidate variance. `color_eyre::install()` costs nothing measurable
   at a ~7 ms macOS spawn floor.

   Rebuild: 0.79-0.88 s warm for every candidate including the 34-crate one,
   because `tests/docker/Dockerfile:21-26` already caches the dependency build
   in its own layer, exactly as its `:17-19` comment says. Cold-build
   differences (1.60 s for `anyhow`, 6.12 s for `miette` + `fancy`) are paid
   once per manifest change.

   The surviving real cost is binary size (+339 KB for `color-eyre`, +253 KB
   for `miette` + `fancy`), which is 0.45% of a `debian:bookworm-slim` image.
   It is a genuine cost and it is not a decisive one, and inflating it into one
   would be dishonest.

## Revisit if

- **An interactive command appears whose primary reader is a human.** That is
  the one shape where `miette` wins outright, and section 6 shows the win is
  real on this repo's own manifest format. The precondition is that the scripted
  consumers move to a stable machine format first, so the pretty output is not
  competing with a byte-exact assertion.
- **A TOML manifest lands.** The `[dependency.package.aptt]` case in the brief
  does not exist yet. When it does, check whether `toml::de::Error::span()`
  plus a `format!` gives a good-enough `file:line:col` message before reaching
  for a diagnostic framework. **I did not verify that API's behavior on a
  bad-key input**; it is documented and untested by me.
- **`color-eyre` gains TTY detection or honors `NO_COLOR`.** That is the single
  finding that eliminates it, and it is a small upstream change. If a release
  after 0.6.5 adds either, re-price it: the panic hook is already declinable and
  the latency is already free, so unconditional color is the whole objection.
- **`git.rs` gets restructured.** That is when the null hypothesis stops costing
  a rewrite and starts being nearly free, and its compiler-checked exhaustive
  `status_for` becomes the better answer to the prior review's exit-code finding.
- **The exit-code contract gets a test, and then someone wants backtraces.**
  Once statuses are pinned by a test, `fn main() -> ExitCode` is protected, and
  the remaining reason to prefer a richer library (a readable backtrace on an
  unexpected failure) can be evaluated on its own. Note `RUST_BACKTRACE=1`
  already gives `anyhow` a 260-byte backtrace with no code change, and
  `thiserror` alone gives none at any setting.
