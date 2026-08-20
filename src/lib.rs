//! GitHub's contribution chart, in the terminal.
//!
//! `mossaic` draws the year the way github.com draws it: [Primer]'s own
//! colours, read out of the stylesheets GitHub serves; cells at its own geometry
//! — an 11px square on a 14px pitch, rounded by 2px; and, on a terminal that
//! draws pixels, actual anti-aliased rounded squares rather than characters
//! shaped like them.
//!
//! This is the library the three binaries share:
//!
//! | binary | what it is |
//! | --- | --- |
//! | `mossaic` | the chart |
//! | `mossaic-art` | writes text into a contribution graph by dating commits |
//! | `mossaic-glyphs` | shows the fallback cells, to check what a terminal renders |
//!
//! # The chart, as pixels
//!
//! [`graphics`] rasterises a year and hands it to whichever protocol the
//! terminal speaks — [`graphics::kitty`] for RGBA over the kitty graphics
//! protocol, [`graphics::sixel`] for a palette and six pixels to a byte. The
//! same image goes to [`png`] for a terminal that draws neither.
//!
//! ```
//! use mossaic::graphics;
//! use mossaic::primer::{Appearance, Palette, Season};
//!
//! // `levels[week][weekday]`: GitHub's shade, or None for a day it does not draw.
//! let mut levels = vec![[None; 7]; 53];
//! levels[20][3] = Some(4);
//!
//! let palette = Palette::new(Appearance::Dark, Season::Default, true);
//! assert_eq!(palette.levels[4], mossaic::primer::Rgb::hex(0x56d364));
//!
//! // One character cell is 10x20 pixels here; a day gets two columns and one row.
//! let image = graphics::grid(&levels, &palette, (10, 20));
//! assert_eq!((image.width, image.height), (53 * 2 * 10, 7 * 20));
//!
//! let escape = graphics::sixel(&image, palette.canvas);
//! assert!(escape.starts_with("\x1bP0;1;0q"));
//! ```
//!
//! # Writing text into a year
//!
//! [`art`] draws characters as pixels on the calendar and maps every lit one to
//! a date. Letters are five columns wide with one between, and five rows tall on
//! Mon–Fri, so **eight characters** is what a year holds: nine need all 53
//! columns, and the first and last are partial weeks.
//!
//! ```
//! use mossaic::art::{self, Grid, Ink, Shades};
//!
//! # fn main() -> Result<(), String> {
//! let grid = Grid::new(2027).unwrap();
//! let columns = art::bitmap("VYNCINT")?;
//! assert_eq!(columns.len(), 41); // 6N - 1
//!
//! // Letters against an empty graph: the classic look.
//! let placed = art::place(&columns, &grid, 1, None, Ink { lit: 4, field: 0 })?;
//! assert_eq!(placed.skipped, 0, "centred, so nothing falls outside the year");
//! assert_eq!(placed.lit.len(), 75);
//! assert!(placed.field.is_empty());
//! # Ok(())
//! # }
//! ```
//!
//! Leaving the background empty means *not contributing* on the other three
//! hundred days. [`art::Shades`] draws it as a colour instead, so the letters
//! are the difference between two greens and the year stays busy:
//!
//! ```
//! use mossaic::art::{self, Grid, Shades};
//! use mossaic::primer::Legibility;
//!
//! # fn main() -> Result<(), String> {
//! let grid = Grid::new(2027).unwrap();
//! let columns = art::bitmap("VYNCINT")?;
//!
//! let shades = Shades { ink: 4, field: 1 };
//! shades.check()?;
//! // Level 1 under level 4 is plain in every palette GitHub ships.
//! assert_eq!(shades.worst().0, Legibility::Clear);
//!
//! // Four commits on a letter day makes four the year'"'"'s peak; one commit is
//! // then level 1, and the background costs a commit a day.
//! let ink = shades.commits(4);
//! assert_eq!((ink.lit, ink.field), (4, 1));
//!
//! let placed = art::place(&columns, &grid, 1, None, ink)?;
//! assert_eq!(placed.lit.len(), 75);
//! assert_eq!(placed.field.len(), 365 - 75, "every other day of the year");
//! # Ok(())
//! # }
//! ```
//!
//! # Asking the terminal
//!
//! [`term::probe`] asks rather than guesses: one write, five questions, one round
//! trip, and a deadline for the terminals that stay silent. What comes back
//! decides the protocol, the image scale, and whether the palette is the light
//! one or the dark one.
//!
//! [Primer]: https://primer.style

/// When to colour output.
///
/// `Auto` is what everything defaults to: colour when stdout is a terminal and
/// [`NO_COLOR`](https://no-color.org) is unset. The other two are for pipes
/// that want it anyway and terminals that do not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Colour {
    /// Colour when stdout is a terminal and `NO_COLOR` is unset.
    #[default]
    Auto,
    /// Always, even into a pipe.
    Always,
    /// Never.
    Never,
}

impl Colour {
    /// From `--color`'s value, if it is one of the three.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "always" | "yes" | "force" => Some(Self::Always),
            "never" | "no" | "none" => Some(Self::Never),
            _ => None,
        }
    }

    /// Whether to actually emit colour, asked once per run.
    pub fn enabled(self) -> bool {
        use std::io::IsTerminal;
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => {
                std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
                    && std::io::stdout().is_terminal()
            }
        }
    }
}

/// Text with its control characters removed, for anything that reaches a
/// terminal.
///
/// A contribution calendar is data from elsewhere — the API, or a file someone
/// sent you — and a terminal executes what it is written. An `ESC` in a login
/// or an error message is a title change, a cursor-position report typed back
/// into the application, or an `OSC 52` clipboard write. The renderer drops
/// control characters when it fills a cell, but the paths that print straight
/// to stdout (`--png`, `--track`, error messages) do not, so untrusted text is
/// cleaned where it enters rather than at each of them.
pub fn printable(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

/// A count with thousands separators, the way github.com writes one.
///
/// Here rather than in three modules: the chart, the tracker and the reports all
/// print counts, and they should all print them the same way.
pub fn thousands(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

pub mod app;
pub mod art;
pub mod calendar;
pub mod cli;
pub mod github;
pub mod graphics;
pub mod plan;
pub mod png;
pub mod primer;
pub mod term;
pub mod ui;

#[cfg(test)]
mod render_tests;
