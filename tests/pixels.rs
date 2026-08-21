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
//! Since termlens 0.6 the payloads themselves are readable, and that moves the
//! assertions from "an image of about the right size went out" to "*this* image
//! went out": the cell extent it was pinned to, the character cell it was placed
//! on, and — decoded — the pixels. Which is how a chart drawn in pixels gets to
//! be checked against the chart described in text directly below it: the number
//! of days drawn in Primer's brightest green is the number the footer calls
//! active. Nothing on screen could say that, because none of it is on screen.
//!
//! What is still out of reach is the composite: termlens decodes an image but
//! never draws it, so how the picture sits over the text under it is not
//! assertable here. `--png` renders the same image to a file for that.
//!
//! ```sh
//! cargo test --test pixels
//! ```

use std::time::Duration;

use termlens::{Graphics, GraphicsPayload, GraphicsSeen, Key, Screen, Terminal};

/// A year of contribution art, every day elapsed and no network anywhere.
const PREVIEW: [&str; 2] = ["--file", "art/vyncint-2027.json"];
/// Wide enough for pixel cells: a day is two columns, so 53 weeks plus the
/// gutter plus the frame needs 112.
const SIZE: (u16, u16) = (176, 34);
/// A plausible character cell. Every geometry claim scales from it.
const CELL: (u16, u16) = (9, 19);
/// A 53-week year, two character columns to the day.
const GRID_CELLS: (u16, u16) = (53 * 2, 7);
/// Screen column of the first cell: past the frame and the four-column gutter.
const GRID_X: u16 = 1 + 4;

/// GitHub's dark theme, levels 0 and 4 — the two colours a year of art with
/// nothing behind it is made of. Read from `src/primer.rs`, which reads them
/// from the stylesheets github.com serves.
const CANVAS: [u8; 3] = [0x15, 0x1b, 0x23];
const BRIGHT: [u8; 3] = [0x56, 0xd3, 0x64];

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
        // `Auto` labels itself, so that landing back on it during the `d` cycle
        // does not look like a keypress that did nothing. The style it resolved
        // to is what most assertions are about.
        .trim_start_matches("auto: ")
        .to_string()
}

/// Whether the chart says it is choosing the style itself.
#[allow(dead_code)]
fn is_auto(screen: &Screen) -> bool {
    screen
        .text()
        .lines()
        .any(|line| line.contains("auto: ") && line.contains(" cells"))
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

/// The payload carrying the whole year: the widest one on the wire, which is
/// the only image 53 weeks across.
fn year(seen: &GraphicsSeen) -> &GraphicsPayload {
    seen.payloads()
        .iter()
        .max_by_key(|payload| payload.size().map_or(0, |(width, _)| width))
        .unwrap_or_else(|| panic!("no image at all went out: {seen:?}"))
}

/// Whether a decoded pixel is `token`, allowing for a sixel's quantisation.
///
/// A sixel colour register is a *percentage* of 255, so most 8-bit values do
/// not survive the round trip — `#56d364` comes back one off on two channels.
/// That is the palette degradation `docs/DESIGN.md` describes, not a wrong
/// colour, and one step of slack is the whole of it: two levels of the ramp
/// are never this close.
fn is(pixel: Option<[u8; 4]>, token: [u8; 3]) -> bool {
    pixel.is_some_and(|pixel| {
        pixel[3] == 0xff
            && (0..3)
                .all(|channel| i32::from(pixel[channel]).abs_diff(i32::from(token[channel])) <= 2)
    })
}

/// The number of days the footer calls active, read off the screen.
fn active_days(screen: &Screen) -> usize {
    screen
        .text()
        .lines()
        .find(|line| line.contains("active days"))
        .and_then(|line| {
            let digits: String = line
                .trim_start_matches('│')
                .trim()
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            digits.parse().ok()
        })
        .unwrap_or_else(|| panic!("no active-day count in:\n{screen}"))
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
fn the_year_is_pinned_to_the_cells_the_layout_reserved() -> termlens::Result<()> {
    // The cell contract of docs/DESIGN.md §3, asserted as the numbers that go
    // out rather than as the absence of text over them. A day is two columns
    // and one row, so a 53-week year is 106x7 character cells and — at this
    // cell size — 954x133 pixels. An image that disagrees with the layout it
    // was drawn for slides out from under the month labels, and every cell on
    // the screen stays exactly as it was while it does.
    for (label, graphics) in [("kitty", Graphics::Kitty), ("sixel", Graphics::Sixel)] {
        let mut terminal = chart(graphics, Some(CELL), SIZE, &PREVIEW)?;
        let screen = terminal.wait_frame(loaded)?;
        let seen = screen.graphics();
        let year = year(&seen);

        assert_eq!(
            year.size(),
            Some((
                u32::from(GRID_CELLS.0) * u32::from(CELL.0),
                u32::from(GRID_CELLS.1) * u32::from(CELL.1),
            )),
            "{label}: {year:?}"
        );
        // The top-left corner is the grid's own origin: past the frame and the
        // weekday gutter, one row above the Monday label.
        assert_eq!(
            year.at(),
            (monday(&screen) - 1, GRID_X),
            "{label}: {year:?}"
        );
        // kitty is told the extent in cells, and then cannot disagree with it.
        if graphics == Graphics::Kitty {
            assert_eq!(year.cells(), Some(GRID_CELLS), "{label}: {year:?}");
        }
    }
    Ok(())
}

#[test]
fn the_year_is_one_image_however_many_escapes_it_takes() -> termlens::Result<()> {
    // kitty caps a payload at 4096 bytes and continues with `m=1`, so the year
    // goes out in two escapes. It is still one picture, and the count has to
    // say so — "how many images did this frame send?" is the question every
    // assertion about the diffing painter below rests on.
    let mut terminal = chart(Graphics::Kitty, Some(CELL), SIZE, &PREVIEW)?;
    let screen = terminal.wait_frame(loaded)?;
    let seen = screen.graphics();
    let year = year(&seen);
    assert!(year.chunks() > 1, "the year needs chunking: {year:?}");
    // The year, the legend and the ring on the opening day: three images, not
    // three plus however many chunks the biggest one took.
    assert_eq!(seen.kitty(), seen.payloads().len() as u32, "{seen:?}");
    Ok(())
}

#[test]
fn the_year_drawn_is_the_year_described() -> termlens::Result<()> {
    // The two halves of the chart, checked against each other for the first
    // time. The footer counts active days in text; the image draws them in
    // Primer's brightest green. Nothing on screen can compare them, because
    // the image is not on screen — it is an escape sequence that leaves every
    // cell it covers exactly as it found it.
    for (label, graphics) in [("kitty", Graphics::Kitty), ("sixel", Graphics::Sixel)] {
        let mut terminal = chart(graphics, Some(CELL), SIZE, &PREVIEW)?;
        let screen = terminal.wait_frame(loaded)?;
        let seen = screen.graphics();
        let image = year(&seen)
            .decode()
            .unwrap_or_else(|error| panic!("{label}: {error}"));

        // Every day's centre pixel, week by week: the square is centred in its
        // two columns, so half a cell in from the day's own corner lands
        // inside the rounded rect rather than on its anti-aliased edge.
        let (mut lit, mut empty, mut outside) = (0usize, 0usize, 0usize);
        for week in 0..u32::from(GRID_CELLS.0) / 2 {
            for weekday in 0..u32::from(GRID_CELLS.1) {
                let pixel = image.pixel(
                    week * 2 * u32::from(CELL.0) + u32::from(CELL.0),
                    weekday * u32::from(CELL.1) + u32::from(CELL.1) / 2,
                );
                if is(pixel, BRIGHT) {
                    lit += 1;
                } else if is(pixel, CANVAS) {
                    empty += 1;
                } else {
                    outside += 1;
                }
            }
        }

        assert_eq!(
            lit,
            active_days(&screen),
            "{label}: the image draws {lit} bright days where the footer counts \
             {} active:\n{screen}",
            active_days(&screen)
        );
        // 2027 opens on a Friday and ends on a Friday, so the six cells before
        // January and after December belong to no day and are drawn as nothing
        // at all — not as an empty day, which is a colour.
        assert_eq!(outside, 6, "{label}: {outside} cells outside the year");
        assert_eq!(
            lit + empty + outside,
            usize::from(GRID_CELLS.0 / 2 * GRID_CELLS.1),
            "{label}: every cell of the grid is accounted for"
        );
    }
    Ok(())
}

#[test]
fn a_year_costs_what_the_design_notes_say() -> termlens::Result<()> {
    // docs/DESIGN.md §4 quotes a year as 4.9 KB over kitty (zlib'd RGBA, base64'd)
    // against 38 KB of run-length-encoded sixel. Those figures were unverifiable
    // from a test until the harness could see the payloads; now the table is a
    // claim with a check behind it, and the print below is where the numbers in it
    // came from. Bounds rather than equalities, because the exact byte count moves
    // with the cell size and the year's contents — but an order of magnitude is
    // exactly what the design decision rests on.
    let mut kitty = chart(Graphics::Kitty, Some(CELL), SIZE, &PREVIEW)?;
    let kitty_bytes = kitty.wait_frame(loaded)?.graphics().bytes();
    let mut sixel = chart(Graphics::Sixel, Some(CELL), SIZE, &PREVIEW)?;
    let sixel_bytes = sixel.wait_frame(loaded)?.graphics().bytes();

    println!("MEASURED kitty={kitty_bytes} sixel={sixel_bytes}");
    assert!(
        (1_000..=12_000).contains(&kitty_bytes),
        "a year over kitty is quoted at 4.9 KB, got {kitty_bytes} bytes"
    );
    assert!(
        (20_000..=80_000).contains(&sixel_bytes),
        "a year over sixel is quoted at 38 KB, got {sixel_bytes} bytes"
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
    // moved cursor would be 38 KB of sixel at 12 frames a second, so `Painter`
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
        println!("MEASURED {label}: year={base} one-move={cost}");
        assert!(cost > 0, "{label}: the ring has to be drawn somehow");
        assert!(
            cost * 10 < base,
            "{label}: one cursor move cost {cost} bytes against a {base}-byte year — \
             that is the whole grid being re-sent"
        );

        // And the payload says what it is, which is stronger than saying it is
        // small: a day, at the day's own cell. A painter that re-sent the year
        // compressed unusually well would pass the byte test and fail this one.
        let after = terminal.screen();
        let seen = after.graphics();
        let last = seen.last().expect("a payload");
        assert_eq!(
            last.size(),
            Some((2 * u32::from(CELL.0), u32::from(CELL.1))),
            "{label}: one moved cursor sent {last:?}"
        );
        // The cursor is on Fri Dec 24, and Friday is four rows below Monday.
        assert_eq!(
            last.at().0,
            monday(&after) + 4,
            "{label}: the ring is drawn on the day's own row: {last:?}"
        );
        assert_eq!(
            (last.at().1 - GRID_X) % 2,
            0,
            "{label}: and on a day boundary, two columns to the day: {last:?}"
        );
    }
    Ok(())
}

#[test]
fn the_chart_takes_its_images_down_before_it_leaves() -> termlens::Result<()> {
    // kitty images outlive the program that drew them: the terminal holds them
    // until told otherwise, so a chart that exits without deleting leaves its
    // year floating over the next thing in the window. That teardown writes no
    // text and changes no cell — the only evidence it happened is the delete
    // itself, which used to be counted as one more image transmitted.
    let mut terminal = chart(Graphics::Kitty, Some(CELL), SIZE, &PREVIEW)?;
    let drawn = terminal.wait_frame(loaded)?.graphics();
    assert_eq!(drawn.deletes(), 0, "nothing to take down yet: {drawn:?}");

    terminal.send(Key::Char('q'))?;
    assert!(terminal.wait_exit()?.success());

    let seen = terminal.screen().graphics();
    assert!(
        seen.deletes() > 0,
        "the chart left its images behind: {seen:?}"
    );
    // The teardown is not a picture, so it must not read as one.
    assert_eq!(seen.kitty(), drawn.kitty(), "{seen:?}");
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
        "crossing {CROSSED} cells produced {payloads} images — one per event \
         would be {CROSSED}"
    );
    // Images, not escapes: each of these is one small ring well under kitty's
    // 4096-byte chunk, so a count inflated by chunking would be invisible here
    // and would not be in the year above.
    assert!(
        after
            .graphics()
            .payloads()
            .iter()
            .rev()
            .take(payloads as usize)
            .all(|payload| payload.chunks() == 1),
        "a ring should not need chunking: {:?}",
        after.graphics().payloads()
    );
    assert!(
        bytes * 4 < base.bytes(),
        "crossing {CROSSED} cells cost {bytes} bytes against a {}-byte year",
        base.bytes()
    );
    Ok(())
}

#[test]
fn pixel_cells_are_refused_rather_than_written_past_the_edge() -> termlens::Result<()> {
    // Sixel clears the character cells it is about to cover by writing spaces, and
    // it did that with no idea how wide the terminal was: a 53-week grid wrote 106
    // of them from column 6, so on an 80-column terminal seven rows wrapped over
    // the weekday gutter, the right border and the rows below — and stayed that
    // way, because ratatui believed it had written those cells.
    //
    // `d` pins pixel cells even where they clip, which is the documented promise
    // ("d forces them, clipped"), so this is the path that reached it.
    let mut terminal = chart(Graphics::Sixel, Some(CELL), (176, 34), &PREVIEW)?;
    terminal.wait_frame(|screen| style(screen) == "pixel cells (sixel)")?;
    terminal.resize(80, 24)?;

    // Whatever it settles on, the frame has to still be a frame: the gutter, the
    // border and the footer all intact.
    let narrow = terminal.wait_frame(|screen| loaded(screen) && screen.cols() == 80)?;
    assert!(
        narrow.contains("Mon") && narrow.contains("Wed"),
        "the weekday gutter was overwritten:\n{narrow}"
    );
    assert!(
        narrow.contains("q quit"),
        "the footer was overwritten:\n{narrow}"
    );
    // Every row of the frame is closed on both sides.
    for row in 0..narrow.rows() {
        let line = narrow.row_text(row);
        if !line.trim().is_empty() && line.starts_with('│') {
            assert!(
                line.trim_end().ends_with('│'),
                "row {row} lost its right border:\n{narrow}"
            );
        }
    }
    Ok(())
}

#[test]
fn a_year_that_has_not_happened_can_be_asked_what_it_will_look_like() -> termlens::Result<()> {
    // `--today` on the chart, which the tracker has had since 0.3.0. A saved
    // calendar has no notion of now — that is what makes it useful for previewing
    // a year that has not happened — so this is the only way to see the
    // still-to-come half of the rendering at all, and the only way to test it.
    let mut plain = chart(Graphics::None, None, SIZE, &PREVIEW)?;
    let all_elapsed = plain.wait_frame(loaded)?;
    assert!(
        !all_elapsed.contains("still to come"),
        "with no date, every day of a saved year is drawn:\n{all_elapsed}"
    );

    let mut dated = chart(
        Graphics::None,
        None,
        SIZE,
        &["--file", "art/vyncint-2027.json", "--today", "2027-06-30"],
    )?;
    let half = dated.wait_frame(loaded)?;
    assert!(
        half.contains("blank = still to come"),
        "half the year is ahead now, and the legend says so:\n{half}"
    );
    // And the cursor can be walked into it, which is where it used to vanish.
    dated.send(Key::End)?;
    let ahead = dated.wait_frame(|screen| screen.contains("still to come"))?;
    assert!(
        ahead.contains("Fri, Dec 31 2027"),
        "the detail line names the day:\n{ahead}"
    );
    Ok(())
}
