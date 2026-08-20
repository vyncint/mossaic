//! The pixel path, end to end: what mossaic actually puts on the wire.
//!
//! Until termlens 0.5 this half of the program was not merely unasserted, it was
//! **unreachable**. mossaic decides whether to draw pixels by *asking* the
//! terminal — one write, five questions, one round trip — and a harness that
//! answered no to all of them could only be driven down the text path. The old
//! suite worked around that with `--graphics sixel --cell 10x20`, which forces
//! the protocol and hands over the cell size, so the probe, the fallbacks and
//! the auto choice were all skipped.
//!
//! [`TerminalBuilder::graphics`] and [`TerminalBuilder::cell_size`] state which
//! terminal is being simulated, the way `background_rgb` states a background, so
//! the real decision runs. [`Screen::graphics`] then reports the payloads that
//! went out, by protocol and in bytes — which is what lets the byte budgets in
//! `docs/DESIGN.md` §4 and the diffing in §5 be checked against the wire rather
//! than against the rasteriser.
//!
//! What is still out of reach is what the images *look like*: termlens consumes
//! APC and DCS strings without rendering them. The encoders are checked against
//! the formats in `src/render_tests.rs`, and `--png` renders the same image to a
//! file. This file checks that the right images, of about the right size, go out
//! at the right moments.
//!
//! ```sh
//! cargo test --test pixels
//! ```

use std::time::Duration;

use termlens::{Graphics, Key, Screen, Terminal};

/// A year of contribution art, every day elapsed and no network anywhere.
const PREVIEW: [&str; 2] = ["--file", "art/vyncint-2027.json"];
/// Wide enough for pixel cells: a day is two columns, so 53 weeks plus the
/// gutter plus the frame needs 112.
const SIZE: (u16, u16) = (176, 34);
/// A plausible character cell. Every geometry claim scales from it.
const CELL: (u16, u16) = (9, 19);

/// A terminal that answers the probe the way the named terminal would.
fn chart(
    graphics: Graphics,
    cell: Option<(u16, u16)>,
    size: (u16, u16),
    args: &[&str],
) -> termlens::Result<Terminal> {
    let mut builder = Terminal::builder()
        .size(size.0, size.1)
        .env_clear()
        .env("COLORTERM", "truecolor")
        .env("TERM", "xterm-256color")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .timeout(Duration::from_secs(20))
        .graphics(graphics)
        .args(args);
    if let Some((width, height)) = cell {
        builder = builder.cell_size(width, height);
    }
    builder.spawn(env!("CARGO_BIN_EXE_mossaic"))
}

fn loaded(screen: &Screen) -> bool {
    screen.contains("q quit") && screen.contains("contributions in")
}

/// The line naming the active cell style, e.g. "… · pixel cells (kitty)".
fn style(screen: &Screen) -> String {
    screen
        .text()
        .lines()
        .find(|line| line.contains(" cells"))
        .unwrap_or_default()
        .rsplit('·')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches('│')
        .trim()
        .to_string()
}

/// Row of the Monday label, which is grid row 1.
fn monday(screen: &Screen) -> u16 {
    (0..screen.rows())
        .find(|row| {
            screen
                .row_text(*row)
                .trim_start_matches('│')
                .starts_with("Mon")
        })
        .expect("a weekday gutter")
}

#[test]
fn the_probe_finds_pixels_without_being_told() -> termlens::Result<()> {
    // No flags: what the terminal answers is the whole input. This is the
    // decision `--graphics`/`--cell` used to skip.
    for (label, graphics, cell, wanted, images) in [
        (
            "kitty",
            Graphics::Kitty,
            Some(CELL),
            "pixel cells (kitty)",
            true,
        ),
        (
            "sixel",
            Graphics::Sixel,
            Some(CELL),
            "pixel cells (sixel)",
            true,
        ),
        // Neither protocol: text, and nothing goes out as an image.
        (
            "neither",
            Graphics::None,
            Some(CELL),
            "rounded cells",
            false,
        ),
        // A protocol but no cell size. An image that cannot be lined up with the
        // labels around it is worse than no image, so this is text on purpose.
        (
            "kitty, no cell",
            Graphics::Kitty,
            None,
            "rounded cells",
            false,
        ),
    ] {
        let mut terminal = chart(graphics, cell, SIZE, &PREVIEW)?;
        let screen = terminal.wait_frame(loaded)?;
        assert_eq!(style(&screen), wanted, "{label}:\n{screen}");
        assert_eq!(
            !screen.graphics().is_empty(),
            images,
            "{label}: transmitted {:?}",
            screen.graphics()
        );
    }
    Ok(())
}

#[test]
fn the_grid_rows_belong_to_the_image_and_the_labels_stay_text() -> termlens::Result<()> {
    // The cell contract from docs/DESIGN.md §3, asserted through a terminal for
    // the first time: a day is two columns and one row, so the image lands on
    // exact character boundaries and everything around it stays text. The text
    // layer must leave those seven rows alone — anything it wrote there would be
    // written over the image, and on sixel that cannot be taken back.
    let mut terminal = chart(Graphics::Kitty, Some(CELL), SIZE, &PREVIEW)?;
    let screen = terminal.wait_frame(loaded)?;
    assert_eq!(style(&screen), "pixel cells (kitty)");
    assert!(
        !screen.graphics().is_empty(),
        "the year went out as an image"
    );

    let top = monday(&screen) - 1;
    for row in top..top + 7 {
        let cells: String = screen
            .row_text(row)
            .chars()
            .skip(5) // past the frame and the four-column weekday gutter
            .take_while(|character| *character != '│')
            .collect();
        assert!(
            cells.trim().is_empty(),
            "row {row} belongs to the painter, but the text layer wrote {cells:?}"
        );
    }

    // The month labels are the point of keeping the grid on character
    // boundaries, so they had better still be there.
    let months = screen.row_text(top - 1);
    for month in ["Jan", "Apr", "Aug", "Dec"] {
        assert!(months.contains(month), "months row reads {months:?}");
    }
    Ok(())
}

#[test]
fn a_year_costs_what_the_design_notes_say() -> termlens::Result<()> {
    // docs/DESIGN.md §4 quotes a year as ~8 KB over kitty (zlib'd RGBA) against
    // ~45 KB of run-length-encoded sixel. Those figures were unverifiable from a
    // test until the harness could see the payloads; now the table is a claim
    // with a check behind it. Bounds rather than equalities, because the exact
    // byte count moves with the cell size and the year's contents — but an order
    // of magnitude is exactly what the design decision rests on.
    let mut kitty = chart(Graphics::Kitty, Some(CELL), SIZE, &PREVIEW)?;
    let kitty_bytes = kitty.wait_frame(loaded)?.graphics().bytes();
    let mut sixel = chart(Graphics::Sixel, Some(CELL), SIZE, &PREVIEW)?;
    let sixel_bytes = sixel.wait_frame(loaded)?.graphics().bytes();

    assert!(
        (1_000..=12_000).contains(&kitty_bytes),
        "a year over kitty is quoted at ~8 KB, got {kitty_bytes} bytes"
    );
    assert!(
        (20_000..=80_000).contains(&sixel_bytes),
        "a year over sixel is quoted at ~45 KB, got {sixel_bytes} bytes"
    );
    // The compression is the reason kitty is preferred where both are offered.
    assert!(
        sixel_bytes > kitty_bytes * 4,
        "sixel {sixel_bytes} should dwarf kitty {kitty_bytes}"
    );
    Ok(())
}

#[test]
fn text_mode_transmits_no_image() -> termlens::Result<()> {
    // The negative assertion: a terminal that *can* draw pixels, told not to.
    // Worth its own test because "it rendered as text" and "it rendered as text
    // and also quietly sent an image nobody asked for" look identical on screen.
    let mut terminal = chart(
        Graphics::Kitty,
        Some(CELL),
        SIZE,
        &["--file", "art/vyncint-2027.json", "--graphics", "text"],
    )?;
    let screen = terminal.wait_frame(loaded)?;
    assert!(
        screen.graphics().is_empty(),
        "--graphics text sent {:?}",
        screen.graphics()
    );
    assert_eq!(style(&screen), "rounded cells", "{screen}");
    // And it is a whole chart, not a degraded one.
    assert!(screen.contains("300 contributions in 2027"), "{screen}");
    Ok(())
}

#[test]
fn moving_the_cursor_sends_a_cell_not_a_year() -> termlens::Result<()> {
    // docs/DESIGN.md §5: "the painter is a diff". Redrawing the year for every
    // moved cursor would be 45 KB of sixel at 12 frames a second, so `Painter`
    // holds what is on screen and writes only what changed. That was asserted
    // in process; this asserts it on the wire, for both protocols.
    for (label, graphics) in [("kitty", Graphics::Kitty), ("sixel", Graphics::Sixel)] {
        let mut terminal = chart(graphics, Some(CELL), SIZE, &PREVIEW)?;
        let base = terminal.wait_frame(loaded)?.graphics().bytes();

        // Land on a known day first, so the move below is one cell either way.
        terminal.send(Key::End)?;
        let settled = terminal
            .wait_frame(|screen| screen.contains("Fri, Dec 31 2027"))?
            .graphics()
            .bytes();
        terminal.send(Key::Left)?;
        let moved = terminal
            .wait_frame(|screen| screen.contains("Fri, Dec 24 2027"))?
            .graphics()
            .bytes();

        let cost = moved - settled;
        assert!(cost > 0, "{label}: the ring has to be drawn somehow");
        assert!(
            cost * 10 < base,
            "{label}: one cursor move cost {cost} bytes against a {base}-byte year — \
             that is the whole grid being re-sent"
        );
    }
    Ok(())
}

#[test]
fn auto_never_asks_for_pixels_it_cannot_fit() -> termlens::Result<()> {
    // Pixel cells need two columns a week plus the gutter plus the frame: 112 for
    // a 53-week year. Below that `Auto` has to choose something narrower, and
    // must not place an image it has no room for — an image is written at the
    // cursor and drawn outwards from there, so one that does not fit lands on
    // whatever is next to it.
    for (width, pixels) in [(176u16, true), (120, true), (111, false), (80, false)] {
        let mut terminal = chart(Graphics::Kitty, Some(CELL), (width, 34), &PREVIEW)?;
        let screen = terminal.wait_frame(loaded)?;
        assert_eq!(
            style(&screen).starts_with("pixel"),
            pixels,
            "at {width} columns Auto chose {:?}",
            style(&screen)
        );
        assert_eq!(
            !screen.graphics().is_empty(),
            pixels,
            "at {width} columns it transmitted {:?}",
            screen.graphics()
        );
    }
    Ok(())
}

#[test]
fn a_resize_puts_the_year_back() -> termlens::Result<()> {
    // A resize wipes the screen under the images, so the painter is invalidated
    // and has to send them again. If it did not, the chart would keep its labels
    // and lose its cells.
    let mut terminal = chart(Graphics::Kitty, Some(CELL), SIZE, &PREVIEW)?;
    let before = terminal.wait_frame(loaded)?.graphics();
    assert!(!before.is_empty());

    terminal.resize(140, 30)?;
    let after = terminal.wait_frame(|screen| loaded(screen) && screen.cols() == 140)?;
    assert!(
        after.graphics().total() > before.total(),
        "the year was not re-sent: {:?} then {:?}",
        before,
        after.graphics()
    );
    assert_eq!(style(&after), "pixel cells (kitty)", "{after}");
    assert!(after.contains("300 contributions in 2027"), "{after}");
    Ok(())
}

#[test]
fn the_capability_report_matches_what_the_chart_draws() -> termlens::Result<()> {
    // `--capabilities` exists because the whole pixel path turns on replies that
    // are easy to get wrong and impossible to see. It had no test at all, because
    // no harness could answer the questions it asks.
    for (graphics, cell, protocol, other) in [
        (Graphics::Kitty, (9u16, 19u16), "kitty", "sixel"),
        (Graphics::Sixel, (10, 20), "sixel", "kitty"),
    ] {
        let mut terminal = chart(graphics, Some(cell), (100, 24), &["--capabilities"])?;
        let status = terminal.wait_exit()?;
        assert!(status.success(), "{status:?}");
        let report = terminal.screen().text();

        // Read each answer off its own line rather than matching the column
        // alignment, which is presentation and not the point.
        let answer = |label: &str| -> String {
            report
                .lines()
                .find(|line| line.trim_start().starts_with(label))
                .unwrap_or_default()
                .split_whitespace()
                .nth(1)
                .unwrap_or_default()
                .to_string()
        };
        assert_eq!(answer(protocol), "yes", "{protocol}:\n{report}");
        assert_eq!(answer(other), "no", "{other}:\n{report}");
        assert_eq!(
            answer("cell"),
            format!("{}x{}", cell.0, cell.1),
            "the cell it measured:\n{report}"
        );
        // The last line is the decision the chart would make, and it has to
        // agree with the three above it.
        assert_eq!(answer("cells"), protocol, "the decision:\n{report}");
    }
    Ok(())
}

#[test]
fn a_flood_of_motion_is_drained_not_replayed() -> termlens::Result<()> {
    // Two design decisions meet here, and neither was measurable before.
    //
    // §8: events are drained to the end of the queue every frame rather than one
    // per frame, "because motion reports arrive in floods, and answering them one
    // at a time leaves the tooltip trailing several cells behind the pointer".
    // §5: the painter writes only what changed.
    //
    // A drag reports one motion per cell it crosses, so crossing forty weeks is
    // forty events. If each were answered with a repaint the cost would be forty
    // rings; if the queue is drained per frame it is a handful. The number of
    // *payloads* is what tells the two apart, and no content assertion can:
    // every intermediate frame shows a perfectly correct tooltip.
    let mut terminal = chart(Graphics::Kitty, Some(CELL), SIZE, &PREVIEW)?;
    let ready = terminal
        .wait_frame(|screen| loaded(screen) && screen.mouse_mode() != termlens::MouseMode::None)?;
    let base = ready.graphics();
    assert!(!base.is_empty(), "the year should be on screen first");

    // A day is two columns, so this crosses forty of them on one weekday row.
    let row = monday(&ready) + 2;
    const CROSSED: u32 = 40;
    let from = 5 + 5 * 2;
    let to = from + CROSSED as u16 * 2;
    terminal.drag(termlens::MouseButton::Left, (from, row), (to, row))?;
    let after = terminal.wait_frame(|screen| screen.text().contains(" on "))?;

    let payloads = after.graphics().total() - base.total();
    let bytes = after.graphics().bytes() - base.bytes();
    assert!(
        payloads > 0 && payloads <= 12,
        "crossing {CROSSED} cells produced {payloads} image payloads — one per \
         event would be about {}",
        CROSSED * 2
    );
    assert!(
        bytes * 4 < base.bytes(),
        "crossing {CROSSED} cells cost {bytes} bytes against a {}-byte year",
        base.bytes()
    );
    Ok(())
}
