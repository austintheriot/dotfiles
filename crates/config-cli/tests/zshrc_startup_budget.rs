//! A regression bar on interactive zsh startup cost.
//!
//! Startup used to run 7-9s: nvm and pyenv each paid their full init at
//! every shell. `zshrc_node_startup` and `zshrc_python_startup` pin down the
//! two fixes by asserting on source text, which is exact and cheap but only
//! covers the causes already known. This suite covers the general case: it
//! measures what a shell actually costs, so the NEXT thing that gets sourced
//! eagerly is caught without anyone having to predict it first.
//!
//! `se` builds ~107 panes, so the cost is multiplied by roughly a hundred
//! before the terminal is usable. 100ms of new startup work is ten seconds
//! there.
//!
//! What makes a timing test survivable rather than flaky:
//!
//! 1. It measures against a same-machine baseline, not a wall-clock
//!    constant. `zsh -f` skips every startup file, so subtracting it removes
//!    the process spawn, the loader, and the machine's general speed. What
//!    remains is what this repo's config costs, which is the only thing a
//!    budget can fairly hold still across a fast laptop and a loaded CI
//!    runner.
//!
//! 2. It takes the MINIMUM of several runs, not the mean. Startup cost has a
//!    hard floor and an unbounded tail: a scheduler preemption or a
//!    competing build can only ever make a sample slower. The minimum is the
//!    closest estimate of the true cost, and it is the statistic that does
//!    not drift when the machine is busy. A mean over the same samples is a
//!    measure of the machine's load.
//!
//! 3. The budget is generous. It is set well above the measured cost, so it
//!    catches a regression of a scale that matters (a newly eager tool init)
//!    and stays quiet about noise. A tight budget on a timing test is a
//!    standing false alarm, and a standing false alarm gets disabled.
//!
//! The budget is deliberately NOT a target to optimise toward. Lowering it
//! as startup improves is how a suite like this becomes brittle; the number
//! moves only when a measurement shows the headroom is gone.
//!
//! **Every constant here is carried over verbatim from the shell suite.** A
//! timing test that is stricter than its predecessor flakes on a loaded
//! machine, and this repo has already been bitten once by a
//! timing-dependent test (`tmux-update-window-names`, fixed in `8591f242`).

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The budget for this repo's config above a bare shell.
///
/// Measured on the development machine at ~161ms for an interactive shell
/// against a ~7ms bare shell, so ~154ms of config. 400ms leaves better than
/// 2x headroom for a slower or busier machine while still failing on the
/// regression this exists to catch: a tool init that is sourced eagerly
/// again costs 380ms (pyenv) to 2.4s (nvm) on its own.
const BUDGET: Duration = Duration::from_millis(400);

/// Runs per measurement. The minimum of 5 is stable in practice; more runs
/// cost suite time for a floor that has already stopped moving.
const RUNS: usize = 5;

/// Wall clock for one `zsh` invocation with the given flags.
///
/// **All three standard streams are null, and stdin especially.** An
/// interactive zsh that inherits a terminal pays that terminal's setup, and
/// measured here that cost was 405ms for a BARE `zsh -f -i` against the 7ms
/// the shell suite recorded. It lands in both measurements, but not equally,
/// so it swamps the ~154ms of config the budget is about: with a tty
/// inherited, deliberately shrinking the budget to 1ms still passed, which
/// is the vacuous-measurement shape this suite exists to prevent. The shell
/// suite ran its timed shells under a pipe and never had a tty to inherit.
///
/// The clock is `Instant`, so the shell suite's `zmodload zsh/datetime`
/// hazard does not carry over: that suite read `$EPOCHREALTIME` from inside
/// a bare zsh, and without the zmodload it expanded to the empty string,
/// every difference computed as zero, and the budget passed while measuring
/// nothing. Rust has no such failure mode, but the floor assertion below is
/// kept anyway for the failure that DOES remain: a shell that exits
/// instantly because it errored out loads no config and costs nothing.
fn elapsed(flags: &[&str]) -> Duration {
    let started = Instant::now();
    let status = Command::new("zsh")
        .args(flags)
        .args(["-c", "exit"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let taken = started.elapsed();
    assert!(status.is_ok(), "zsh could not be spawned: {status:?}");
    taken
}

/// The floor of [`RUNS`] measurements. See note 2 in the module docs for why
/// the minimum is the right statistic here.
fn best_of(flags: &[&str]) -> Duration {
    (0..RUNS)
        .map(|_| elapsed(flags))
        .min()
        .unwrap_or_else(|| unreachable!("RUNS is a non-zero constant"))
}

/// Both assertions, in one test, because the second is only meaningful after
/// the first and they share one expensive set of measurements.
///
/// The shell suite ordered them the same way and for the same reason: a
/// measurement of zero passes a budget, so the floor is asserted BEFORE the
/// budget rather than alongside it.
#[test]
fn interactive_startup_stays_within_the_config_budget() {
    if !dotfiles_test_support::zsh::available() {
        dotfiles_test_support::skip("startup budget: no zsh on this machine");
        return;
    }

    // This suite measures the developer's own shell: `zsh -i` loads
    // $HOME/.zshrc. That is only the config under test when HOME is the
    // repo. Elsewhere it would time somebody else's shell and report it as
    // this repo's cost.
    let root = dotfiles_test_support::repo::root();
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    if home.as_deref() != Some(root.as_path()) || !root.join(".zshrc").is_file() {
        dotfiles_test_support::skip("startup budget: HOME is not the repo");
        return;
    }

    // One untimed run first. A cold start pays for filesystem cache misses
    // on every file the config touches, which is a property of the machine's
    // recent history rather than of the config, and it lands entirely in the
    // first sample.
    let _ = elapsed(&["-i"]);

    let bare = best_of(&["-f", "-i"]);
    let full = best_of(&["-i"]);
    // A negative difference means the two measurements overlapped in noise,
    // which only happens when the config costs almost nothing. Saturating to
    // zero reports it as free rather than as a negative cost.
    let config = full.saturating_sub(bare);

    // The floor, asserted BEFORE the budget, and on `full` ALONE.
    //
    // `config` is excluded because zero is a legitimate result there: the
    // saturating subtraction above says why.
    //
    // `bare` is excluded because zero is legitimate there too, which the
    // shell suite's first version of this floor got wrong. A bare `zsh -f`
    // inside the test container measures 0-1ms, so the minimum of five
    // samples is genuinely 0, and the container run reported "the clock read
    // as zero" while printing a full time that could only have come from a
    // working clock.
    //
    // What remains is real signal. A full interactive shell that spawns a
    // process and sources this entire config cannot cost zero, so a zero
    // there means the shell never ran the config at all.
    assert!(
        full > Duration::ZERO,
        "the harness measured a zero startup time for a full interactive \
         shell, so it loaded no config and the budget below would pass \
         having measured nothing"
    );

    assert!(
        config <= BUDGET,
        "interactive startup costs {config:?} above a bare shell, over the \
         {BUDGET:?} budget (bare {bare:?}, full {full:?}). The regression \
         this catches is a tool init that is sourced eagerly again: pyenv \
         costs 380ms and nvm 2.4s on their own, multiplied by the ~107 panes \
         `se` builds."
    );
}
