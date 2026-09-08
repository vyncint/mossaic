//! What the emulator can and cannot see of mossaic — the assertion the rest
//! of the suite rests on.
//!
//! Every other test here reads a grid that a VT emulator produced from
//! mossaic's bytes. If mossaic emits a sequence the emulator does not
//! implement, that grid is quietly wrong and *every* screen assertion in this
//! repository is being made against a plausible-looking fiction. termlens
//! 0.10 made that checkable: `Screen::unsupported` lists what was dropped.
//!
//! These are deliberately whole-suite invariants rather than feature tests.
//! They are cheap, and when one breaks the right response is to distrust the
//! other files until it is understood.

use std::time::Duration;

use termlens::{Graphics, Key, Screen, Terminal};

/// A year of contribution art from a file, at a pinned date.
const PREVIEW: [&str; 4] = ["--file", "art/vyncint-2027.json", "--today", "2027-06-30"];

/// The only sequence mossaic emits that termlens does not model.
///
/// `SGR 59` is "underline colour: default", which ratatui writes as part of
/// resetting a style. termlens carries no underline colour, so it records the
/// sequence and moves on — and because the attribute changes no cell, nothing
/// on the grid is wrong as a result. That is the whole reason this list can
/// be pinned exactly: anything joining it is a sequence that *might* change a
/// cell, and would need reading before the suite is trusted again.
const EXPECTED_UNSUPPORTED: [&str; 1] = ["^[[59m"];

fn chart(graphics: Option<Graphics>, cols: u16, rows: u16) -> termlens::Result<Terminal> {
    let mut builder = Terminal::builder()
        .size(cols, rows)
        .env_clear()
        .env("COLORTERM", "truecolor")
        .env("TERM", "xterm-256color")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .timeout(Duration::from_secs(20))
        .args(PREVIEW);
    if let Some(graphics) = graphics {
        builder = builder.graphics(graphics).cell_size(10, 20);
    }
    let mut t = builder.spawn(env!("CARGO_BIN_EXE_mossaic"))?;
    t.wait_frame(|s| s.contains("q quit") && s.contains("contributions in"))?;
    Ok(t)
}

fn unsupported(screen: &Screen) -> Vec<String> {
    screen.unsupported().iter().map(|s| s.to_string()).collect()
}

/// The invariant, in all three rendering modes. The image paths are the ones
/// worth checking hardest: they put bytes on the wire that no cell shows, so
/// a dropped sequence there is invisible in every other assertion.
#[test]
fn the_emulator_drops_nothing_that_could_change_a_cell() -> termlens::Result<()> {
    for (label, graphics) in [
        ("text cells", None),
        ("kitty", Some(Graphics::Kitty)),
        ("sixel", Some(Graphics::Sixel)),
    ] {
        let t = chart(graphics, 120, 30)?;
        let screen = t.screen();
        assert_eq!(
            unsupported(&screen),
            EXPECTED_UNSUPPORTED,
            "{label}: mossaic emitted a sequence termlens does not model. \
             Until it is understood, every screen assertion in this suite is \
             being made against a grid that may be wrong."
        );
        assert_eq!(
            screen.unsupported_overflow(),
            0,
            "{label}: the record is complete, not truncated"
        );
        if graphics.is_some() {
            assert!(
                !screen.graphics().is_empty(),
                "{label}: the image path was actually exercised"
            );
        }
    }
    Ok(())
}

/// Three smaller invariants that would each make the grid a lie, and that
/// nothing else in the suite would notice.
#[test]
fn mossaic_leaves_the_terminal_modes_alone() -> termlens::Result<()> {
    let t = chart(None, 176, 34)?;
    let screen = t.screen();

    // Insert mode pushes the rest of a row right. An application that left
    // it on would draw a correct-looking chart with every row shifted.
    assert!(!screen.insert_mode(), "mossaic never sets IRM");
    // A visual bell is a flash the grid cannot show; mossaic should ring none.
    assert_eq!(screen.visual_bells(), 0);
    assert_eq!(screen.bells(), 0, "and no audible one either");
    // Nothing wraps: mossaic lays out to the width it measured, so a wrapped
    // row means the layout overflowed and the text is silently on two rows.
    assert!(
        !(0..screen.rows()).any(|row| screen.row_wrapped(row)),
        "a wrapped row means the layout overflowed:\n{screen}"
    );
    Ok(())
}

/// A mossaic screen has to survive being saved and read back, because that is
/// what a bug report is: the palette, the block glyphs and the box drawing all
/// come back, or the format is not carrying what mossaic draws.
#[test]
fn a_chart_survives_the_snapshot_format_and_json() -> termlens::Result<()> {
    let t = chart(None, 176, 34)?;
    let screen = t.screen();

    // The text format, which is what a saved `.snap` or a pasted CI log is.
    let saved = screen.with_styles().to_string();
    let parsed = Screen::parse(&saved)?;
    assert!(screen.diff(&parsed).is_empty(), "{}", screen.diff(&parsed));
    assert_eq!(parsed.with_styles().to_string(), saved, "byte for byte");

    // And JSON, which is what `TERMLENS_ARTIFACT_DIR` writes in CI.
    let json = serde_json::to_string(&screen).expect("a Screen serializes");
    let back: Screen = serde_json::from_str(&json).expect("and comes back");
    assert!(screen.diff(&back).is_empty(), "{}", screen.diff(&back));

    // The palette specifically: mossaic's whole output is colour, so a round
    // trip that dropped it would still pass a text comparison.
    let coloured = screen
        .find_by(|cell| matches!(cell.style().fg, termlens::Color::Rgb(..)))
        .expect("the chart draws truecolour");
    assert_eq!(
        back.cell(coloured.0, coloured.1).unwrap().style(),
        screen.cell(coloured.0, coloured.1).unwrap().style()
    );
    Ok(())
}

/// `diff` says what a keystroke changed, and how much it left alone. The
/// second half is the interesting one: mossaic redraws the whole screen on
/// navigation, so "only these rows differ" is a real claim about the layout
/// staying put rather than about the diff being small.
#[test]
fn moving_the_cursor_changes_the_rows_it_should_and_no_others() -> termlens::Result<()> {
    let mut t = chart(None, 176, 34)?;
    let before = t.screen();

    t.send(Key::Right)?;
    let after = t.wait_frame(|s| !s.contains("Jun 30 2027"))?;

    let diff = before.diff(&after);
    assert!(!diff.is_empty(), "the cursor moved");
    let mut rows: Vec<u16> = diff.cells().map(|(row, ..)| row).collect();
    rows.dedup();
    assert!(
        rows.len() <= 4,
        "moving one day should touch the cursor's row and the detail line, \
         not repaint the chart: rows {rows:?}\n{diff}"
    );
    // The header and the footer are not among them.
    assert!(
        after.contains("contributions in") && after.contains("q quit"),
        "the frame is still whole:\n{after}"
    );
    Ok(())
}
