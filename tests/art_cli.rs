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
    let out = art(&[
        "VYNCINT",
        "--year",
        "2027",
        "--track",
        "--merge",
        "art/vyncint-2027.json",
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
    let out = std::env::temp_dir().join("mossaic-art-cli.json");
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
    let png = std::env::temp_dir().join("mossaic-art-cli.png");
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
    let dir = std::env::temp_dir().join("mossaic-plan-test");
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
    let dir = std::env::temp_dir().join("mossaic-plan-background-test");
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
