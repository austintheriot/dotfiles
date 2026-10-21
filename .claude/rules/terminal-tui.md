---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-10-21
---

# Terminal and TUI

A reference for reviewing and advising on programs that treat the terminal as a rendering and input device: anything that enters raw mode, moves the cursor, uses the alternate screen, emits escape sequences beyond plain SGR color, decodes key or mouse events, draws progress or live regions, or is built on a TUI framework. Used by the `terminal-tui` subagent. The scope is the TTY and PTY layer, escape sequences and capability detection, terminal emulator and multiplexer differences, Windows console and ConPTY, terminal state and its restoration, TUI architecture and rendering, input decoding, inline progress output, terminal accessibility, escape-injection security, and testing. It is not argv, streams, exit status, or the *decision* to emit color or progress to a stream (`cli-design`), not shell code, rc files, or the shell side of OSC 133 integration (`shell-scripting`), not Unicode width and segmentation algorithms (`text-engineering`), not WCAG in general (`accessibility`), and not GPU text rendering (`graphics-programming`).

The unifying model: **a terminal is a 1978 device protocol with fifty years of private extensions, no capability-query standard, and an emulator on the other end whose identity is a lie.** Every program writes bytes into an in-band channel that the terminal interprets, and reads replies from the same channel it reads keys from. Most defects in this domain come from four facts. **State is global and outlives the process:** raw mode, the alternate screen, mouse reporting, and keyboard flags stay set after a crash. **Capabilities are guessed:** `TERM` says `xterm-256color` almost everywhere, probes go unanswered or answered falsely, and an intermediary (tmux, SSH, ConPTY) can change everything. **Output and input share one channel:** replies arrive as fake keystrokes, and untrusted text in the output can become commands in the input. **The scrollback above the viewport is read-only:** an inline renderer that needs to change something that scrolled off has to clear and redraw.

The operational question, for every terminal interaction: **what state does this put the terminal in, who restores it on every exit path, which terminals and intermediaries understand it, and what happens on the ones that do not?**

Empirical priority order, by how often each bites:

1. **Terminal left broken.** Raw mode, alternate screen, hidden cursor, mouse reporting, bracketed paste, or kitty keyboard flags not restored on panic, early return, signal, or suspend. The user's shell is unusable until `reset`.
2. **Escape injection.** Untrusted strings (filenames, logs, model output, git and HTTP data) printed raw. Ranges from garbled screens and title spam to clipboard writes and command execution through reply-echo bugs in terminals.
3. **Capability assumptions.** A feature used without detection, detection that blocks on a timeout, a probe that prints garbage on a terminal that does not understand it, or an intermediary (tmux, Zellij, SSH, ConPTY) that silently drops the feature.
4. **Rendering defects.** Flicker from many small writes, full clears in inline mode, frames taller than the viewport, scrollback duplication, resize handling that does not reflow.
5. **Input ambiguity.** Ctrl-I versus Tab, Esc versus Alt versus sequence start, Shift+Enter, terminal replies delivered as keys, Ctrl-C and Ctrl-Z in raw mode, paste handled as keystrokes.
6. **Non-terminal and assistive contexts.** TUI output in CI logs, no plain mode, spinners and redraws that break screen readers, a fake caret that misplaces the IME window.

## Volatile surface

`last-verified` (see frontmatter, do not restate the date here). These rot. The rest of this file is comparatively durable.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Per-terminal feature support (DEC 2026, mode 2027, kitty keyboard, graphics, OSC 52 policy) | Fast (monthly) | https://ucs-detect.readthedocs.io/results.html, the kitty keyboard-protocol page's implementer list, each terminal's release notes |
| Ghostty version, features, and where its issues live | Fast | ghostty.org release notes, mitchellh.com/writing |
| Windows Terminal and ConPTY behavior | Fast | `gh api repos/microsoft/terminal/releases` |
| tmux features and defaults | Medium (twice a year) | https://github.com/tmux/tmux/blob/master/CHANGES |
| TUI library versions and APIs (ratatui, Bubble Tea, Ink, Textual, OpenTUI, tcell, crossterm) | Fast | `gh api repos/<owner>/<repo>/releases`, docs.rs, pkg.go.dev |
| Which framework and screen mode each agent CLI uses | Very fast | Each tool's docs and package manifest |
| Escape-sequence CVEs | Fast | NVD search, kitty and iTerm2 advisories |
| Proposals (OSC 7501, mode 2027, OSC 66 text sizing) | Fast | The proposal pages, contour-terminal/terminal-unicode-core |
| xterm control sequences | Slow | https://invisible-island.net/xterm/ctlseqs/ctlseqs.html |
| ECMA-48, termios, POSIX job control | Stable | man7 `termios(3)` |

## History that explains the present

- **Teletypes** had separate carriage-return and line-feed motions. The kernel's `ONLCR` output flag still turns `\n` into `\r\n`. Turn off `OPOST` in raw mode and `\n` only moves down, so output "staircases".
- **Standards.** ECMA-48 (first 1976, 5th edition 1991, the current text), ANSI X3.64 (1979, withdrawn 1994 for ISO/IEC 6429; the source of the name "ANSI escape"). The DEC VT100 (1978) made them popular. CSI is `ESC [`, OSC is `ESC ]` ended by ST (`ESC \`, and xterm also accepts BEL), DCS is `ESC P`. DEC private modes (`CSI ? Ps h` / `l`) are where nearly every modern feature lives.
- **termcap (Bill Joy, 1978), curses (Ken Arnold, ~1978, for Rogue), terminfo (Mary Ann Horton, 1981-82, System V).** terminfo added compiled entries and a parameter language. Thomas E. Dickey has maintained ncurses since 1996 (6.6 shipped 2025-12-31) and maintains xterm and its control-sequence document ("ctlseqs", updated for patch #411, 2026-08).
- **Why xterm is the de facto standard.** ECMA-48 says nothing about mouse, bracketed paste, clipboard, or most of what TUIs use. xterm defined those and everyone copied xterm (Julia Evans, "escape code standards", 2025).
- **The `TERM` lie.** Almost every emulator sets `xterm-256color`. Ghostty's own terminfo is `xterm-ghostty` because a plain `ghostty` name "broke too many applications" that string-search for "xterm". `TERM` identifies a terminfo entry, not a terminal. Julia Evans compares it to browser user-agent sniffing.
- **terminfo is being abandoned (2025-2026).** tcell v3.0 (Garrett D'Amore, 2025-12) removed terminfo: it "has not kept up with newer terminal features such as 24-bit color, different mouse reporting modes, bracketed paste, advanced text styling", unknown `TERM` values are assumed xterm-compatible, and padding is gone. fish 4.0 dropped ncurses, fish 4.1 stopped reading terminfo and requires a Primary Device Attributes (DA1) reply, and fish 4.5 states it "should continue to work on all known terminal emulators released in the 21st century". kakoune, prompt_toolkit, linenoise, libvaxis, and chalk hardcode a common set. The live argument is in Schools of thought.

## TTY and PTY mechanics

### termios modes

`cfmakeraw` clears `IGNBRK BRKINT PARMRK ISTRIP INLCR IGNCR ICRNL IXON` (input), `OPOST` (output), `ECHO ECHONL ICANON ISIG IEXTEN` (local), and sets `CS8`. Consequences a TUI author must handle:

- **`ISIG` off:** Ctrl-C is byte 0x03, Ctrl-Z is 0x1A, Ctrl-\ is 0x1C. No signal fires. A `tokio::signal::ctrl_c()` or `process.on('SIGINT')` handler never runs. Map the byte (or the key event) to cancel or quit yourself.
- **`IXON` off:** Ctrl-S and Ctrl-Q reach the program instead of pausing output.
- **`ICRNL` off:** Enter arrives as `\r` (0x0D), not `\n`.
- **`OPOST` off:** the program must write `\r\n`.
- **cbreak** (Python `tty.setcbreak`): `ICANON` and `ECHO` off, `ISIG` on. Keys arrive one at a time and Ctrl-C still signals. Often the right mode for a simple prompt.
- **`VMIN` / `VTIME`:** 0/0 polls, >0/0 blocks, 0/>0 reads with a timeout in tenths of a second, >0/>0 is an inter-byte timer.
- **Canonical-mode line limit.** Linux truncates a cooked-mode line at 4096 bytes including the newline. A long paste into a cooked `read` is silently cut. macOS `MAX_CANON` is smaller (1024, not verified). `IUTF8` (Linux 2.6.4+) makes erase work on multibyte characters.

### Sessions, job control, and signals

- The controlling terminal belongs to a session. Job control moves which process group is in the foreground. **SIGINT from the terminal goes to the whole foreground process group.**
- **SIGHUP** on terminal close goes to the foreground process group.
- **SIGWINCH** on resize. Read the size with `ioctl(TIOCGWINSZ)`. The struct carries pixel sizes too, which many terminals leave at 0.
- Background reads get **SIGTTIN**. Background writes get **SIGTTOU** when `TOSTOP` is set.
- **Suspend and resume (SIGTSTP / SIGCONT).** With `ISIG` off, Ctrl-Z never produces SIGTSTP, so the program must read 0x1A and suspend itself. Codex's `job_control.rs` is a reference implementation: on suspend, record whether to restore the alternate screen or realign the inline viewport, leave the alternate screen, cache the inline cursor row, restore terminal modes, then `raise(SIGTSTP)`. On SIGCONT, re-enter raw mode and redraw inside a synchronized update.

### Escape-sequence ambiguity in input

A lone ESC (0x1B) can be the Esc key, the Alt prefix for the next key, or the start of a sequence. Legacy input forces a timeout. tmux's `escape-time` default dropped from 500 ms to 10 ms in tmux 3.5. The kitty keyboard protocol and mintty's application-escape mode remove the ambiguity. Any custom decoder needs a timeout and must treat a partial sequence at a read boundary correctly.

### Bracketed paste (DECSET 2004)

The terminal wraps pasted text in `ESC [200~` and `ESC [201~`. A program that enables it can treat paste as data, not keystrokes, so a pasted newline does not submit and pasted text does not trigger keybindings. **Security:** a payload that contains `ESC [201~` ends paste mode early and the rest runs as typed input. Terminals defend by filtering control characters out of pastes (xterm, VTE, Windows Terminal). The protocol cannot carry a literal ESC. A program that processes paste content must still never auto-execute it.

### Focus events (DECSET 1004)

`CSI I` and `CSI O`. ConPTY injects focus events into the stream without regard to parser state (Windows Terminal PR #17510), so a decoder must handle a focus report arriving mid-sequence.

## Terminal state and its restoration

### What must be undone on every exit path

termios (raw mode); the alternate screen (1049); cursor visibility (DECTCEM `?25`); mouse modes (1000, 1002, 1003, 1006, 1016); bracketed paste (2004); focus reporting (1004); the kitty keyboard flag stack (`CSI < u`); xterm modifyOtherKeys (`CSI > 4 ; 0 m`); theme-change notifications (2031); cursor shape (DECSCUSR `CSI 0 q`); the window-title stack; SGR (`CSI 0 m`).

"Every exit path" means normal return, early return through `?` or an exception, panic, SIGTERM, SIGHUP, and suspend. SIGKILL cannot be handled. The user's recovery is `reset`, `stty sane`, or `printf '\e[?1049l\e[?25h\e[<u'`. A supervising parent process can restore the terminal on the child's behalf.

### Patterns

- **RAII guard plus panic hook.** A guard type whose `Drop` restores the terminal handles early returns. A panic hook handles panics, because the panic message prints to a terminal still in the alternate screen and raw mode otherwise, and the user never sees it. Install the hook after any other panic hooks, restore inside it, ignore restore errors, then call the previous hook (ratatui panic-hook recipe). If the restore itself panics, the original reason is lost. Under `panic = "abort"`, the hook runs but `Drop` guards do not.
- **ratatui 0.30:** `ratatui::init()` enables raw mode and the alternate screen and installs a restoring panic hook. `ratatui::restore()` undoes it. Prefer `init` over `try_init`, because `init` restores the terminal if a setup step fails.
- **Declarative modes (Bubble Tea v2, 2026-02).** Alternate screen, mouse, paste, focus, title, and keyboard enhancements are fields on the returned `View`, not imperative commands. The runtime diffs them and owns teardown, which removes the "forgot to disable" class.
- **kitty keyboard flags outlive the process.** fish 4.0's changelog: a SIGSTOP or SIGKILL of fish can leave the terminal sending CSI u for modified keys, so bash and Python break until `reset`. The kitty spec requires **separate flag stacks for the main and alternate screens**. Pop with `CSI < u` before leaving the alternate screen and at exit. Codex: "Pair alternate-screen transitions with that screen's independent keyboard-mode stack. Push only on entry and pop before leaving." Also from Codex: "Panic and exit cleanup cannot borrow Tui. Track the actual screen independently of its owner", meaning the cleanup path must know what state was set without access to the object that set it.

## Escape sequences and capability detection

### Color

- SGR 30-37 and 90-97 are the 16 colors, `38;5;n` is 256 colors, `38;2;r;g;b` is truecolor. ITU T.416 specifies colon separators (`38:2::r:g:b`). The semicolon form is the most widely supported.
- Underline styles (kitty extension, now common): `4:2` double, `4:3` curly, `4:4` dotted, `4:5` dashed. Underline color `58:2::r:g:b`.
- **Truecolor detection:** `COLORTERM=truecolor` (not forwarded over SSH by default), XTGETTCAP `RGB`, or DECRQSS. Gating only on `COLORTERM` gives a 256-color fallback on capable remote terminals. Bubble Tea v2's `colorprofile` downsamples automatically.
- **Light or dark theme:** OSC 11 `?` returns the background as `rgb:`. Mode 2031 (Contour): enable `CSI ? 2031 h`, query `CSI ? 996 n`, reply `CSI ? 997 ; 1 n` (dark) or `; 2 n` (light), also sent unsolicited when the theme changes. Supported by Contour, Ghostty, kitty 0.38.1+, VTE 0.82+, Zellij 0.44.2+, tmux 3.6+ (**VOLATILE**).
- **The 16-color school.** Julia Evans: "don't use more than 16 colours", so output follows the user's theme. The GitHub CLI team restricted its palettes to the 4-bit ANSI colors for accessibility: "Unlike in a web browser, a terminal's background color is not set by the application." A hard-coded truecolor theme with no 16-color fallback is unreadable on some backgrounds.
- **Terminal.app** gained 24-bit color in macOS 26 (**VOLATILE**). It is still weak elsewhere (below).
- Whether to emit color at all, and `NO_COLOR` semantics, belong to `cli-design`.

### Capability probing

There is no capability-query standard. The tools:

- **DA1** (`CSI c`): every terminal answers. Use it as a sentinel.
- **DA2** (`CSI > c`), **XTVERSION** (`CSI > 0 q`, reply `DCS > | name ST`), **XTGETTCAP** (`DCS + q <hex capname> ST`), **DECRQM** (`CSI ? Ps $ p`, reply `CSI ? Ps ; Pm $ y` with Pm 0 unknown, 1 set, 2 reset, 3 permanently set, 4 permanently reset), **CPR** (`CSI 6n`).
- **The canonical probe pattern:** send the queries, then DA1. A reply that arrives before the DA1 reply means "supported". Reaching the DA1 reply first means "unsupported". Never block on a timeout. The kitty keyboard spec states this pattern for `CSI ? u`.
- **Probe answers are unreliable** (Jeff Quast, "State of Terminal Emulators in 2025"): iTerm2 answers "supported, but disabled, and cannot be changed" for every DEC mode, including fictional ones; Konsole does not answer DECRQM at all but supports many of the modes; Contour answered with the wrong mode number. **Terminal.app** answers DA1 as a plain VT100, sends no XTVERSION reply, mishandles DECRQM and prints a stray `p`, and **prints the contents of OSC, DCS, and APC sequences on screen** (Garrett D'Amore, 2025-12).
- **fish's workaround:** switch to the alternate screen before printing DCS queries, so a terminal that prints them leaves no garbage in the main screen.
- **Verify by effect when it matters.** Quast: "The only practical approach to determining Unicode support of a terminal is to interactively test for specific features": print the glyph, send CPR, measure the cursor advance.

### Hyperlinks (OSC 8)

`OSC 8 ; id=… ; URI ST` text `OSC 8 ; ; ST` (spec by Egmont Koblinger). VTE and iTerm2 cap the URI at 2083 bytes and VTE caps `id` at 250. A `file://` URI must carry the hostname, and terminals refuse a mismatch. Terminals should open on Ctrl- or Cmd-click. Full-screen apps should keep ids stable across redraws, and multiplexers should prefix them. Render OSC 8 from structured data, never pass through an OSC 8 found in untrusted text.

### Clipboard (OSC 52)

Write is the feature. **Read (`?`) is exfiltration.** Current defaults (**VOLATILE**): kitty `clipboard_control write-clipboard write-primary read-clipboard-ask read-primary-ask`; iTerm2 blocks until the user enables application clipboard access; Windows Terminal 1.25 allows writes only while the app is focused; Ghostty's read default is off or ask (not verified exactly); tmux needs `set-clipboard`. Use OSC 52 write as a fallback over SSH. Never design a feature that depends on OSC 52 read.

### Other OSCs

- **OSC 0 / 1 / 2:** icon and title, tab title, window title. Untrusted text in a title is an injection vector (title report echoback CVEs, below).
- **OSC 7:** current directory as `file://HOSTNAME/DIR`, for new tabs in the same directory.
- **OSC 133** semantic prompt marks: A prompt start, B prompt end, C command start, D;status command end. kitty extends it (`click_events=1`, `cmdline_url`). tmux 3.8 runs hooks on OSC 133. Claude Code emits OSC 133 at turn boundaries so screen-reader users can jump between turns. The shell-prompt side of emitting these belongs to `shell-scripting`.
- **Notifications:** OSC 9 (iTerm2, ConEmu), OSC 777 (urxvt, Windows Terminal 1.26 preview off by default), OSC 99 (kitty).
- **OSC 9;4 progress** (ConEmu origin): Ghostty 1.2 shows a native progress bar.
- **OSC 7501 "Program Status"** (Mitchell Hashimoto, proposed 2026-10-06): `ESC ] 7501 ; state=blocked:kind=permission:app=terraform:msg=<base64> ESC \`, states idle, working, done, blocked, error. Proofs of concept exist for Claude Code, Codex, Terraform, and Homebrew. **VOLATILE, a proposal.** The post says "well-behaved terminals ignore unknown OSCs", and Terminal.app does not.

### Synchronized output (DEC mode 2026)

`CSI ? 2026 h` begins a frame, `CSI ? 2026 l` ends it, and the terminal presents the whole frame at once. Probe with DECRQM. This is the single most effective anti-flicker tool. Detected support (**VOLATILE**): kitty, Ghostty, foot, iTerm2, WezTerm, Alacritty, Contour, Rio, xterm.js 6.0 (the VS Code terminal), Windows Terminal 1.24+. Not detected: XTerm 411, GNOME Terminal and VTE (sources conflict), Terminal.app. **tmux added it in 3.7**, and releases through 3.6 do not implement it. Send it only when output is a TTY and not CI. Contour's own adoption table is stale and must not be cited as current.

### Alternate screen (1049)

1049 saves the cursor, switches to the alternate buffer, and clears it (1047 plus 1048). The alternate buffer has no scrollback in most terminals. Zellij's alternate screen has no scrollback and cannot be configured to have one. iTerm2's `tmux -CC` integration is incompatible with alternate screen plus mouse tracking.

### Mouse

1000 press and release, 1002 button-motion, 1003 any-motion, 1006 SGR encoding (`CSI < b;x;y M/m`, no +32 offset, no 223-column limit, use this one), 1016 SGR pixel coordinates (Textual uses pixel reporting for smooth scrolling on kitty and Ghostty), 1005 UTF-8 encoding (avoid), 1015 urxvt. The mouse protocol cannot encode the Cmd key. **tmux 3.8 turned its `mouse` option on by default**, so wheel events now go to tmux unless the pane's program requests mouse reporting.

### Keyboard protocols

- **Without an enhanced protocol**, these are the same bytes: Ctrl-I and Tab, Ctrl-M and Enter, Ctrl-H and Backspace (on many terminals), Ctrl-[ and Esc. Shift with Ctrl+letter is lost. Shift+Enter does not exist.
- **kitty keyboard protocol** (Kovid Goyal), progressive enhancement flags: 1 disambiguate, 2 report event types (press, repeat, release), 4 report alternate keys, 8 report all keys as escape codes, 16 report associated text. Query `CSI ? u`, push `CSI > flags u`, pop `CSI < n u`. Implementers per the kitty docs (**VOLATILE**): Alacritty, foot, Ghostty, iTerm2, Windows Terminal (1.25, GA 2026-03), rio, WezTerm, xterm.js, Warp, and others. Konsole has an implementation that fish disables as buggy. VTE has an open request. tmux 3.8 does not implement it (inferred from its CHANGES file) and offers CSI u through modifyOtherKeys. Terminal.app has none. Libraries: crossterm, Textual, notcurses, vaxis, Bubble Tea (disambiguation on by default in v2), tcell. Apps: Neovim, Vim, Helix, kakoune, fish, nushell.
- **xterm modifyOtherKeys** (`CSI > 4 ; Pv m`): at level 2, Shift-Tab sends `CSI 27;2;9~`. `formatOtherKeys=1` gives `CSI 9;3u`, which "applications may confuse... with CSI u (restore cursor)". tmux 3.5 always requests modifyOtherKeys mode 2 from the outer terminal.
- **fixterms / libtickit** (Paul "LeoNerd" Evans) is the CSI u lineage the kitty protocol extends.
- **Layout-independent bindings:** Bubble Tea v2 `key.BaseCode` and the kitty protocol's base-layout key let `ctrl+w` match on a Cyrillic layout.

### Graphics

Three protocols: sixel, the kitty graphics protocol, and iTerm2 inline images (OSC 1337 File=). Detected support (**VOLATILE**): kitty and Ghostty do kitty graphics and not sixel (Ghostty by decision: Hashimoto, 2024-11: "Ghostty will not support sixels"); foot does sixel and not kitty graphics; WezTerm and iTerm2 do both; Windows Terminal 1.22+ does sixel; Konsole does both. tmux needs `--enable-sixel` at build time and `allow-passthrough` (off by default since 3.3) with DCS wrapping for the others.

### Unicode width in terminals

The algorithms belong to `text-engineering`. What a TUI author must know is that **terminals disagree with each other and with `wcwidth`**:

- **Mode 2027** (Christian Parpart's Terminal Unicode Core spec, still alpha): when on, the terminal clusters graphemes, VS16 makes a cluster 2 cells, a ZWJ emoji sequence is one 2-cell glyph, and VS15 does not change width. Detected in Ghostty, foot, WezTerm, Contour, Rio, Windows Terminal 1.24. Not detected in kitty or iTerm2 (**VOLATILE**). Only Ghostty and kitty handled VS15 correctly in Quast's tests.
- **Ambiguous-width characters** (East Asian Ambiguous): Windows Terminal 1.25 added `compatibility.ambiguousWidth` (narrow by default). A TUI that computes 1 cell where the terminal draws 2 shifts every later cell on the row.
- **The practical rule:** segment by grapheme cluster, compute widths with a maintained library, and when exact alignment matters, measure with CPR on the actual terminal. Never use `String.length` or byte length for cell math.

## Terminals, multiplexers, and transports: current state

**VOLATILE throughout.** The best current matrix is Jeff Quast's ucs-detect results. Its columns mean "detected by query", not "supported": Konsole shows no 2004 or 2026 only because it does not answer DECRQM. Treat the table as proof that probing is unreliable, not only as a feature list. Unicode scores at last check ranged from Contour 99, kitty 98, Ghostty 93, foot 89, down through iTerm2 74, WezTerm 71, Windows Terminal 67, tmux 66, XTerm 42, Alacritty 37, Terminal.app 31, GNOME Terminal 29, xterm.js 26, GNU Screen 22.

- **Ghostty** (Mitchell Hashimoto; 1.3.x; a non-profit since 2025-12; announced 2026-04 that it is leaving GitHub, with a read-only mirror remaining, so re-check where issues live). Scrollback search, native scrollbar, OSC 133, kitty keyboard and graphics, OSC 9;4 progress, opt-in `ssh-terminfo` and `ssh-env`, a read-only macOS accessibility API (1.2), `mouse-reporting = false` option. No sixel by decision. libghostty-vt offers a zero-dependency C API for the parser and terminal state.
- **kitty** (Kovid Goyal): the origin of the keyboard and graphics protocols, `allow_remote_control no` and `allow_cloning ask` by default.
- **iTerm2** (George Nachman): DECRQM answers cannot be trusted, slowest in Quast's throughput tests, mouse reporting is per profile.
- **WezTerm** (Wez Furlong): active repository, but no tagged release since 20240203, nightlies only.
- **Alacritty, foot:** minimal-scope, fast. foot does sixel.
- **VTE / GNOME Terminal:** low Unicode scores, no kitty keyboard, synchronized output not detected.
- **Konsole:** kitty keyboard arriving and buggy, fish disables OSC 133 on its default config.
- **Terminal.app:** truecolor since macOS 26. Otherwise prints unknown OSC, DCS, and APC content, answers DA1 as a VT100, no kitty keyboard, no synchronized output. Gate probes on `TERM_PROGRAM=Apple_Terminal` (not forwarded over SSH), or probe inside the alternate screen.
- **Windows Terminal:** 1.24 synchronized output and mode 2027, 1.25 kitty keyboard, ambiguous-width setting, and OSC 52 focus gating, 1.26 preview OSC 777.
- **xterm.js** (the VS Code integrated terminal): 6.0 added DEC 2026 and OSC 52, removed the canvas renderer. Among the worst for throughput (Claude Code docs).
- **JetBrains terminal:** the 2025.2 terminal sends spurious arrow keys on scroll (Claude Code docs).
- **Warp:** kitty keyboard and graphics detected, bracketed paste not detected. An OSC 52 CVE (CVE-2026-48725, not verified).

### Multiplexers re-emulate

tmux, GNU Screen, Zellij, and mosh parse the program's output and re-emit their own. **Any feature the multiplexer does not implement is lost, whatever the outer terminal supports.**

- **tmux 3.8** (2026-09): `mouse` on by default, built-in light and dark themes reported to panes, OSC 133 hooks, flicker fixes under synchronized updates. `default-terminal` is `tmux-256color` (since 3.3). `allow-passthrough` off by default (since 3.3), with an `all` value (3.4). Synchronized output for panes since 3.7. OSC 52 detected through DA since 3.6. No kitty keyboard protocol.
- **Zellij:** alternate screen has no scrollback. Codex skips the alternate screen inside Zellij.
- **mosh:** no release since 1.4.0 (2022), which added OSC 52 and truecolor only.
- **GNU Screen** prints DCS queries it does not understand.
- Detect `$TMUX`, `$ZELLIJ`, and `$STY`, and document a minimum tmux version for features you rely on.

### SSH

A terminal-specific `TERM` (`xterm-kitty`, `xterm-ghostty`) on a host without that terminfo entry gives "missing or unsuitable terminal" and breaks remote programs. Fixes: `infocmp -x xterm-ghostty | ssh host -- tic -x -` (macOS `tic` cannot read stdin); `kitten ssh` (copies terminfo and shell integration over the TTY); Ghostty's opt-in `ssh-terminfo`; or `SetEnv TERM=xterm-256color` in OpenSSH 8.7+, which loses styled underlines. `COLORTERM` and `TERM_PROGRAM` are not forwarded by default.

## Windows console

- **Default terminal:** Windows Terminal on Windows 11 22H2 and later (with Windows Terminal 1.15+). Windows 10 still defaults to conhost.
- **VT processing:** Windows 10 1511 (2016) added ANSI escape support to the console. Output needs `ENABLE_VIRTUAL_TERMINAL_PROCESSING`, input needs `ENABLE_VIRTUAL_TERMINAL_INPUT`. ConPTY (the pseudo-console API, Windows 10 1809) lets terminals host console programs.
- **ConPTY used to rewrite output.** Before 2024, ConPTY re-rendered its own buffer and emitted new VT, so unknown sequences were dropped or changed. PR #17510 ("Goodbye VtEngine Edition", merged 2024-08) passes "any VT output that an application generates... to the terminal unmodified" and improves throughput about 20x. Remaining quirks: with VT processing on and `DISABLE_NEWLINE_AUTO_RETURN` off, every LF becomes CRLF, which can break DCS payloads; focus events are injected mid-sequence; a delayed end-of-line wrap emits `\r\n`, which breaks reflow.
- **ConPTY sends DA1 to its host at startup.** Windows Terminal 1.23 fixed a hang when no well-formed reply came back, and 1.26 stops waiting. A custom ConPTY host must answer DA1.
- **Inline-renderer traps:** Windows consoles scroll as soon as the bottom-right cell is written, where xterm-like terminals defer the wrap, so a frame that exactly fills the viewport scrolls one line too many (Ink source). Windows Terminal and VS Code push the viewport into scrollback on `CSI 2J`, so full clears duplicate content. Claude Code reports ConPTY hosts that "coalesce these positioned writes incorrectly and leave fragments" and offers a full-repaint switch.
- **crossterm** (0.29, slow release cadence) supports Windows 7 and up: ANSI on Windows 10+, WinAPI on older systems. Windows input uses console input records, which deliver key-up and repeat natively.
- Console code pages and UTF-8 output are covered in `cli-design`.

## TUI architecture

### Schools

- **Immediate mode with a cell diff: ratatui** (Rust, 0.30). "Immediate rendering with intermediate buffers": the program redraws every widget each frame into a buffer, ratatui diffs against the previous buffer and writes only changed cells. No widget state to synchronize. ANSI escapes inside cell strings are not interpreted, which is also a security property. Codex uses ratatui with a forked crossterm.
- **Retained widget tree with CSS: Textual** (Python, Will McGugan). A compositor of segments with visibility culling by spatial tiles ("Scrolling a view of 8 widgets, takes much the same time as a view of 1000"), partial updates, one write per frame. Textualize the company closed in 2025-05. Textual and Rich remain maintained as open source. McGugan's newer project is Toad, a Textual front end for agent CLIs. Others in this school: Cursive (Rust), tview on tcell (Go), urwid (Python). blessed (Node) is abandoned (last commit 2016).
- **The Elm Architecture: Bubble Tea** (Go, Charm, v2 2026-02). `Update(msg) -> (Model, Cmd)`: the update is pure, and `Cmd` is a description of an effect the runtime performs. v2 adds a renderer "modeled on the ncurses rendering algorithm" and declarative terminal modes in the `View`. Lip Gloss v2 became pure because in v1 it queried the background color while Bubble Tea read stdin, and the two fought over input. Charm's Crush runs on it.
- **React reconciler: Ink** (Node, Vadim Demedes, v8 2026-10, React 19.3+). Components render to a frame, Ink diffs and redraws. v8 added `incrementalRendering` (skip unchanged line prefixes) and an `alternateScreen` option. Before v8, `useInput` received mouse reports, focus events, and CPR replies as if typed. Gemini CLI uses a fork of Ink. Claude Code started on Ink and replaced the renderer while keeping React (reported by a third party, internals closed).
- **OpenTUI** (now `anomalyco/opentui`, formerly `sst/opentui`): a Zig core with TypeScript, React, and Solid bindings, pre-1.0, needs Bun. OpenCode uses it.
- **pi-tui** (Mario Zechner): retained components `render(width) -> lines`, diffed from the first changed line. Deliberately inline.
- **Others:** notcurses (Nick Black, little recent activity), ncurses, libvaxis (Zig, Tim Culverhouse), tcell v3 (Go, with a mock backend for tests).

### Rendering: why inline TUIs flicker

The constraint, in Zechner's words: "The terminal doesn't let you write to the scrollback buffer above the viewport." McGugan on the agent CLIs: "These apps update the terminal by removing the previous lines and writing new output." Ink's source lists when it must fully clear: the previous frame overflowed the viewport, the viewport shrank while full, the frame is leaving full-screen height, or the width decreased.

Mitigations, in order of payoff:

1. **One `write` per frame.** Build the whole frame in a buffer. Many small writes tear.
2. **Wrap the frame in synchronized output (2026)** where supported.
3. **Diff and write only changes**, from the first changed line or by cell.
4. **Keep the live region short.** Freeze finished output as immutable scrollback (Ink `<Static>`) and redraw only the active tail.
5. **Or take the alternate screen and virtualize:** render only what is visible. Claude Code's full-screen mode "only renders messages that are currently visible" to keep bytes per frame low.

Do not debounce rendering to hide missing synchronized output. It adds input latency. pi accepts residual flicker on terminals without 2026.

**Throughput differs by orders of magnitude** between terminals (Quast's tests: VTE took more than 5 hours on the full suite, iTerm2 is CPU-bound). Claude Code's docs name the VS Code terminal, tmux, and iTerm2 as the worst. Bytes per frame are the budget.

**Resize:** on a width change, soft wraps change, so do a full clear and re-render. On a height shrink, the terminal scrolls "only as far as it must" (Ink). Never cache the size once at startup. Each SIGWINCH (or resize event) is a full re-layout.

### Alternate screen or inline: the 2025-2026 shift

The major agent CLIs moved to the alternate screen: Amp (2025-09), OpenCode (always), Codex (full screen by default, with `--no-alt-screen`), Claude Code (full screen by default for users who started on or after 2026-05-06). Gemini CLI tried it and rolled back within a week. pi stays inline on purpose. **VOLATILE.** The trade-off is in Schools of thought.

## Input

- **Keybinding conventions** (Julia Evans, "terminal rules"): Ctrl-C quits a non-interactive program and cancels the current operation in an interactive one; `q` quits a full-screen tool; Ctrl-D on an empty line quits a REPL; readline editing keys (Ctrl-A, Ctrl-E, Ctrl-W deletes a word, Ctrl-U); Ctrl-Z suspends; Ctrl-\ sends SIGQUIT. A TUI in raw mode must implement the ones it keeps, because the kernel no longer does.
- **Shift+Enter** exists only with the kitty protocol or modifyOtherKeys. That is why agent CLIs ship a terminal-setup step. Always offer an alternate binding (Alt+Enter, Ctrl+J, or a trailing backslash).
- **Terminal replies arrive on the input channel.** DA, CPR, DECRQM, OSC 11, focus, and mouse reports must be parsed and routed, never delivered to the application as typed text (the Ink-before-v8 bug produced phantom keystrokes such as `[I` and `[<0;5;3M`). **One component owns stdin.** Two libraries reading it lose keys and swallow replies (the Lip Gloss v1 and Bubble Tea v1 fight).
- **IME composition** is weakly supported in terminals. The recurring bug: the TUI hides the hardware cursor and draws its own caret, so the operating system anchors the candidate window at the hardware cursor's position, which is somewhere else. Fix: after each frame, move the real cursor (CUP) to the logical caret cell, accounting for wide characters. Textual documents the same requirement. These fixes regress often (several Claude Code issues). Windows Terminal added `defaultInputScope` to force English IME input.
- **Mouse capture breaks native selection, search, and copy.** Bypass keys: Shift in most terminals, Option in iTerm2, Fn in Terminal.app. A program that captures the mouse must say how to bypass it, offer an opt-out, and usually reimplement copy (pbcopy, wl-copy or xclip with PRIMARY, `Set-Clipboard`, the tmux buffer, OSC 52 over SSH).
- **Line editors and prompt libraries** (**VOLATILE** versions): rustyline, reedline (nushell), linenoise (antirez), prompt_toolkit (Python), inquire and dialoguer (Rust), @clack/prompts (Node), gum and huh (Go, Charm). GitHub CLI's accessible prompter is built on huh's accessible mode.

## Functional core for a TUI

The Elm architecture is functional core, imperative shell applied to a TUI:

- **State plus event in, new state plus effect descriptions out:** `update(state, event) -> (state, effects)`. Pure, total, and tested with plain values.
- **View is a pure function of state:** `view(state, size, caps) -> Frame`. A frame is data (a cell buffer, a list of lines, or a `View` with mode fields).
- **The shell** owns the terminal: it reads input and turns bytes into typed events (keys, paste, resize, focus, mouse, capability replies), performs effects, diffs frames, writes bytes, and owns setup and teardown.
- **Terminal capabilities are input, not ambient state.** Size, color profile, keyboard-enhancement support, theme, and whether stdout is a TTY arrive as data (Bubble Tea's `KeyboardEnhancementsMsg` and size messages, tcell v3's attribute negotiation step). A widget that calls `isatty` or reads `COLUMNS` mid-render cannot be tested and cannot react to a resize.
- **Terminal modes as data** (Bubble Tea v2's `View` fields) make setup and teardown a diff, not a sequence of imperative calls that can be forgotten on one path.

### Testing

- **Buffer snapshots.** ratatui: `TestBackend::new(80, 20)` plus `insta::assert_snapshot!(terminal.backend())`. Color is not asserted (issue #1402), and the event loop, setup, and teardown are not covered. Textual: `App.run_test(size=(w, h))` returns a `Pilot` (`press`, `click`, `pause`); `pytest-textual-snapshot` produces SVG snapshots. Bubble Tea: `teatest` (`charmbracelet/x/exp/teatest`, experimental, a pseudo-version) with `WithInitialTermSize`, `WaitFor`, `FinalOutput`, golden files. tcell v3: a mock backend. Ink: tests assert the real emitted sequences.
- **Byte-stream tests through a terminal emulator library.** Replay the program's output through libghostty-vt, libvterm, or pyte and assert on the resulting screen state. This tests what the user's terminal will show, including escape handling, which a buffer snapshot cannot.
- **End to end in a PTY:** pexpect or expect, tmux `send-keys` plus `capture-pane`, Charm's `vhs` for scripted recordings. Use a few, for setup, teardown, suspend and resume, resize, and panic restoration.
- **Pin the environment:** size, `TERM`, `COLORTERM`, `NO_COLOR`, locale. A test that inherits the developer's terminal is flaky in CI.

## Inline output: between a plain CLI and a full TUI

- **indicatif** (Rust, 0.18): bars are "completely hidden" when output is not a terminal, draw to stderr, and refresh at most 20 times a second. `suspend(f)` hides the bars, runs `f`, and redraws. **`println()` prints above the bar but does nothing when the bar is hidden** (not a TTY, CI), so logs routed through it vanish in CI. Use `suspend` or a separate logger. Spinners need `enable_steady_tick`.
- **Ink in CI:** non-interactive mode drops erase sequences, cursor moves, synchronized output, resize handling, and keyboard detection, and writes only the final frame.
- **Carriage-return redraws** in a non-terminal log viewer keep every frame or join them into one line. Width math against an unknown width wraps the bar, so each redraw adds a line. Detect the TTY and width, truncate to width, and fall back to periodic plain lines.
- **Live regions** (cursor-up redraw of the last N lines) work only while those lines are inside the viewport.
- The decision to show progress on a given stream belongs to `cli-design`. The mechanics are here.

## Accessibility in terminals

Screen readers read the character grid and follow the cursor. The GitHub CLI team (Ryan Hecht and Andrew Feller, 2025-05): "Assistive technologies access this matrix of characters, analyze its layout, and try to infer structure." The failures come from "Non-alphanumeric visual cues and uses of constant screen redraws."

- **"The text mode lie"** (xogium.me, 2026-01): canvas-style frameworks (Ink, Bubble Tea, tcell) are hostile. Under Speakup, a spinner and a timer that jump the cursor interleave status ticks with chat fragments, and NVDA crashes or lags up to 10 seconds under heavy redraw. Good examples: nano and vim with the cursor-position display off, menuconfig (one vertical list), Irssi (VT100 scrolling regions). The asks: a mode with no cursor jumping, no constant redraws, and use of real terminal scrolling.
- **GitHub CLI (gh v2.72):** an accessible prompter on huh, static text in place of the braille spinner, 4-bit palettes.
- **Claude Code's screen-reader mode** is a concrete design reference: an explicit flag and environment variable (not auto-detected); flat labeled lines (`you:`, `tool:`, `error:`); no box drawing; no color-only cues; static spinners; tables as "Header: value"; always inline, never the alternate screen; the cursor stays on the input caret; typing writes only changed characters; menus become numbered lists; a bell on attention; OSC 133 marks for jump-to-prompt.
- **Takeaways:** the alternate screen has no native scrollback, so screen-reader users lose history. Ship a plain, linear mode, and keep non-TTY output linear. Keep the hardware cursor at the logical focus point. Never encode meaning only in color or position. Ghostty 1.2 added a read-only macOS accessibility API. No primary source was found on NVDA with Windows Terminal's UI Automation or on Orca specifics.

## Escape-injection security

- **The class:** any untrusted string printed to a terminal can contain control sequences. Consequences range from a garbled screen and title spam, to hyperlink spoofing and clipboard writes, to **command execution** when the terminal echoes attacker-chosen bytes back into the input stream.
- **G-Research, "The terminal escapes"** (2023-10, research attributed to David Leadbeater, not confirmed on the page): ten CVEs with remote-code-execution potential, including ConEmu title-report echoback (CVE-2022-46387, CVE-2023-39150), kubectl title spam (CVE-2021-25743) and `kubectl describe` through a termination log (CVE-2022-44702), Python `http.server` logging raw escapes combined with iTerm2 echoback (CVE-2022-45872), and `git log` through `less` with OSC 8 and a mintty bug. Its rules: "any control character (except tabs and newlines) printed to the screen should be escaped", and "the more features the terminal has, the more likely it is to have security issues."
- **kitty before 0.47.3, CVE-2026-54057** (2026-06): the OSC 21 color-query reply echoed an attacker-chosen key, newlines included, into PTY input, so `cat` of a crafted file ran commands. Same class as xterm's CVE-2008-2383 and CVE-2022-45063. **iTerm2 3.5.x before 3.5.2, CVE-2024-38396:** title report plus tmux integration (not verified at NVD).
- **Echo-back replies** are the dangerous surface: title report (`CSI 21 t`), DECRQSS, OSC color queries with unknown keys, font queries, OSC 52 read. xterm gates these with `allowWindowOps` and related resources that escape sequences cannot change.
- **What a TUI or CLI author must do:** sanitize at the boundary every untrusted string before it reaches the screen: filenames, log lines, model output, git data, HTTP data, error messages that echo input. Strip C0 controls except `\t` and `\n`, strip C1 (0x80-0x9F as raw bytes, and U+0080-U+009F in UTF-8), and strip ESC. Ink v8 ships a sanitizer. ratatui's buffer model does not interpret escapes in cell strings. Render hyperlinks from data. Never pass through an OSC sequence found in content.

## Schools of thought

### terminfo or a hardcoded modern set

**Hardcode (tcell v3, Garrett D'Amore; fish 4.1+; kakoune; prompt_toolkit; linenoise; libvaxis; chalk; crossterm).** terminfo "often takes 10+ years to add new terminal features" (quoted by Julia Evans), and it still lacks truecolor, modern mouse modes, bracketed paste, styled underlines, and keyboard protocols. Every terminal of this century understands a common xterm-derived core. Reading terminfo adds a dependency, a failure mode over SSH, and wrong answers.

**terminfo (Thomas Dickey and ncurses, the curses tradition, users who depend on `TERM=dumb` and per-terminal behavior).** It is the only mechanism that ever worked for odd terminals, serial consoles, and embedded displays, and abandoning it hardcodes today's majority. Julia Evans: "there's no standard for how that should work in a post-terminfo world."

**Query first (fish, tmux):** ask the terminal (DA1, XTGETTCAP, XTVERSION) and fall back to the common core. This is the middle position, and it inherits all the probe-reliability problems above.

### Alternate screen or inline

**Alternate screen (Amp, OpenCode, Codex default, Claude Code default since 2026-05).** No flicker from clearing scrollback, flat memory with virtualized rendering, full mouse support, and the program owns the whole surface. The cost: the program must reimplement search, selection, copy, and scrolling, native Cmd-F stops working, history disappears on exit unless printed back, and screen-reader users lose scrollback. It breaks in Zellij and under iTerm2's `tmux -CC`.

**Inline (pi and Mario Zechner: "constraints make for minimal programs"; Claude Code's earlier stance, Thariq: "We value this native experience a lot"; the accessibility writers).** Output stays in the terminal's own scrollback, native search, selection, and copy work, and it composes with the user's terminal. The cost: nothing above the viewport can change, so any frame that grows past the screen forces clears and duplicates, and flicker is structural.

When each fits: a conversation or log that the user wants to keep and search leans inline. A dashboard, editor, or file manager that owns the screen leans alternate. An `auto` mode that falls back to inline in known-bad environments, plus an explicit flag both ways, is the common resolution in shipping tools.

### Immediate or retained mode

**Immediate (ratatui):** redraw everything each frame from state, let the diff find changes. Simple, no widget state to keep consistent with application state, easy to reason about. **Retained (Textual, Cursive, tview, urwid, pi-tui):** widgets keep state and the compositor updates only what changed. McGugan argues retained compositing is what makes single-cell partial updates and culling cheap at scale.

### Elm, React, or widget objects

**Elm (Charm):** one owner of all IO, effects as data, update is pure. **React (Ink, OpenTUI bindings):** the component model web developers already know, and a large ecosystem. Zechner: "I definitely don't want to write my TUI like a React app." **Widget objects (Textual, tview, Cursive):** the classic GUI-toolkit model with CSS-like styling in Textual. The accessibility critics blame canvas-style rendering in all three, not one model.

### Mouse capture by default

**Capture (Claude Code full-screen mode with opt-outs; tmux 3.8 turning `mouse` on by default):** scrolling, clicking, and in-app selection work the way users of GUI apps expect. **Do not capture:** it breaks native selection and search, especially over SSH and inside tmux, and breaks terminal features such as kitty hints. Users who live in the terminal treat native selection as non-negotiable.

### Should rich TUIs exist at all

**Plain and composable (the accessibility writers, "The text mode lie", GitHub CLI's plain modes, Julia Evans' rules):** line-oriented output works with screen readers, pipes, logs, `grep`, and every terminal, and a TUI is a choice to exclude some users. **Rich TUIs (Charm, Textual, the agent CLIs):** many tasks are interactive by nature, and a well-built TUI is faster for the people who can use it. The common ground: a rich TUI must ship a plain mode.

### kitty keyboard protocol or legacy input

**Adopt:** it fixes every ambiguity (Tab and Ctrl-I, Esc, Shift+Enter, key release, layout independence), and implementations now cover most modern terminals. **Hold back:** implementations are buggy in places (Konsole, older iTerm2), the flags survive a crash and break the user's shell, tmux does not implement it, and legacy fallbacks are still required. modifyOtherKeys and fixterms are the older proposals.

### Graphics protocols

**kitty graphics (Ghostty, Tim Culverhouse: "the Kitty graphics protocol is the current leader"; "sixel is underspecified"):** a modern, well-specified protocol. **sixel (foot, WezTerm, Windows Terminal, xterm, libsixel and chafa, arewesixelyet.com; Jeffrey Ollie: refusing sixel is "like saying that you shouldn't implement GIF support because animated PNGs are a thing"):** the only protocol with wide legacy support. iTerm2's OSC 1337 is a third, iTerm2-centric option. A program that shows images needs detection and a text fallback whichever it picks.

### Richer or minimal terminals

**Richer (Hashimoto's libghostty and OSC 7501, kitty's protocols, Warp, Contour's extension catalog):** the terminal is a platform and should grow structured features. **Minimal (G-Research: "the more features the terminal has, the more likely it is to have security issues"; Alacritty's scope; suckless st):** every feature is attack surface, and the reply-echo CVE record supports the concern.

### Probe or assume

**Probe (Quast: test interactively for specific features):** the only way to know. **Assume (D'Amore's tcell v3, the hardcode camp):** probes print garbage on some terminals (Terminal.app, GNU Screen) and get false answers from others (iTerm2). Both camps agree probes must never block on a timeout.

## Anti-pattern catalog

Each entry: pattern, trigger, consequence, fix.

1. **Raw mode with no guard or panic hook.** A panic or early `?` return. No echo, the screen stuck in the alternate buffer, the panic message lost. `ratatui::init` or a `Drop` guard plus a restoring panic hook installed after other hooks.
2. **kitty flags pushed and never popped.** Crash, SIGKILL, or a missed exit path. bash and Python receive CSI u for modified keys. Pop on every exit path and before leaving the alternate screen. Document `reset`.
3. **One keyboard-mode stack for both screens.** Entering and leaving the alternate screen. Wrong modes on the main screen. Separate stacks per the kitty spec.
4. **A SIGINT handler used for quit in raw mode.** `ISIG` is off. Ctrl-C does nothing. Handle byte 0x03 or the key event.
5. **Ctrl-Z ignored in raw mode.** The user presses Ctrl-Z. Nothing happens, or 0x1A is inserted. Restore, `raise(SIGTSTP)`, re-enter and redraw on SIGCONT.
6. **`\n` with `OPOST` off.** Any output in raw mode. Staircase lines. Write `\r\n`.
7. **Capability query with a timeout.** A terminal that never answers. Startup stalls. Follow each probe with DA1.
8. **Probing Terminal.app or GNU Screen with OSC, DCS, or DECRQM.** Stray characters on screen. Gate on `TERM_PROGRAM`, or probe inside the alternate screen.
9. **Trusting DECRQM.** iTerm2 says "supported" for fictional modes. Features enabled that do nothing. Verify by effect.
10. **`TERM=xterm-256color` taken to mean xterm.** Wrong feature and key assumptions. Identify with XTVERSION and degrade.
11. **Truecolor gated only on `COLORTERM`.** SSH drops it. 256-color fallback on capable terminals. Also check XTGETTCAP `RGB`.
12. **Hard-coded truecolor theme, no 16-color fallback.** A user's light background. Unreadable text, the user's palette ignored. Default to ANSI 16 roles, detect the theme with OSC 11 or 2031.
13. **`CSI 2J` or `3J` in an inline renderer.** VS Code or Windows Terminal. Duplicated frames in scrollback, or scrollback erased. Home plus erase-below within the frame region.
14. **Redrawing content that has scrolled off.** A frame taller than the viewport. Full clears, flicker, duplication. Freeze finished output, keep the live region short, or use the alternate screen.
15. **Many small writes per frame.** Any terminal. Tearing and flicker. One buffer, one write, wrapped in DEC 2026.
16. **Synchronized output or cursor control sent to CI or a pipe.** Non-TTY output. Escape garbage in logs. Emit only to a TTY and not under CI.
17. **A frame that exactly fills the viewport on Windows.** Windows consoles wrap eagerly. Extra scroll and stale copies. Never write the bottom-right cell, or full-repaint on Windows.
18. **A spinner that moves the hardware cursor.** A screen reader. Fragmented speech, NVDA instability. A plain or reduced-motion mode, static text.
19. **A fake caret with the real cursor parked elsewhere.** IME input, a magnifier. Candidate window in the wrong place, lost focus. CUP the real cursor to the caret every frame.
20. **Mouse capture with no bypass hint or opt-out.** SSH, tmux, a user who selects text. Native select, copy, and search gone. Opt-out flag, documented bypass key, in-app copy with an OSC 52 fallback.
21. **Paste handled as keystrokes.** Bracketed paste off, or paste routed through bindings. Pasted text triggers commands or submits early. Enable 2004 and treat content as data.
22. **Untrusted strings printed raw.** Filenames, logs, model output, HTTP data. Title, clipboard, or input injection, and command execution on echo-back terminals. Strip C0 except `\t\n`, C1, and ESC at the boundary.
23. **OSC 52 read as a feature.** Any terminal with read gated. Blocked, or a permission prompt, and it normalizes exfiltration. Write only. Use the native clipboard API locally.
24. **Size cached once, or SIGWINCH ignored.** A resize. Broken layout. Full re-layout on each resize, full clear on a width change.
25. **Width from string length, or `wcwidth` alone.** Emoji with VS16, ZWJ sequences, ambiguous CJK. Misaligned columns, cursor drift. Grapheme segmentation, a maintained width library, CPR measurement when exact (algorithms in `text-engineering`).
26. **Binding Ctrl-I, Ctrl-M, or Ctrl-H without the kitty protocol.** Legacy terminals. The binding fires on Tab, Enter, or Backspace. Check enhancement support, offer fallbacks.
27. **Relying on Shift+Enter.** Legacy terminals, tmux, Terminal.app. Indistinguishable from Enter. An alternate binding and a setup command.
28. **Two components reading stdin.** A styling library querying the terminal while the event loop reads. Lost keys, swallowed replies. One IO owner routes every reply.
29. **Terminal replies delivered as typed input.** DA, CPR, focus, mouse reports. Phantom keystrokes. Parse and route every CSI, OSC, and DCS reply.
30. **Assuming tmux passes everything through.** Graphics, kitty keyboard, older tmux and synchronized output. Features silently lost. Detect `$TMUX`, use passthrough with DCS wrapping, document a minimum version.
31. **Alternate screen inside Zellij or iTerm2 `tmux -CC`.** No scrollback, broken mouse. An `auto` mode that falls back to inline.
32. **No non-TTY path.** CI, pipes, `-p` modes. Frames and escapes flood logs. Detect TTY, emit plain lines, provide `--no-tui`.
33. **Logging through indicatif `println` in CI.** Bars hidden. Logs vanish. `suspend`, or a separate logger.
34. **A terminal-specific `TERM` on a remote host without its terminfo.** SSH. "unknown terminal type", remote programs break. `kitten ssh`, Ghostty `ssh-terminfo`, `infocmp | tic -x -`.
35. **Debouncing renders to hide flicker.** Terminals without 2026. Input latency. Use 2026 where supported, accept residual flicker elsewhere.
36. **`TestBackend` snapshots treated as full coverage.** Color, setup and teardown, event loop untested. Add PTY end-to-end tests and replay output through an emulator library.
37. **Imperative mode toggles scattered through the code.** Many paths enable mouse, paste, or the alternate screen. One path forgets to disable. Modes as data owned by one runtime, or one guard type per mode.
38. **Widget code that reads `isatty`, `COLUMNS`, or env mid-render.** A resize, a test. Stale layout, untestable widgets. Pass size and capabilities into the view as data.

## Authorities

- **Thomas E. Dickey:** "XTerm Control Sequences" (https://invisible-island.net/xterm/ctlseqs/ctlseqs.html), ncurses, the terminfo database. The reference for what a sequence means.
- **ECMA-48 (5th edition, 1991)** and the DEC VT100, VT220, and VT510 manuals (vt100.net). Paul Williams' DEC-compatible parser state machine for anyone writing a decoder.
- **Kovid Goyal, kitty protocol docs** (https://sw.kovidgoyal.net/kitty/): keyboard, graphics, clipboard, notifications, and the implementer list.
- **Christian Parpart, Contour VT extensions** (https://contour-terminal.org/vt-extensions/): modes 2026, 2027, 2031. Its adoption tables are stale.
- **Egmont Koblinger:** the OSC 8 hyperlink spec.
- **Jeff Quast:** ucs-detect results (the best current support matrix), "State of Terminal Emulators in 2025", wcwidth.
- **Julia Evans (jvns.ca):** "terminal rules" (2024), "escape code standards" (2025), the "Secret Rules of the Terminal" zine. The clearest map of conventions and why they exist.
- **Mitchell Hashimoto:** Ghostty, libghostty, OSC 7501. **Wez Furlong:** WezTerm. **The Windows Terminal team (Leonard Hecker, James Holderness):** ConPTY's 2024 rewrite.
- **Garrett D'Amore:** tcell v3 and the case for dropping terminfo, the Terminal.app write-up.
- **Will McGugan:** Textual's rendering algorithms posts (2024). **Christian Rocha and Charm:** Bubble Tea v2 and its renderer. **Vadim Demedes:** Ink. **Mario Zechner:** pi-tui and the inline argument. **Tim Culverhouse:** libvaxis. **Nick Black:** notcurses.
- **The fish shell terminal-compatibility document** (doc_src/terminal-compatibility.rst): a minimal list of required sequences, usable as a spec.
- **Codex `codex-rs/tui`** (open source): reference code for suspend and resume, keyboard-stack handling, and cleanup.
- **G-Research, "The terminal escapes":** escape-injection CVE catalog and rules.
- **Accessibility:** the GitHub CLI accessibility post (Hecht and Feller, 2025), "The text mode lie" (xogium.me, 2026), Claude Code's accessibility docs.

## Severity rubric

What the levels mean in this domain specifically.

- **blocker**: untrusted text printed raw to a terminal in a tool that handles attacker-reachable data (filenames from archives, model output, HTTP responses, logs from other users); raw mode or the alternate screen entered with no restoration on panic or error paths, in a tool users run daily; a feature that depends on OSC 52 read; pasted content executed without confirmation.
- **major**: kitty keyboard flags or mouse modes not popped on some exit path; Ctrl-C or Ctrl-Z broken in raw mode; a capability probe that blocks on a timeout or prints garbage on a mainstream terminal; no non-TTY or plain mode for a TUI that also runs in CI or scripts; full clears in an inline renderer producing visible flicker or duplicated scrollback; two components reading stdin; terminal replies leaking into input; a fake caret that breaks IME input.
- **minor**: no synchronized output where it is supported; many writes per frame; truecolor detection only by `COLORTERM`; no 16-color fallback; size not re-read on resize; Shift+Enter with no alternate; mouse capture without a documented bypass; missing tmux or Zellij handling.
- **nit**: mouse mode 1005 instead of 1006; SGR reset missing at exit when nothing else is left set; a binding that differs from readline convention without reason.
- **insight**: structural observations. "Terminal modes are toggled imperatively in six places; making them fields of the view would make teardown a diff." "Every widget reads the terminal size itself; passing size and capabilities in as data would make the view a pure function and the snapshot tests meaningful." "This app is inline by default and its frames routinely exceed the viewport, so flicker is structural, not a tuning problem."

## Changelog

**Source research**: `~/.claude/local/research-notes/terminal-tui-research.md` (raw notes with URLs) and `~/.claude/local/research-notes/terminal-tui-research-summary.md` (structured findings with [V] / [F] / [I] tags and the gaps list). Read both before a refresh: they record that ucs-detect is the only current support matrix and its columns mean "detected", that Contour's adoption tables are stale, that Ghostty is moving off GitHub, and that Claude Code internals are reported second-hand.

- **2026-10-21**: Initial version. Corrections to the original research brief: Terminal.app has truecolor in macOS 26, the major agent CLIs moved to the alternate screen in 2025-2026, Textualize the company closed in 2025-05 (Textual and Rich continue), OpenTUI moved to `anomalyco/opentui`, tcell v3 and fish 4.1+ dropped terminfo. Known gaps: tmux kitty keyboard status (inferred from absence), Konsole kitty keyboard (one source), VTE synchronized output (sources conflict), Ghostty OSC 52 read default, Codex alternate-screen config key, Claude Code renderer internals, three CVEs not verified at NVD, Windows code pages, NVDA with Windows Terminal, Orca and VoiceOver specifics, Warp architecture.
