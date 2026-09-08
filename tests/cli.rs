//! `termlens-cli` — the command that ships beside the harness this suite
//! already uses.
//!
//! The rest of the suite asks whether mossaic draws the right thing. This
//! asks what a maintainer does *after* it draws the wrong thing: capture the
//! screen, save it, and read it back somewhere else. mossaic is the awkward
//! case for that — a truecolour palette, box drawing, wide glyphs and images
//! on the wire — so it is worth knowing the saved-screen format carries what
//! mossaic puts on a terminal, rather than assuming it.
//!
//! Ignored by default. These need `termlens-cli` on the machine, and a
//! `cargo test` that quietly `cargo install`s something is a surprise a
//! published crate should not spring on anyone. CI runs them with
//! `--ignored`, which is the same arrangement `smoke.rs` uses for the tests
//! that need `gh`.
//!
//! ```sh
//! cargo test --test cli -- --ignored
//! ```

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::time::Duration;

use termlens::{Screen, Terminal};

/// The termlens version this suite is measured against, read from the
/// lockfile so the tool and the library can never be two different releases.
fn version_under_test() -> &'static str {
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION.get_or_init(|| {
        let lock = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock"))
            .expect("Cargo.lock is committed");
        let mut lines = lock.lines();
        while let Some(line) = lines.next() {
            if line.trim() == "name = \"termlens\"" {
                for next in lines.by_ref() {
                    if let Some(rest) = next.trim().strip_prefix("version = \"") {
                        return rest.trim_end_matches('"').to_owned();
                    }
                }
            }
        }
        panic!("no termlens version in Cargo.lock");
    })
}

/// The `termlens` binary: `$TERMLENS_CLI` if the environment provides one,
/// otherwise installed once into `target/` at the version under test.
fn cli() -> &'static PathBuf {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        if let Some(given) = std::env::var_os("TERMLENS_CLI") {
            return PathBuf::from(given);
        }
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("termlens-cli");
        let bin = root
            .join("bin")
            .join(format!("termlens{}", std::env::consts::EXE_SUFFIX));
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let status = Command::new(cargo)
            .args(["install", "termlens-cli", "--version", version_under_test()])
            .args(["--locked", "--root"])
            .arg(&root)
            .status()
            .expect("cargo install termlens-cli");
        assert!(
            status.success(),
            "cargo install termlens-cli --version {} failed. It is published \
             alongside the library; if this version of termlens exists on \
             crates.io and termlens-cli does not, the two releases went out \
             of lockstep.",
            version_under_test()
        );
        bin
    })
}

fn run(args: &[&str]) -> Output {
    Command::new(cli())
        .args(args)
        .output()
        .expect("run termlens")
}

/// A year of contribution art from a file, at a pinned date: no network, no
/// dependence on when the test runs.
const PREVIEW: [&str; 4] = ["--file", "art/vyncint-2027.json", "--today", "2027-06-30"];

/// Drive the real mossaic and hand back a complete frame.
fn chart(cols: u16, rows: u16) -> termlens::Result<Terminal> {
    let mut t = Terminal::builder()
        .size(cols, rows)
        .env_clear()
        .env("COLORTERM", "truecolor")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .timeout(Duration::from_secs(20))
        .args(PREVIEW)
        .spawn(env!("CARGO_BIN_EXE_mossaic"))?;
    t.wait_frame(|s| s.contains("q quit") && s.contains("contributions in"))?;
    Ok(t)
}

/// Save a screen in the snapshot text format the CLI reads.
fn save(screen: &Screen, name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("mossaic-{}-{name}.snap", std::process::id()));
    std::fs::write(&path, screen.with_styles().to_string()).expect("write the saved screen");
    path
}

#[test]
#[ignore = "needs termlens-cli; run with --ignored (CI does)"]
fn the_tool_and_the_library_are_one_release() {
    let out = run(&["--version"]);
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        format!("termlens {}", version_under_test()),
        "the installed CLI is not the version this suite tests against"
    );
}

/// The point of the format, for a program whose output *is* colour: a saved
/// mossaic screen must come back with mossaic's palette, not an approximation
/// of it. The colours are read off the live screen first, so this compares
/// against what mossaic actually chose rather than a hardcoded green.
#[test]
#[ignore = "needs termlens-cli; run with --ignored (CI does)"]
fn a_saved_chart_keeps_the_palette_mossaic_chose() -> termlens::Result<()> {
    let t = chart(176, 34)?;
    let screen = t.screen();

    // The distinct foreground colours on the chart, as drawn.
    let mut palette: Vec<termlens::Color> = Vec::new();
    for row in 0..screen.rows() {
        for col in 0..screen.cols() {
            if let Some(cell) = screen.cell(row, col) {
                let fg = cell.style().fg;
                if fg != termlens::Color::Default && !palette.contains(&fg) {
                    palette.push(fg);
                }
            }
        }
    }
    let rgb: Vec<(u8, u8, u8)> = palette
        .iter()
        .filter_map(|colour| match colour {
            termlens::Color::Rgb(r, g, b) => Some((*r, *g, *b)),
            _ => None,
        })
        .collect();
    assert!(
        rgb.len() >= 4,
        "with COLORTERM=truecolor the chart draws a truecolour ramp, got {palette:?}"
    );

    let saved = save(&screen, "palette");
    let svg = run(&["render", "--svg", saved.to_str().unwrap()]);
    assert!(
        svg.status.success(),
        "{}",
        String::from_utf8_lossy(&svg.stderr)
    );
    let body = String::from_utf8_lossy(&svg.stdout);

    // Every truecolour the chart drew survives into the image, spelled the
    // way SVG spells it.
    for (r, g, b) in &rgb {
        let hex = format!("#{r:02x}{g:02x}{b:02x}");
        assert!(body.contains(&hex), "{hex} is missing from the SVG");
    }
    // And the text, so the image is the chart rather than a coloured grid.
    assert!(
        body.contains("contributions in"),
        "the footer is in the image"
    );

    let _ = std::fs::remove_file(&saved);
    Ok(())
}

/// The workflow a maintainer runs when a frame changed and they want to know
/// exactly how: save both, diff them. Exit 1 is the signal a script reads.
#[test]
#[ignore = "needs termlens-cli; run with --ignored (CI does)"]
fn diff_says_what_moving_the_cursor_changed() -> termlens::Result<()> {
    let mut t = chart(176, 34)?;
    let before = save(&t.screen(), "before");

    t.send(termlens::Key::Right)?;
    // Wait on what the key *changes*: the detail line names another day. It
    // does not always name a contribution count — a day past today reads
    // "still to come" — so the honest predicate is that Jun 30 is gone.
    let moved = t.wait_frame(|s| !s.contains("Jun 30 2027"))?;
    let after = save(&moved, "after");

    let same = run(&[
        "diff",
        "--color",
        "never",
        before.to_str().unwrap(),
        before.to_str().unwrap(),
    ]);
    assert_eq!(same.status.code(), Some(0), "a screen equals itself");
    assert!(String::from_utf8_lossy(&same.stdout).contains("no difference"));

    let out = run(&[
        "diff",
        "--color",
        "never",
        before.to_str().unwrap(),
        after.to_str().unwrap(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "moving the cursor changed the picture"
    );
    let rendered = String::from_utf8_lossy(&out.stdout);
    assert!(rendered.contains("size: 176x34"), "the header:\n{rendered}");
    assert!(
        rendered.contains("rows unchanged"),
        "and a count of what did not move:\n{rendered}"
    );

    for path in [&before, &after] {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

/// `inspect` points the harness at a binary without writing a test, which is
/// the first thing a contributor reaches for. Pointed at mossaic, it has to
/// survive everything mossaic does to a terminal.
#[test]
#[ignore = "needs termlens-cli; run with --ignored (CI does)"]
fn inspect_drives_mossaic_itself() {
    let mut args = vec!["inspect", "--size", "176x34", "--idle", "600"];
    args.push(env!("CARGO_BIN_EXE_mossaic"));
    args.extend_from_slice(&PREVIEW);
    let out = Command::new(cli())
        .args(&args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run inspect");

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let screen = String::from_utf8_lossy(&out.stdout);
    assert!(screen.contains("size: 176x34"), "{screen}");
    assert!(
        screen.contains("contributions in"),
        "the real chart:\n{screen}"
    );
    assert!(
        screen.contains("still running at the deadline"),
        "mossaic is a TUI, so inspect reports the deadline rather than an exit:\n{screen}"
    );
}
