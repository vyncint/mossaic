//! Show the glyphs mossaic falls back to, to check what a terminal renders.
//!
//! ```sh
//! cargo run --bin mossaic-glyphs
//! ```
//!
//! The `rounded` style needs block sextants (the U+1FB00 block, Unicode 13).
//! Terminals that draw box characters themselves — VTE 0.66+, kitty, foot,
//! WezTerm — render them whatever the font holds. If the rounded row below shows
//! boxes, question marks or blanks instead of small squares, start mossaic with
//! `--cells squares`, which needs nothing beyond U+2580.
//!
//! None of this applies when the terminal draws pixels: `mossaic
//! --capabilities` says whether yours does.

use std::io::IsTerminal;

use mossaic::cli::Args;
use mossaic::primer::{Appearance, Palette, Rgb, Season};
use mossaic::Colour;
use ratatui::style::Color;

const STYLES: [(&str, &str, &str); 3] = [
    (
        "rounded",
        "\u{1FB2B}\u{1FB1B}",
        "U+1FB2B U+1FB1B  block sextants",
    ),
    ("squares", "▀", "U+2580           upper half block"),
    ("blocks ", "██", "U+2588           full block"),
];

const HELP: &str = "\
mossaic-glyphs — show the cells mossaic falls back to

usage:
  mossaic-glyphs [options]

  --color WHEN   (--colour) auto (default), always or never
  --no-color     (--no-colour) the same as --color never
  -V, --version  print the version
  -h, --help     show this help

Terminals that draw pixels never use these. `mossaic --capabilities` says
whether yours does.";

fn main() {
    // Before anything prints: a reader that closes early is not a crash.
    mossaic::quiet_broken_pipe();
    // The shared parser, like the other two binaries. Parsing by hand made this
    // one disagree with them about everything a user notices: `--color=never` was
    // silently ignored, an unknown option exited 0, a stray argument was dropped,
    // and a missing value defaulted instead of saying so.
    let mut args = Args::from_env("mossaic-glyphs");
    let mut colour = Colour::default();
    while let Some(arg) = args.next_arg() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{HELP}");
                return;
            }
            "-V" | "--version" => {
                println!("mossaic-glyphs {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--color" | "--colour" => {
                let raw = args.value("--color");
                colour = Colour::parse(&raw).unwrap_or_else(|| {
                    args.fail(&format!("--color wants auto, always or never, not {raw:?}"))
                });
            }
            "--no-color" | "--no-colour" => colour = Colour::Never,
            other => args.fail(&format!("unknown option {other:?} — try --help")),
        }
    }

    // The colours the chart would use on this terminal, since this is the check
    // for the terminals that differ: truecolour only where `COLORTERM` says so,
    // the 256-colour ramp otherwise, and the theme of the background the
    // terminal reports. It was always the dark theme in truecolour, so on
    // Terminal.app it showed colours the chart never draws there (#143).
    let colourful = colour.enabled();
    let background = colourful.then(background).flatten();
    let palette = Palette::new(
        background.map_or(Appearance::Dark, Appearance::from_background),
        Season::Default,
        Palette::truecolor_env(),
    );
    let paint = |glyph: &str, level: u8| {
        if !colourful {
            return glyph.to_string();
        }
        format!("\x1b[{}m{glyph}\x1b[0m", foreground(palette.level(level)))
    };

    println!("Show the glyphs mossaic falls back to, to check what this terminal renders.\n");
    for (name, glyph, note) in STYLES {
        let legend: Vec<String> = (0..5).map(|level| paint(glyph, level)).collect();
        // A short stretch of cells, the way the chart draws them.
        let run: Vec<String> = (0..12u8)
            .map(|index| paint(glyph, (index * 7) % 5))
            .collect();
        println!("  {name}  {}   {}", legend.join(" "), run.join(" "));
        println!("           {note}");
    }
    println!();
    println!("  Each row should read as separated small squares, dark to bright green.");
    println!("  The rounded row should have visibly softened corners.");
    println!();
    // Named, rather than "press d": the chart picks rounded again on every
    // launch, so a font without sextants meant pressing it every time (#142).
    println!("  If rounded looks wrong, everything else still works:");
    println!("    mossaic --cells squares    start in squares, or any row above");
    println!("    mossaic                    then press d to cycle styles");
}

/// The background the terminal reports, when there is a terminal on both ends
/// to ask — never into a pipe, where the question would be written into
/// somebody's file and nothing would answer it — and on unix, the only place
/// the probe asks at all.
fn background() -> Option<Rgb> {
    use ratatui::crossterm::terminal;
    if !cfg!(unix) || !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) {
        return None;
    }
    // Raw mode, or the replies come back echoed and line-buffered.
    terminal::enable_raw_mode().ok()?;
    let caps = mossaic::term::probe(mossaic::term::TIMEOUT);
    let _ = terminal::disable_raw_mode();
    caps.background
}

/// The SGR parameters for a foreground colour, as [`Palette::level`] gives one.
fn foreground(colour: Color) -> String {
    match colour {
        Color::Rgb(r, g, b) => format!("38;2;{r};{g};{b}"),
        Color::Indexed(index) => format!("38;5;{index}"),
        _ => "39".to_string(),
    }
}
