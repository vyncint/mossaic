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

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("-h" | "--help") => {
            println!(
                "mossaic-glyphs — show the cells mossaic falls back to\n\n\
                 usage:\n  mossaic-glyphs [--color WHEN]\n\n\
                 Terminals that draw pixels never use these. `mossaic \
                 --capabilities`\nsays whether yours does.\n\n  \
                 --color WHEN   auto (default), always or never\n  \
                 -V, --version  print the version\n  \
                 -h, --help     show this help"
            );
            return;
        }
        Some("-V" | "--version") => {
            println!("mossaic-glyphs {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        _ => {}
    }
    // `--color never` for a pipe, and NO_COLOR because that is the convention.
    let choice = match std::env::args().nth(1).as_deref() {
        Some("--color" | "--colour") => std::env::args()
            .nth(2)
            .and_then(|value| Colour::parse(&value))
            .unwrap_or_default(),
        Some("--no-colour" | "--no-color") => Colour::Never,
        _ => Colour::default(),
    };
    let colourful = choice.enabled();
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
