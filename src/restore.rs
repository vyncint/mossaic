//! Putting the terminal back, on every way out.
//!
//! A TUI borrows the terminal: the alternate screen, raw mode, mouse
//! reporting. The borrow has to be returned on **every** exit, and there are
//! four — the ordinary one, a panic, a signal, and a write that fails
//! because the reader has gone.
//!
//! Before 0.7.0 two of the four were covered. `main` restored on the way out
//! and installed a panic hook that says why in as many words: "A panic that
//! unwinds past the event loop would otherwise leave mouse reporting on, and
//! the shell printing escape codes at every click." A **signal** reached the
//! same state by a path with no guard at all: under a pty, `kill -INT` on
//! either binary emitted zero bytes to the tty, leaving the shell inside the
//! alternate screen with mouse tracking on, ECHO, ICANON and ISIG off, and
//! no working Ctrl-C. The way out is to type `reset` blind, which a
//! first-time user does not know, and nothing on screen says so — the
//! failure is in what was *not* written.
//!
//! Whoever reaches for a signal is already having a bad time: the chart is
//! sitting on "loading" because `gh` is slow, so they `kill %1` from another
//! pane, or they wrapped it in `timeout`, or a script signalled the process
//! group.
//!
//! ## Why `signal-hook` rather than `libc::sigaction`
//!
//! `Cargo.toml` is `unsafe_code = "forbid"`, and `SECURITY.md` advertises
//! that posture ("the one libc dependency is used for a single constant, not
//! a call"). `sigaction` is an unsafe call, so the obvious route is barred.
//!
//! `signal-hook` registers safely and — the part that matters — is *already
//! in the tree*: crossterm pulls it through ratatui for its event stream, at
//! the same version pinned here. So this adds an import, not a dependency:
//! no new third-party code, no new licence to review, no new supply chain.
//!
//! The work happens on a thread rather than in a handler, which is what
//! makes it safe to do anything at all: a real signal handler may call only
//! async-signal-safe functions, and writing escape sequences through Rust's
//! stdout is not one of them.

use std::io;

use ratatui::crossterm::cursor::Show;
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::execute;

/// Give the terminal back: mouse reporting off, alternate screen off, raw
/// mode off, cursor shown.
///
/// Best-effort by construction. This runs on the way out of a process that
/// may already be failing, and on a terminal that may already have gone; an
/// error here must not mask the reason we are leaving.
pub fn terminal() {
    // Mouse first: `try_restore` leaves the alternate screen, and a mouse
    // report arriving after that lands on the shell's own screen.
    let _ = execute!(io::stdout(), DisableMouseCapture);
    let _ = ratatui::try_restore();
    // Explicitly, and last. Leaving the alternate screen restores that
    // screen's cursor state, which is not necessarily the shell's: a hidden
    // cursor is the one piece of this a user cannot see is missing, and
    // cannot guess the cure for.
    let _ = execute!(io::stdout(), Show);
}

/// Restore the terminal on a panic, then let the previous hook run.
///
/// Kept beside [`on_signal`] because they are the same obligation: the two
/// exits nobody writes code for.
pub fn on_panic() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        terminal();
        previous(info);
    }));
}

/// Restore the terminal on `SIGINT`, `SIGTERM` and `SIGHUP`, then die of the
/// signal.
///
/// Re-raising with the default disposition matters: a process killed by a
/// signal must still *report* as killed by that signal, or a shell's `$?`, a
/// `timeout` wrapper and a supervisor all learn the wrong thing about why it
/// stopped. `emulate_default_handler` does exactly that.
///
/// Ctrl-C *typed into* a TUI is a key, handled by the event loop, and does
/// not come through here — the damage always needed an actual signal.
///
/// Failing to register is not worth failing the run over: the terminal is
/// no worse off than it was before 0.7.0, and the user asked to draw a
/// chart, not to install a signal handler.
pub fn on_signal() {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    let Ok(mut signals) = signal_hook::iterator::Signals::new([SIGINT, SIGTERM, SIGHUP]) else {
        return;
    };
    std::thread::spawn(move || {
        for signal in signals.forever() {
            terminal();
            // Unregisters our handler and re-raises, so the exit status is
            // death-by-signal rather than a plain code.
            let _ = signal_hook::low_level::emulate_default_handler(signal);
        }
    });
}

/// Everything a TUI owes the terminal, installed in one call.
///
/// Both binaries take the alternate screen and both enable mouse reporting,
/// so both need all three guards. `mossaic-art --draw` had none of them.
pub fn guard_terminal() {
    on_panic();
    on_signal();
}

/// Turn mouse reporting on, for a view that reads the pointer.
///
/// Here rather than at the call sites so that the enable and the disable are
/// written next to each other and cannot drift apart.
///
/// # Errors
///
/// Propagates the write to stdout.
pub fn capture_mouse(on: bool) -> io::Result<()> {
    let mut out = io::stdout();
    if on {
        execute!(out, EnableMouseCapture)
    } else {
        execute!(out, DisableMouseCapture)
    }
}
