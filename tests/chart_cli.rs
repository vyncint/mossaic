//! The `mossaic` chart binary run *without* a terminal — a script, a pipe, CI.
//!
//! No PTY here on purpose: this file is about what the chart says when it
//! cannot open one at all. `smoke.rs` covers everything it does once it can.

use std::process::{Command, Output};

fn chart(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mossaic"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("the chart binary runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
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
        text.contains("interactive terminal"),
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
