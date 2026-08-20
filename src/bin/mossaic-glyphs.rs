//! Show the glyphs mossaic falls back to, to check what a terminal renders.
//!
//! ```sh
//! cargo run --bin mossaic-glyphs
//! ```
//!
//! The `rounded` style needs block sextants (the U+1FB00 block, Unicode 13).
//! Terminals that draw box characters themselves — VTE 0.66+, kitty, foot,
//! WezTerm — render them whatever the font holds. If the rounded row below shows
//! boxes, question marks or blanks instead of small squares, press `d` in
//! mossaic to fall back to `squares`, which needs nothing beyond U+2580.
//!
//! None of this applies when the terminal draws pixels: `mossaic
//! --capabilities` says whether yours does.

use mossaic::cli::Args;
use mossaic::primer::{Appearance, Palette, Rgb, Season};
use mossaic::Colour;

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

    let colourful = colour.enabled();
    let palette = Palette::new(Appearance::Dark, Season::Default, true);
    let paint = |glyph: &str, colour: Rgb| {
        if !colourful {
            return glyph.to_string();
        }
        format!(
            "\x1b[38;2;{};{};{}m{glyph}\x1b[0m",
            colour.0, colour.1, colour.2
        )
    };

    println!("Show the glyphs mossaic falls back to, to check what this terminal renders.\n");
    for (name, glyph, note) in STYLES {
        let legend: Vec<String> = palette
            .levels
            .iter()
            .map(|colour| paint(glyph, *colour))
            .collect();
        // A short stretch of cells, the way the chart draws them.
        let run: Vec<String> = (0..12)
            .map(|index| paint(glyph, palette.levels[(index * 7) % 5]))
            .collect();
        println!("  {name}  {}   {}", legend.join(" "), run.join(" "));
        println!("           {note}");
    }
    println!();
    println!("  Each row should read as separated small squares, dark to bright green.");
    println!("  The rounded row should have visibly softened corners.");
    println!();
    println!("  If rounded looks wrong, everything else still works:");
    println!("    mossaic            then press d to cycle styles");
}
