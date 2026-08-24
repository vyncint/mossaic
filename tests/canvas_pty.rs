//! The pixel-art surfaces, in a real pseudo-terminal.
//!
//! `tests/art_cli.rs` drives `mossaic-art` the way a shell does, which is right
//! for a command that prints and exits. Three of the new ones are not that:
//! `--list-templates` and `--template` paint colour that a pipe never sees, and
//! `--draw` is an event loop with no output at all until something is typed at
//! it. [`termlens`] spawns the real binary under a pty and hands back the
//! rendered screen, which is the only place those claims can be checked.
//!
//! The editor's *logic* is unit-tested in `src/render_tests.rs`, where undo,
//! the cost estimate and the mouse mapping are assertions about state. What is
//! left for here is the half that needs a terminal: that keystrokes reach the
//! model at all, that the screen shows what was drawn, and that quitting puts
//! the terminal back.

use std::time::Duration;

use termlens::{Key, Screen, Terminal};

/// Big enough for 53 columns plus the row-label gutter, and the whole HUD.
const SIZE: (u16, u16) = (100, 30);

fn spawn(args: &[&str]) -> termlens::Result<Terminal> {
    Terminal::builder()
        .size(SIZE.0, SIZE.1)
        .env_clear()
        .env("COLORTERM", "truecolor")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .timeout(Duration::from_secs(20))
        .args(args)
        .spawn(env!("CARGO_BIN_EXE_mossaic-art"))
}

/// A scratch path no other process will touch. The same reasoning as
/// `art_cli.rs`: a fixed global path is shared state between concurrent runs.
fn scratch(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("mossaic-pty-{}-{name}", std::process::id()))
}

/// The editor has painted once its footer is on screen — it is drawn last, so a
/// frame carrying it carries the grid and the HUD above it too.
fn ready(screen: &Screen) -> bool {
    screen.contains("q quit")
}

// ------------------------------------------------------- the catalogue

#[test]
fn list_templates_shows_a_picture_and_not_just_a_name() -> termlens::Result<()> {
    let mut terminal = spawn(&["--list-templates"])?;
    // Waiting for the *process* rather than for a line: this command prints
    // and exits, so a predicate matching part way down its output can be
    // satisfied by a screen the rest of it has not reached yet. Every
    // assertion below is then a race, and it loses on a loaded runner.
    terminal.wait_exit()?;
    // History *and* screen. The catalogue outgrew one screenful when it stopped
    // being a catalogue of one, so the first entry has scrolled off by the time
    // the process exits — and "did this reach the terminal" is the question the
    // test is actually asking, not "is it still visible".
    let text = terminal.screen().full_text();

    assert!(text.contains("dragon"), "the name:\n{text}");
    assert!(text.contains("Dragon"), "the title:\n{text}");
    assert!(text.contains("@vyncint"), "the author:\n{text}");
    assert!(text.contains("53 columns"), "the size:\n{text}");
    assert!(text.contains("built in"), "where it came from:\n{text}");

    // Every template in the catalogue, not just the first: a listing that
    // silently stopped after one would satisfy the assertions above.
    for name in ["dragon", "invader", "pulse", "wave"] {
        assert!(
            text.contains(name),
            "{name} is missing from the catalogue:\n{text}"
        );
    }

    // The thumbnail is the reason to run this rather than read a directory
    // listing, and a name with no picture beside it would satisfy every
    // assertion above.
    let blocks = text
        .lines()
        .filter(|line| line.contains('\u{2588}') || line.contains('\u{2593}'))
        .count();
    assert!(
        blocks >= 5,
        "expected a seven-row thumbnail, found {blocks} rows of blocks:\n{text}"
    );
    Ok(())
}

// ------------------------------------------------------- drawing a template

#[test]
fn a_template_draws_the_year_and_prices_every_level() -> termlens::Result<()> {
    let mut terminal = spawn(&[
        "--template",
        "dragon",
        "--year",
        "2027",
        "--plan",
        "/dev/null",
    ])?;
    // Not `wait_until("commits each")`: that line is the price table's header
    // and the contrast figure is printed after it, so the screen could be read
    // between the two. It flaked on macOS at four threads, which is exactly
    // what the stress workflow is for.
    terminal.wait_exit()?;
    let screen = terminal.screen();

    assert!(screen.contains("Dragon"), "{screen}");
    assert!(screen.contains("2027"), "{screen}");
    assert!(screen.contains("53 of 53 columns"), "{screen}");
    // The months come from the calendar renderer, so their presence says the
    // picture went through the same preview a text plan does.
    assert!(screen.contains("Jan") && screen.contains("Dec"), "{screen}");
    // Every level the dragon uses is priced, and level 0 is named as the one
    // that costs nothing and must stay that way.
    assert!(screen.contains("must stay dark"), "{screen}");
    // Derived from the template rather than hardcoded: which shades it draws
    // in is a design decision that has already changed once, and a test that
    // pins the decision rather than the property fails on the next redraw for
    // no reason at all.
    let dragon = mossaic::templates::find("dragon").expect("dragon");
    for level in dragon
        .canvas
        .palette()
        .into_iter()
        .filter(|level| *level > 0)
    {
        assert!(
            (0..screen.rows()).any(|row| screen
                .row_text(row)
                .trim_start()
                .starts_with(&level.to_string())),
            "level {level} is missing from the price table:\n{screen}"
        );
    }
    // And the honest contrast figure: the closest pair, not the widest.
    assert!(screen.contains("closest pair"), "{screen}");
    Ok(())
}

#[test]
fn an_unknown_template_lists_what_there_was() -> termlens::Result<()> {
    let mut terminal = spawn(&["--template", "wyvern", "--plan", "/dev/null"])?;
    terminal.wait_exit()?;
    let screen = terminal.screen();
    assert!(
        screen.contains("wyvern"),
        "names what was asked for:\n{screen}"
    );
    assert!(screen.contains("dragon"), "and what there is:\n{screen}");
    Ok(())
}

// ------------------------------------------------------- the editor

#[test]
fn the_editor_opens_on_the_year_and_says_which_keys_do_what() -> termlens::Result<()> {
    let mut terminal = spawn(&["--draw", "--year", "2027", "--plan", "/dev/null"])?;
    let screen = terminal.wait_frame(ready)?;

    assert!(screen.contains("untitled"), "{screen}");
    assert!(screen.contains("2027"), "{screen}");
    // The weekday gutter, which is what makes a column a week rather than a
    // number.
    for day in ["Sun", "Wed", "Sat"] {
        assert!(screen.contains(day), "{day} missing:\n{screen}");
    }
    // The HUD's whole reason to exist.
    assert!(screen.contains("commits in total"), "{screen}");
    assert!(screen.contains("level 0"), "{screen}");
    // A blank canvas has one shade, so there is nothing to compare yet — and it
    // says so rather than printing a meaningless contrast figure.
    assert!(screen.contains("one shade only"), "{screen}");
    Ok(())
}

#[test]
fn painting_shows_up_on_screen_and_in_the_numbers() -> termlens::Result<()> {
    let mut terminal = spawn(&["--draw", "--year", "2027", "--plan", "/dev/null"])?;
    terminal.wait_until(ready)?;

    // Four level-4 days, on a row the year definitely holds.
    for _ in 0..10 {
        terminal.send(Key::Char('l'))?;
    }
    for _ in 0..3 {
        terminal.send(Key::Char('j'))?;
    }
    for _ in 0..4 {
        terminal.send(Key::Char('4'))?;
        terminal.send(Key::Char('l'))?;
    }
    let screen = terminal.wait_frame(|screen| screen.contains("level 4     4 days"))?;

    // Four days at level 4 cost four commits each at the default --commits.
    assert!(
        screen.contains("16 commits in total"),
        "the estimate follows the drawing:\n{screen}"
    );
    // And with two shades on the canvas there is a contrast to report.
    assert!(
        screen.contains("closest shades"),
        "the legibility line appears once there are two shades:\n{screen}"
    );
    assert!(!screen.contains("one shade only"), "{screen}");
    Ok(())
}

#[test]
fn undo_takes_the_drawing_back_and_says_when_there_is_no_more() -> termlens::Result<()> {
    let mut terminal = spawn(&["--draw", "--year", "2027", "--plan", "/dev/null"])?;
    terminal.wait_until(ready)?;

    terminal.send(Key::Char('3'))?;
    terminal.wait_until(|screen| screen.contains("level 3     1 day "))?;
    terminal.send(Key::Char('u'))?;
    let screen = terminal.wait_frame(|screen| screen.contains("undone"))?;
    assert!(screen.contains("level 3     0 days"), "{screen}");

    terminal.send(Key::Char('u'))?;
    terminal.wait_until(|screen| screen.contains("nothing to undo"))?;
    Ok(())
}

#[test]
fn the_help_overlay_lists_the_keys_and_closes_again() -> termlens::Result<()> {
    let mut terminal = spawn(&["--draw", "--year", "2027", "--plan", "/dev/null"])?;
    terminal.wait_until(ready)?;

    terminal.send(Key::Char('?'))?;
    let screen = terminal.wait_frame(|screen| screen.contains("Drawing on the year"))?;
    assert!(screen.contains("undo"), "{screen}");
    assert!(screen.contains("right-click clears"), "{screen}");

    terminal.send(Key::Char('?'))?;
    terminal.wait_until(|screen| !screen.contains("Drawing on the year"))?;
    Ok(())
}

#[test]
fn a_template_can_be_opened_edited_and_saved() -> termlens::Result<()> {
    let output = scratch("edited.art");
    let _ = std::fs::remove_file(&output);
    let mut terminal = spawn(&[
        "--template",
        "dragon",
        "--draw",
        "--year",
        "2027",
        "--plan",
        "/dev/null",
        "-o",
        output.to_str().expect("a UTF-8 path"),
    ])?;
    let screen = terminal.wait_frame(ready)?;
    assert!(screen.contains("Dragon"), "opened the template:\n{screen}");
    assert!(screen.contains("53x7"), "{screen}");

    terminal.send(Key::Char('s'))?;
    terminal.wait_until(|screen| screen.contains("saved"))?;
    terminal.send(Key::Char('q'))?;
    terminal.wait_exit()?;

    let body = std::fs::read_to_string(&output).expect("the saved picture");
    let canvas = mossaic::art::Canvas::parse(&body).expect("it is a canvas");
    assert_eq!(canvas.width(), 53);
    assert_eq!(
        canvas,
        mossaic::templates::find("dragon").expect("dragon").canvas,
        "saved unchanged, because nothing was drawn on it"
    );
    let _ = std::fs::remove_file(&output);
    Ok(())
}

#[test]
fn quitting_puts_the_terminal_back() -> termlens::Result<()> {
    let mut terminal = spawn(&["--draw", "--year", "2027", "--plan", "/dev/null"])?;
    terminal.wait_until(ready)?;
    terminal.send(Key::Char('q'))?;
    let status = terminal.wait_exit()?;
    assert!(status.success(), "left cleanly: {status}");
    Ok(())
}

#[test]
fn an_unsaved_drawing_is_not_lost_quietly() -> termlens::Result<()> {
    let mut terminal = spawn(&["--draw", "--year", "2027", "--plan", "/dev/null"])?;
    terminal.wait_until(ready)?;
    terminal.send(Key::Char('4'))?;
    terminal.wait_until(|screen| screen.contains("level 4     1 day "))?;
    terminal.send(Key::Char('q'))?;
    terminal.wait_exit()?;

    // Printed after the alternate screen is given back, so it is readable
    // rather than painted over on the way out.
    let screen = terminal.screen();
    assert!(
        screen.contains("not saved"),
        "leaving with unsaved work should say so:\n{screen}"
    );
    Ok(())
}

/// A day the picture does not cover is **free**, and the report has to say so.
///
/// Both a day the picture wants dark and a day outside it have a `need` of
/// zero, and the advice for them is opposite: one must be left alone or the
/// drawing is damaged, the other is nobody's business. Reading the number
/// rather than the intent told people to stay dark on every day after the
/// picture ends.
#[test]
fn a_day_outside_the_picture_is_free_not_dark() -> termlens::Result<()> {
    let art = scratch("outside.art");
    // Three lit columns, placed early, so a day late in the year is plainly
    // outside them.
    std::fs::write(
        &art,
        "# name: Bar\n# description: three columns\n\n444\n444\n444\n444\n444\n444\n444\n",
    )
    .expect("write");

    let calendar = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("art/vyncint-2027.json");
    let mut terminal = spawn(&[
        "--matrix",
        art.to_str().expect("a UTF-8 path"),
        "--year",
        "2027",
        "--start-week",
        "2",
        "--track",
        "--merge",
        calendar.to_str().expect("a UTF-8 path"),
        // Deep in the year and nowhere near columns 2 to 4 — **and a day that
        // already has contributions**. That second half is the whole test: a
        // day outside the picture with nothing on it is not recorded at all,
        // so it takes the "no entry" path and never reaches the branch this
        // is about. The first version of this test used a quiet day and
        // passed against the bug it was written for.
        "--today",
        "2027-06-15",
        "--plan",
        "/dev/null",
        "--no-color",
    ])?;
    terminal.wait_exit()?;
    let screen = terminal.screen();

    assert!(
        screen.contains("outside the picture"),
        "a day the picture does not reach is free:\n{screen}"
    );
    assert!(
        !screen.contains("today        must stay dark"),
        "and must not be told to stay dark:\n{screen}"
    );
    let _ = std::fs::remove_file(&art);
    Ok(())
}

/// The other half: a day the picture *does* want dark still says so.
#[test]
fn a_dark_day_inside_the_picture_still_says_stay_dark() -> termlens::Result<()> {
    let art = scratch("inside.art");
    // A hole in the middle of a block: row 3, column 1 is dark.
    std::fs::write(
        &art,
        "# name: Ring\n# description: a hole in the middle\n\n444\n444\n444\n404\n444\n444\n444\n",
    )
    .expect("write");

    let calendar = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("art/vyncint-2027.json");
    // 2027-01-13 is the Wednesday of calendar column 2, which is the dark cell.
    let mut terminal = spawn(&[
        "--matrix",
        art.to_str().expect("a UTF-8 path"),
        "--year",
        "2027",
        "--start-week",
        "1",
        "--track",
        "--merge",
        calendar.to_str().expect("a UTF-8 path"),
        "--today",
        "2027-01-13",
        "--plan",
        "/dev/null",
        "--no-color",
    ])?;
    terminal.wait_exit()?;
    let screen = terminal.screen();
    assert!(
        screen.contains("must stay dark"),
        "the hole in the ring is still a day to leave alone:\n{screen}"
    );
    let _ = std::fs::remove_file(&art);
    Ok(())
}
