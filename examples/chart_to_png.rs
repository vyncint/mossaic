//! Render a saved calendar to a PNG, without a terminal in sight.
//!
//! The chart's own `--png` flag does this for a fetched or saved year; this
//! shows the library underneath it, which is three calls: a calendar, a
//! palette, an image.
//!
//! ```sh
//! cargo run --example chart_to_png -- art/vyncint-2027.json /tmp/chart.png
//! ```

use std::path::Path;

use mossaic::primer::{Appearance, Palette, Season};
use mossaic::{github, graphics, png};

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(calendar), Some(out)) = (args.next(), args.next()) else {
        eprintln!("usage: chart_to_png <calendar.json> <out.png>");
        std::process::exit(2);
    };

    // A saved `gh api graphql` response, which is what `mossaic-art --snapshot` writes.
    let calendar = github::from_file(&calendar, None).unwrap_or_else(|error| {
        eprintln!("chart_to_png: {error}");
        std::process::exit(1);
    });

    // `levels[week][weekday]`, with None where there is no day to draw: outside
    // the year, or not yet happened.
    let levels: Vec<[Option<u8>; 7]> = calendar
        .weeks
        .iter()
        .map(|week| {
            let mut column = [None; 7];
            for (weekday, day) in week.days.iter().enumerate() {
                column[weekday] = day.filter(|day| !day.future).map(|day| day.level);
            }
            column
        })
        .collect();

    let palette = Palette::new(Appearance::Dark, Season::Default, true);
    // One character cell in pixels. A day gets two columns and one row, so this
    // is what sets the scale.
    let image = graphics::grid(&levels, &palette, (10, 20));

    png::write(Path::new(&out), &image, palette.canvas).unwrap_or_else(|error| {
        eprintln!("chart_to_png: could not write {out}: {error}");
        std::process::exit(1);
    });
    println!(
        "wrote {out} — {}x{} px, {} {}",
        image.width, image.height, calendar.login, calendar.year
    );
}
