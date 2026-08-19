//! The `mossaic-art` binary, run as a user runs it.
//!
//! No PTY here: `mossaic-art` prints and exits, so this drives it the way a shell does
//! and asserts on what comes back. Everything is offline — `--merge` reads a
//! saved calendar, so nothing needs `gh`.

use std::path::Path;
use std::process::{Command, Output};

fn art(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mossaic-art"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(args)
        .output()
        .expect("the art binary runs")
}

/// A scratch path in the temp directory that no other process will touch.
///
/// The names here used to be constants, which made them shared state: a stress
/// loop beside an ordinary `cargo test`, or two people on one machine, raced
/// over the same file and failed in ways that looked like the code rather than
/// like the harness. CONTRIBUTING.md §3 asks for hermetic tests, and a fixed
/// global path is not one.
fn scratch(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("mossaic-{}-{name}", std::process::id()))
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn the_font_can_be_looked_at() {
    let out = art(&["--font", "--no-colour"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("39 glyphs, 5x5 each"), "{text}");
    assert!(text.contains("add one to FONT in src/art.rs"), "{text}");
    // Every character it claims, drawn.
    for character in ['A', 'Z', '0', '9', '-', '.'] {
        assert!(
            text.contains(character),
            "the font view is missing {character}"
        );
    }
    assert!(
        text.contains("space"),
        "including the one with no glyph: {text}"
    );
}

#[test]
fn an_unknown_character_says_what_there_is() {
    let out = art(&["HI+THERE", "--year", "2027"]);
    assert!(!out.status.success(), "it should refuse rather than guess");
    let error = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(error.contains('+'), "{error}");
    assert!(error.contains("ABCDEFGHIJKLMNOPQRSTUVWXYZ"), "{error}");
}

#[test]
fn tracking_a_finished_year_says_so() {
    // art/vyncint-2027.json is VYNCINT drawn on an otherwise empty year, at the
    // placement centring gives — so tracking it back is a completed plan.
    //
    // `--today` is pinned because the last assertion is about a year that has
    // not started, which the clock would make true only until it did. Every
    // test that reads a report pins it, for the same reason.
    let out = art(&[
        "VYNCINT",
        "--year",
        "2027",
        "--track",
        "--merge",
        "art/vyncint-2027.json",
        "--today",
        "2026-08-19",
        "--no-colour",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = stdout(&out);
    assert!(text.contains("VYNCINT is drawn."), "{text}");
    assert!(text.contains("75 of 75 bright"), "{text}");
    assert!(!text.contains("cannot be drawn"), "{text}");
    // A year still ahead has no advice for this afternoon.
    assert!(text.contains("2027 has not started"), "{text}");
}

#[test]
fn tracking_a_busy_year_says_why_it_cannot_be_drawn() {
    // The same text over a year that already has contributions of its own: the
    // letters can be brightened, but the days inside them that are already lit
    // are permanent.
    let out = art(&[
        "VYNCINT",
        "--year",
        "2026",
        "--track",
        "--merge",
        "art/vyncint-2026.json",
        "--no-colour",
        "--today",
        "2026-08-19",
        "--start-week",
        "1",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = stdout(&out);
    assert!(
        text.contains("VYNCINT cannot be drawn cleanly in 2026."),
        "{text}"
    );
    assert!(
        text.contains("inside the letters already have contributions"),
        "and it should say why:\n{text}"
    );
    assert!(
        text.contains("a letter day has to reach"),
        "and what a day costs:\n{text}"
    );
    // The verdict is a fact about the data, not a mood: the same run twice says
    // the same thing.
    assert_eq!(
        text,
        stdout(&art(&[
            "VYNCINT",
            "--year",
            "2026",
            "--track",
            "--merge",
            "art/vyncint-2026.json",
            "--no-colour",
            "--today",
            "2026-08-19",
            "--start-week",
            "1",
        ]))
    );
}

#[test]
fn tracking_without_a_calendar_explains_itself() {
    // No --merge and no `gh` reachable: it has to say what it needed, not fail
    // with whatever the process error happened to be.
    let out = Command::new(env!("CARGO_BIN_EXE_mossaic-art"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("PATH", "/nonexistent")
        .args(["VYNCINT", "--year", "2026", "--track"])
        .output()
        .expect("the art binary runs");
    assert!(!out.status.success());
    let error = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        error.contains("--track USER") || error.contains("gh auth login"),
        "{error}"
    );
}

#[test]
fn a_snapshot_round_trips_through_the_chart() {
    let out = scratch("art-cli.json");
    let path = out.to_string_lossy().into_owned();
    let made = art(&["HI", "--year", "2027", "--snapshot", &path, "--no-colour"]);
    assert!(
        made.status.success(),
        "{}",
        String::from_utf8_lossy(&made.stderr)
    );
    assert!(
        stdout(&made).contains("mossaic --file"),
        "it says what to do next"
    );
    assert!(Path::new(&path).exists());

    // And the chart renders it without a terminal.
    let png = scratch("art-cli.png");
    let drawn = Command::new(env!("CARGO_BIN_EXE_mossaic"))
        .args(["--file", &path, "--png", &png.to_string_lossy()])
        .output()
        .expect("the chart runs");
    assert!(
        drawn.status.success(),
        "{}",
        String::from_utf8_lossy(&drawn.stderr)
    );
    assert!(png.exists());
    let _ = std::fs::remove_file(&out);
    let _ = std::fs::remove_file(&png);
}

#[test]
fn the_help_lists_every_flag_the_parser_takes() {
    // Derived from the parser rather than from a list kept alongside it: adding
    // a flag and forgetting the help is exactly the mistake this catches, and a
    // hand-written list would have to be remembered too.
    let source = include_str!("../src/bin/mossaic-art.rs");
    // Every quoted token in a match arm, not just the first: an arm like
    // `"-h" | "--help" =>` documents two flags, and only checking one of them
    // is how `--help` itself went unverified.
    let arms: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('"') && line.contains("=>"))
        .flat_map(|line| line.split('"').skip(1).step_by(2))
        .filter(|token| token.starts_with('-'))
        .collect();
    assert!(
        arms.len() > 15,
        "only found {} flags — did the parser move?",
        arms.len()
    );

    let help = stdout(&art(&["--help"]));
    for flag in arms {
        assert!(help.contains(flag), "--help never mentions {flag}");
    }
    assert!(
        help.contains("examples:"),
        "and it should show how, not just what"
    );
    // The thing that removes the trap: a saved plan.
    assert!(help.contains("--save"), "{help}");
    assert!(help.contains("mossaic-art --track"), "{help}");
}

#[test]
fn the_report_is_machine_readable() {
    let out = art(&[
        "VYNCINT",
        "--year",
        "2026",
        "--track",
        "--merge",
        "art/vyncint-2026.json",
        "--start-week",
        "1",
        "--today",
        "2026-08-19",
        "--format",
        "json",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout(&out)).expect("--format json emits json");

    // The fields the GitHub Action reads. A rename here breaks every workflow
    // in the wild, so they are pinned by name.
    for field in [
        "text",
        "year",
        "source",
        "start_week",
        "columns",
        "year_total",
        "peak",
        "need_per_day",
        "letters",
        "bright",
        "owing_days",
        "owing_commits",
        "holes",
        "around",
        "verdict",
        "headline",
        "ahead_days",
        "overdue_days",
    ] {
        assert!(!json[field].is_null(), "the report lost `{field}`");
    }
    assert_eq!(json["text"], "VYNCINT");
    assert_eq!(json["year"], 2026);
    assert_eq!(json["verdict"], "holed");
    assert_eq!(json["letters"], 75);
    assert!(json["holes"].as_u64().unwrap() > 0);
    // The headline is carried, not reconstructed downstream.
    let headline = json["headline"].as_str().unwrap();
    assert!(headline.contains("VYNCINT"), "{headline}");
    assert!(headline.contains("hole"), "{headline}");

    // Markdown says the same things, in a shape a message can carry.
    let md = stdout(&art(&[
        "VYNCINT",
        "--year",
        "2026",
        "--track",
        "--merge",
        "art/vyncint-2026.json",
        "--start-week",
        "1",
        "--today",
        "2026-08-19",
        "--format",
        "markdown",
    ]));
    assert!(md.starts_with("### VYNCINT · 2026"), "{md}");
    assert!(md.contains("Cannot be drawn cleanly"), "{md}");
    assert!(md.contains("| letters bright | 75 of 75 |"), "{md}");
    assert!(
        !md.contains('\x1b'),
        "a message body carries no escape codes:\n{md}"
    );
}

#[test]
fn an_unknown_format_is_refused() {
    let out = art(&["VYNCINT", "--year", "2027", "--track", "--format", "yaml"]);
    assert!(!out.status.success());
    let error = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(error.contains("text, json or markdown"), "{error}");
}

#[test]
fn a_saved_plan_makes_the_flags_optional() {
    let dir = scratch("plan-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_mossaic-art"))
            .current_dir(&dir)
            .args(args)
            .output()
            .expect("the art binary runs")
    };

    // With no plan and no text, it says what to do rather than what went wrong.
    let lost = run(&["--track"]);
    assert!(!lost.status.success());
    let hint = String::from_utf8_lossy(&lost.stderr).into_owned();
    assert!(hint.contains("--save"), "{hint}");
    assert!(hint.contains("no plan at"), "{hint}");

    // Save one. The placement is stored resolved, not as typed: this run never
    // said --start-week, and centring is what has to survive.
    let saved = run(&["VYNCINT", "--year", "2027", "--save", "--no-colour"]);
    assert!(
        saved.status.success(),
        "{}",
        String::from_utf8_lossy(&saved.stderr)
    );
    assert!(
        stdout(&saved).contains("mossaic-art --track"),
        "it says what is next"
    );
    let spec: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("mossaic-plan.json")).unwrap())
            .unwrap();
    assert_eq!(spec["text"], "VYNCINT");
    assert_eq!(spec["year"], 2027);
    assert_eq!(spec["start_week"], 6, "the centred column, resolved");
    assert_eq!(spec["top"], 1);

    // Now the flags are optional, and the plan is the same one.
    let tracked = run(&["--track", "--merge", "../../nonexistent.json"]);
    let error = String::from_utf8_lossy(&tracked.stderr).into_owned();
    assert!(
        error.contains("nonexistent.json"),
        "it should have got as far as reading the calendar: {error}"
    );

    // A typed flag still wins over the saved one.
    let overridden = run(&["--year", "2028", "--no-colour"]);
    assert!(
        stdout(&overridden).contains("2028"),
        "{}",
        stdout(&overridden)
    );
    assert!(
        !stdout(&overridden).contains("· 2027 ·"),
        "{}",
        stdout(&overridden)
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_binary_answers_the_basics() {
    for (name, binary) in [
        ("mossaic", env!("CARGO_BIN_EXE_mossaic")),
        ("mossaic-art", env!("CARGO_BIN_EXE_mossaic-art")),
        ("mossaic-glyphs", env!("CARGO_BIN_EXE_mossaic-glyphs")),
    ] {
        for flag in ["--help", "-h", "--version", "-V"] {
            let out = Command::new(binary).arg(flag).output().expect("runs");
            assert!(out.status.success(), "{name} {flag} failed");
            let text = String::from_utf8_lossy(&out.stdout).into_owned();
            assert!(!text.trim().is_empty(), "{name} {flag} printed nothing");
            if flag.contains("version") || flag == "-V" {
                assert!(
                    text.contains(env!("CARGO_PKG_VERSION")),
                    "{name} {flag} does not print the version: {text}"
                );
                assert!(text.starts_with(name), "{name} {flag} says: {text}");
            }
        }
    }
}

#[test]
fn bad_input_is_refused_the_same_way_by_every_binary() {
    // The two CLIs used to disagree about a year: one range-checked it, the
    // other passed 999999 through to a panic in the calendar.
    for binary in [
        env!("CARGO_BIN_EXE_mossaic"),
        env!("CARGO_BIN_EXE_mossaic-art"),
    ] {
        for bad in ["abc", "999999", "-5", "2101", "0"] {
            let out = Command::new(binary)
                .args(["HI", "--year", bad])
                .output()
                .expect("runs");
            let error = String::from_utf8_lossy(&out.stderr).into_owned();
            assert!(!out.status.success(), "{binary} accepted --year {bad}");
            assert!(
                error.contains("wants a year between 2000 and 2100"),
                "{binary} --year {bad}: {error}"
            );
            assert!(
                !error.contains("panicked"),
                "{binary} --year {bad} panicked"
            );
        }
        // And a missing value is a missing value, not the next flag.
        let out = Command::new(binary)
            .args(["--year"])
            .output()
            .expect("runs");
        assert!(String::from_utf8_lossy(&out.stderr).contains("--year needs a value"));
    }
}

#[test]
fn colour_follows_the_convention() {
    // Piped: no escapes. NO_COLOR: no escapes even when forced by a tty. And
    // --color always: escapes regardless.
    let piped = stdout(&art(&["--font"]));
    assert!(!piped.contains('\x1b'), "a pipe should get no colour");

    let forced = Command::new(env!("CARGO_BIN_EXE_mossaic-art"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["--font", "--color", "always"])
        .output()
        .expect("runs");
    assert!(
        String::from_utf8_lossy(&forced.stdout).contains('\x1b'),
        "--color always should colour a pipe"
    );

    let no_color = Command::new(env!("CARGO_BIN_EXE_mossaic-art"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("NO_COLOR", "1")
        .args(["--font", "--color", "auto"])
        .output()
        .expect("runs");
    assert!(!String::from_utf8_lossy(&no_color.stdout).contains('\x1b'));

    let bad = Command::new(env!("CARGO_BIN_EXE_mossaic-art"))
        .args(["--font", "--color", "chartreuse"])
        .output()
        .expect("runs");
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("auto, always or never"));
}

#[test]
fn a_background_is_drawn_as_a_shade_and_priced_as_one() {
    let out = art(&[
        "VYNCINT",
        "--year",
        "2027",
        "--background",
        "1",
        "--no-colour",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = stdout(&out);

    // The two shades, and what a reader will make of them.
    assert!(
        text.contains("background level 1 under letters at level 4"),
        "{text}"
    );
    assert!(text.contains("290 background day(s), 1 each"), "{text}");
    assert!(text.contains("clear"), "level 1 under 4 is clear: {text}");
    assert!(!text.contains("faint"), "{text}");

    // 75 letter days at 4 plus 290 background days at 1.
    assert!(text.contains("590 commits"), "{text}");

    // The preview has to show three states now, not two, or there is no way to
    // check the art before making a single commit.
    assert!(text.contains('░'), "the field: {text}");
    assert!(text.contains('█'), "the letters: {text}");
}

#[test]
fn neighbouring_shades_are_drawn_but_said_to_be_faint() {
    let out = art(&["VYNCINT", "--year", "2027", "--bg", "3", "--no-colour"]);
    assert!(out.status.success(), "faint is a warning, not a refusal");
    let text = stdout(&out);
    assert!(text.contains("faint"), "{text}");
    assert!(
        text.contains("neighbouring shades"),
        "it should say why: {text}"
    );
    assert!(
        text.contains("--background 2"),
        "and what to do instead: {text}"
    );
}

#[test]
fn a_background_that_would_hide_the_letters_is_refused() {
    // Equal shades draw nothing at all, so this is an error rather than a note.
    let same = art(&["VYNCINT", "--year", "2027", "--background", "4"]);
    assert!(!same.status.success());
    assert_eq!(same.status.code(), Some(2));
    let error = String::from_utf8_lossy(&same.stderr).into_owned();
    assert!(error.contains("darker"), "{error}");

    let wild = art(&["VYNCINT", "--year", "2027", "--background", "7"]);
    assert!(!wild.status.success());
    let error = String::from_utf8_lossy(&wild.stderr).into_owned();
    assert!(error.contains("between 0 and 4"), "{error}");

    // The subtle one: the shades are fine, but --commits is too small for the
    // year to hold both of them, so the letters would come out the same colour
    // as the field.
    let cramped = art(&[
        "VYNCINT",
        "--year",
        "2027",
        "--background",
        "1",
        "--commits",
        "1",
    ]);
    assert!(!cramped.status.success());
    let error = String::from_utf8_lossy(&cramped.stderr).into_owned();
    assert!(error.contains("would not show"), "{error}");
    assert!(
        error.contains("at least 2 commits"),
        "it should quote the fix: {error}"
    );
}

#[test]
fn tracking_reports_the_letters_and_the_field_apart() {
    // The saved calendar is VYNCINT at four commits a day on an otherwise empty
    // year: the letters are finished, the background has not been started.
    let out = art(&[
        "VYNCINT",
        "--year",
        "2027",
        "--track",
        "--merge",
        "art/vyncint-2027.json",
        "--background",
        "1",
        "--today",
        "2027-06-01",
        "--no-colour",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = stdout(&out);

    assert!(
        text.contains("75 of 75 bright"),
        "the letters are done: {text}"
    );
    assert!(
        text.contains("0 of 290 at level 1"),
        "and the field is not: {text}"
    );
    assert!(
        text.contains("the shades"),
        "the plan says which two shades it means: {text}"
    );
    assert!(
        text.contains("a background day has to reach 1"),
        "and what one costs: {text}"
    );
    assert!(
        !text.contains("VYNCINT is drawn."),
        "finished letters are not a finished picture: {text}"
    );
    // The bug this guards: the verdict used to count only the letters, and so
    // announced "0 contributions to go" with the whole field still bare.
    assert!(
        text.contains("0 for the letters, 290 for the field"),
        "the verdict has to count both: {text}"
    );
}

#[test]
fn a_saved_plan_remembers_the_background() {
    let dir = scratch("plan-background-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_mossaic-art"))
            .current_dir(&dir)
            .args(args)
            .output()
            .expect("the art binary runs")
    };

    let saved = run(&[
        "VYNCINT",
        "--year",
        "2027",
        "--background",
        "2",
        "--save",
        "--no-colour",
    ]);
    assert!(
        saved.status.success(),
        "{}",
        String::from_utf8_lossy(&saved.stderr)
    );
    let spec: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("mossaic-plan.json")).unwrap())
            .unwrap();
    assert_eq!(spec["background"], 2, "the shade is part of the plan");

    // Later runs need no flags, and get the same picture.
    let again = stdout(&run(&["--no-colour"]));
    assert!(
        again.contains("background level 2 under letters at level 4"),
        "{again}"
    );

    // A plan written before backgrounds existed still loads, as no background.
    std::fs::write(
        dir.join("mossaic-plan.json"),
        r#"{"text":"HI","year":2027,"start_week":10,"top":1,"commits":4,"user":null}"#,
    )
    .unwrap();
    let old = run(&["--no-colour"]);
    assert!(
        old.status.success(),
        "{}",
        String::from_utf8_lossy(&old.stderr)
    );
    assert!(
        !stdout(&old).contains("background level"),
        "an old plan draws no background: {}",
        stdout(&old)
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_background_check_meant_for_drawing_does_not_block_tracking() {
    // `--commits` is what `--write` puts on a lit day. Tracking never writes
    // one — it works out what a letter day needs from the year's real peak —
    // so the "these shades cannot be told apart" guard must not fire there.
    //
    // The regression: over a busy year, `--track --merge --background` was
    // refused with "the letters would not show", telling the user their plan
    // was impossible when the only thing wrong was a flag that run ignores.
    // The GitHub Action never hit it (it passes no --merge), so the two
    // disagreed about the same plan.
    let tracking = art(&[
        "VYNCINT",
        "--year",
        "2026",
        "--start-week",
        "6",
        "--background",
        "1",
        "--commits",
        "4",
        "--track",
        "--merge",
        "art/vyncint-2026.json",
        "--today",
        "2026-08-19",
        "--no-colour",
    ]);
    assert!(
        tracking.status.success(),
        "tracking must not be refused over --commits: {}",
        String::from_utf8_lossy(&tracking.stderr)
    );
    let text = stdout(&tracking);
    assert!(
        text.contains("letters at level 4, background at level 1"),
        "and it reports the shades the plan actually wants: {text}"
    );

    // The same flags without --track *would* draw, and there the guard is
    // right: 4 commits in a year peaking in the hundreds is level 1, the same
    // as the background, so the letters would be invisible.
    let drawing = art(&[
        "VYNCINT",
        "--year",
        "2026",
        "--start-week",
        "6",
        "--background",
        "1",
        "--commits",
        "4",
        "--merge",
        "art/vyncint-2026.json",
        "--no-colour",
    ]);
    assert!(
        !drawing.status.success(),
        "drawing with indistinguishable shades is still refused"
    );
    let error = String::from_utf8_lossy(&drawing.stderr).into_owned();
    assert!(error.contains("would not show"), "{error}");
}

#[test]
fn a_hole_is_called_a_hole_not_a_background_at_level_zero() {
    // With no background there is no field, so describing a spoiled day as
    // "background — 113 contributions, 113 too many for level 0" names a shade
    // the plan never asked for. Inside the letters it is a hole, and that is
    // what it has to say — the markdown report always got this right, so the
    // text report was the one disagreeing.
    //
    // 2026-08-19 is a lit day inside the letters in the calendar this reads, and
    // it is pinned rather than left to the clock. Without `--today` this test
    // asserted that whatever day it happened to run on was one of those, which
    // held on the day it was written and failed the morning after.
    let out = art(&[
        "VYNCINT",
        "--year",
        "2026",
        "--track",
        "--merge",
        "art/vyncint-2026.json",
        "--today",
        "2026-08-19",
        "--no-colour",
    ]);
    assert!(out.status.success());
    let text = stdout(&out);
    let today = text
        .lines()
        .find(|line| line.trim_start().starts_with("today"))
        .expect("a year under way reports on today");
    assert!(
        !today.contains("level 0"),
        "there is no level-0 background to be over: {today}"
    );
    assert!(
        today.contains("permanent hole"),
        "a lit day inside the letters is a hole: {today}"
    );
}

#[test]
fn a_day_that_must_stay_dark_says_so_before_it_arrives() {
    // 2026-08-20 sits inside VYNCINT's block and is not part of a letter, so a
    // contribution on it punches a hole nothing takes back. Every surface used
    // to call it free — "not part of the text — anything you commit today
    // shows" — because a clean day inside the text was dropped from the plan
    // for having nothing to say. It has the most to say of any day in the year.
    let args = |format: &'static str| -> Vec<&'static str> {
        vec![
            "VYNCINT",
            "--year",
            "2026",
            "--track",
            "--merge",
            "art/vyncint-2026.json",
            "--today",
            "2026-08-20",
            "--no-colour",
            "--format",
            format,
        ]
    };

    let text = stdout(&art(&args("text")));
    let today = text
        .lines()
        .find(|line| line.trim_start().starts_with("today"))
        .expect("a year under way reports on today");
    assert!(
        today.contains("keep it dark"),
        "the instruction has to be the instruction: {today}"
    );
    assert!(
        !today.contains("anything committed on it shows"),
        "and never the opposite of it: {today}"
    );
    // The seven-day schedule is where it is read a day early, which is the only
    // time the warning is worth anything.
    assert!(
        text.contains("Thu Aug 20   keep dark"),
        "the schedule names it too:\n{text}"
    );

    // And a machine reading the report gets the same answer, with a ceiling of
    // zero rather than the `null` that said "nothing is asked of this day".
    let json: serde_json::Value = serde_json::from_str(&stdout(&art(&args("json")))).expect("json");
    assert_eq!(json["today"]["kind"], "keep-dark");
    assert_eq!(json["today"]["ceiling"], 0);
    assert_eq!(json["today"]["over"], 0, "clean, so not yet a hole");
    // The count of real holes must not move: a day that must stay dark and has
    // is not damage.
    assert_eq!(json["holes"], 61);

    let md = stdout(&art(&args("markdown")));
    assert!(
        md.contains("| today | inside the letters — keep it dark |"),
        "{md}"
    );
}

#[test]
fn today_is_an_input_so_a_report_is_reproducible() {
    // The whole reason `--today` exists: the answer to "what does today owe"
    // has to be a fact about a calendar and a date, not about when the command
    // ran. Two different dates, two different answers, both stable forever.
    let on = |date: &str| {
        stdout(&art(&[
            "VYNCINT",
            "--year",
            "2026",
            "--track",
            "--merge",
            "art/vyncint-2026.json",
            "--today",
            date,
            "--no-colour",
        ]))
    };

    let lit = on("2026-08-19");
    assert!(
        lit.contains("today       Wed Aug 19"),
        "the date it was told, not the date it is: {lit}"
    );
    let dark = on("2026-08-20");
    assert!(dark.contains("today       Thu Aug 20"), "{dark}");
    assert_ne!(
        lit, dark,
        "two days inside the same block, reported differently"
    );
    // Run twice, byte for byte the same — which is what the clock never was.
    assert_eq!(lit, on("2026-08-19"));

    // A date the plan's year does not hold is not an answer about that plan.
    // Tracking 2026 in 2027 used to report on a day in 2027 and call it
    // `outside`, which is true of the wrong calendar.
    let out = art(&[
        "VYNCINT",
        "--year",
        "2026",
        "--track",
        "--merge",
        "art/vyncint-2026.json",
        "--today",
        "2027-03-01",
        "--format",
        "json",
    ]);
    let json: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("json");
    assert!(json["today"].is_null(), "{}", json["today"]);
    assert!(json["tomorrow"].is_null(), "{}", json["tomorrow"]);
    // And it says so, because `--today` carries no year of its own: the whole
    // "what to do next" half of the report disappears otherwise, which looks
    // like a bug rather than like an answer.
    let note = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(note.contains("2027-03-01 is after 2026"), "{note}");
    assert!(
        note.contains("no today or tomorrow"),
        "and why that matters: {note}"
    );

    for bad in ["notadate", "2026-13-01", "1999-01-01"] {
        let out = art(&["VYNCINT", "--track", "--today", bad]);
        assert!(!out.status.success(), "--today {bad} was accepted");
        assert_eq!(out.status.code(), Some(2));
    }
}

#[test]
fn numeric_flags_are_bounded_rather_than_cast() {
    // `--year` was range-checked because one binary once passed 999999 through
    // to a panic. Every other number took the unguarded path and `as` is not a
    // check: `--commits -1` came out as 4,294,967,295 — which `--write` would
    // then try to build a fast-import stream for — and `--start-week -1`
    // reached `usize::MAX`, where building a date from it panicked.
    for (flag, value, wanted) in [
        ("--commits", "-1", "between 1 and 1000000"),
        ("--commits", "0", "between 1 and 1000000"),
        ("--top", "-1", "between 0 and 2"),
        ("--top", "9", "between 0 and 2"),
        ("--start-week", "-1", "between 0 and 60"),
        ("--background", "-1", "between 0 and 4"),
    ] {
        let out = art(&["VYNCINT", "--year", "2027", flag, value]);
        let error = String::from_utf8_lossy(&out.stderr).into_owned();
        assert!(!out.status.success(), "{flag} {value} was accepted");
        assert_eq!(out.status.code(), Some(2), "{flag} {value}");
        assert!(error.contains(wanted), "{flag} {value}: {error}");
        assert!(!error.contains("panicked"), "{flag} {value}: {error}");
    }

    // In range for the flag but not for the year: the tight bound needs both
    // the calendar and the text, so `place` is what refuses it — and it names
    // the last column that would have fitted rather than drawing nothing.
    let out = art(&["VYNCINT", "--year", "2027", "--start-week", "20"]);
    assert!(!out.status.success());
    let error = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(error.contains("past the end of 2027"), "{error}");
    assert!(error.contains("the last one that fits is 12"), "{error}");
}

#[test]
fn a_saved_plan_remembers_who_to_track() {
    // The plan stores whose contributions to compare against, and `--track`
    // threw it away: the user was read out of the file and then overwritten
    // with the nothing a bare `--track` carries. With `gh` off PATH the loss is
    // visible; with `gh` present it was worse than visible, because it tracked
    // the authenticated user while reporting against someone else's plan.
    let dir = scratch("plan-user-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("mossaic-plan.json"),
        r#"{"text":"HI","year":2026,"start_week":10,"top":1,"commits":4,
            "background":0,"user":"octocat"}"#,
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_mossaic-art"))
        .current_dir(&dir)
        .env("PATH", "/nonexistent")
        .args(["--track"])
        .output()
        .expect("the art binary runs");
    let error = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        error.contains("octocat"),
        "the plan named who to track: {error}"
    );
    assert!(
        !error.contains("could not tell whose"),
        "it knew all along — it just threw the answer away: {error}"
    );

    // A user typed on the command line still wins over the saved one.
    let typed = Command::new(env!("CARGO_BIN_EXE_mossaic-art"))
        .current_dir(&dir)
        .env("PATH", "/nonexistent")
        .args(["--track", "someone-else"])
        .output()
        .expect("the art binary runs");
    let error = String::from_utf8_lossy(&typed.stderr).into_owned();
    assert!(error.contains("someone-else"), "{error}");
    assert!(!error.contains("octocat"), "{error}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_calendar_from_another_year_says_so() {
    // Every day of a 2026 calendar falls outside a 2027 grid, so it filters
    // down to nothing and everything downstream is correct about an empty year:
    // 9,527 contributions read as none, and a plan that cannot be drawn reads
    // as reachable at one commit a day. Silence there is the same mistake the
    // plan file was introduced to prevent.
    let out = art(&[
        "VYNCINT",
        "--year",
        "2027",
        "--track",
        "--merge",
        "art/vyncint-2026.json",
        "--today",
        "2027-06-01",
        "--no-colour",
    ]);
    assert!(out.status.success(), "a warning, not a refusal");
    let error = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(error.contains("holds no contributions in 2027"), "{error}");
    assert!(error.contains("it covers 2026"), "{error}");
    assert_eq!(
        error
            .lines()
            .filter(|line| line.contains("holds no"))
            .count(),
        1,
        "said once, not once per read: {error}"
    );

    // The matching year is quiet.
    let fine = art(&[
        "VYNCINT",
        "--year",
        "2026",
        "--track",
        "--merge",
        "art/vyncint-2026.json",
        "--today",
        "2026-08-19",
        "--no-colour",
    ]);
    assert!(!String::from_utf8_lossy(&fine.stderr).contains("holds no"));
}

#[test]
fn backfill_commits_only_what_is_short() {
    // The command `--track` used to print was a plain `--write`, which puts the
    // same flat count on every lit day — including the ones already bright, and
    // adding to the busiest of them raises the very peak every letter day is
    // measured against. A shortfall cannot do that.
    let dir = scratch("backfill-test");
    let _ = std::fs::remove_dir_all(&dir);
    let repo = dir.to_string_lossy().into_owned();

    // A finished plan is short of nothing at all.
    let done = art(&[
        "VYNCINT",
        "--year",
        "2027",
        "--backfill",
        "--merge",
        "art/vyncint-2027.json",
        "--repo",
        &repo,
        "--today",
        "2027-12-31",
        "--no-colour",
    ]);
    assert!(
        done.status.success(),
        "{}",
        String::from_utf8_lossy(&done.stderr)
    );
    assert!(
        stdout(&done).contains("nothing to backfill"),
        "{}",
        stdout(&done)
    );
    assert!(!dir.exists(), "and it made no repository to do it in");

    // HI over that same calendar: some of its letter days are already lit by
    // VYNCINT's, and those must be left alone.
    let out = art(&[
        "HI",
        "--year",
        "2027",
        "--backfill",
        "--merge",
        "art/vyncint-2027.json",
        "--repo",
        &repo,
        "--write",
        // Past-only, so the run has to be told about a day after the letters it
        // is meant to reach. Without this the whole of 2027 is still to come and
        // there is correctly nothing to back-date.
        "--today",
        "2027-12-31",
        "--no-colour",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = stdout(&out);
    assert!(text.contains("backfilling against"), "{text}");
    assert!(text.contains("never a flat count"), "{text}");
    assert!(text.contains("nothing has been pushed"), "{text}");

    // What git actually holds: every commit dated in the plan's year, and every
    // day it touched short of the four that calendar's peak asks for.
    let log = Command::new("git")
        .current_dir(&dir)
        .args(["log", "--format=%ad", "--date=short"])
        .output()
        .expect("git log runs");
    let dates: Vec<String> = String::from_utf8_lossy(&log.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    assert!(!dates.is_empty(), "it wrote commits");
    assert!(
        dates.iter().all(|date| date.starts_with("2027-")),
        "back-dated into the plan's year: {dates:?}"
    );
    let mut per_day: std::collections::BTreeMap<&String, usize> = std::collections::BTreeMap::new();
    for date in &dates {
        *per_day.entry(date).or_default() += 1;
    }
    assert!(
        per_day.values().all(|count| *count == 4),
        "each short day gets exactly what it lacked of 4: {per_day:?}"
    );

    // A dry run is a dry run: nothing new after it.
    let before = dates.len();
    let dry = art(&[
        "HI",
        "--year",
        "2027",
        "--backfill",
        "--merge",
        "art/vyncint-2027.json",
        "--repo",
        &repo,
        "--today",
        "2027-12-31",
        "--no-colour",
    ]);
    assert!(
        stdout(&dry).contains("this was a dry run"),
        "{}",
        stdout(&dry)
    );
    let after = Command::new("git")
        .current_dir(&dir)
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .expect("git rev-list runs");
    assert_eq!(
        String::from_utf8_lossy(&after.stdout).trim(),
        before.to_string(),
        "a dry run wrote something"
    );

    // The two modes are different questions, and running both would mean
    // guessing which was meant.
    let both = art(&["VYNCINT", "--track", "--backfill"]);
    assert!(!both.status.success());
    assert!(String::from_utf8_lossy(&both.stderr).contains("pick one"));

    // And it needs somewhere to write.
    let nowhere = art(&["VYNCINT", "--year", "2027", "--backfill"]);
    assert!(!nowhere.status.success());
    assert!(String::from_utf8_lossy(&nowhere.stderr).contains("--repo"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_plan_file_cannot_smuggle_past_the_flag_bounds() {
    // The flags are bounded; a plan file was not, and `--plan PATH` is a path.
    // Each of these got past a different guard: `top` wrapped `place`'s check
    // and drew the letters on scrambled rows, `commits` quoted four billion a
    // day, and the two years panicked or drew a calendar `--year` refuses.
    let dir = scratch("plan-bounds-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let run = |plan: &str| {
        std::fs::write(dir.join("mossaic-plan.json"), plan).unwrap();
        Command::new(env!("CARGO_BIN_EXE_mossaic-art"))
            .current_dir(&dir)
            .args(["--no-colour"])
            .output()
            .expect("the art binary runs")
    };
    let plan = |field: &str, value: &str| {
        let mut fields = [
            ("text", "\"HI\""),
            ("year", "2027"),
            ("start_week", "10"),
            ("top", "1"),
            ("commits", "4"),
            ("background", "0"),
            ("user", "null"),
        ];
        for entry in fields.iter_mut() {
            if entry.0 == field {
                entry.1 = value;
            }
        }
        let body: Vec<String> = fields
            .iter()
            .map(|(key, val)| format!("\"{key}\":{val}"))
            .collect();
        format!("{{{}}}", body.join(","))
    };

    for (field, value, wanted) in [
        ("top", "18446744073709551615", "between 0 and 2"),
        ("commits", "4294967295", "between 1 and 1000000"),
        ("commits", "0", "between 1 and 1000000"),
        ("year", "-262143", "between 2000 and 2100"),
        ("year", "180000", "between 2000 and 2100"),
        ("start_week", "18446744073709551615", "between 0 and 60"),
        ("background", "9", "between 0 and 4"),
    ] {
        let out = run(&plan(field, value));
        let error = String::from_utf8_lossy(&out.stderr).into_owned();
        assert!(!out.status.success(), "plan {field}={value} was accepted");
        assert_eq!(out.status.code(), Some(2), "{field}={value}");
        assert!(
            error.contains("is not a plan these tools can use"),
            "{field}={value}: {error}"
        );
        // And it names the way out, because 0.2.0 could write a plan this
        // refuses: it accepted `--commits -1`, which `--save` wrote down as four
        // billion.
        assert!(error.contains("--save"), "{field}={value}: {error}");
        assert!(error.contains(wanted), "{field}={value}: {error}");
        assert!(!error.contains("panicked"), "{field}={value}: {error}");
        // The refusal names the value it refused, not a wrapped version of it.
        assert!(error.contains(value), "{field}={value}: {error}");
    }

    // And a sound plan still loads, or this test would pass by refusing
    // everything.
    let out = run(&plan("text", "\"HI\""));
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout(&out).contains("HI  ·  2027"), "{}", stdout(&out));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn backfill_reaches_only_the_days_that_have_gone() {
    // `--track` prints this command under "only back-dated commits reach
    // those", and the help calls it the flag for catching up — but it wrote the
    // whole year, future days included, and ignored `--today` entirely. A day
    // still to come wants an ordinary commit when it arrives, and whether
    // GitHub counts a future-dated one is unverified.
    let args = |today: &'static str| -> Vec<&'static str> {
        vec![
            "VYNCINT",
            "--year",
            "2026",
            "--backfill",
            "--merge",
            "art/vyncint-2026.json",
            "--repo",
            "/nonexistent-dry-run",
            "--today",
            today,
            "--no-colour",
        ]
    };

    let mid = stdout(&art(&args("2026-08-19")));
    // The same 34 days and 3,464 commits `--track` calls "already past": both
    // answers come from one plan and one date, so they agree by construction.
    assert!(mid.contains("3,464 commit(s) across 34 day(s)"), "{mid}");
    assert!(
        mid.contains("latest 2026-08-14"),
        "nothing after today: {mid}"
    );
    assert!(
        mid.contains("23 day(s) from 2026-08-19 on are short too, and left alone"),
        "and it says what it is leaving: {mid}"
    );

    // Later in the year, more of it is reachable.
    let end = stdout(&art(&args("2026-12-31")));
    assert!(end.contains("5,994 commit(s) across 57 day(s)"), "{end}");

    // Before the year starts, none of it is — and it says why rather than
    // claiming the plan is finished.
    let early = stdout(&art(&args("2026-01-01")));
    assert!(
        early.contains("every day the plan is short of is still to come"),
        "{early}"
    );

    // A year that cannot be drawn cleanly says so before an irreversible write.
    assert!(
        mid.contains("cannot be drawn cleanly in 2026") && mid.contains("61 day(s) inside the"),
        "a warning belongs where the commits do: {mid}"
    );
}

#[test]
fn tracking_a_year_that_has_not_started_is_not_a_mistake() {
    // The first example in the help, the README and docs/ART.md. The clock is
    // not in 2027, and that is the normal case — the report answers it properly.
    // A note saying "add --year 2026" told people their own year was wrong.
    let out = art(&[
        "VYNCINT",
        "--year",
        "2027",
        "--track",
        "--merge",
        "art/vyncint-2027.json",
        "--today",
        "2026-08-19",
        "--no-colour",
    ]);
    assert!(out.status.success());
    let note = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(note.is_empty(), "nothing to warn about: {note}");
    assert!(
        stdout(&out).contains("2027 has not started"),
        "{}",
        stdout(&out)
    );
    // Nor when the date comes from the clock, whatever year that turns out to
    // be: the note is about a year that has ended, not about one it is not.
    let bare = art(&[
        "VYNCINT",
        "--year",
        "2100",
        "--track",
        "--merge",
        "art/vyncint-2027.json",
        "--no-colour",
    ]);
    assert!(String::from_utf8_lossy(&bare.stderr)
        .lines()
        .all(|line| !line.contains("no today or tomorrow")));

    // A typed `--today` that misses the year still explains itself, because
    // there the missing today/tomorrow section is a surprise.
    let missed = art(&[
        "VYNCINT",
        "--year",
        "2026",
        "--track",
        "--merge",
        "art/vyncint-2026.json",
        "--today",
        "2027-03-01",
        "--no-colour",
    ]);
    assert!(String::from_utf8_lossy(&missed.stderr).contains("2027-03-01 is after 2026"));
}
