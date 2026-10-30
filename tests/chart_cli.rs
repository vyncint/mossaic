//! The `mossaic` chart binary run *without* a terminal — a script, a pipe, CI.
//!
//! No PTY here on purpose: this file is about what the chart says when it
//! cannot open one at all. `smoke.rs` covers everything it does once it can.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn chart(args: &[&str]) -> Output {
    chart_in(Path::new(env!("CARGO_MANIFEST_DIR")), args)
}

/// From `dir`: anything that might write a file runs in a scratch directory,
/// never in the checkout. `--png -` once wrote a file named `-` at the
/// repository root on every test run (#134).
fn chart_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mossaic"))
        .current_dir(dir)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("the chart binary runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A fresh, empty directory of this test's own.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mossaic-chart-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// A directory holding a `gh` that runs `body`, for putting alone on `PATH`.
///
/// Written by `sh` rather than `std::fs::write`: a file this process wrote
/// can still be open for writing in a sibling test's fork when it is
/// executed, and that is `ETXTBSY`.
#[cfg(unix)]
fn fake_gh(name: &str, body: &str) -> PathBuf {
    let dir = scratch(name);
    let mut sh = Command::new("/bin/sh")
        .args(["-c", "cat > \"$1/gh\" && chmod +x \"$1/gh\"", "sh"])
        .arg(&dir)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("sh runs");
    std::io::Write::write_all(
        sh.stdin.as_mut().expect("piped"),
        format!("#!/bin/sh\n{body}\n").as_bytes(),
    )
    .expect("write the script");
    drop(sh.stdin.take());
    assert!(sh.wait().expect("sh exits").success());
    dir
}

/// The chart with `dir` as the whole of `PATH`.
#[cfg(unix)]
fn chart_with_path(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mossaic"))
        .current_dir(dir)
        .env("PATH", dir)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("the chart binary runs")
}

/// The chart shows a missing `--file` *inside* the chart, with `r` to retry
/// (`smoke.rs::a_missing_file_is_reported_not_swallowed`). Without a terminal
/// there is no chart to show it in, and it used to report only the missing
/// terminal — true, and no help at all to someone with a typo in a script.
#[test]
fn without_a_terminal_a_missing_file_is_named_rather_than_the_terminal() {
    let out = chart(&["--file", "/no/such/calendar.json"]);
    let text = stderr(&out);
    assert!(
        text.contains("no calendar file at /no/such/calendar.json"),
        "the file is the problem, and it should say so:\n{text}"
    );
    assert!(
        !text.contains("interactive terminal"),
        "and it should not blame the terminal:\n{text}"
    );
    assert!(
        text.contains("--snapshot") && text.contains("--demo"),
        "and it should say how to get one:\n{text}"
    );
}

/// The other half, which keeps the message above specific rather than blanket:
/// a file that *is* there leaves the terminal as the only thing wrong.
#[test]
fn without_a_terminal_an_existing_file_still_reports_the_terminal() {
    let out = chart(&["--file", "Cargo.toml"]);
    let text = stderr(&out);
    assert!(
        text.contains("stdout is not a terminal") && text.contains("--png FILE"),
        "nothing is wrong with the path, so the terminal is the problem:\n{text}"
    );
    assert!(
        !text.contains("no calendar file"),
        "and the file must not be blamed:\n{text}"
    );
}

/// `--png` is the documented way to get a chart out of a machine with no
/// terminal, so it must not need one.
#[test]
fn png_needs_no_terminal() {
    let path = std::env::temp_dir().join(format!("mossaic-{}-headless.png", std::process::id()));
    let out = chart(&["--demo", "--png", path.to_str().expect("utf-8 temp path")]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(path.is_file(), "it wrote {}", path.display());
    let _ = std::fs::remove_file(&path);
}

/// `-` is stdout (#134). It wrote a file called `-` — committed, shipped in the
/// crate, and rewritten by this suite on every run — and the status line went
/// to stdout too, so `--png /dev/stdout > chart.png` was a PNG with a sentence
/// in it. Run in an empty directory, so a stray `-` would be seen.
#[test]
fn png_dash_is_stdout_and_the_status_goes_to_stderr() {
    let dir = scratch("png-dash");
    let mut targets = vec!["-"];
    if cfg!(unix) {
        targets.push("/dev/stdout");
    }
    for target in targets {
        let out = chart_in(&dir, &["--demo", "--png", target]);
        assert!(out.status.success(), "{target}: {}", stderr(&out));
        assert!(
            out.stdout.starts_with(b"\x89PNG\r\n\x1a\n"),
            "{target}: stdout is the PNG"
        );
        // A PNG ends with its IEND chunk and its CRC; anything after that is
        // the status line landing in the image.
        assert!(
            out.stdout.ends_with(b"IEND\xae\x42\x60\x82"),
            "{target}: and only the PNG"
        );
        assert!(
            stderr(&out).contains("wrote") && stderr(&out).contains("px"),
            "{target}: the status line is on stderr:\n{}",
            stderr(&out)
        );
    }
    assert_eq!(
        std::fs::read_dir(&dir).expect("readable").count(),
        0,
        "nothing was written to the working directory"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A value-taking option took whatever came next, flags included (#135):
/// `--png --demo` fetched the signed-in user's calendar and wrote it to a file
/// called `--demo`. None of these may run, or write anything.
#[test]
fn an_option_value_does_not_swallow_the_next_flag() {
    let dir = scratch("swallow");
    for (args, flag, swallowed) in [
        (&["--png", "--demo"][..], "--png", "--demo"),
        (&["--file", "--demo"][..], "--file", "--demo"),
        (
            &["--demo", "--year", "2024", "--png", "-y"][..],
            "--png",
            "-y",
        ),
    ] {
        let out = chart_in(&dir, args);
        let text = stderr(&out);
        assert_eq!(
            out.status.code(),
            Some(2),
            "{args:?} is a usage error:\n{text}"
        );
        assert!(
            text.contains(&format!(
                "{flag} needs a value; {swallowed:?} looks like an option"
            )),
            "{args:?}:\n{text}"
        );
    }
    // The way to say "yes, really": glue it on.
    let out = chart_in(&dir, &["--demo", "--png=-y"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(dir.join("-y").is_file(), "--png=-y names the file");
    let _ = std::fs::remove_dir_all(&dir);
}

/// One chart, one login: `--file nope.json alice bob` charted bob and said
/// nothing about alice (#135).
#[test]
fn a_second_login_is_refused() {
    let out = chart(&["--demo", "alice", "bob"]);
    let text = stderr(&out);
    assert_eq!(out.status.code(), Some(2), "{text}");
    assert!(
        text.contains("one login at a time")
            && text.contains("\"alice\"")
            && text.contains("\"bob\""),
        "{text}"
    );
}

/// 2 is for a command line that was wrong, 1 for a run that failed (#138).
/// Both used to be 2, so a script could not tell a typo from an outage.
#[test]
fn usage_errors_exit_2_and_failures_exit_1() {
    let dir = scratch("exits");
    for args in [
        &["--year", "abc"][..],
        &["--demo", "--file", "x.json"],
        &["--bogus"],
        &["--png"],
    ] {
        let out = chart_in(&dir, args);
        assert_eq!(out.status.code(), Some(2), "{args:?}:\n{}", stderr(&out));
    }
    for args in [
        &["--file", "/no/such/calendar.json", "--png", "out.png"][..],
        &["--demo", "--png", "/no/such/dir/out.png"],
        // Not a terminal: nothing wrong with the command line.
        &["--demo"],
    ] {
        let out = chart_in(&dir, args);
        assert_eq!(out.status.code(), Some(1), "{args:?}:\n{}", stderr(&out));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A chart written into a pipe is escape codes in the pipe and a blank
/// terminal (#138): crossterm opens `/dev/tty` behind a redirected stdout, so
/// the chart started anyway. Refused, and pointed at what does work.
#[test]
fn a_chart_into_a_pipe_points_at_png() {
    let out = chart(&["--demo"]);
    let text = stderr(&out);
    assert!(
        text.contains("stdout is not a terminal — use --png FILE for an image"),
        "{text}"
    );
    assert!(out.stdout.is_empty(), "and nothing was drawn into the pipe");
}

/// `--help`'s key list had fallen behind README's Keys table: no `u`, no
/// Home/End (#138). Every key the table names, `--help` names too.
#[test]
fn help_lists_every_key_the_readme_does() {
    let out = chart(&["--help"]);
    let help = String::from_utf8_lossy(&out.stdout);
    let keys = help
        .split_once("in the chart:")
        .expect("a key list")
        .1
        .split_once("also installed:")
        .expect("and an end to it")
        .0
        .to_string();
    let readme =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md")).expect("README");
    let table = readme
        .split_once("## Keys")
        .expect("a Keys section")
        .1
        .split_once("\n## ")
        .expect("that ends")
        .0;
    let mut checked = 0;
    for row in table.lines().filter(|line| line.starts_with("| `")) {
        let first = row.split('|').nth(1).expect("a key column");
        for key in first.split('`').skip(1).step_by(2) {
            // The arrows are "arrows" in `--help`, which is plain ASCII.
            if key.is_ascii() {
                checked += 1;
                assert!(
                    keys.split_whitespace().any(|word| word == key),
                    "README's Keys table has `{key}` and --help does not:\n{keys}"
                );
            }
        }
    }
    assert!(checked >= 15, "the table was read: {checked} keys");
}

/// `--png` is a file, and a file should not change colour with the day it was
/// made (#136): from Oct 25 to 31 every image came out orange — README's own
/// hero command included. The default palette unless one is asked for, and
/// `--palette auto` takes its season from `--today`, not the clock.
#[test]
fn a_png_is_in_the_default_palette_whatever_the_date() {
    let dir = scratch("palette");
    let png = |name: &str, extra: &[&str]| -> Vec<u8> {
        let mut args = vec!["--demo", "--year", "2024", "--png", name];
        args.extend_from_slice(extra);
        let out = chart_in(&dir, &args);
        assert!(out.status.success(), "{extra:?}: {}", stderr(&out));
        std::fs::read(dir.join(name)).expect("written")
    };
    let june = png("june.png", &["--today", "2026-06-01"]);
    let halloween = png("halloween.png", &["--today", "2026-10-28"]);
    let named = png("named.png", &["--palette", "default"]);
    assert_eq!(june, named, "June is the default palette");
    assert_eq!(halloween, named, "and so is the last week of October");
    let auto = png("auto.png", &["--today", "2026-10-28", "--palette", "auto"]);
    assert_ne!(
        auto, named,
        "unless the season is asked for, by --today's date"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// `--today` decides the demo's year, and the days after it are still to come
/// (#136): `--demo --today 2025-06-01` drew and counted the whole year.
#[test]
fn the_demo_is_read_as_of_today() {
    let dir = scratch("demo-today");
    let out = chart_in(
        &dir,
        &["--demo", "--today", "2025-06-01", "--png", "part.png"],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stderr(&out).contains("demo 2025"), "{}", stderr(&out));
    let whole = chart_in(&dir, &["--demo", "--year", "2025", "--png", "whole.png"]);
    assert!(whole.status.success(), "{}", stderr(&whole));
    assert_ne!(
        std::fs::read(dir.join("part.png")).expect("written"),
        std::fs::read(dir.join("whole.png")).expect("written"),
        "June to December is still to come, so it is not drawn"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// What a `gh` that answers with a calendar for whichever year it is asked
/// about looks like, logging each year to `log`.
#[cfg(unix)]
fn calendar_gh(log: &Path) -> String {
    format!(
        r#"case "$2" in user) echo alice; exit 0;; esac
year=
for arg in "$@"; do
  case "$arg" in from=*) year=${{arg#from=}}; year=${{year%%-*}};; esac
done
echo "$year" >> '{}'
printf '{{"data":{{"user":{{"login":"alice","contributionsCollection":{{"contributionYears":[2027,2026,2025,2024,2023,2022,2021],"contributionCalendar":{{"totalContributions":2,"weeks":[{{"contributionDays":[{{"date":"%s-03-04","contributionCount":2,"contributionLevel":"FIRST_QUARTILE"}}]}}]}}}}}}}}}}\n' "$year""#,
        log.display()
    )
}

/// The default year is `--today`'s, not the clock's (#136):
/// `mossaic alice --today 2025-03-01` asked GitHub for the clock's year.
#[cfg(unix)]
#[test]
fn the_default_year_is_todays_year() {
    let log = scratch("today-log").join("years");
    let dir = fake_gh("today-year", &calendar_gh(&log));
    let out = chart_with_path(
        &dir,
        &["alice", "--today", "2025-03-01", "--png", "out.png"],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        std::fs::read_to_string(&log).expect("gh was asked").trim(),
        "2025"
    );
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(log.parent().expect("a directory"));
}

/// A bare `mossaic` blamed sign-in for every way `gh` can fail (#133). Each
/// failure now says what it is, and only "not signed in" says `gh auth login`.
#[cfg(unix)]
#[test]
fn a_bare_chart_says_why_gh_could_not_say_who_you_are() {
    // No gh at all: the install message, not "run gh auth login" into
    // "command not found".
    let empty = scratch("no-gh");
    let out = chart_with_path(&empty, &["--png", "out.png"]);
    let text = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.contains("asking gh who you are"), "{text}");
    assert!(text.contains("was not found on PATH"), "{text}");
    assert!(!text.contains("gh auth login       "), "{text}");
    let _ = std::fs::remove_dir_all(&empty);

    // Signed out: gh's own two lines, kept apart by a space rather than run
    // together, and the sign-in hint.
    let dir = fake_gh(
        "signed-out",
        "echo 'To get started with GitHub CLI, please run:  gh auth login' >&2\n\
         echo 'Alternatively, populate the GH_TOKEN environment variable.' >&2\n\
         exit 4",
    );
    let out = chart_with_path(&dir, &["--png", "out.png"]);
    let text = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.contains("gh auth login Alternatively"), "{text}");
    assert!(text.contains("gh auth login       sign in"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);

    // Anything else: gh's message, and no suggestion to sign in again.
    let dir = fake_gh(
        "offline",
        "echo 'Get \"https://api.github.com/user\": proxyconnect tcp: dial tcp 127.0.0.1:9' >&2\n\
         echo 'connect: connection refused' >&2\n\
         exit 1",
    );
    let out = chart_with_path(&dir, &["--png", "out.png"]);
    let text = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(
        text.contains("dial tcp 127.0.0.1:9 connect: connection refused"),
        "{text}"
    );
    assert!(!text.contains("gh auth login"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A reader that closes first is not a crash.
///
/// Rust sets `SIGPIPE` to `SIG_IGN`, so `println!` into a closed pipe
/// panicked and the process exited **101** — outside the set these binaries
/// document — with a backtrace note that reads like a crash in mossaic
/// rather than the user quitting a pager. Which commands escaped was a
/// pipe-buffer accident, so it read as a flake: the coloured glyph sheet is
/// 53 KB and always went, the uncoloured one is 9 KB and survived on Linux
/// but not on macOS.
///
/// Nothing in CI could see it: `install.yml` only ever pipes into `tee`,
/// which never exits early, and Actions' `bash -e` has no `pipefail`, so a
/// panicking writer left the step green. This asserts the **writer's own**
/// status, which is the thing a pipeline hides.
#[test]
fn a_reader_that_closes_early_is_not_a_crash() {
    use std::process::Stdio;

    // One case per binary, plus the two the report named by hand.
    let cases: [(&str, &[&str]); 5] = [
        (
            env!("CARGO_BIN_EXE_mossaic-art"),
            &["--font", "--color", "always"],
        ),
        (env!("CARGO_BIN_EXE_mossaic-art"), &["--list-templates"]),
        (env!("CARGO_BIN_EXE_mossaic-glyphs"), &["--no-colour"]),
        (env!("CARGO_BIN_EXE_mossaic"), &["--capabilities"]),
        (
            env!("CARGO_BIN_EXE_mossaic"),
            &["--demo", "--graphics", "text", "--png", "-"],
        ),
    ];

    for (binary, args) in cases {
        let mut child = std::process::Command::new(binary)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("{binary} runs: {e}"));
        // Close the read end while the writer is still going.
        drop(child.stdout.take());
        let out = child.wait_with_output().expect("wait");
        let text = String::from_utf8_lossy(&out.stderr);
        assert_ne!(
            out.status.code(),
            Some(101),
            "{binary} {args:?}: a closed reader must not be a panic\n{text}"
        );
        assert!(
            !text.contains("panicked"),
            "{binary} {args:?}: no panic text\n{text}"
        );
    }
}

/// `--png` writes the file, so the status is 0 even when the reader has gone.
///
/// The opposite failure to closed #29 ("--png writes an invalid zero-width
/// PNG and reports success"): here a valid, complete PNG is on disk and the
/// caller was handed 101, so a wrapper that checks the status deletes the
/// file and retries. It is also why this is a write-side fix rather than
/// restoring the default `SIGPIPE` disposition, which would give the wrong
/// answer here.
///
/// The status line moved to stderr in 0.9.0 (#134), so that is the stream
/// closed here.
#[test]
fn a_written_png_reports_success_even_when_the_reader_has_gone() {
    use std::process::Stdio;

    let png = std::env::temp_dir().join(format!("mossaic-bp-{}.png", std::process::id()));
    let _ = std::fs::remove_file(&png);
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_mossaic"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["--demo", "--png", png.to_str().unwrap()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the chart binary runs");
    drop(child.stderr.take());
    let out = child.wait_with_output().expect("wait");

    assert_eq!(
        out.status.code(),
        Some(0),
        "the file is written, so the status is success\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let bytes = std::fs::read(&png).expect("the PNG is on disk");
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "and it is a PNG");
    assert!(
        bytes.len() > 1000,
        "and a complete one: {} bytes",
        bytes.len()
    );
    let _ = std::fs::remove_file(&png);
}
