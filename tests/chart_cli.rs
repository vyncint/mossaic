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
    drop(child.stdout.take());
    let out = child.wait_with_output().expect("wait");

    assert_eq!(
        out.status.code(),
        Some(0),
        "the file is written, so the status is success\n{}",
        String::from_utf8_lossy(&out.stderr)
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

/// The bug template asks for two things this binary prints, and it described
/// both wrongly (#147): the version was what "`mossaic --help` prints", which
/// it does not, and `--capabilities` was "four lines" when it is six — seven
/// when the terminal answered nothing, which is the case a reporter is most
/// likely to be in. The placeholder is what a reporter checks their paste
/// against, so its keys are the ones the binary prints, in its order.
#[test]
fn the_bug_template_asks_for_what_the_binary_prints() {
    let template = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(".github/ISSUE_TEMPLATE/bug_report.yml"),
    )
    .expect("the bug template");

    let version = chart(&["--version"]);
    assert!(
        String::from_utf8_lossy(&version.stdout).starts_with("mossaic "),
        "`--version` is what prints the version"
    );
    assert!(
        template.contains("`mossaic --version` prints it"),
        "the template sends a reporter to the flag that prints the version"
    );

    let caps = chart(&["--capabilities"]);
    let caps = String::from_utf8_lossy(&caps.stdout).into_owned();
    let lines = caps.lines().count();
    let prose = template.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        (6..=7).contains(&lines) && prose.contains("it is six or seven lines"),
        "the template's line count is the binary's: {lines}\n{caps}"
    );

    // Keys are the unindented lines' first words; an indented line continues
    // the one above it.
    let keys = |text: &str| {
        text.lines()
            .filter(|line| !line.starts_with(' '))
            .filter_map(|line| line.split_whitespace().next())
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let placeholder = template
        .split("placeholder: |\n")
        .nth(1)
        .and_then(|rest| rest.split("validations:").next())
        .expect("the capabilities placeholder")
        .lines()
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        keys(&placeholder),
        keys(&caps),
        "the placeholder is shaped like the output:\n{placeholder}\n---\n{caps}"
    );
}
