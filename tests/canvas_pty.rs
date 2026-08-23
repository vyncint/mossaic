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
    let screen = terminal.screen();

    assert!(screen.contains("dragon"), "the name:\n{screen}");
    assert!(screen.contains("Dragon"), "the title:\n{screen}");
    assert!(screen.contains("@vyncint"), "the author:\n{screen}");
    assert!(screen.contains("53 columns"), "the size:\n{screen}");
    assert!(screen.contains("built in"), "where it came from:\n{screen}");

    // The thumbnail is the reason to run this rather than read a directory
    // listing, and a name with no picture beside it would satisfy every
    // assertion above.
    let blocks = (0..screen.rows())
        .filter(|row| {
            let text = screen.row_text(*row);
            text.contains('\u{2588}') || text.contains('\u{2593}')
        })
        .count();
    assert!(
        blocks >= 5,
        "expected a seven-row thumbnail, found {blocks} row(s) of blocks:\n{screen}"
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
    let screen = terminal.wait_frame(|screen| screen.contains("level 4     4 day(s)"))?;

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
    terminal.wait_until(|screen| screen.contains("level 3     1 day(s)"))?;
    terminal.send(Key::Char('u'))?;
    let screen = terminal.wait_frame(|screen| screen.contains("undone"))?;
    assert!(screen.contains("level 3     0 day(s)"), "{screen}");

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
    terminal.wait_until(|screen| screen.contains("level 4     1 day(s)"))?;
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
