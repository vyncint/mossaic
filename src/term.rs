//! What the terminal can actually do — asked, not guessed.
//!
//! Environment sniffing (`TERM`, `KITTY_WINDOW_ID`, `TERM_PROGRAM`) gets this wrong
//! in both directions: it misses terminals it has never heard of, and claims support
//! inside tmux or ssh where the escape never arrives. So mossaic asks the terminal
//! itself and reads the reply, with a deadline for the terminals that stay silent.
//!
//! One write, four questions, one round trip:
//!
//! | query | answer | tells us |
//! | --- | --- | --- |
//! | `APC _Gi=…,a=q` | `_Gi=…;OK` | it speaks the kitty graphics protocol |
//! | `OSC 11 ?` | `rgb:rrrr/gggg/bbbb` | the background colour, so light/dark is not a guess |
//! | `CSI 16 t` | `CSI 6;h;w t` | one character cell in pixels |
//! | `CSI c` | `CSI ?…;4;… c` | attribute 4 is sixel — and this reply is the sentinel |
//!
//! The device-attributes reply comes last and every terminal answers it, so it
//! doubles as the "everything that is coming has come" marker: the usual round trip
//! is a millisecond or two, not the timeout.

use std::time::Duration;

use crate::primer::Rgb;

/// Arbitrary id for the kitty query. `a=q` only asks, so nothing is ever stored.
const PROBE_ID: u32 = 7379;

const QUERIES: &str = concat!(
    "\x1b_Gi=7379,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\",
    "\x1b]11;?\x1b\\",
    "\x1b[16t",
    "\x1b[14t",
    "\x1b[c",
);

/// What the terminal said it can do.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Caps {
    /// The terminal answered `OK` to the kitty graphics query.
    pub kitty: bool,
    /// Attribute 4 was among its device attributes.
    pub sixel: bool,
    /// One character cell in pixels (width, height), which sets the image scale.
    pub cell: Option<(u16, u16)>,
    /// The whole window in pixels, a fallback for terminals that answer `CSI 14 t`
    /// but not `CSI 16 t`.
    pub window: Option<(u16, u16)>,
    /// The background colour it reported, which decides light or dark.
    pub background: Option<Rgb>,
    /// The terminal said something. A silent terminal is not a terminal that said no:
    /// it is a pipe, a recording, or something too old to be asked.
    pub answered: bool,
}

/// Ask, with a deadline. Call this with the terminal already in raw mode, or the
/// replies arrive line-buffered and echoed.
#[cfg(unix)]
/// Ask the terminal what it can do, with a deadline.
///
/// Call this with the terminal already in raw mode, or the replies arrive
/// line-buffered and echoed. A terminal that never answers costs the timeout
/// and nothing else.
pub fn probe(timeout: Duration) -> Caps {
    use std::fs::OpenOptions;
    use std::io::{ErrorKind, Read, Write};
    use std::os::unix::fs::OpenOptionsExt;
    use std::time::Instant;

    // The controlling terminal rather than stdout: stdout may be a pipe, and
    // O_NONBLOCK means a terminal that never answers costs us the timeout, not a
    // wedged thread holding the keyboard.
    let Ok(mut tty) = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open("/dev/tty")
    else {
        return Caps::default();
    };
    if tty.write_all(QUERIES.as_bytes()).is_err() || tty.flush().is_err() {
        return Caps::default();
    }

    let deadline = Instant::now() + timeout;
    let mut reply = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        match tty.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                reply.extend_from_slice(&chunk[..read]);
                if attributes(&String::from_utf8_lossy(&reply)).is_some() {
                    break;
                }
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    break;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(_) => break,
        }
    }
    parse(&String::from_utf8_lossy(&reply))
}

#[cfg(not(unix))]
pub fn probe(_timeout: Duration) -> Caps {
    Caps::default()
}

/// Read a terminal's replies. Public so the parsing can be tested without a
/// terminal to ask.
pub fn parse(reply: &str) -> Caps {
    let mut caps = Caps {
        answered: !reply.is_empty(),
        ..Caps::default()
    };

    // Only OK counts. A terminal that knows the protocol but not this transmission
    // medium answers ENOTSUPPORTED, and would then be sent images it cannot draw.
    caps.kitty = reply.contains(&format!("_Gi={PROBE_ID};OK"));

    // Device attributes: a parameter list, in which 4 means sixel.
    if let Some(attributes) = attributes(reply) {
        caps.sixel = attributes.split(';').any(|item| item == "4");
    }

    // CSI 6 ; height ; width t — note the reply is height first.
    caps.cell = report(reply, "\x1b[6;").map(|(height, width)| (width, height));
    caps.window = report(reply, "\x1b[4;").map(|(height, width)| (width, height));
    caps.background = background(reply);
    caps
}

/// The body of a primary device attributes reply, `CSI ? … c`.
fn attributes(reply: &str) -> Option<&str> {
    let rest = reply.split("\x1b[?").nth(1)?;
    let end = rest.find('c')?;
    let body = &rest[..end];
    body.chars()
        .all(|c| c.is_ascii_digit() || c == ';')
        .then_some(body)
}

/// A two-number `CSI` report, e.g. `CSI 6 ; 20 ; 10 t`.
fn report(reply: &str, prefix: &str) -> Option<(u16, u16)> {
    let rest = reply.split(prefix).nth(1)?;
    let body = &rest[..rest.find('t')?];
    let (first, second) = body.split_once(';')?;
    let (first, second) = (first.trim().parse().ok()?, second.trim().parse().ok()?);
    (first > 0 && second > 0).then_some((first, second))
}

/// `OSC 11 ; rgb:rrrr/gggg/bbbb`, terminated by BEL or ST, with 1 to 4 hex digits
/// per channel — 16 bits per channel is the common answer, so scale rather than
/// truncate.
fn background(reply: &str) -> Option<Rgb> {
    let rest = reply.split("\x1b]11;").nth(1)?;
    let body = rest.split(['\x07', '\x1b']).next()?;
    let channels = body
        .strip_prefix("rgb:")
        .or_else(|| body.strip_prefix("rgba:"))?;
    let mut scaled = channels.split('/').take(3).map(|channel| {
        let value = u32::from_str_radix(channel, 16).ok()?;
        let full = ((1u32 << (4 * channel.len().min(4))) - 1).max(1);
        // Rounded, not truncated: half of a 16-bit channel is 0x8000, and that is
        // 128 rather than 127.
        Some(((value * 255 + full / 2) / full) as u8)
    });
    Some(Rgb(scaled.next()??, scaled.next()??, scaled.next()??))
}

/// The largest character cell taken seriously, in pixels. Real ones are 6 to 30
/// across; the ceiling is what stops a terminal that reports nonsense — or a
/// multiplexer answering for one that does — from sizing an image by it. A
/// year at 64 pixels a cell is already 6,784 pixels wide.
pub const MAX_CELL: u16 = 64;

/// One character cell in pixels. `TIOCGWINSZ` is the cheapest and most widely
/// answered source; the `CSI` reports cover terminals that leave it zeroed.
///
/// Clamped to [`MAX_CELL`]: the answer is used to allocate an image.
pub fn cell_size(caps: &Caps) -> Option<(u16, u16)> {
    let sane = |(width, height): (u16, u16)| -> Option<(u16, u16)> {
        (width >= 2 && height >= 2 && width <= MAX_CELL && height <= MAX_CELL)
            .then_some((width, height))
    };
    if let Ok(size) = ratatui::crossterm::terminal::window_size() {
        if size.width > 0 && size.height > 0 && size.columns > 0 && size.rows > 0 {
            if let Some(cell) = sane((size.width / size.columns, size.height / size.rows)) {
                return Some(cell);
            }
        }
    }
    if let Some(cell) = caps.cell.and_then(sane) {
        return Some(cell);
    }
    let (width, height) = caps.window?;
    let (columns, rows) = ratatui::crossterm::terminal::size().ok()?;
    (columns > 0 && rows > 0)
        .then(|| (width / columns, height / rows))
        .and_then(sane)
}
