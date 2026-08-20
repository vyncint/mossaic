//! End-to-end tests: the real binary, in a real PTY, asserted on what a user
//! would see.
//!
//! The unit tests in `src/render_tests.rs` render through ratatui's
//! `TestBackend`, which covers layout but not the event loop, the PTY, or
//! anything mossaic writes outside ratatui — which is where the images go.
//! [`termlens`] covers that half: it spawns the binary under a pty, renders its
//! output with a VT emulator, and hands back a screen grid to assert on.
//!
//! Everything here is hermetic, driven from `art/vyncint-2027.json` so no test
//! needs the network. The year and user navigation that does is behind
//! `#[ignore]`, the same way the live unit test is:
//!
//! ```sh
//! cargo test --test smoke
//! cargo test --test smoke -- --ignored --nocapture    # needs gh
//! ```

use std::time::Duration;

use termlens::{Color, Key, MouseButton, Screen, Scroll, Terminal};

/// A year of contribution art, drawn from a file: every day elapsed, every lit
/// day at level 4, and no network anywhere.
const PREVIEW: [&str; 2] = ["--file", "art/vyncint-2027.json"];
/// Wide enough for the rounded cells, tall enough for the whole chart.
const SIZE: (u16, u16) = (176, 34);

fn chart(args: &[&str]) -> termlens::Result<Terminal> {
    spawn(args, SIZE, |builder| builder)
}

/// `env_clear` for hermeticity, then back in only what the chart's appearance is
/// allowed to depend on. Without `COLORTERM` mossaic drops to the 256-colour
/// ramp, which is a different assertion — see [`the_palette_degrades_without_truecolor`].
fn spawn(
    args: &[&str],
    size: (u16, u16),
    configure: impl FnOnce(termlens::TerminalBuilder) -> termlens::TerminalBuilder,
) -> termlens::Result<Terminal> {
    let builder = Terminal::builder()
        .size(size.0, size.1)
        .env_clear()
        .env("COLORTERM", "truecolor")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .timeout(Duration::from_secs(20))
        .args(args);
    configure(builder).spawn(env!("CARGO_BIN_EXE_mossaic"))
}

/// The chart is on screen once its footer is: mossaic draws the footer last,
/// so a frame carrying it carries everything above it too.
fn loaded(screen: &Screen) -> bool {
    screen.contains("q quit") && screen.contains("contributions in")
}

/// The five legend swatches' colours, in order. The chart draws them with the
/// palette it chose, so this is that choice, read back off the screen.
fn legend_colours(screen: &Screen) -> Vec<Color> {
    let Some(row) = (0..screen.rows()).find(|row| screen.row_text(*row).contains("Less")) else {
        return Vec::new();
    };
    (0..screen.cols())
        .filter_map(|col| screen.cell(row, col))
        .filter(|cell| cell.contents().contains('\u{1FB2B}'))
        .map(|cell| cell.style().fg)
        .collect()
}

/// The line naming the active cell style, e.g. "… · rounded cells".
fn style(screen: &Screen) -> String {
    let line = screen
        .text()
        .lines()
        .find(|line| line.contains(" cells"))
        .unwrap_or_default()
        .to_string();
    // The row spans the terminal, so the frame's right edge comes along with it.
    line.rsplit('·')
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

/// Screen column of the first cell: past the frame and the four-column gutter.
const GRID_X: u16 = 1 + 4;

#[test]
fn the_chart_draws_and_quits_cleanly() -> termlens::Result<()> {
    let mut t = chart(&PREVIEW)?;
    // `wait_frame`, not `wait_until`: the predicate then only ever sees complete
    // repaints. Waiting on content alone can match a frame half-applied — the
    // header already carrying the new total while the legend below it is still
    // the one from the loading screen.
    let screen = t.wait_frame(loaded)?;
    assert!(screen.alternate_screen(), "the chart owns the whole screen");
    assert!(
        screen.contains("vyncint  ·  2027  ·  300 contributions in 2027"),
        "header should read like github.com's:\n{screen}"
    );
    assert!(
        screen.contains("Less") && screen.contains("More"),
        "the legend is github.com's wording:\n{screen}"
    );
    // A file has one year and one user, so the keys that move between them are
    // not offered.
    assert!(screen.contains("preview"), "{screen}");
    assert!(!screen.contains("[ ] year"), "{screen}");

    t.send(Key::Char('q'))?;
    let exit = t.wait_exit()?;
    assert!(exit.success(), "clean exit, got {exit:?}");
    assert!(
        !t.screen().alternate_screen(),
        "the terminal is handed back the way it was found"
    );
    Ok(())
}

#[test]
fn the_cursor_moves_by_day_and_by_week() -> termlens::Result<()> {
    let mut t = chart(&PREVIEW)?;
    t.wait_frame(loaded)?;
    // Start from a known day rather than from wherever the cursor landed. It
    // opens on today when today is in the calendar and on the last day
    // otherwise, so a fixture year that is in the future today becomes the
    // current year eventually — and every relative assertion below it shifts by
    // however far into the year that is. `End` is the same key the test exercises
    // later, so this costs no coverage.
    t.send(Key::End)?;
    t.wait_frame(|s| s.contains("Fri, Dec 31 2027"))?;

    for (key, expected) in [
        (Key::Left, "Fri, Dec 24 2027"),
        (Key::Up, "Thu, Dec 23 2027"),
        (Key::Down, "Fri, Dec 24 2027"),
        (Key::Right, "Fri, Dec 31 2027"),
        (Key::Char('h'), "Fri, Dec 24 2027"),
        (Key::Char('k'), "Thu, Dec 23 2027"),
        (Key::Char('j'), "Fri, Dec 24 2027"),
        (Key::Char('l'), "Fri, Dec 31 2027"),
        (Key::Home, "Fri, Jan 1 2027"),
        (Key::End, "Fri, Dec 31 2027"),
    ] {
        t.send(key)?;
        t.wait_frame(|s| s.contains(expected))?;
    }

    // The range is a hard edge, not a wrap: four days back off the first day is
    // still the first day, so one day forward from there is the second.
    t.send(Key::Home)?;
    t.wait_frame(|s| s.contains("Fri, Jan 1 2027"))?;
    for _ in 0..4 {
        t.send(Key::Char('k'))?;
    }
    t.send(Key::Down)?;
    t.wait_frame(|s| s.contains("Sat, Jan 2 2027"))?;
    Ok(())
}

#[test]
fn cell_styles_cycle_through_every_shape() -> termlens::Result<()> {
    let mut t = chart(&PREVIEW)?;
    let first = t.wait_frame(loaded)?;
    assert_eq!(style(&first), "rounded cells", "auto picks rounded here");

    assert!(is_auto(&first), "and says it chose:\n{first}");

    // The first press pins what auto had already chosen; from there each press
    // is a different shape. Pixels are skipped: this terminal draws none.
    for expected in [
        "rounded cells",
        "snug cells",
        "squares cells",
        "grid cells",
        "spaced cells",
        "blocks cells",
        "slim cells",
        "compact cells",
    ] {
        t.send(Key::Char('d'))?;
        t.wait_frame(|s| style(s) == expected && !is_auto(s))?;
    }

    // Round the end of the cycle: compact goes back to `Auto`, which resolves to
    // rounded here — so the *shape* repeats, and only the label distinguishes the
    // press from one that did nothing. That is why the label exists.
    t.send(Key::Char('d'))?;
    let back = t.wait_frame(is_auto)?;
    assert_eq!(style(&back), "rounded cells", "{back}");
    t.send(Key::Char('d'))?;
    t.wait_frame(|s| style(s) == "rounded cells" && !is_auto(s))?;
    Ok(())
}

#[test]
fn the_mouse_hovers_and_clicks() -> termlens::Result<()> {
    let mut t = chart(&PREVIEW)?;
    // Mouse reporting is turned on once the loop is running, and termlens
    // encodes for whatever mode the application actually asked for.
    let ready = t.wait_frame(|s| loaded(s) && s.mouse_mode() != termlens::MouseMode::None)?;
    let row = monday(&ready) + 2; // Wednesday

    // Rounded cells are three columns a week. A drag reports its motion, which
    // is the same event a hover is: press one day, move to another.
    let stride = 3;
    let from = GRID_X + 10 * stride;
    let to = GRID_X + 20 * stride;
    t.drag(MouseButton::Left, (from, row), (to, row))?;

    let screen =
        t.wait_frame(|s| s.contains("contributions on ") || s.contains("No contributions on "))?;
    let tooltip = screen
        .text()
        .lines()
        .find(|line| line.contains(" on "))
        .expect("a tooltip")
        .to_string();
    assert!(
        tooltip.contains('▐') && tooltip.contains('▌'),
        "the tooltip is a pill:\n{tooltip}"
    );
    assert!(
        tooltip.ends_with('▌') || tooltip.contains("▌"),
        "…closed at both ends:\n{tooltip}"
    );
    assert!(
        screen.text().contains('▼'),
        "…with a pointer at the week it describes:\n{screen}"
    );
    // It floats above the grid rather than over it: those rows belong to the
    // image when there is one, and text over a sixel cannot be taken back.
    let pointer_row = (0..screen.rows())
        .find(|r| screen.row_text(*r).contains('▼'))
        .expect("a pointer");
    assert_eq!(
        pointer_row,
        monday(&screen) - 2,
        "the pointer sits just above the grid:\n{screen}"
    );

    // Clicking is what terminals without motion reporting still deliver, and it
    // moves the cursor rather than only pointing at a day.
    t.click(from, row)?;
    t.wait_frame(|s| {
        s.text()
            .lines()
            .any(|line| line.starts_with("Wed, ") || line.starts_with("│Wed, "))
    })?;
    Ok(())
}

#[test]
fn mouse_reporting_can_be_given_back() -> termlens::Result<()> {
    let mut t = chart(&PREVIEW)?;
    t.wait_frame(|s| loaded(s) && s.mouse_mode() != termlens::MouseMode::None)?;

    // Mouse tracking takes click-to-select away from the terminal, so it has to
    // be possible to hand it back — and the footer has to say which state it is in.
    t.send(Key::Char('m'))?;
    t.wait_frame(|s| s.mouse_mode() == termlens::MouseMode::None && s.contains("m mouse on"))?;

    t.send(Key::Char('m'))?;
    t.wait_frame(|s| s.mouse_mode() != termlens::MouseMode::None && s.contains("m mouse off"))?;
    Ok(())
}

#[test]
fn pixel_cells_leave_the_grid_rows_to_the_painter() -> termlens::Result<()> {
    // This terminal draws no pixels — its device attributes say so — but the
    // protocol and the cell size can both be given, which is what makes the
    // pixel layout testable at all. What it draws lands in a DCS string the
    // emulator consumes, so the text layer underneath is what is asserted here.
    let args = [
        "--file",
        "art/vyncint-2027.json",
        "--graphics",
        "sixel",
        "--cell",
        "10x20",
    ];
    let mut t = chart(&args)?;
    let screen = t.wait_frame(loaded)?;
    assert_eq!(style(&screen), "pixel cells (sixel)", "{screen}");
    // Seven rows of nothing but the weekday gutter: anything drawn there would
    // be drawn over the image.
    let top = monday(&screen) - 1;
    for row in top..top + 7 {
        let text = screen.row_text(row);
        let cells = text
            .chars()
            .skip(usize::from(GRID_X))
            .take_while(|c| *c != '│')
            .collect::<String>();
        assert!(
            cells.trim().is_empty(),
            "row {row} should be left for the painter: {cells:?}"
        );
    }
    // The legend reserves room for its swatches the same way.
    let legend = screen
        .text()
        .lines()
        .find(|line| line.contains("Less"))
        .unwrap_or_default()
        .to_string();
    let gap = legend
        .split_once("Less")
        .and_then(|(_, rest)| rest.split_once("More"))
        .map(|(between, _)| between.to_string())
        .unwrap_or_default();
    assert!(
        gap.len() >= 10 && gap.trim().is_empty(),
        "the swatches are an image too, so the text only holds their place: {legend:?}"
    );
    Ok(())
}

#[test]
fn the_theme_follows_the_terminals_background() -> termlens::Result<()> {
    // Primer's own values, from the stylesheets github.com serves. No flag is
    // passed here: the only thing that differs is what the terminal answers to
    // OSC 11, which is how a browser picks a theme too.
    let expect = |levels: [u32; 5]| -> Vec<Color> {
        levels
            .iter()
            .map(|hex| Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, *hex as u8))
            .collect()
    };

    let mut dark = spawn(&PREVIEW, SIZE, |b| b.background_rgb(0x0d, 0x11, 0x17))?;
    let screen = dark.wait_frame(loaded)?;
    assert_eq!(
        legend_colours(&screen),
        expect([0x151b23, 0x033a16, 0x196c2e, 0x2ea043, 0x56d364]),
        "a dark terminal gets Primer's dark scale:\n{screen}"
    );

    let mut light = spawn(&PREVIEW, SIZE, |b| b.background_rgb(0xff, 0xff, 0xff))?;
    let screen = light.wait_frame(loaded)?;
    assert_eq!(
        legend_colours(&screen),
        expect([0xeff2f5, 0xaceebb, 0x4ac26b, 0x2da44e, 0x116329]),
        "a light terminal gets Primer's light scale:\n{screen}"
    );
    Ok(())
}

#[test]
fn the_palette_degrades_without_truecolor() -> termlens::Result<()> {
    // No COLORTERM: the five levels are chosen from the 256-colour cube, and the
    // one thing that must not happen is two of them landing on the same entry.
    let mut t = Terminal::builder()
        .size(SIZE.0, SIZE.1)
        .env_clear()
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .timeout(Duration::from_secs(20))
        .args(PREVIEW)
        .spawn(env!("CARGO_BIN_EXE_mossaic"))?;
    let screen = t.wait_frame(loaded)?;
    let drawn = legend_colours(&screen);
    assert!(
        drawn.len() == 5 && drawn.iter().all(|c| matches!(c, Color::Indexed(_))),
        "without truecolor the levels come from the 256-colour ramp, got {drawn:?}"
    );
    // Distinct, which is the one thing the cube cannot manage on its own.
    let mut unique = drawn.clone();
    unique.dedup();
    assert_eq!(unique.len(), 5, "two levels collapsed: {drawn:?}");
    Ok(())
}

#[test]
fn a_narrower_terminal_chooses_a_smaller_cell() -> termlens::Result<()> {
    let mut t = chart(&PREVIEW)?;
    let screen = t.wait_frame(loaded)?;
    assert_eq!(style(&screen), "rounded cells");

    // Rounded needs 165 columns; squares fit in 112, and the chart says which it
    // took rather than leaving the sharper corners unexplained.
    t.resize(120, 34)?;
    let narrow = t.wait_frame(|s| style(s) == "squares cells")?;
    assert!(
        narrow.contains("rounded corners need 165 columns"),
        "it should say what it wants:\n{narrow}"
    );

    t.resize(70, 34)?;
    t.wait_frame(|s| style(s) == "compact cells")?;

    t.resize(SIZE.0, SIZE.1)?;
    t.wait_frame(|s| style(s) == "rounded cells")?;
    Ok(())
}

#[test]
fn a_missing_file_is_reported_not_swallowed() -> termlens::Result<()> {
    let mut t = chart(&["--file", "nope.json"])?;
    let screen = t.wait_frame(|s| s.contains("could not read"))?;
    assert!(screen.contains("nope.json"), "{screen}");
    assert!(
        screen.contains("retry"),
        "and it offers a way out:\n{screen}"
    );
    t.send(Key::Char('q'))?;
    assert!(t.wait_exit()?.success());
    Ok(())
}

#[test]
fn esc_does_not_quit_the_chart() -> termlens::Result<()> {
    let mut t = chart(&PREVIEW)?;
    t.wait_frame(loaded)?;
    // `Esc` used to be a second quit key, so pressing it to dismiss a tooltip
    // exited the program. It is now unbound in normal mode, and an unbound key is
    // ignored — which is only observable by the chart still answering a bound one.
    t.send(Key::Esc)?;
    t.send(Key::End)?;
    let after = t.wait_frame(|s| s.contains("Fri, Dec 31 2027"))?;
    assert!(
        after.alternate_screen(),
        "the chart still owns the screen after Esc:\n{after}"
    );
    // And `q` is still the way out.
    t.send(Key::Char('q'))?;
    assert!(t.wait_exit()?.success());
    Ok(())
}

/// The `?` overlay has to agree with `on_key_normal` about the quit key. Three
/// surfaces claimed `Esc` quits — this one, `--help` and the README — and nothing
/// tied any of them to the binding, so all three were free to go stale the moment
/// the binding changed. This pins the one a reader in the chart actually consults.
#[test]
fn the_help_surfaces_do_not_promise_a_key_that_does_nothing() -> termlens::Result<()> {
    let mut t = chart(&PREVIEW)?;
    t.wait_frame(loaded)?;
    t.send(Key::Char('?'))?;
    let help = t.wait_frame(|s| s.contains("This terminal"))?;
    let quit = help
        .text()
        .lines()
        .find(|line| line.contains("quit"))
        .expect("the overlay lists the quit key")
        .to_string();
    assert!(
        !quit.contains("Esc"),
        "the overlay still offers Esc as a quit key:\n{quit}"
    );
    Ok(())
}

#[test]
fn the_help_overlay_answers_what_this_terminal_can_do() -> termlens::Result<()> {
    let mut t = chart(&PREVIEW)?;
    t.wait_frame(loaded)?;
    assert!(t.screen().contains("? help"), "the footer points at it");

    t.send(Key::Char('?'))?;
    let help = t.wait_frame(|s| s.contains("This terminal"))?;
    for wanted in [
        "Moving",
        "Mouse",
        "hover a day",
        "kitty graphics",
        "sixel",
        "character cell",
        "drawing with",
        "any key closes this",
    ] {
        assert!(help.contains(wanted), "help is missing {wanted:?}:\n{help}");
    }
    // Nothing in the box may be clipped by its own border.
    for row in 0..help.rows() {
        let line = help.row_text(row);
        if let Some((body, _)) = line
            .split_once('│')
            .and_then(|(_, rest)| rest.rsplit_once('│'))
        {
            assert!(
                !body.ends_with(char::is_alphanumeric),
                "row {row} is cut off at the border: {body:?}"
            );
        }
    }

    // Modal and forgiving: whatever you press to get out of it, gets you out.
    t.send(Key::Char('j'))?;
    t.wait_frame(|s| !s.contains("This terminal"))?;
    assert!(
        t.screen().contains("Less"),
        "the chart is back:\n{}",
        t.screen()
    );
    Ok(())
}

#[test]
fn the_demo_needs_no_account_and_no_network() -> termlens::Result<()> {
    // `env_clear` takes PATH with it, so `gh` cannot be found even if it is
    // installed — which is the point: this must work on a machine that has
    // never heard of the GitHub CLI.
    let mut t = chart(&["--demo"])?;
    let screen = t.wait_frame(loaded)?;
    assert!(screen.contains("demo  ·  "), "{screen}");
    assert!(
        screen.contains("contributions in"),
        "a full sample year:\n{screen}"
    );
    assert!(
        screen.contains("demo — `mossaic <user>` for a real one"),
        "and it says how to get off the demo:\n{screen}"
    );
    assert!(
        !screen.contains("[ ] year"),
        "one year only, so no year keys"
    );

    t.send(Key::Char('q'))?;
    assert!(t.wait_exit()?.success());
    Ok(())
}

/// Hits the network through `gh`. Run with:
///   cargo test --test smoke -- --ignored --nocapture
#[test]
#[ignore = "requires gh and network access"]
fn years_users_and_errors() -> termlens::Result<()> {
    // The one test that needs the host: `gh` on PATH and its config in HOME, so
    // this is the one that does not clear the environment.
    let mut t = Terminal::builder()
        .size(SIZE.0, SIZE.1)
        .env("COLORTERM", "truecolor")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .timeout(Duration::from_secs(30))
        .spawn(env!("CARGO_BIN_EXE_mossaic"))?;
    let screen = t.wait_frame(loaded)?;
    let year = chrono_year(&screen);

    // `[` and `]` step through the years GitHub reports contributions for.
    t.send(Key::Char('['))?;
    // The header shows the new year while the fetch is still in flight, so wait
    // for a frame that has the chart as well as the number.
    let earlier = t.wait_frame(|s| chrono_year(s) < year && loaded(s))?;
    assert!(earlier.contains("Dec 31"), "{earlier}");
    t.send(Key::Char(']'))?;
    t.wait_frame(|s| chrono_year(s) == year && loaded(s))?;

    // The wheel does the same thing, for a hand already on the mouse.
    let ready = t.wait_frame(|s| s.mouse_mode() != termlens::MouseMode::None)?;
    let row = monday(&ready);
    t.scroll(GRID_X, row, Scroll::Up)?;
    t.wait_frame(|s| chrono_year(s) < year && loaded(s))?;
    t.scroll(GRID_X, row, Scroll::Down)?;
    t.wait_frame(|s| chrono_year(s) == year && loaded(s))?;

    // A different user, then one that cannot exist.
    t.send(Key::Char('u'))?;
    t.wait_frame(|s| s.contains("esc cancel"))?;
    for _ in 0..40 {
        t.send(Key::Backspace)?;
    }
    t.send_str("octocat")?;
    t.send(Key::Enter)?;
    t.wait_frame(|s| s.contains("octocat") && loaded(s))?;

    t.send(Key::Char('u'))?;
    t.wait_frame(|s| s.contains("esc cancel"))?;
    for _ in 0..40 {
        t.send(Key::Backspace)?;
    }
    t.send_str("zz-no-such-user-9x8")?;
    t.send(Key::Enter)?;
    t.wait_frame(|s| s.contains("Could not resolve"))?;

    t.send(Key::Char('q'))?;
    assert!(t.wait_exit()?.success());
    Ok(())
}

/// The year in the header, e.g. "vyncint  ·  2026  ·  …".
fn chrono_year(screen: &Screen) -> i32 {
    screen
        .text()
        .lines()
        .find_map(|line| {
            let (_, rest) = line.split_once("  ·  ")?;
            rest.split("  ·  ").next()?.trim().parse().ok()
        })
        .unwrap_or_default()
}

#[test]
fn an_idle_chart_stops_writing() -> termlens::Result<()> {
    // The chart repaints on a timer — 80 ms, so the loading spinner can turn —
    // which means a frame goes out whether or not anything changed. What must
    // *not* happen is that each of those frames rewrites the screen: ratatui
    // diffs its buffer and the painter diffs the images, so a settled chart
    // should cost a pair of synchronized-update brackets and nothing else.
    //
    // Invisible to every content predicate, because each of those frames shows
    // exactly the right content. `printable_chars` is what sees it.
    let mut t = chart(&PREVIEW)?;
    t.wait_frame(loaded)?;
    let settled = t.screen().repaints();
    // Let several more frames go by with no input at all.
    t.wait_frame(|s| s.repaints() >= settled + 4)?;

    let idle: Vec<u32> = t
        .frame_timings()
        .iter()
        .filter(|frame| frame.index() > settled)
        .map(|frame| frame.printable_chars())
        .collect();
    assert!(
        idle.len() >= 3,
        "expected several idle frames to inspect, got {idle:?}"
    );
    assert!(
        idle.iter().all(|written| *written == 0),
        "an idle chart rewrote the screen: printable characters per frame {idle:?}"
    );
    Ok(())
}

#[test]
fn nothing_rings_the_bell() -> termlens::Result<()> {
    // The bell is often the only feedback a rejected key produces, so "this key
    // does nothing" and "this key is refused with a bell" are the same screen.
    // mossaic ignores what it does not bind — silently, on purpose — and this is
    // the assertion that says so rather than assuming it.
    let mut t = chart(&PREVIEW)?;
    t.wait_frame(loaded)?;

    // Keys it binds, keys it does not, and a couple that mean something only in
    // the username prompt.
    for key in [
        Key::Char('d'),
        Key::Char('x'),
        Key::Char('9'),
        Key::Char('Z'),
        Key::Tab,
        Key::Backspace,
        Key::Char('m'),
        Key::Char('r'),
        Key::Char('?'),
        Key::Char('j'),
    ] {
        t.send(key)?;
    }
    // A frame after the last of them, so every key has been through the loop.
    let settled = t.screen().repaints();
    let screen = t.wait_frame(|s| s.repaints() > settled + 1)?;
    assert_eq!(
        screen.bells(),
        0,
        "mossaic rang the bell {} time(s)",
        screen.bells()
    );
    Ok(())
}

#[test]
fn a_drag_lands_on_the_day_it_ended_on() -> termlens::Result<()> {
    // A drag reports one motion per cell crossed, so this is twenty events
    // arriving faster than frames. The tooltip has to end up on the day under
    // the pointer, not several cells behind it — which is the whole reason the
    // event loop drains the queue rather than taking one event per frame.
    let mut t = chart(&PREVIEW)?;
    let ready = t.wait_frame(|s| loaded(s) && s.mouse_mode() != termlens::MouseMode::None)?;

    // Rounded cells are three columns a week; row Monday+2 is Wednesday. Week 30
    // Wednesday is 2027-07-28 in this fixture, which holds no contributions.
    let row = monday(&ready) + 2;
    let stride = 3;
    t.drag(
        MouseButton::Left,
        (GRID_X + 10 * stride, row),
        (GRID_X + 30 * stride, row),
    )?;

    let screen = t.wait_frame(|s| s.contains(" on "))?;
    assert!(
        screen.contains("No contributions on July 28th."),
        "the tooltip should name the day the drag ended on:\n{screen}"
    );
    Ok(())
}

#[test]
fn a_small_terminal_still_says_how_to_get_out() -> termlens::Result<()> {
    // At 80x24 the footer was one 111-character line, hard-truncated, so neither
    // `q quit` nor `? help` was on screen — and the overlay that would have said
    // so truncated from the bottom and dropped `any key closes this` too. 80x24
    // is the canonical terminal.
    for size in [(80u16, 24u16), (100, 24), (60, 20)] {
        let mut t = spawn(&PREVIEW, size, |b| b)?;
        let screen = t.wait_frame(loaded)?;
        assert!(
            screen.contains("q quit"),
            "at {size:?} there is no way out on screen:\n{screen}"
        );
        assert!(screen.contains("? help"), "at {size:?}:\n{screen}");

        t.send(Key::Char('?'))?;
        // Waiting on the *content*, not on a heading: a short panel drops its
        // headings before its facts, which is the right order — a fact reads on
        // its own, a heading with nothing under it does not.
        let help = t.wait_frame(|s| s.contains("drawing with"))?;
        assert!(
            help.contains("any key closes this"),
            "at {size:?} the overlay dropped its own way out:\n{help}"
        );
    }
    Ok(())
}

#[test]
fn the_advice_line_never_points_at_something_smaller_that_does_not_exist() -> termlens::Result<()> {
    // At 50x22 `Auto` is already at its narrowest, and the note said "press d for
    // a smaller style" — where `d` from `Auto` goes to pixels, three times wider.
    let mut t = spawn(&PREVIEW, (50, 22), |b| b)?;
    let screen = t.wait_frame(loaded)?;
    assert!(
        !screen.contains("press d for a smaller style"),
        "nothing smaller exists here:\n{screen}"
    );
    assert!(
        screen.contains("at its narrowest"),
        "it should say what a year actually needs:\n{screen}"
    );
    Ok(())
}

#[test]
fn a_click_that_misses_a_day_puts_the_tooltip_away() -> termlens::Result<()> {
    // A miss left the last tooltip on screen, still naming an unrelated date. On a
    // terminal that reports clicks but not motion — which is the case the press arm
    // exists for — nothing later corrects it.
    let mut t = chart(&PREVIEW)?;
    let ready = t.wait_frame(|s| loaded(s) && s.mouse_mode() != termlens::MouseMode::None)?;
    let row = monday(&ready) + 2;

    t.click(GRID_X + 10 * 3, row)?;
    t.wait_frame(|s| s.text().contains(" on "))?;

    // The header is not a day.
    t.click(40, 1)?;
    let after = t.wait_frame(|s| !s.text().contains(" on "))?;
    assert!(
        !after.text().contains(" on "),
        "the tooltip outlived the day it described:\n{after}"
    );
    Ok(())
}

#[test]
fn the_wheel_does_nothing_behind_the_help_overlay() -> termlens::Result<()> {
    // `on_mouse` guarded the username prompt but not the overlay, so the wheel
    // changed year and started a fetch for a year the reader could not see —
    // behind a panel that says "any key closes this".
    let mut t = chart(&["--demo"])?;
    let ready = t.wait_frame(|s| loaded(s) && s.mouse_mode() != termlens::MouseMode::None)?;
    let year = chrono_year(&ready);
    let row = monday(&ready);

    t.send(Key::Char('?'))?;
    let help = t.wait_frame(|s| s.contains("This terminal"))?;
    t.scroll(GRID_X, row, Scroll::Up)?;

    // Give it frames to have acted in, then check nothing did.
    let settled = help.repaints();
    let after = t.wait_frame(|s| s.repaints() > settled + 2)?;
    assert!(
        after.contains("This terminal"),
        "the overlay should still be up:\n{after}"
    );
    assert_eq!(
        chrono_year(&after),
        year,
        "the wheel changed the year behind it:\n{after}"
    );
    Ok(())
}

/// The sizes the README's "Known limits" quotes, in the units a user's terminal
/// reports them in — which is the drawable area plus the border mossaic draws.
/// The in-process test pins the arithmetic; this pins that the arithmetic is
/// about the right thing, because the two disagreed: the advice line told a
/// reader with a 17-row window that it "has 15", so resizing to the 17 it asked
/// for still did not fit.
#[test]
fn the_documented_window_sizes_are_the_ones_a_terminal_reports() -> termlens::Result<()> {
    for (size, expected) in [
        ((112u16, 19u16), "squares cells"),
        ((111, 19), "compact cells"),
        ((112, 18), "compact cells"),
        ((165, 19), "rounded cells"),
        ((164, 19), "squares cells"),
    ] {
        let mut t = spawn(&PREVIEW, size, |b| b)?;
        // Not `loaded`: at a height where nothing fits, the advice line takes the
        // footer's row, because "needs 19 rows" is the more actionable of the two.
        let screen = t.wait_frame(|s| s.contains(" cells") && s.contains("contributions in"))?;
        assert_eq!(
            style(&screen),
            expected,
            "at {}x{}:\n{screen}",
            size.0,
            size.1
        );
        t.send(Key::Char('q'))?;
        let _ = t.wait_exit()?;
    }
    Ok(())
}

/// And when nothing fits, the advice names a number the reader can resize to.
#[test]
fn the_advice_line_names_the_window_size_not_the_drawable_area() -> termlens::Result<()> {
    let mut t = spawn(&PREVIEW, (112, 18), |b| b)?;
    let screen = t.wait_frame(|s| s.contains("needs") && s.contains("rows"))?;
    assert!(
        screen.contains("the chart needs 19 rows — this window has 18"),
        "the numbers a terminal reports, not the ones inside the border:\n{screen}"
    );
    t.send(Key::Char('q'))?;
    let _ = t.wait_exit()?;
    Ok(())
}
