//! Renders frames through `TestBackend` so the layout can be inspected as text.

use chrono::{Datelike, Duration, Local, NaiveDate};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use crate::app::{App, Load};
use crate::calendar::{Calendar, Day};
use crate::ui;

/// Deterministic stand-in for a fetched year, so snapshots do not need the network.
/// A wholly elapsed year of deterministic data, so snapshots need no network.
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

fn synthetic(year: i32, login: &str) -> Calendar {
    let mut date = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(year, 12, 31).unwrap();
    let mut seed: u32 = 12_345;
    let mut days = Vec::new();
    let mut total = 0;

    while date <= end {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let count = match (seed >> 16) % 10 {
            0..=3 => 0,
            4..=6 => 1 + seed % 3,
            7..=8 => 4 + seed % 8,
            _ => 15 + seed % 40,
        };
        let level = match count {
            0 => 0,
            1..=3 => 1,
            4..=8 => 2,
            9..=20 => 3,
            _ => 4,
        };
        total += count;
        days.push(Day {
            date,
            count,
            level,
            future: false,
        });
        date += Duration::days(1);
    }
    Calendar::build(login.to_string(), year, total, vec![year - 1, year], days)
}

fn ready(calendar: Calendar) -> App {
    let mut app = App::new(
        calendar.login.clone(),
        calendar.year,
        crate::app::Source::GitHub,
    );
    app.years = calendar.years.clone();
    if let Some(last) = calendar.last_date() {
        app.cursor = last;
    }
    app.load = Load::Ready(Box::new(calendar));
    app
}

/// `&mut` because drawing records where the grid landed, for the mouse and the
/// painter to read back.
fn render(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui::draw(frame, app)).unwrap();
    terminal.backend().to_string()
}

#[test]
fn snapshot_spaced() {
    let mut app = ready(synthetic(2025, "octocat"));
    println!(
        "\n=== 170x22 (separated squares) ===\n{}",
        render(&mut app, 170, 22)
    );
}

#[test]
fn snapshot_wide() {
    let mut app = ready(synthetic(2025, "octocat"));
    app.cells = crate::app::CellStyle::Blocks;
    println!("\n=== 120x22 (blocks) ===\n{}", render(&mut app, 120, 22));
}

#[test]
fn snapshot_narrow() {
    let mut app = ready(synthetic(2025, "octocat"));
    println!(
        "\n=== 80x22 (single-column cells) ===\n{}",
        render(&mut app, 80, 22)
    );
}

#[test]
fn snapshot_clipped() {
    let mut app = ready(synthetic(2025, "octocat"));
    println!(
        "\n=== 44x22 (too narrow, clipped) ===\n{}",
        render(&mut app, 44, 22)
    );
}

#[test]
fn snapshot_loading_and_error() {
    let mut app = App::new("octocat".into(), 2025, crate::app::Source::GitHub);
    println!("\n=== loading ===\n{}", render(&mut app, 80, 12));
    app.load = Load::Failed("Could not resolve to a User with the login of 'nope'.".into());
    println!("\n=== error ===\n{}", render(&mut app, 80, 12));
}

#[test]
fn auto_picks_the_most_faithful_style_that_fits() {
    use crate::app::CellStyle;
    use crate::ui::{resolve, Cells};

    const SQUARES: Cells = Cells::Squares;
    const ROUNDED: Cells = Cells::Rounded { gap: 1 };
    const SNUG: Cells = Cells::Rounded { gap: 0 };
    const GRID: Cells = Cells::Grid { fill: 2 };
    const SLIM: Cells = Cells::Grid { fill: 1 };
    const SPACED: Cells = Cells::Solid { fill: 2, gap: 1 };
    const BLOCKS: Cells = Cells::Solid { fill: 2, gap: 0 };
    const COMPACT: Cells = Cells::Solid { fill: 1, gap: 0 };

    // 53 weeks. Squares needs one column per day plus a gap; the bordered styles
    // pay for borders on top of two-column cells, and rounded shares none of them.
    assert_eq!(SQUARES.width(53), 110);
    assert_eq!(ROUNDED.width(53), 163);
    // Dropping the gap column buys back a third of the width.
    assert_eq!(SNUG.width(53), 110);
    assert_eq!(SNUG.height(), 7);
    assert_eq!(GRID.width(53), 164);
    assert_eq!(SPACED.width(53), 163);
    assert_eq!(SLIM.width(53), 111);
    assert_eq!(BLOCKS.width(53), 110);
    assert_eq!(COMPACT.width(53), 57);
    assert_eq!(SQUARES.height(), 7);
    assert_eq!(ROUNDED.height(), 7);
    assert_eq!(GRID.height(), 15);

    let auto = |width, height| resolve(CellStyle::Auto, 53, width, height, false);
    assert_eq!(
        auto(240, 40),
        ROUNDED,
        "rounded corners win wherever they fit"
    );
    assert_eq!(auto(163, 16), ROUNDED, "rounded at its exact minimum");
    assert_eq!(
        auto(162, 40),
        SQUARES,
        "one column short: sharp corners instead"
    );
    assert_eq!(auto(110, 16), SQUARES, "squares at its exact minimum");
    assert_eq!(auto(109, 40), COMPACT, "too narrow for squares");
    assert_eq!(auto(240, 15), COMPACT, "too short for either");
    assert_eq!(auto(10, 5), COMPACT, "never panics on a tiny terminal");

    // Rounded and squares are the two faithful shapes and both cost less than any
    // bordered style, so nothing else can ever win on preference. The rest are
    // reachable only by asking for them with `d`.
    for width in 0..=260 {
        for height in [16, 24, 30, 40] {
            let picked = auto(width, height);
            // Snug is deliberately not in Auto's chain: at 110 columns it is the
            // same width as squares but loses both gaps, so which reads better is
            // a matter of taste rather than something to decide for the user.
            assert!(
                picked == ROUNDED || picked == SQUARES || picked == COMPACT,
                "auto at {width}x{height} picked {picked:?}"
            );
        }
    }

    for (style, expected) in [
        (CellStyle::Squares, SQUARES),
        (CellStyle::Rounded, ROUNDED),
        (CellStyle::Grid, GRID),
        (CellStyle::Slim, SLIM),
        (CellStyle::Spaced, SPACED),
        (CellStyle::Blocks, BLOCKS),
        (CellStyle::Compact, COMPACT),
    ] {
        assert_eq!(
            resolve(style, 53, 10, 5, false),
            expected,
            "{style:?} should not be overridden"
        );
    }
}

/// Column of each drawn cell on every weekday row, measured from the frame.
fn cell_columns(frame: &str, glyph: char) -> Vec<Vec<usize>> {
    frame
        .lines()
        .filter(|line| line.contains(glyph) && !line.contains("Less"))
        .map(|line| {
            let body = line.trim().trim_matches('"');
            body.chars()
                .enumerate()
                .filter(|(_, c)| *c == glyph)
                .map(|(i, _)| i)
                .collect()
        })
        .collect()
}

#[test]
fn every_weekday_row_lines_up_in_the_same_columns() {
    // 2025 starts on a Wednesday, so the first week is missing Sun/Mon/Tue. Those
    // gaps must be exactly as wide as a drawn cell plus its gap or the rows above
    // Wednesday slide out of column.
    for (style, glyph, stride) in [
        (crate::app::CellStyle::Rounded, '\u{1FB2B}', 3),
        (crate::app::CellStyle::Squares, '▀', 2),
    ] {
        let mut app = ready(synthetic(2025, "octocat"));
        app.cells = style;
        let frame = render(&mut app, 180, 22);
        let rows = cell_columns(&frame, glyph);
        assert_eq!(rows.len(), 7, "{style:?} should draw seven weekday rows");

        // Every cell in every row sits on the same lattice.
        let base = rows[3][0];
        for (index, row) in rows.iter().enumerate() {
            for &column in row {
                assert_eq!(
                    (column % stride, base % stride),
                    (base % stride, base % stride),
                    "{style:?} row {index} column {column} is off the {stride}-column grid"
                );
            }
        }
        // And the last drawn column matches across rows that run the full year.
        let ends: Vec<usize> = rows.iter().map(|row| *row.last().unwrap()).collect();
        let spread = ends.iter().max().unwrap() - ends.iter().min().unwrap();
        assert!(
            spread <= stride,
            "{style:?} rows end {spread} apart: {ends:?}"
        );
    }
}

#[test]
fn a_narrow_terminal_says_why_the_corners_are_sharp() {
    let mut app = ready(synthetic(2025, "octocat")); // Auto
                                                     // Below rounded's 163 inner columns the fallback is silent unless we say so.
    let narrow = render(&mut app, 120, 22);
    assert!(narrow.contains("squares cells"), "{narrow}");
    assert!(
        narrow.contains("rounded corners need 165 columns"),
        "should name the width it wants:\n{narrow}"
    );

    // Wide enough: rounded is chosen and there is nothing to explain.
    let wide = render(&mut app, 200, 22);
    assert!(wide.contains("rounded cells"), "{wide}");
    assert!(!wide.contains("need"), "{wide}");

    // Wide but too short for any style: rounding is not the reason there, so the
    // hint must stay out of the way. (The size warning itself sits below the fold
    // in a frame this short, which is why only its absence is asserted here.)
    let short = render(&mut app, 200, 14);
    assert!(!short.contains("rounded corners need"), "{short}");

    // Asking for a style explicitly is a choice, not a fallback: stay quiet.
    let mut chosen = ready(synthetic(2025, "octocat"));
    chosen.cells = crate::app::CellStyle::Squares;
    let quiet = render(&mut chosen, 120, 22);
    assert!(!quiet.contains("need"), "{quiet}");
}

#[test]
fn squares_match_githubs_shape() {
    let mut app = ready(synthetic(2025, "octocat"));
    app.cells = crate::app::CellStyle::Squares;
    let frame = render(&mut app, 120, 22);

    // Half-height glyphs with a gap column, and no outline anywhere: github.com
    // draws the fill, not a box around it.
    assert!(
        frame.contains("▀ ▀ ▀"),
        "separated half-height cells:\n{frame}"
    );
    // Strip the app's own window frame and TestBackend's quoting; only the cells
    // themselves are under test.
    let cells_only: String = frame
        .lines()
        .filter(|line| line.contains('▀'))
        .map(|line| line.trim().trim_matches('"').trim_matches('│'))
        .collect::<Vec<_>>()
        .join("\n");
    for drawn in ['│', '─', '┌', '╭', '┬', '┼', '█'] {
        assert!(
            !cells_only.contains(drawn),
            "squares should draw no {drawn}:\n{cells_only}"
        );
    }
    let mon = frame.lines().find(|line| line.contains("Mon")).unwrap();
    assert_eq!(
        mon.matches('▀').count(),
        52,
        "one square per drawn day:\n{mon}"
    );
    // The legend must use the same glyph as the cells, as it does on github.com.
    assert!(
        frame.contains("Less ▀ ▀ ▀ ▀ ▀ More"),
        "legend should match:\n{frame}"
    );
}

#[test]
fn grid_cells_are_two_columns_wide_so_they_read_as_squares() {
    let mut app = ready(synthetic(2025, "octocat"));
    app.cells = crate::app::CellStyle::Grid;
    let frame = render(&mut app, 170, 30);

    // Arc corners outside, crosses inside: a shared joint serves four cells and
    // cannot be rounded.
    assert!(
        frame.contains("╭──┬──"),
        "cells should be two columns wide:\n{frame}"
    );
    assert!(
        frame.contains("├──┼──"),
        "rules should match the cell width:\n{frame}"
    );
    let mon = frame.lines().find(|line| line.contains("Mon")).unwrap();
    // 53 cells of two blocks each, between 54 shared borders.
    assert_eq!(
        mon.matches("██").count(),
        52,
        "one filled pair per drawn day:\n{mon}"
    );
    assert_eq!(
        mon.matches('│').count(),
        54 + 2,
        "one border per cell edge:\n{mon}"
    );

    app.cells = crate::app::CellStyle::Slim;
    let slim = render(&mut app, 120, 30);
    assert!(
        slim.contains("╭─┬─"),
        "slim keeps one-column cells:\n{slim}"
    );
    assert!(!slim.contains("╭──┬"), "slim is not two columns:\n{slim}");
}

#[test]
fn rounded_cells_shave_all_four_corners() {
    let mut app = ready(synthetic(2025, "octocat"));
    app.cells = crate::app::CellStyle::Rounded;
    let frame = render(&mut app, 180, 22);

    // The pair of block sextants whose filled sub-blocks are  .## .   ->  .##.
    //                                                         ###  #       ####
    //                                                         .## .        .##.
    assert!(
        frame.contains("\u{1FB2B}\u{1FB1B}"),
        "rounded pair missing:\n{frame}"
    );
    assert!(
        frame.contains("\u{1FB2B}\u{1FB1B} \u{1FB2B}\u{1FB1B}"),
        "cells should be separated by a gap:\n{frame}"
    );
    // No outline, exactly as github.com has none.
    let cells_only: String = frame
        .lines()
        .filter(|line| line.contains('\u{1FB2B}') && !line.contains("Less"))
        .map(|line| line.trim().trim_matches('"').trim_matches('│'))
        .collect::<Vec<_>>()
        .join("\n");
    for drawn in ['│', '─', '┌', '╭', '┬', '┼', '█', '▀'] {
        assert!(
            !cells_only.contains(drawn),
            "rounded should draw no {drawn}:\n{cells_only}"
        );
    }
    let mon = frame.lines().find(|line| line.contains("Mon")).unwrap();
    assert_eq!(
        mon.matches('\u{1FB2B}').count(),
        52,
        "one rounded cell per drawn day:\n{mon}"
    );
    // The legend chip is one cell, not two.
    assert!(
        frame.contains("Less \u{1FB2B}\u{1FB1B} \u{1FB2B}\u{1FB1B} \u{1FB2B}\u{1FB1B} \u{1FB2B}\u{1FB1B} \u{1FB2B}\u{1FB1B} More"),
        "legend should show five single cells:\n{frame}"
    );
}

#[test]
fn snug_rounds_the_corners_but_gives_up_the_gaps() {
    let mut app = ready(synthetic(2025, "octocat"));
    app.cells = crate::app::CellStyle::Snug;
    let frame = render(&mut app, 116, 22);

    // Same rounded pair, no column between cells.
    assert!(
        frame.contains("\u{1FB2B}\u{1FB1B}\u{1FB2B}\u{1FB1B}"),
        "cells should touch:\n{frame}"
    );
    // Scoped to the chart: the legend spaces its five chips whatever the style.
    let cells_only: String = frame
        .lines()
        .filter(|line| line.contains('\u{1FB2B}') && !line.contains("Less"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !cells_only.contains("\u{1FB2B}\u{1FB1B} \u{1FB2B}"),
        "no gap column in snug:\n{cells_only}"
    );
    assert!(frame.contains("snug cells"), "{frame}");

    // It fits where rounded does not, which is the whole point.
    let rounded_width = crate::ui::Cells::Rounded { gap: 1 }.width(53);
    let snug_width = crate::ui::Cells::Rounded { gap: 0 }.width(53);
    assert!(snug_width < 116 - 2 && rounded_width > 116 - 2);

    // Rows still line up, since a skipped day is still one cell wide.
    let rows = cell_columns(&frame, '\u{1FB2B}');
    assert_eq!(rows.len(), 7);
    for row in &rows {
        for &column in row {
            assert_eq!(
                column % 2,
                rows[3][0] % 2,
                "off the two-column grid: {column}"
            );
        }
    }
}

/// Every day of `year` still to come, as the current year's tail is.
fn all_future(year: i32) -> Calendar {
    let end = NaiveDate::from_ymd_opt(year, 12, 31).unwrap();
    let mut date = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
    let mut days = Vec::new();
    while date <= end {
        days.push(Day {
            date,
            count: 0,
            level: 0,
            future: true,
        });
        date += Duration::days(1);
    }
    Calendar::build("octocat".to_string(), year, 0, vec![year], days)
}

/// A year GitHub reports but that yields no days at all.
fn empty_calendar(year: i32) -> Calendar {
    Calendar::build("octocat".to_string(), year, 0, Vec::new(), Vec::new())
}

#[test]
fn starts_after_compares_calendar_years() {
    let calendar = empty_calendar(2027);
    assert!(calendar.starts_after(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()));
    assert!(!calendar.starts_after(NaiveDate::from_ymd_opt(2027, 1, 1).unwrap()));
    assert!(!calendar.starts_after(NaiveDate::from_ymd_opt(2028, 6, 1).unwrap()));
}

#[test]
fn a_future_year_draws_a_full_empty_grid() {
    // Relative to today, so this keeps holding as years pass.
    let next = Local::now().year() + 1;
    let mut app = ready(all_future(next));
    app.cells = crate::app::CellStyle::Grid;
    let frame = render(&mut app, 170, 30);

    assert!(
        frame.contains("╭──┬"),
        "the whole year should still be drawn:\n{frame}"
    );
    assert!(
        frame.contains("╰──┴"),
        "the whole year should still be drawn:\n{frame}"
    );
    let mon = frame.lines().find(|line| line.contains("Mon")).unwrap();
    assert!(!mon.contains('█'), "no day should be filled: {mon}");
    assert!(
        frame.contains(&format!("{next} hasn't started yet")),
        "{frame}"
    );
    assert!(
        frame.contains("still to come"),
        "the legend should explain blanks:\n{frame}"
    );
    println!("\n=== future year: full grid, all cells empty ===\n{frame}");
}

#[test]
fn a_streak_is_not_broken_by_the_unwritten_rest_of_the_year() {
    // Three active days ending at the cutoff, then the year's remainder still to come.
    let cutoff = NaiveDate::from_ymd_opt(2025, 6, 30).unwrap();
    let end = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
    let mut date = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
    let mut days = Vec::new();
    while date <= end {
        let active = date <= cutoff && date > cutoff - Duration::days(3);
        // Rising counts so the busiest day is unambiguously the last elapsed one.
        let count = if active {
            5 + (date - (cutoff - Duration::days(2))).num_days() as u32
        } else {
            0
        };
        days.push(Day {
            date,
            count,
            level: if active { 2 } else { 0 },
            future: date > cutoff,
        });
        date += Duration::days(1);
    }
    let calendar = Calendar::build("octocat".to_string(), 2025, 18, vec![2025], days);

    // The grid spans the whole year even though half of it has not happened.
    assert_eq!(calendar.days().count(), 365);
    assert_eq!(calendar.last_date(), Some(end));
    assert_eq!(calendar.elapsed().count(), 181);

    let stats = calendar.stats();
    assert_eq!(
        stats.current_streak, 3,
        "future days must not count as a broken streak"
    );
    assert_eq!(stats.longest_streak, 3);
    assert_eq!(stats.active_days, 3);
    assert_eq!(
        stats.best,
        Some((cutoff, 7)),
        "the busiest day is the last elapsed one"
    );
}

#[test]
fn an_empty_range_still_explains_itself() {
    let next = Local::now().year() + 1;
    let future = render(&mut ready(empty_calendar(next)), 80, 12);
    assert!(
        future.contains(&format!("{next} hasn't started yet")),
        "{future}"
    );
    assert!(!future.contains("no contribution data"), "{future}");

    let past = Local::now().year() - 1;
    let elapsed = render(&mut ready(empty_calendar(past)), 80, 12);
    assert!(
        elapsed.contains(&format!("no contribution data for {past}")),
        "{elapsed}"
    );
    assert!(!elapsed.contains("started yet"), "{elapsed}");
}

#[test]
fn a_saved_calendar_loads_with_nothing_in_the_future() {
    // A year wholly ahead of today: from a file every day must still be drawn,
    // which is what makes previewing contribution art possible at all.
    let year = Local::now().year() + 1;
    let path = scratch("snapshot-test.json");
    let body = format!(
        r#"{{"data":{{"user":{{"login":"preview","contributionsCollection":{{
        "contributionYears":[{year}],"contributionCalendar":{{"totalContributions":9,
        "weeks":[{{"contributionDays":[
          {{"date":"{year}-06-14","contributionCount":9,"contributionLevel":"FOURTH_QUARTILE"}},
          {{"date":"{year}-06-15","contributionCount":0,"contributionLevel":"NONE"}}
        ]}}]}}}}}}}},"errors":null}}"#
    );
    std::fs::write(&path, body).unwrap();
    let loaded = crate::github::from_file(path.to_str().unwrap());
    let _ = std::fs::remove_file(&path);

    let calendar = loaded.expect("snapshot should parse");
    assert_eq!(
        calendar.year, year,
        "the year comes from the file, not the clock"
    );
    assert_eq!(calendar.days().count(), 2);
    assert!(
        calendar.days().all(|day| !day.future),
        "a snapshot has no future days"
    );
    assert!(calendar.has_elapsed_days());
    assert_eq!(calendar.total, 9);
    assert_eq!(calendar.stats().active_days, 1);
}

#[test]
fn a_missing_snapshot_reports_the_path() {
    let error = crate::github::from_file("/nonexistent/nope.json").unwrap_err();
    assert!(error.contains("nope.json"), "{error}");
}

#[test]
fn grid_positions_round_trip() {
    let calendar = synthetic(2025, "octocat");
    // 2025-01-01 is a Wednesday, so the first week is partial: Sun..Tue are empty.
    assert_eq!(
        calendar.position(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()),
        Some((0, 3))
    );
    assert!(calendar.weeks[0].days[0].is_none());
    assert!(calendar.weeks[0].days[3].is_some());

    for day in calendar.days() {
        let (week, weekday) = calendar.position(day.date).unwrap();
        assert_eq!(calendar.weeks[week].days[weekday].unwrap().date, day.date);
    }
    assert_eq!(calendar.first_date(), NaiveDate::from_ymd_opt(2025, 1, 1));
    assert_eq!(calendar.last_date(), NaiveDate::from_ymd_opt(2025, 12, 31));
    assert_eq!(calendar.days().count(), 365);
}

#[test]
fn month_labels_are_ordered_and_spaced() {
    let labels = synthetic(2025, "octocat").month_labels();
    assert_eq!(labels.len(), 12);
    assert_eq!(labels[0].1, "Jan");
    assert_eq!(labels[11].1, "Dec");
    for pair in labels.windows(2) {
        // Labels are three characters wide; two-column cells must not overlap.
        assert!(pair[1].0 > pair[0].0, "labels out of order: {pair:?}");
    }
}

#[test]
fn stats_match_the_underlying_days() {
    let calendar = synthetic(2025, "octocat");
    let stats = calendar.stats();
    let active = calendar.days().filter(|d| d.count > 0).count() as u32;
    let best = calendar.days().map(|d| d.count).max().unwrap();

    assert_eq!(stats.active_days, active);
    assert_eq!(stats.best.unwrap().1, best);
    assert!(stats.longest_streak >= stats.current_streak);
    assert!(stats.longest_streak > 0 && stats.longest_streak <= 365);
}

/// Hits the network through `gh`. Run with:
///   cargo test live -- --ignored --nocapture
#[test]
#[ignore = "requires gh and network access"]
fn live() {
    let year = chrono::Local::now().year();
    let login = crate::github::whoami().expect("gh is not authenticated");
    let calendar = crate::github::fetch(&login, year).expect("fetch failed");

    let days: Vec<_> = calendar.days().collect();
    let today = chrono::Local::now().date_naive();
    assert_eq!(
        days.first().unwrap().date,
        NaiveDate::from_ymd_opt(year, 1, 1).unwrap()
    );
    assert_eq!(
        days.last().unwrap().date,
        NaiveDate::from_ymd_opt(year, 12, 31).unwrap(),
        "the whole year is drawn, not just the elapsed part"
    );
    let elapsed: Vec<_> = calendar.elapsed().collect();
    assert_eq!(
        elapsed.last().unwrap().date,
        today,
        "elapsed days end at today"
    );
    assert!(
        days.iter().all(|day| day.future == (day.date > today)),
        "every day past today must be flagged future"
    );
    assert!(
        calendar.elapsed().all(|day| !day.future),
        "elapsed() must not yield future days"
    );
    // Days must be contiguous, which is what cursor clamping relies on.
    for pair in days.windows(2) {
        assert_eq!(pair[1].date - pair[0].date, Duration::days(1));
    }
    assert!(
        calendar.years.contains(&year),
        "years: {:?}",
        calendar.years
    );
    assert!(
        calendar.years.windows(2).all(|p| p[0] < p[1]),
        "years must be ascending"
    );

    let mut app = ready(calendar);
    println!(
        "\n=== live: {login} {year} ===\n{}",
        render(&mut app, 120, 30)
    );
}

// ---------------------------------------------------------------- colour

#[test]
fn the_palette_is_the_one_github_ships() {
    use crate::primer::{Appearance, Palette, Rgb, Season};

    // Lifted from github.com's own stylesheets: --contribution-default-bgColor-0..4
    // in light-*.css, dark-*.css and dark_dimmed-*.css. If GitHub restyles the
    // graph, this is the test that should fail.
    let levels = |appearance, season| {
        Palette::new(appearance, season, true)
            .levels
            .map(|Rgb(r, g, b)| u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b))
    };
    assert_eq!(
        levels(Appearance::Light, Season::Default),
        [0xeff2f5, 0xaceebb, 0x4ac26b, 0x2da44e, 0x116329]
    );
    assert_eq!(
        levels(Appearance::Dark, Season::Default),
        [0x151b23, 0x033a16, 0x196c2e, 0x2ea043, 0x56d364]
    );
    assert_eq!(
        levels(Appearance::Dimmed, Season::Default),
        [0x2a313c, 0x1b4721, 0x2b6a30, 0x46954a, 0x6bc46d]
    );
    // A holiday repaints levels 1-4 and leaves an empty day the neutral it was,
    // exactly as `.ContributionCalendar[data-holiday]` does.
    assert_eq!(
        levels(Appearance::Dark, Season::Halloween),
        [0x151b23, 0xfac68f, 0xc46212, 0x984b10, 0xe3d04f]
    );
    assert_eq!(
        levels(Appearance::Light, Season::Winter),
        [0xeff2f5, 0xb6e3ff, 0x54aeff, 0x0969da, 0x0a3069]
    );

    let dark = Palette::new(Appearance::Dark, Season::Default, true);
    assert_eq!(dark.canvas, Rgb::hex(0x0d1117), "--bgColor-default");
    assert_eq!(dark.tooltip_bg, Rgb::hex(0x3d444d), "--bgColor-emphasis");
    assert_eq!(dark.accent, Rgb::hex(0x4493f8), "--fgColor-accent");
    assert_eq!(
        dark.edge,
        Rgb::hex(0x010409),
        "--contribution-…-borderColor-0"
    );
}

#[test]
fn the_theme_follows_the_terminals_own_background() {
    use crate::primer::{Appearance, Rgb};
    assert_eq!(
        Appearance::from_background(Rgb::hex(0x0d1117)),
        Appearance::Dark
    );
    assert_eq!(
        Appearance::from_background(Rgb::hex(0xffffff)),
        Appearance::Light
    );
    // Solarized light is not white but is still a light terminal.
    assert_eq!(
        Appearance::from_background(Rgb::hex(0xfdf6e3)),
        Appearance::Light
    );
}

#[test]
fn seasonal_colours_follow_the_calendar_like_github() {
    use crate::primer::Season;
    let on = |month, day| Season::on(NaiveDate::from_ymd_opt(2026, month, day).unwrap());
    assert_eq!(on(10, 24), Season::Default);
    assert_eq!(on(10, 25), Season::Halloween);
    assert_eq!(on(10, 31), Season::Halloween);
    assert_eq!(on(11, 1), Season::Default);
    assert_eq!(on(6, 15), Season::Default);
}

#[test]
fn colours_degrade_to_the_256_colour_cube() {
    use crate::primer::Rgb;
    use ratatui::style::Color;

    // Truecolor is passed through untouched.
    assert_eq!(Rgb::hex(0x39d353).ansi(true), Color::Rgb(0x39, 0xd3, 0x53));
    // Near-greys take the grey ramp, which is finer than the cube there.
    assert_eq!(Rgb(18, 18, 18).ansi(false), Color::Indexed(233));
    // Chromatic colours take the cube.
    assert_eq!(Rgb::hex(0x00d700).ansi(false), Color::Indexed(40));

    // The five levels are the one thing that must not collapse: converting them
    // would land two of GitHub's dark greens on the same grey, so they are chosen.
    use crate::primer::{Appearance, Palette, Season};
    for appearance in [Appearance::Light, Appearance::Dark, Appearance::Dimmed] {
        for season in [Season::Default, Season::Winter, Season::Halloween] {
            let palette = Palette::new(appearance, season, false);
            let levels: Vec<Color> = (0..5).map(|level| palette.level(level)).collect();
            for (index, pair) in levels.windows(2).enumerate() {
                assert_ne!(
                    pair[0],
                    pair[1],
                    "{appearance:?}/{season:?}: levels {index} and {} are the same colour",
                    index + 1
                );
            }
            // And a truecolor terminal gets the exact Primer value.
            let exact = Palette::new(appearance, season, true);
            assert_eq!(
                exact.level(4),
                Color::Rgb(exact.levels[4].0, exact.levels[4].1, exact.levels[4].2)
            );
        }
    }
}

// ---------------------------------------------------------------- capabilities

#[test]
fn what_the_terminal_answers_is_what_we_believe() {
    use crate::primer::Rgb;
    use crate::term;

    // kitty: OK to the graphics query, and no sixel in its attributes.
    let kitty =
        term::parse("\x1b_Gi=7379;OK\x1b\\\x1b]11;rgb:0000/0000/0000\x07\x1b[6;20;10t\x1b[?62;22c");
    assert!(kitty.kitty);
    assert!(!kitty.sixel);
    assert_eq!(
        kitty.cell,
        Some((10, 20)),
        "width first, though the reply is height first"
    );
    assert_eq!(kitty.background, Some(Rgb(0, 0, 0)));
    assert!(kitty.answered);

    // xterm with sixel: attribute 4 among the device attributes.
    let sixel = term::parse("\x1b[?63;1;2;4;6;9;15;22;29c");
    assert!(sixel.sixel);
    assert!(!sixel.kitty);

    // A terminal that knows the protocol but not this transmission medium says so,
    // and must not be sent images.
    assert!(!term::parse("\x1b_Gi=7379;ENOTSUPPORTED\x1b\\\x1b[?62c").kitty);

    // Silence is not a no: it is a terminal that cannot be asked.
    let quiet = term::parse("");
    assert!(!quiet.answered && !quiet.kitty && !quiet.sixel && quiet.cell.is_none());

    // OSC 11 answers with 16 bits per channel, and some terminals with fewer.
    assert_eq!(
        term::parse("\x1b]11;rgb:ffff/8000/0000\x1b\\").background,
        Some(Rgb(255, 128, 0))
    );
    assert_eq!(
        term::parse("\x1b]11;rgb:0d/11/17\x07").background,
        Some(Rgb(0x0d, 0x11, 0x17))
    );
    // Window size as a fallback for terminals that will not report a cell.
    assert_eq!(term::parse("\x1b[4;646;1584t").window, Some((1584, 646)));
}

// ---------------------------------------------------------------- graphics

fn levels_of(calendar: &Calendar) -> Vec<[Option<u8>; 7]> {
    calendar
        .weeks
        .iter()
        .map(|week| {
            let mut column = [None; 7];
            for (weekday, day) in week.days.iter().enumerate() {
                column[weekday] = day.filter(|day| !day.future).map(|day| day.level);
            }
            column
        })
        .collect()
}

/// Read a sixel back into pixels, so the encoder is checked against the format
/// rather than against itself.
fn decode_sixel(payload: &str) -> (usize, usize, Vec<Option<[u8; 3]>>) {
    let body = payload
        .strip_prefix("\x1bP0;1;0q")
        .expect("sixel introducer with a transparent background")
        .strip_suffix("\x1b\\")
        .expect("string terminator");
    let (raster, rest) = body.split_once('#').expect("raster attributes");
    let numbers: Vec<usize> = raster
        .trim_start_matches('"')
        .split(';')
        .map(|n| n.parse().unwrap())
        .collect();
    let (width, height) = (numbers[2], numbers[3]);

    let mut palette = std::collections::HashMap::new();
    let mut pixels = vec![None; width * height];
    let (mut x, mut top, mut colour) = (0usize, 0usize, 0usize);
    // The split above ate the first '#'; put it back.
    let data = format!("#{rest}");
    let mut chars = data.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '#' => {
                let mut number = String::new();
                while chars.peek().is_some_and(char::is_ascii_digit) {
                    number.push(chars.next().unwrap());
                }
                colour = number.parse().unwrap();
                if chars.peek() == Some(&';') {
                    let mut parts = Vec::new();
                    while chars.peek() == Some(&';') {
                        chars.next();
                        let mut value = String::new();
                        while chars.peek().is_some_and(char::is_ascii_digit) {
                            value.push(chars.next().unwrap());
                        }
                        parts.push(value.parse::<u32>().unwrap());
                    }
                    assert_eq!(parts[0], 2, "colours are declared as RGB");
                    palette.insert(
                        colour,
                        [
                            (parts[1] * 255 / 100) as u8,
                            (parts[2] * 255 / 100) as u8,
                            (parts[3] * 255 / 100) as u8,
                        ],
                    );
                }
            }
            '$' => x = 0,
            '-' => {
                x = 0;
                top += 6;
            }
            '!' => {
                let mut number = String::new();
                while chars.peek().is_some_and(char::is_ascii_digit) {
                    number.push(chars.next().unwrap());
                }
                let run: usize = number.parse().unwrap();
                let bits = chars.next().unwrap() as u8 - b'?';
                for _ in 0..run {
                    paint(&mut pixels, width, height, x, top, bits, palette[&colour]);
                    x += 1;
                }
            }
            c if ('?'..='~').contains(&c) => {
                let bits = c as u8 - b'?';
                paint(&mut pixels, width, height, x, top, bits, palette[&colour]);
                x += 1;
            }
            other => panic!("unexpected sixel byte {other:?}"),
        }
    }
    (width, height, pixels)
}

fn paint(
    pixels: &mut [Option<[u8; 3]>],
    width: usize,
    height: usize,
    x: usize,
    top: usize,
    bits: u8,
    colour: [u8; 3],
) {
    for row in 0..6 {
        if bits >> row & 1 == 1 {
            let y = top + row;
            if x < width && y < height {
                pixels[y * width + x] = Some(colour);
            }
        }
    }
}

#[test]
fn a_sixel_is_the_image_the_rasteriser_drew() {
    use crate::graphics;
    use crate::primer::{Appearance, Palette, Season};

    let palette = Palette::new(Appearance::Dark, Season::Default, true);
    let cell = (9, 19);
    let image = graphics::grid(&levels_of(&synthetic(2025, "octocat")), &palette, cell);
    let (width, height, pixels) = decode_sixel(&graphics::sixel(&image, palette.canvas));

    assert_eq!((width, height), (image.width, image.height));
    let mut painted = 0;
    for y in 0..height {
        for x in 0..width {
            let [r, g, b, a] = image.rgba_at(x, y);
            match pixels[y * width + x] {
                // Sixel has no alpha: a pixel nobody paints is one the terminal
                // leaves alone, which is how the gaps stay the terminal's own.
                None => assert_eq!(a, 0, "an opaque pixel at {x},{y} went unpainted"),
                Some(got) => {
                    painted += 1;
                    let want =
                        crate::primer::Rgb(r, g, b).over(palette.canvas, f32::from(a) / 255.0);
                    for (got, want) in got.iter().zip([want.0, want.1, want.2]) {
                        // Sixel colours are percentages, so 255/100 of a channel is
                        // the most precision the format has.
                        assert!(got.abs_diff(want) <= 2, "at {x},{y}: {got} vs {want}");
                    }
                }
            }
        }
    }
    assert!(
        painted > width * height / 4,
        "only {painted} pixels painted"
    );
}

#[test]
fn a_kitty_transmission_carries_the_image_back() {
    use crate::graphics;
    use crate::primer::{Appearance, Palette, Season};

    let palette = Palette::new(Appearance::Dark, Season::Default, true);
    let image = graphics::grid(&levels_of(&synthetic(2025, "octocat")), &palette, (9, 19));
    let escape = graphics::kitty(&image, 7, 106, 7, -2);

    let mut payload = String::new();
    let mut control = String::new();
    for (index, chunk) in escape.split("\x1b\\").filter(|s| !s.is_empty()).enumerate() {
        let body = chunk.strip_prefix("\x1b_G").expect("APC introducer");
        let (keys, data) = body.split_once(';').expect("control data then payload");
        if index == 0 {
            control = keys.to_string();
        } else {
            // Continuations carry nothing but whether more follow.
            assert!(keys == "m=1" || keys == "m=0", "stray control data: {keys}");
        }
        assert!(data.len() <= 4096, "chunk of {} bytes", data.len());
        payload.push_str(data);
    }

    for wanted in [
        "a=T", "q=2", "f=32", "o=z", "i=7", "c=106", "r=7", "z=-2", "C=1",
    ] {
        assert!(control.contains(wanted), "{wanted} missing from {control}");
    }
    assert!(control.contains(&format!("s={}", image.width)));
    assert!(control.contains(&format!("v={}", image.height)));
    assert!(
        escape.ends_with("m=0;\x1b\\") || escape.contains("m=0;"),
        "last chunk must say so"
    );

    // Compression is the whole reason a year of pixels is affordable to send.
    let bytes = decode_base64(&payload);
    assert!(
        bytes.len() * 20 < image.width * image.height * 4,
        "zlib bought less than 20x: {} bytes",
        bytes.len()
    );
    let raw = miniz_oxide::inflate::decompress_to_vec_zlib(&bytes).expect("zlib");
    assert_eq!(raw.len(), image.width * image.height * 4);
    for y in 0..image.height {
        for x in 0..image.width {
            let at = (y * image.width + x) * 4;
            assert_eq!(&raw[at..at + 4], &image.rgba_at(x, y), "at {x},{y}");
        }
    }
}

fn decode_base64(text: &str) -> Vec<u8> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut bits = 0u32;
    let mut held = 0;
    for byte in text.bytes().filter(|b| *b != b'=') {
        let value = ALPHABET.iter().position(|c| *c == byte).expect("base64") as u32;
        bits = bits << 6 | value;
        held += 6;
        if held >= 8 {
            held -= 8;
            out.push((bits >> held) as u8);
        }
    }
    out
}

#[test]
fn cells_are_square_whatever_the_font_is() {
    use crate::graphics;
    use crate::primer::{Appearance, Palette, Season};

    let palette = Palette::new(Appearance::Dark, Season::Default, true);
    // A wide cell, a tall one, and one that is exactly half as wide as it is tall.
    for cell in [(9u16, 19u16), (6, 20), (10, 20), (12, 16)] {
        let image = graphics::patch(Some(4), None, &palette, cell);
        assert_eq!(image.width, usize::from(cell.0) * 2);
        assert_eq!(image.height, usize::from(cell.1));

        // Measure the drawn square by its opaque extent.
        let opaque = |x: usize, y: usize| image.rgba_at(x, y)[3] > 128;
        let columns: Vec<usize> = (0..image.width)
            .filter(|x| (0..image.height).any(|y| opaque(*x, y)))
            .collect();
        let rows: Vec<usize> = (0..image.height)
            .filter(|y| (0..image.width).any(|x| opaque(x, *y)))
            .collect();
        let (wide, tall) = (columns.len(), rows.len());
        assert!(
            wide.abs_diff(tall) <= 1,
            "cell {cell:?} drew {wide}x{tall}, which is not square"
        );
        // And it leaves a gap: github.com's 11px cell sits on a 14px pitch.
        assert!(
            wide < image.width && tall < image.height,
            "cell {cell:?} filled its whole pitch"
        );
        // Rounded. A 2px radius on an 11px cell is a sub-pixel bite out of the
        // corner at this size, so count coverage rather than pixels: a block at the
        // corner has to be less covered than the same block along the top edge.
        let block = |x: usize, y: usize| -> u32 {
            (0..3)
                .flat_map(|dy| (0..3).map(move |dx| (dx, dy)))
                .map(|(dx, dy)| u32::from(image.rgba_at(x + dx, y + dy)[3]))
                .sum()
        };
        let corner = block(columns[0], rows[0]);
        let edge = block(columns[wide / 2], rows[0]);
        assert!(
            corner < edge,
            "cell {cell:?} has square corners: {corner} at the corner, {edge} along the edge"
        );
    }
}

#[test]
fn the_painter_writes_only_what_changed() {
    use crate::graphics::{Mark, Painter, Protocol, Ring, Scene};
    use crate::primer::{Appearance, Palette, Season};

    let palette = Palette::new(Appearance::Dark, Season::Default, true);
    // A real year: a flat one compresses to almost nothing, which would make a
    // single repainted cell look expensive by comparison.
    let levels = levels_of(&synthetic(2025, "octocat"));
    let scene = |marks: [Option<Mark>; 2]| Scene {
        palette: &palette,
        grid: (5, 4),
        legend: Some((6, 16)),
        levels: levels.clone(),
        marks,
        key: 1,
    };
    let ring = |week, ring| {
        Some(Mark {
            week,
            weekday: 3,
            level: Some(2),
            ring,
        })
    };

    for protocol in [Protocol::Kitty, Protocol::Sixel] {
        let mut painter = Painter::new(protocol, (9, 19), palette.canvas);
        let mut out = Vec::new();
        painter.paint(&mut out, &scene([None, None])).unwrap();
        let first = out.len();
        assert!(first > 1000, "{protocol:?} drew nothing");

        // Nothing changed: nothing is sent. Redrawing the year on every frame would
        // be a flicker at best and a stall at worst.
        out.clear();
        painter.paint(&mut out, &scene([None, None])).unwrap();
        assert!(out.is_empty(), "{protocol:?} redrew an unchanged frame");

        // One hovered day costs one cell, not one year.
        out.clear();
        painter
            .paint(&mut out, &scene([None, ring(20, Ring::Hover)]))
            .unwrap();
        assert!(
            !out.is_empty() && out.len() < first / 10,
            "{protocol:?} spent {} bytes of {first} on one cell",
            out.len()
        );
        let moved = String::from_utf8_lossy(&out).into_owned();
        // Placed at the hovered cell: row 4 + weekday, column 5 + week * 2, both
        // one-based on the wire.
        assert!(
            moved.contains(&format!("\x1b[{};{}H", 4 + 3 + 1, 5 + 20 * 2 + 1)),
            "{moved:?}"
        );

        // Moving the pointer erases the old ring and draws the new one.
        out.clear();
        painter
            .paint(&mut out, &scene([None, ring(21, Ring::Hover)]))
            .unwrap();
        let text = String::from_utf8_lossy(&out).into_owned();
        assert!(
            text.contains(&format!("\x1b[{};{}H", 8, 5 + 21 * 2 + 1)),
            "{text:?}"
        );
        match protocol {
            // Kitty keeps the year underneath, so the ring only has to be deleted.
            Protocol::Kitty => assert!(text.contains("a=d,d=I,i=7383"), "{text:?}"),
            // Sixel has nothing underneath: the cell is blanked and painted again.
            Protocol::Sixel => assert!(text.contains("\x1b[0m  "), "{text:?}"),
        }

        // A new year is a new image.
        out.clear();
        let mut next = scene([None, None]);
        next.key = 2;
        painter.paint(&mut out, &next).unwrap();
        assert!(out.len() > first / 2, "a new year should be redrawn whole");

        // Taking the chart down happens once. Text cells are chosen for whole runs
        // of frames, and an escape per frame to delete images that are already gone
        // would be sent for every one of them.
        out.clear();
        painter.clear(&mut out).unwrap();
        let torn_down = out.len();
        out.clear();
        painter.clear(&mut out).unwrap();
        assert!(out.is_empty(), "{protocol:?} cleared an empty screen again");
        match protocol {
            Protocol::Kitty => assert!(torn_down > 0, "kitty holds images until told"),
            Protocol::Sixel => assert_eq!(torn_down, 0, "sixel has nothing to take down"),
        }
    }
}

// ---------------------------------------------------------------- pixels and mouse

/// An app as it would be on a terminal that draws pixels, without needing one.
fn on_a_graphics_terminal(calendar: Calendar) -> App {
    use crate::app::Options;
    use crate::primer::Rgb;
    use crate::term::Caps;

    let mut app = ready(calendar);
    app.configure(
        Caps {
            kitty: true,
            cell: Some((9, 19)),
            background: Some(Rgb::hex(0x0d1117)),
            answered: true,
            ..Caps::default()
        },
        Options::default(),
    );
    app
}

/// The rendered frame as plain lines, without `TestBackend`'s quoting.
fn rows(frame: &str) -> Vec<String> {
    frame
        .lines()
        .map(|line| line.trim().trim_matches('"').to_string())
        .collect()
}

#[test]
fn auto_takes_pixels_when_the_terminal_draws_them() {
    use crate::app::CellStyle;
    use crate::ui::{resolve, Cells};

    // Two columns per day and seven rows: the same footprint as squares, so
    // wherever squares fit, pixels do.
    assert_eq!(Cells::Pixels.width(53), 110);
    assert_eq!(Cells::Pixels.height(), 7);
    assert_eq!(Cells::Pixels.stride(), 2);

    let auto = |width, height, pixels| resolve(CellStyle::Auto, 53, width, height, pixels);
    assert_eq!(
        auto(240, 40, true),
        Cells::Pixels,
        "pixels beat every glyph"
    );
    assert_eq!(auto(110, 16, true), Cells::Pixels, "at their exact minimum");
    assert_eq!(
        auto(109, 40, true),
        Cells::Solid { fill: 1, gap: 0 },
        "too narrow for pixels, and for squares too"
    );
    assert_eq!(
        auto(240, 15, true),
        Cells::Solid { fill: 1, gap: 0 },
        "too short: the chrome does not fit around it"
    );
    assert_eq!(
        auto(240, 40, false),
        Cells::Rounded { gap: 1 },
        "without a protocol, the sextants are the closest thing"
    );
    // Asking for pixels on a terminal without them falls back rather than failing.
    assert_eq!(
        resolve(CellStyle::Pixels, 53, 240, 40, false),
        Cells::Rounded { gap: 1 }
    );
    assert_eq!(resolve(CellStyle::Pixels, 53, 240, 40, true), Cells::Pixels);
}

#[test]
fn the_pixel_grid_is_left_blank_for_the_painter_to_fill() {
    use crate::ui::Cells;

    let mut app = on_a_graphics_terminal(synthetic(2025, "octocat"));
    assert!(
        app.pixels_available(),
        "a kitty terminal should get a painter"
    );
    let frame = render(&mut app, 170, 24);
    let layout = app.layout.expect("the layout is recorded for the painter");
    assert_eq!(layout.cells, Cells::Pixels);
    assert_eq!(layout.weeks, 53);
    // Inside the frame's border, past the weekday gutter.
    assert_eq!(layout.x, 1 + 4);

    // Anything drawn in those rows would be drawn over the image — and under a
    // sixel, whatever is written over it wins for good.
    let lines = rows(&frame);
    for row in layout.y..layout.y + 7 {
        let line = &lines[row as usize];
        let body: String = line
            .chars()
            .skip(usize::from(layout.x))
            .take_while(|c| *c != '│')
            .collect();
        assert!(
            body.trim().is_empty(),
            "row {row} is not empty for the painter: {body:?}"
        );
    }
    // The legend reserves room for its swatches the same way.
    let (x, y) = layout.legend.expect("legend swatches are an image too");
    assert!(lines[y as usize].contains("Less"));
    assert_eq!(x, 1 + "Less ".len() as u16);
    assert!(frame.contains("pixel cells (kitty)"), "{frame}");
}

#[test]
fn the_mouse_finds_the_day_under_it() {
    use crate::ui::{Cells, Layout};

    // Every style is hit-tested with its own stride, and the bordered ones spend a
    // row on a rule between weekdays.
    let layout = |cells| Layout {
        x: 5,
        y: 4,
        weeks: 53,
        cells,
        legend: None,
        bottom: 40,
    };
    // Cells that would be drawn past the last row are not drawn: an image placed
    // there scrolls the screen out from under the rest of the frame.
    assert!(layout(Cells::Pixels).has_room());
    assert!(!Layout {
        y: 8,
        bottom: 12,
        ..layout(Cells::Pixels)
    }
    .has_room());

    for cells in [
        Cells::Pixels,
        Cells::Squares,
        Cells::Rounded { gap: 1 },
        Cells::Rounded { gap: 0 },
        Cells::Solid { fill: 2, gap: 1 },
        Cells::Solid { fill: 1, gap: 0 },
        Cells::Grid { fill: 2 },
    ] {
        let layout = layout(cells);
        let stride = cells.stride() as u16;
        let row = |weekday: u16| match cells {
            Cells::Grid { .. } => 4 + 1 + weekday * 2,
            _ => 4 + weekday,
        };
        assert_eq!(layout.hit(5, row(0)), Some((0, 0)), "{cells:?}");
        assert_eq!(
            layout.hit(5 + 20 * stride, row(3)),
            Some((20, 3)),
            "{cells:?}"
        );
        assert_eq!(
            layout.hit(5 + 52 * stride, row(6)),
            Some((52, 6)),
            "{cells:?}"
        );
        // Off the grid in every direction.
        assert_eq!(layout.hit(4, row(0)), None, "{cells:?} left of the grid");
        assert_eq!(layout.hit(5, 3), None, "{cells:?} above the grid");
        assert_eq!(
            layout.hit(5 + 53 * stride, row(0)),
            None,
            "{cells:?} past the last week"
        );
        assert_eq!(
            layout.hit(5, 4 + cells.height() as u16),
            None,
            "{cells:?} below the grid"
        );
    }
}

#[test]
fn hovering_a_day_reads_it_out_the_way_github_does() {
    use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

    let mut app = on_a_graphics_terminal(synthetic(2025, "octocat"));
    render(&mut app, 170, 24);
    let layout = app.layout.unwrap();

    let hover = |app: &mut App, week: u16, weekday: u16| {
        app.on_mouse(MouseEvent {
            kind: MouseEventKind::Moved,
            column: layout.x + week * layout.cells.stride() as u16,
            row: layout.y + weekday,
            modifiers: KeyModifiers::NONE,
        });
    };

    hover(&mut app, 20, 3);
    let Load::Ready(calendar) = &app.load else {
        unreachable!()
    };
    let day = calendar.weeks[20].days[3].unwrap();
    assert_eq!(app.hover, Some(day.date));

    let frame = render(&mut app, 170, 24);
    let wanted = match day.count {
        0 => "No contributions".to_string(),
        1 => "1 contribution".to_string(),
        n => format!("{n} contributions"),
    };
    let tip = format!("{wanted} on {} {}", day.date.format("%B"), day.date.day());
    assert!(frame.contains(&tip), "expected {tip:?} in:\n{frame}");

    // The tooltip floats above the grid, never over it: those rows are the image's.
    let lines = rows(&frame);
    assert!(
        lines[(layout.y - 2) as usize].contains(&tip),
        "the tooltip should sit two rows above the grid:\n{frame}"
    );
    assert!(
        lines[(layout.y - 1) as usize].contains('▼'),
        "and point down at the column it describes:\n{frame}"
    );
    // By character, not by byte: the frame is full of multi-byte box drawing.
    let pointer = lines[(layout.y - 1) as usize]
        .chars()
        .position(|c| c == '▼')
        .unwrap() as u16;
    assert!(
        pointer.abs_diff(layout.x + 20 * layout.cells.stride() as u16) <= 1,
        "pointer at {pointer}, cell at {}",
        layout.x + 40
    );

    // Ordinals, which is the fiddly half of GitHub's wording.
    for (day, suffix) in [
        (1u32, "1st"),
        (2, "2nd"),
        (3, "3rd"),
        (4, "4th"),
        (11, "11th"),
        (12, "12th"),
        (13, "13th"),
        (21, "21st"),
        (22, "22nd"),
        (23, "23rd"),
        (31, "31st"),
    ] {
        let date = NaiveDate::from_ymd_opt(2025, 12, day).unwrap();
        app.hover = Some(date);
        let frame = render(&mut app, 170, 24);
        assert!(
            frame.contains(&format!("on December {suffix}.")),
            "December {day} should read {suffix}:\n{}",
            rows(&frame)[(layout.y - 2) as usize]
        );
    }

    // Clicking moves the cursor; the detail line follows it, not the pointer.
    app.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: layout.x + 10 * layout.cells.stride() as u16,
        row: layout.y + 2,
        modifiers: KeyModifiers::NONE,
    });
    let Load::Ready(calendar) = &app.load else {
        unreachable!()
    };
    let clicked = calendar.weeks[10].days[2].unwrap().date;
    assert_eq!(app.cursor, clicked);

    // Off the grid, there is nothing to say.
    app.on_mouse(MouseEvent {
        kind: MouseEventKind::Moved,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.hover, None);
    let frame = render(&mut app, 170, 24);
    assert!(!frame.contains("▼"), "the tooltip should be gone:\n{frame}");
}

#[test]
fn a_day_still_to_come_has_no_tooltip() {
    use ratatui::crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};

    // The current year, whose tail has not happened: github.com draws nothing there
    // and says nothing about it.
    let year = Local::now().year();
    let mut app = ready(all_future(year));
    render(&mut app, 170, 24);
    let layout = app.layout.unwrap();
    app.on_mouse(MouseEvent {
        kind: MouseEventKind::Moved,
        column: layout.x + 30 * layout.cells.stride() as u16,
        row: layout.y + 3,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.hover, None);
    assert!(!render(&mut app, 170, 24).contains('▼'));
}

// ---------------------------------------------------------------- art

#[test]
fn letters_are_five_columns_with_one_between() {
    use crate::art;

    let a = art::bitmap("A").unwrap();
    assert_eq!(a.len(), 5, "one letter is five columns wide");
    // The glyph, read back out of the bitmap.
    let drawn: Vec<String> = (0..5)
        .map(|row| {
            a.iter()
                .map(|column| if column[row] { '#' } else { '.' })
                .collect()
        })
        .collect();
    assert_eq!(drawn, [".###.", "#...#", "#####", "#...#", "#...#"]);

    // 6N - 1: five columns a letter, one blank between.
    assert_eq!(art::bitmap("AB").unwrap().len(), 11);
    assert_eq!(art::bitmap("VYNCINT").unwrap().len(), 41);
    assert_eq!(art::bitmap("ABCDEFGHI").unwrap().len(), 53);
    assert!(art::bitmap("a").is_ok(), "case is not a distinction here");

    let error = art::bitmap("A+B").unwrap_err();
    assert!(error.contains('+'), "unknown glyphs are named: {error}");
    // Every character the font claims to have, it has.
    for character in art::alphabet() {
        assert!(art::glyph(character).is_some());
        assert_eq!(art::bitmap(&character.to_string()).unwrap().len(), 5);
    }
}

#[test]
fn a_year_that_is_full_of_letters_loses_its_edges() {
    use crate::art::{self, Grid};

    // 2027 starts on a Friday, so the first calendar column holds only Jan 1 on
    // the rows letters use. Nine letters need all 53 columns, so the leading
    // letter loses the part of itself that falls in 2026.
    let grid = Grid::new(2027).unwrap();
    assert_eq!(grid.weeks, 53);
    assert_eq!(
        grid.usable_weeks(),
        52,
        "the first column is a partial week"
    );

    let nine = art::place(
        &art::bitmap("ABCDEFGHI").unwrap(),
        &grid,
        1,
        None,
        art::Ink { lit: 4, field: 0 },
    )
    .unwrap();
    assert_eq!(nine.start_week, 0, "no room to centre 53 columns in 53");
    assert_eq!(nine.skipped, 3, "the A loses Dec 29-31 of the year before");

    // Eight fit whole in every year, which is what makes eight the limit.
    for year in 2024..=2032 {
        let grid = Grid::new(year).unwrap();
        let eight = art::place(
            &art::bitmap("ABCDEFGH").unwrap(),
            &grid,
            1,
            None,
            art::Ink { lit: 4, field: 0 },
        )
        .unwrap();
        assert_eq!(eight.skipped, 0, "{year} clipped an eight-letter word");
        assert_eq!(grid.usable_weeks() / 6, 8, "{year}");
    }

    // Rows are Mon-Fri, so Sunday and Saturday stay clear.
    let placed = art::place(
        &art::bitmap("A").unwrap(),
        &grid,
        1,
        Some(10),
        art::Ink { lit: 4, field: 0 },
    )
    .unwrap();
    for day in placed.lit.keys() {
        let weekday = day.weekday().num_days_from_sunday();
        assert!((1..=5).contains(&weekday), "{day} is on weekday {weekday}");
    }
    assert!(art::place(
        &art::bitmap("A").unwrap(),
        &grid,
        3,
        None,
        art::Ink { lit: 4, field: 0 }
    )
    .is_err());
}

#[test]
fn shading_is_priced_off_the_years_busiest_day() {
    use crate::art;

    // level = min(4, ceil(count * 4 / peak)), verified against a real calendar.
    assert_eq!(art::level(0, 100), 0);
    assert_eq!(art::level(1, 100), 1);
    assert_eq!(
        art::level(25, 100),
        1,
        "a quarter of the peak is still level 1"
    );
    assert_eq!(art::level(26, 100), 2);
    assert_eq!(art::level(75, 100), 3);
    assert_eq!(art::level(76, 100), 4, "three quarters buys the brightest");
    assert_eq!(art::level(100, 100), 4);
    assert_eq!(art::level(4, 4), 4, "an empty year sets its own peak");

    // Solved, not estimated: the art raises the peak it is measured against, so
    // matching a busy day takes more than that day's count.
    let days: Vec<NaiveDate> = (1..=5)
        .map(|day| NaiveDate::from_ymd_opt(2027, 3, day).unwrap())
        .collect();
    let mut existing = std::collections::BTreeMap::new();
    existing.insert(NaiveDate::from_ymd_opt(2027, 6, 1).unwrap(), 100);
    let need = art::commits_for_level(&days, &existing, 4).expect("reachable");
    assert!(
        need >= 75,
        "beating a peak of 100 takes at least three quarters of it, got {need}"
    );
    assert_eq!(
        art::level(need, need.max(100)),
        4,
        "and the answer actually reaches level 4"
    );
    // An empty year is cheap: one commit a day lights everything.
    assert_eq!(
        art::commits_for_level(&days, &std::collections::BTreeMap::new(), 4),
        Some(1)
    );
}

#[test]
fn a_snapshot_is_something_the_renderer_can_read_back() {
    use crate::art::{self, Grid};

    let grid = Grid::new(2027).unwrap();
    let placed = art::place(
        &art::bitmap("VYNCINT").unwrap(),
        &grid,
        1,
        None,
        art::Ink { lit: 4, field: 0 },
    )
    .unwrap();
    let body = art::snapshot(&placed.lit, &grid, "preview");

    let path = scratch("art-roundtrip.json");
    std::fs::write(&path, &body).unwrap();
    let calendar = crate::github::from_file(path.to_str().unwrap()).expect("parses");
    let _ = std::fs::remove_file(&path);

    assert_eq!(calendar.year, 2027);
    assert_eq!(calendar.login, "preview");
    assert_eq!(
        calendar.days().count(),
        365,
        "every day of the year is there"
    );
    assert_eq!(calendar.total, placed.lit.values().sum::<u32>());
    // The lit days are exactly the ones the bitmap named, at the top level since
    // nothing else is in the year.
    let lit: Vec<NaiveDate> = calendar
        .days()
        .filter(|day| day.count > 0)
        .map(|day| day.date)
        .collect();
    assert_eq!(lit, placed.lit.keys().copied().collect::<Vec<_>>());
    assert!(calendar
        .days()
        .all(|day| day.level == u8::from(day.count > 0) * 4));
}

// ---------------------------------------------------------------- png

#[test]
fn the_png_is_a_png() {
    use crate::graphics;
    use crate::primer::{Appearance, Palette, Rgb, Season};

    let palette = Palette::new(Appearance::Dark, Season::Default, true);
    let mut levels = vec![[None; 7]; 3];
    levels[0][0] = Some(4);
    levels[1][3] = Some(0);
    let image = graphics::grid(&levels, &palette, (10, 20));

    let path = scratch("png-test.png");
    crate::png::write(&path, &image, palette.canvas).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "signature");
    // IHDR: length, tag, then width/height/depth/colour.
    assert_eq!(&bytes[12..16], b"IHDR");
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap()) as usize;
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap()) as usize;
    assert_eq!((width, height), (image.width, image.height));
    assert_eq!(
        bytes[24..29],
        [8, 2, 0, 0, 0],
        "8-bit truecolour, no interlace"
    );

    // Every chunk's CRC has to check out, or half the viewers refuse the file.
    let mut at = 8;
    let mut chunks = Vec::new();
    while at + 8 <= bytes.len() {
        let length = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        let kind = String::from_utf8_lossy(&bytes[at + 4..at + 8]).into_owned();
        let body = &bytes[at + 8..at + 8 + length];
        let stated =
            u32::from_be_bytes(bytes[at + 8 + length..at + 12 + length].try_into().unwrap());
        let mut digest = crc32fast_like(&bytes[at + 4..at + 8]);
        digest = crc32fast_like_continue(digest, body);
        assert_eq!(stated, digest, "{kind} CRC");
        chunks.push(kind);
        at += 12 + length;
    }
    assert_eq!(chunks, ["IHDR", "IDAT", "IEND"]);
    assert_eq!(at, bytes.len(), "no trailing bytes");

    // And the pixels are the ones the rasteriser drew, composited on the canvas.
    let idat_start = 8 + 12 + 13 + 8;
    let idat_length =
        u32::from_be_bytes(bytes[8 + 12 + 13..8 + 16 + 13].try_into().unwrap()) as usize;
    let raw =
        miniz_oxide::inflate::decompress_to_vec_zlib(&bytes[idat_start..idat_start + idat_length])
            .expect("zlib");
    assert_eq!(raw.len(), height * (1 + width * 3));
    for y in 0..height {
        assert_eq!(raw[y * (1 + width * 3)], 0, "filter: none");
        for x in 0..width {
            let pixel = image.rgba_at(x, y);
            let alpha = f32::from(pixel[3]) / 255.0;
            let over = |channel: u8, under: u8| {
                (f32::from(channel) * alpha + f32::from(under) * (1.0 - alpha)).round() as u8
            };
            let at = y * (1 + width * 3) + 1 + x * 3;
            let want = [
                over(pixel[0], palette.canvas.0),
                over(pixel[1], palette.canvas.1),
                over(pixel[2], palette.canvas.2),
            ];
            assert_eq!(&raw[at..at + 3], &want, "pixel {x},{y}");
        }
    }
    let _ = Rgb::hex(0);
}

/// The CRC the tests check against, written out longhand so it is not the same
/// code under test.
fn crc32fast_like(data: &[u8]) -> u32 {
    crc32fast_like_continue(0, data)
}

fn crc32fast_like_continue(previous: u32, data: &[u8]) -> u32 {
    let mut crc = !previous;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let carry = crc & 1 == 1;
            crc >>= 1;
            if carry {
                crc ^= 0xEDB8_8320;
            }
        }
    }
    !crc
}

#[test]
fn the_demo_year_looks_like_a_real_one() {
    let year = 2025;
    let demo = crate::calendar::demo(year);

    // A whole year, every day of it elapsed: a demo of a half-drawn year would
    // demonstrate the wrong thing.
    assert_eq!(demo.days().count(), 365);
    assert_eq!(demo.login, "demo");
    assert_eq!(demo.year, year);
    assert!(demo.days().all(|day| !day.future));
    assert_eq!(demo.total, demo.days().map(|day| day.count).sum::<u32>());

    // Deterministic, so the screenshot in the README and the assertions here
    // describe the same year.
    let again = crate::calendar::demo(year);
    let (left, right): (Vec<_>, Vec<_>) = (
        demo.days().map(|day| (day.date, day.count)).collect(),
        again.days().map(|day| (day.date, day.count)).collect(),
    );
    assert_eq!(left, right);
    assert_ne!(
        crate::calendar::demo(year + 1)
            .days()
            .map(|day| day.count)
            .collect::<Vec<_>>(),
        demo.days().map(|day| day.count).collect::<Vec<_>>(),
        "a different year is a different year"
    );

    // Plausible: all five levels present, weekends quieter, a streak to show.
    let mut seen = [false; 5];
    for day in demo.days() {
        seen[usize::from(day.level)] = true;
    }
    assert!(seen.iter().all(|level| *level), "every shade should appear");

    let active = |weekend: bool| {
        demo.days()
            .filter(|day| {
                matches!(
                    day.date.weekday(),
                    chrono::Weekday::Sat | chrono::Weekday::Sun
                ) == weekend
                    && day.count > 0
            })
            .count() as f32
    };
    assert!(
        active(true) / 104.0 < active(false) / 261.0,
        "weekends should be quieter than weekdays"
    );
    let stats = demo.stats();
    assert!(stats.longest_streak >= 3, "a streak worth showing");
    assert!(stats.active_days > 150 && stats.active_days < 320);
}

#[test]
fn the_font_stays_readable_as_it_grows() {
    use crate::art::{self, GLYPH_COLS, GLYPH_ROWS};

    // The shape rules — every row present, uniform width, only '#' and '.' —
    // are checked when the crate compiles, so a contributed glyph that breaks
    // them never gets as far as a test. What is left is the part a compiler
    // cannot judge: whether the glyph is worth drawing.
    let mut shapes: Vec<(char, String)> = Vec::new();
    for character in art::alphabet() {
        let glyph = art::glyph(character).expect("the font lists it");
        let lit = glyph
            .iter()
            .map(|row| row.chars().filter(|c| *c == '#').count())
            .sum::<usize>();

        if character == ' ' {
            assert_eq!(lit, 0, "space draws nothing");
            continue;
        }
        assert!(lit > 0, "{character:?} would draw nothing at all");
        assert!(
            lit < GLYPH_ROWS * GLYPH_COLS,
            "{character:?} is a solid block, which reads as no character"
        );
        shapes.push((character, glyph.join("")));
    }

    // Two characters that draw the same pixels are indistinguishable once they
    // are on the graph — the one bug a new glyph is most likely to introduce.
    for (index, (character, shape)) in shapes.iter().enumerate() {
        for (other, other_shape) in &shapes[index + 1..] {
            assert_ne!(
                shape, other_shape,
                "{character:?} and {other:?} draw the same thing"
            );
        }
    }

    // The set the README and the error message promise. A *lower* bound on
    // purpose: adding a glyph is the point of the font, and a test that counted
    // them would fail on the one contribution it exists to protect.
    for expected in ('A'..='Z').chain('0'..='9').chain([' ', '-', '.']) {
        assert!(art::glyph(expected).is_some(), "the font lost {expected:?}");
    }
    assert!(art::alphabet().count() >= 39);
}

#[test]
fn a_glyph_is_looked_up_as_written_before_folding() {
    use crate::art;

    // Lowercase draws the uppercase glyph today, because the font has no
    // lowercase — but the exact character is tried first, so adding one later
    // is a table entry and nothing else.
    assert_eq!(art::glyph('a'), art::glyph('A'));
    assert_eq!(
        art::bitmap("vyncint").unwrap(),
        art::bitmap("VYNCINT").unwrap()
    );

    // And the message for a character nobody has drawn yet says what there is.
    let error = art::bitmap("A+B").unwrap_err();
    assert!(error.contains('+'), "{error}");
    assert!(
        error.contains("ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
        "it should name the alphabet: {error}"
    );
    assert!(
        error.contains("space"),
        "including the ones with no glyph: {error}"
    );
}

// ---------------------------------------------------------------- plan

#[test]
fn a_price_is_quoted_the_same_way_everywhere() {
    use crate::art::{commits_to_reach, level};

    // The formula and the shading agree, at every level and every peak.
    for peak in [1u32, 4, 7, 12, 99, 112, 146, 1_000] {
        for target in 1..=4u8 {
            let need = commits_to_reach(target, peak);
            assert!(
                level(need, peak.max(need)) >= target,
                "{need} should reach level {target} against a peak of {peak}"
            );
            if need > 1 {
                assert!(
                    level(need - 1, peak.max(need)) < target,
                    "{} should not reach level {target} against a peak of {peak}",
                    need - 1
                );
            }
        }
    }
    // The README's worked example.
    assert_eq!(commits_to_reach(4, 112), 85);
}

#[test]
fn tracking_says_whether_the_text_can_still_be_drawn() {
    use crate::art::{self, Grid};
    use crate::plan::{Plan, Verdict, Want};

    let year = 2027;
    let grid = Grid::new(year).unwrap();
    let columns = art::bitmap("HI").unwrap();
    let placed = art::place(&columns, &grid, 1, Some(10), art::Ink { lit: 4, field: 0 }).unwrap();
    let lit: Vec<NaiveDate> = placed.lit.keys().copied().collect();

    // An empty year: everything is owed, nothing is in the way.
    let empty = std::collections::BTreeMap::new();
    let plan = Plan::build(
        "HI",
        &grid,
        &placed,
        columns.len(),
        1,
        &empty,
        art::Shades::default(),
    );
    assert_eq!(plan.verdict(), Verdict::Reachable);
    assert_eq!(plan.bright(), 0);
    assert_eq!(plan.owing().0, lit.len());
    assert_eq!(
        plan.need, 1,
        "an empty year sets its own peak, so one will do"
    );

    // The same year with every letter day contributed to: drawn.
    let done: std::collections::BTreeMap<NaiveDate, u32> =
        lit.iter().map(|date| (*date, 4)).collect();
    let plan = Plan::build(
        "HI",
        &grid,
        &placed,
        columns.len(),
        1,
        &done,
        art::Shades::default(),
    );
    assert_eq!(plan.verdict(), Verdict::Done);
    assert_eq!(plan.bright(), lit.len());
    assert_eq!(plan.owing(), (0, 0));

    // One contribution on a day inside the letters that should be dark, and the
    // text can never be clean again — the thing no amount of committing fixes.
    let hole = (1..grid.weeks)
        .flat_map(|week| (1..6).map(move |row| (week, row)))
        .map(|(week, row)| grid.date_at(week, row))
        .find(|date| {
            grid.holds(*date) && !placed.lit.contains_key(date) && {
                let (week, _) = (
                    ((*date - grid.start).num_days() / 7) as usize,
                    date.weekday(),
                );
                (10..10 + columns.len()).contains(&week)
            }
        })
        .expect("a dark day inside the text");
    let mut spoiled = done.clone();
    spoiled.insert(hole, 1);
    let plan = Plan::build(
        "HI",
        &grid,
        &placed,
        columns.len(),
        1,
        &spoiled,
        art::Shades::default(),
    );
    assert_eq!(plan.verdict(), Verdict::Holed { holes: 1 });
    assert_eq!(plan.holes().len(), 1);
    assert_eq!(plan.holes()[0].date, hole);
    assert!(
        plan.holes()[0].short() == 0,
        "a hole is not a shortfall — it cannot be paid off"
    );

    // A busy day anywhere raises what every letter day costs.
    let mut busy = empty.clone();
    busy.insert(NaiveDate::from_ymd_opt(year, 6, 1).unwrap(), 112);
    let plan = Plan::build(
        "HI",
        &grid,
        &placed,
        columns.len(),
        1,
        &busy,
        art::Shades::default(),
    );
    assert_eq!(plan.need, 85, "the peak sets the price");
    assert_eq!(plan.peak, 112);
    assert_eq!(plan.peak_day, NaiveDate::from_ymd_opt(year, 6, 1));
    assert!(plan
        .days
        .iter()
        .any(|day| day.want == Want::Around && day.have == 112));
}

#[test]
fn tracking_splits_what_is_left_by_whether_it_has_happened() {
    use crate::art::{self, Grid};
    use crate::plan::Plan;

    let grid = Grid::new(2027).unwrap();
    let columns = art::bitmap("I").unwrap();
    let placed = art::place(&columns, &grid, 1, Some(2), art::Ink { lit: 4, field: 0 }).unwrap();
    let plan = Plan::build(
        "I",
        &grid,
        &placed,
        columns.len(),
        1,
        &Default::default(),
        art::Shades::default(),
    );

    let dates: Vec<NaiveDate> = plan.letters().map(|day| day.date).collect();
    let midpoint = dates[dates.len() / 2];
    let (past, _) = plan.overdue(midpoint);
    let (future, _) = plan.ahead(midpoint);
    assert_eq!(
        past + future,
        dates.len(),
        "every owed day is one or the other"
    );
    assert!(past > 0 && future > 0);
    assert_eq!(past, dates.iter().filter(|date| **date < midpoint).count());

    // A year that has not started owes nothing yet, and says so.
    assert!(!plan.under_way(NaiveDate::from_ymd_opt(2026, 8, 19).unwrap()));
    assert!(plan.under_way(midpoint));

    // The schedule never wanders outside the year it is for.
    let early = plan.schedule(NaiveDate::from_ymd_opt(2026, 12, 30).unwrap(), 5);
    assert!(early.iter().all(|day| day.date.year() == 2027));
    let late = plan.schedule(NaiveDate::from_ymd_opt(2027, 12, 30).unwrap(), 5);
    assert_eq!(late.len(), 2, "two days left in the year");
}

#[test]
fn the_emptiest_placement_is_the_one_suggested() {
    use crate::art::{self, Grid};
    use crate::plan::best_start_week;

    let grid = Grid::new(2027).unwrap();
    let columns = art::bitmap("O").unwrap();

    // Contributions clustered at the start of the year: the text should be
    // advised to move past them.
    let mut busy = std::collections::BTreeMap::new();
    for week in 0..6 {
        for row in 1..6 {
            let date = grid.date_at(week, row);
            if grid.holds(date) {
                busy.insert(date, 5);
            }
        }
    }
    let (best, holes) = best_start_week(&grid, columns.len(), 1, &columns, &busy, 0).unwrap();
    assert!(
        best >= 6,
        "it should clear the busy stretch, got week {best}"
    );
    assert_eq!(holes, 0, "and land somewhere with no holes at all");

    // With nothing in the way, the earliest placement wins — a stable answer
    // rather than an arbitrary one.
    let (best, holes) =
        best_start_week(&grid, columns.len(), 1, &columns, &Default::default(), 0).unwrap();
    assert_eq!((best, holes), (0, 0));

    // Text that cannot fit has no placement at all.
    let wide = art::bitmap("ABCDEFGHIJ").unwrap();
    assert!(best_start_week(&grid, wide.len(), 1, &wide, &Default::default(), 0).is_none());
}

// ---------------------------------------------------------------- untrusted input
//
// A contribution calendar is data from elsewhere: the API, or a file someone
// sent you. Each of these is a thing that got through once.

#[test]
fn control_characters_never_leave_the_parser() {
    // An ESC in a login is a title change, a cursor-position report typed back
    // into the application, or an OSC 52 clipboard write — on any path that
    // prints to stdout rather than through the renderer's cell grid.
    let evil = "\u{1b}]0;PWNED\u{7}\u{1b}[6nme";
    let body = format!(
        r#"{{"data":{{"user":{{"login":{},"contributionsCollection":{{
        "contributionYears":[2027],"contributionCalendar":{{"totalContributions":1,
        "weeks":[{{"contributionDays":[
          {{"date":"2027-06-01","contributionCount":1,"contributionLevel":"NONE"}}
        ]}}]}}}}}}}},"errors":null}}"#,
        serde_json::to_string(evil).unwrap()
    );
    let path = scratch("evil-login.json");
    std::fs::write(&path, body).unwrap();
    let calendar = crate::github::from_file(path.to_str().unwrap()).expect("parses");
    let _ = std::fs::remove_file(&path);

    assert!(
        !calendar.login.chars().any(char::is_control),
        "the login still carries control characters: {:?}",
        calendar.login
    );
    assert!(
        calendar.login.contains("PWNED"),
        "the text itself is harmless"
    );

    // And the same for an error message, which is server-controlled text.
    let body = "{\"data\":null,\"errors\":[{\"message\":\"bad\u{1b}]0;PWNED\"}]}";
    let path = scratch("evil-error.json");
    std::fs::write(&path, body).unwrap();
    let error = crate::github::from_file(path.to_str().unwrap()).unwrap_err();
    let _ = std::fs::remove_file(&path);
    assert!(!error.chars().any(char::is_control), "{error:?}");

    assert_eq!(crate::printable("a\u{1b}[31mb\u{7}c"), "a[31mbc");
    assert_eq!(crate::printable("héllo ✓"), "héllo ✓", "text is left alone");
}

#[test]
fn a_calendar_cannot_span_more_than_a_year() {
    // Two dates millennia apart used to size the grid by the distance between
    // them: a few hundred bytes of JSON asking for gigabytes of Vec.
    let body = r#"{"data":{"user":{"login":"x","contributionsCollection":{
        "contributionYears":[2027],"contributionCalendar":{"totalContributions":2,
        "weeks":[{"contributionDays":[
          {"date":"2027-01-01","contributionCount":1,"contributionLevel":"NONE"},
          {"date":"9999-12-31","contributionCount":1,"contributionLevel":"NONE"}
        ]}]}}}},"errors":null}"#;
    let path = scratch("far-dates.json");
    std::fs::write(&path, body).unwrap();
    let error = crate::github::from_file(path.to_str().unwrap()).unwrap_err();
    let _ = std::fs::remove_file(&path);
    assert!(error.contains("spans"), "{error}");
    assert!(error.contains("a year is at most 366"), "{error}");

    // The constructor is public, so it holds the line on its own too.
    let far = vec![
        Day {
            date: NaiveDate::from_ymd_opt(2027, 1, 1).unwrap(),
            count: 1,
            level: 1,
            future: false,
        },
        Day {
            date: NaiveDate::from_ymd_opt(9999, 12, 31).unwrap(),
            count: 1,
            level: 1,
            future: false,
        },
    ];
    let calendar = Calendar::build("x".into(), 2027, 2, vec![2027], far);
    assert!(
        calendar.weeks.len() <= 60,
        "the grid grew to {} columns",
        calendar.weeks.len()
    );
}

#[test]
fn counts_from_elsewhere_cannot_overflow_the_shading() {
    use crate::art::{commits_to_reach, level};

    // Debug builds panic on overflow; release wraps and quietly reports the
    // wrong number. Neither is acceptable for arithmetic on parsed input.
    assert_eq!(level(u32::MAX, u32::MAX), 4);
    assert_eq!(level(u32::MAX, 1), 4);
    assert_eq!(level(1, u32::MAX), 1);
    assert!(commits_to_reach(4, u32::MAX) > 0);
    assert_eq!(commits_to_reach(1, u32::MAX), 1);

    // A calendar of enormous days still totals to something finite.
    let days: Vec<Day> = (1..=3)
        .map(|day| Day {
            date: NaiveDate::from_ymd_opt(2027, 1, day).unwrap(),
            count: u32::MAX,
            level: 4,
            future: false,
        })
        .collect();
    let calendar = Calendar::build("x".into(), 2027, u32::MAX, vec![2027], days);
    let stats = calendar.stats();
    assert_eq!(stats.active_days, 3);
    assert_eq!(stats.best.unwrap().1, u32::MAX);
}

#[test]
fn a_reported_cell_size_has_a_ceiling() {
    use crate::term::{self, Caps, MAX_CELL};

    // The cell size sizes an image allocation, and it arrives from the terminal
    // — or from whatever is pretending to be one.
    let absurd = Caps {
        cell: Some((20_000, 20_000)),
        ..Caps::default()
    };
    assert_eq!(
        term::cell_size(&absurd),
        None,
        "a cell of 20,000 pixels would ask for a terabyte of image"
    );

    let sane = Caps {
        cell: Some((9, 19)),
        ..Caps::default()
    };
    // Only when the ioctl did not already answer, which it will not off a tty.
    if let Some(cell) = term::cell_size(&sane) {
        assert!(cell.0 <= MAX_CELL && cell.1 <= MAX_CELL);
    }
}

#[test]
fn a_commit_identity_cannot_carry_a_newline() {
    use std::collections::BTreeMap;

    // An identity is written into the fast-import stream as a line of its own.
    let mut lit = BTreeMap::new();
    lit.insert(NaiveDate::from_ymd_opt(2027, 6, 1).unwrap(), 1);
    let repo = scratch("ident-test");

    for (name, email) in [
        ("evil\ncommit refs/heads/backdoor", "x@x.invalid"),
        ("evil", "x@x> 0 +0000\ncommit refs/heads/backdoor"),
        ("evil\r", "x@x.invalid"),
    ] {
        let error = crate::art::write_commits(&lit, &repo, "art", name, email).unwrap_err();
        assert!(
            error.contains("may not contain control characters"),
            "{name:?}/{email:?} was accepted: {error}"
        );
    }
    assert!(
        !repo.exists(),
        "nothing should have been created for a refused identity"
    );
}

// --------------------------------------------------------------- shades
//
// Drawing art by leaving days empty tests one thing: does a day have commits.
// Drawing it as a background shade under a brighter one tests something
// harder — can a reader tell the two greens apart. That is a question about
// colour, not about counts, so these measure it in CIELAB and they measure it
// in every palette GitHub ships, because the answer differs in each.

/// Every palette a reader might have the graph open in, named so a failure
/// says which one broke.
fn readers() -> Vec<(String, crate::primer::Palette)> {
    use crate::primer::{Appearance, Palette, Season};
    let mut out = Vec::new();
    for appearance in [Appearance::Light, Appearance::Dark, Appearance::Dimmed] {
        for season in [Season::Default, Season::Winter, Season::Halloween] {
            out.push((
                format!("{appearance:?}/{season:?}"),
                Palette::new(appearance, season, true),
            ));
        }
    }
    out
}

#[test]
fn lab_puts_the_familiar_colours_where_they_belong() {
    use crate::primer::Rgb;

    let (l, a, b) = Rgb::hex(0xffffff).lab();
    assert!((l - 100.0).abs() < 0.01, "white is L*100, got {l}");
    assert!(
        a.abs() < 0.01 && b.abs() < 0.01,
        "white is neutral, got {a},{b}"
    );

    let (l, _, _) = Rgb::hex(0x000000).lab();
    assert!(l.abs() < 0.01, "black is L*0, got {l}");

    // Mid grey sits near L*54, not L*50: the scale is perceptual, and this is
    // the whole reason the metric is not a subtraction of RGB channels.
    let (l, _, _) = Rgb::hex(0x808080).lab();
    assert!((l - 53.6).abs() < 0.5, "mid grey is about L*53.6, got {l}");

    // Blue is far up the b* axis in the negative direction; green down a*.
    let (_, _, blue_b) = Rgb::hex(0x0000ff).lab();
    let (_, green_a, _) = Rgb::hex(0x00ff00).lab();
    assert!(blue_b < -100.0, "blue is strongly -b*, got {blue_b}");
    assert!(green_a < -70.0, "green is strongly -a*, got {green_a}");
}

#[test]
fn separation_behaves_like_a_distance() {
    use crate::primer::Rgb;

    let a = Rgb::hex(0x196c2e);
    let b = Rgb::hex(0x56d364);
    let c = Rgb::hex(0xffffff);

    assert_eq!(a.separation(a), 0.0, "no distance to itself");
    assert!(
        (a.separation(b) - b.separation(a)).abs() < 1e-4,
        "symmetric"
    );
    assert!(
        a.separation(c) <= a.separation(b) + b.separation(c) + 1e-3,
        "triangle inequality"
    );

    // The failure this whole feature exists to prevent: two of GitHub's greens
    // that are 22 apart in one RGB channel and nearly the same to look at.
    let close = Rgb::hex(0x033a16).separation(Rgb::hex(0x196c2e));
    let far = Rgb::hex(0x151b23).separation(Rgb::hex(0x56d364));
    assert!(close < far / 2.0, "{close} should be far under {far}");
}

#[test]
fn shades_two_levels_apart_are_legible_in_every_palette() {
    use crate::primer::Legibility;

    let mut worst = f32::INFINITY;
    let mut worst_where = String::new();
    for (name, palette) in readers() {
        for field in 0u8..=4 {
            for ink in 0u8..=4 {
                if ink < field + 2 {
                    continue;
                }
                let delta = palette.separation(field, ink);
                assert!(
                    Legibility::of(delta) == Legibility::Clear,
                    "{name}: levels {field} and {ink} are only ΔE {delta:.1} apart"
                );
                if delta < worst {
                    worst = delta;
                    worst_where = format!("{name} levels {field}/{ink}");
                }
            }
        }
    }
    // Pinned, because the docs and the CLI's advice quote this number. If a
    // palette changes under us, this is the test that should say so.
    assert!(
        (35.0..36.0).contains(&worst),
        "the tightest two-level gap is {worst:.1} ({worst_where}); the guidance \
         everywhere says 35.4"
    );
}

#[test]
fn adjacent_shades_are_not_always_legible_which_is_why_the_rule_exists() {
    use crate::primer::Legibility;

    let mut tightest = f32::INFINITY;
    let mut faint = Vec::new();
    for (name, palette) in readers() {
        for level in 0u8..4 {
            let delta = palette.separation(level, level + 1);
            tightest = tightest.min(delta);
            if Legibility::of(delta) == Legibility::Faint {
                faint.push(format!("{name} {level}/{level}+1 ΔE {delta:.1}"));
            }
        }
    }
    assert!(
        !faint.is_empty(),
        "if no adjacent pair were faint the two-level rule would be superstition"
    );
    assert!(
        tightest < 11.0,
        "the tightest adjacent pair is ΔE {tightest:.1}; the docs say 10.8"
    );
}

#[test]
fn shades_refuse_what_cannot_be_drawn() {
    use crate::art::Shades;

    assert!(Shades { ink: 4, field: 0 }.check().is_ok());
    assert!(
        Shades { ink: 4, field: 3 }.check().is_ok(),
        "faint, but drawable"
    );

    // Equal shades are not faint art, they are a blank graph.
    let same = Shades { ink: 2, field: 2 }.check().unwrap_err();
    assert!(same.contains("darker"), "{same}");
    let inverted = Shades { ink: 1, field: 3 }.check().unwrap_err();
    assert!(inverted.contains("darker"), "{inverted}");

    let dark = Shades { ink: 0, field: 0 }.check().unwrap_err();
    assert!(dark.contains("level 0"), "{dark}");
    let wild = Shades { ink: 9, field: 0 }.check().unwrap_err();
    assert!(wild.contains("0 to 4"), "{wild}");
}

#[test]
fn a_background_needs_a_peak_that_can_express_it() {
    use crate::art::{level, Shades};

    let plain = Shades { ink: 4, field: 0 };
    let with_field = Shades { ink: 4, field: 1 };
    assert_eq!(
        plain.min_peak(),
        1,
        "letters against nothing need no headroom"
    );
    assert_eq!(with_field.min_peak(), 4);

    // The bug this guards: at a peak of 1 there are two shades in the whole
    // year, so both the letters and the background round to one commit and the
    // art disappears.
    let cramped = with_field.commits(1);
    assert_eq!(
        level(cramped.lit, 1),
        level(cramped.field, 1),
        "a peak of 1 cannot tell them apart — which is why min_peak exists"
    );

    // At the floor it can, and every level gets its own commit count.
    let ink = with_field.commits(with_field.min_peak());
    assert_eq!((ink.lit, ink.field), (4, 1));
    for level_wanted in 1u8..=4 {
        let count = crate::art::commits_to_reach(level_wanted, 4);
        assert_eq!(
            level(count, 4),
            level_wanted,
            "at a peak of 4, {count} commits should be level {level_wanted}"
        );
    }
}

#[test]
fn a_background_day_has_a_ceiling_as_well_as_a_floor() {
    use crate::art::{level, Shades};

    for peak in [4u32, 7, 40, 112, 1_000] {
        for field in 1u8..=3 {
            let shades = Shades { ink: 4, field };
            let floor = shades.commits(peak).field;
            let ceiling = shades.ceiling(peak).expect("a field shade has a ceiling");

            assert_eq!(
                level(floor, peak),
                field,
                "peak {peak}: {floor} commits should be exactly level {field}"
            );
            assert_eq!(
                level(ceiling, peak),
                field,
                "peak {peak}: the ceiling {ceiling} is still level {field}"
            );
            assert!(
                level(ceiling + 1, peak) > field,
                "peak {peak}: one past the ceiling must change shade"
            );
            assert!(floor <= ceiling, "peak {peak}: the band cannot be empty");
        }
    }

    // With no background the ceiling is zero: the day has to stay dark.
    assert_eq!(Shades { ink: 4, field: 0 }.ceiling(112), Some(0));
}

#[test]
fn a_year_with_a_background_holds_exactly_two_colours() {
    use crate::art::{self, Grid, Shades};
    use crate::primer::Legibility;
    use std::collections::BTreeSet;

    let grid = Grid::new(2027).unwrap();
    let columns = art::bitmap("VYNCINT").unwrap();
    let shades = Shades { ink: 4, field: 1 };
    let placed = art::place(&columns, &grid, 1, None, shades.commits(4)).unwrap();

    assert_eq!(placed.lit.len(), 75, "the letters");
    assert_eq!(
        placed.field.len(),
        365 - 75,
        "and every other day of the year"
    );
    assert!(
        placed.lit.keys().all(|day| !placed.field.contains_key(day)),
        "no day is both a letter and its own background"
    );

    let shading = art::shading(&placed, shades);
    assert_eq!(shading.len(), 365, "the whole year is spoken for");

    // The point of the whole feature: what a reader sees is two colours, and
    // neither of them is "nothing".
    for (name, palette) in readers() {
        let colours: BTreeSet<(u8, u8, u8)> = shading
            .values()
            .map(|level| {
                let rgb = palette.cell(*level);
                (rgb.0, rgb.1, rgb.2)
            })
            .collect();
        assert_eq!(colours.len(), 2, "{name}: the year should be two colours");

        let letter = palette.cell(shades.ink);
        let field = palette.cell(shades.field);
        assert_ne!(letter, field, "{name}");
        assert_ne!(
            field,
            palette.cell(0),
            "{name}: the background must not be the empty colour"
        );

        let delta = letter.separation(field);
        assert_eq!(
            Legibility::of(delta),
            Legibility::Clear,
            "{name}: the letters are only ΔE {delta:.1} from the field they sit on"
        );
    }

    // And every letter day really is the brighter of the two.
    for date in placed.lit.keys() {
        assert_eq!(shading[date], shades.ink, "{date}");
    }
    for date in placed.field.keys() {
        assert_eq!(shading[date], shades.field, "{date}");
    }
}

#[test]
fn the_shades_survive_a_round_trip_through_githubs_own_encoding() {
    use crate::art::{self, Grid, Shades};
    use std::collections::BTreeSet;

    // The commit counts are only a means: what has to come out right is the
    // *shade* GitHub computes from them. So write the calendar the way the API
    // serves it, read it back through the real parser, and compare colours.
    let grid = Grid::new(2027).unwrap();
    let columns = art::bitmap("HI").unwrap();
    let shades = Shades { ink: 4, field: 1 };
    let placed = art::place(&columns, &grid, 1, Some(10), shades.commits(4)).unwrap();

    let path = scratch("shades-roundtrip.json");
    std::fs::write(&path, art::snapshot(&placed.all(), &grid, "preview")).unwrap();
    let calendar = crate::github::from_file(path.to_str().unwrap());
    let _ = std::fs::remove_file(&path);
    let calendar = calendar.expect("the snapshot should parse");

    let intended = art::shading(&placed, shades);
    let mut seen = BTreeSet::new();
    let mut checked = 0;
    for day in calendar.days() {
        let want = intended.get(&day.date).copied().unwrap_or(0);
        assert_eq!(
            day.level, want,
            "{}: {} commits came back as level {}, wanted {want}",
            day.date, day.count, day.level
        );
        seen.insert(day.level);
        checked += 1;
    }
    assert_eq!(checked, 365, "every day of the year made the round trip");
    assert_eq!(
        seen,
        BTreeSet::from([1, 4]),
        "the year should hold the two shades asked for and nothing else"
    );
}

#[test]
fn tracking_a_background_measures_both_shades() {
    use crate::art::{self, Grid, Shades};
    use crate::plan::{Plan, Verdict, Want};
    use std::collections::BTreeMap;

    let grid = Grid::new(2027).unwrap();
    let columns = art::bitmap("HI").unwrap();
    let shades = Shades { ink: 4, field: 1 };
    let placed = art::place(&columns, &grid, 1, Some(10), shades.commits(4)).unwrap();
    let letters: Vec<NaiveDate> = placed.lit.keys().copied().collect();

    // An empty year owes the letters *and* the paper they are printed on.
    let plan = Plan::build(
        "HI",
        &grid,
        &placed,
        columns.len(),
        1,
        &BTreeMap::new(),
        shades,
    );
    assert_eq!(plan.verdict(), Verdict::Reachable);
    assert_eq!(plan.owing().0, letters.len(), "every letter day is short");
    assert_eq!(
        plan.field_owing().0,
        365 - letters.len(),
        "and so is every background day"
    );
    assert_eq!(plan.field_need, 1);
    assert_eq!(plan.field_ceiling, Some(1), "one commit, and no more");

    // Fill only the letters: with a background asked for, that is not finished.
    let mut lit_only: BTreeMap<NaiveDate, u32> = letters.iter().map(|day| (*day, 4)).collect();
    let plan = Plan::build("HI", &grid, &placed, columns.len(), 1, &lit_only, shades);
    assert_eq!(plan.bright(), letters.len());
    assert_eq!(
        plan.verdict(),
        Verdict::Reachable,
        "the letters are drawn but the field is bare"
    );

    // Fill the background too, and it is done.
    let mut whole = lit_only.clone();
    for (date, count) in &placed.field {
        whole.insert(*date, *count);
    }
    let plan = Plan::build("HI", &grid, &placed, columns.len(), 1, &whole, shades);
    assert_eq!(
        plan.verdict(),
        Verdict::Done,
        "letters and field both laid down"
    );
    assert_eq!(plan.field_owing(), (0, 0));

    // A background day inside the letters that runs away is a hole, exactly as
    // a lit day would have been without a background — the damage is the same,
    // it is only arrived at by being the wrong *colour* rather than by being
    // lit at all.
    let inside = grid.date_at(10, 1); // top-left of the block; H's stem is lit
    let hole_at = (10..16)
        .flat_map(|week| (1..6).map(move |row| (week, row)))
        .map(|(week, row)| grid.date_at(week, row))
        .find(|date| !placed.lit.contains_key(date))
        .expect("HI has gaps inside its block");
    assert!(grid.holds(inside));
    lit_only.insert(hole_at, 400);
    let plan = Plan::build("HI", &grid, &placed, columns.len(), 1, &lit_only, shades);
    match plan.verdict() {
        Verdict::Holed { holes } => assert_eq!(holes, 1),
        other => panic!("a runaway background day inside the letters is a hole, got {other:?}"),
    }
    let hole = plan.on(hole_at).unwrap();
    assert!(hole.over() > 0, "it is past its ceiling");
    assert_eq!(hole.want, Want::Hole);
}

#[test]
fn without_a_background_a_quiet_day_outside_is_still_nobodys_business() {
    use crate::art::{self, Grid, Shades};
    use crate::plan::Plan;
    use std::collections::BTreeMap;

    // The old behaviour has to survive: with no background, days outside the
    // text are noise, not debt. Setting a background is what makes the rest of
    // the year part of the picture.
    let grid = Grid::new(2027).unwrap();
    let columns = art::bitmap("HI").unwrap();
    let placed = art::place(&columns, &grid, 1, Some(10), art::Ink { lit: 4, field: 0 }).unwrap();
    let outside = grid.date_at(2, 3);
    let actual = BTreeMap::from([(outside, 9)]);

    let plain = Plan::build(
        "HI",
        &grid,
        &placed,
        columns.len(),
        1,
        &actual,
        Shades::default(),
    );
    assert_eq!(plain.field_owing(), (0, 0), "nothing is owed out there");
    assert_eq!(plain.around(), 1, "but it is counted as noise");
    assert_eq!(
        plain.on(outside).unwrap().ceiling,
        None,
        "and it has no ceiling"
    );

    let with_field = Plan::build(
        "HI",
        &grid,
        &placed,
        columns.len(),
        1,
        &actual,
        Shades { ink: 4, field: 1 },
    );
    assert!(
        with_field.field_owing().0 > 300,
        "with a background the whole year is owed something"
    );
    assert!(
        with_field.on(outside).unwrap().over() > 0,
        "and a day nine times too bright is now visible damage"
    );
}

#[test]
fn a_background_lets_the_letters_land_where_a_bare_graph_could_not() {
    use crate::art::{self, Grid};
    use crate::plan::best_start_week;
    use std::collections::BTreeMap;

    // A year with light activity everywhere. Against an empty background every
    // one of those days is a hole; against a level-1 field they are exactly
    // what the field wants, and the text can be placed anywhere.
    let grid = Grid::new(2027).unwrap();
    let columns = art::bitmap("HI").unwrap();
    let mut busy = BTreeMap::new();
    let mut date = grid.first;
    while date <= grid.last {
        busy.insert(date, 1);
        date = date.succ_opt().unwrap();
    }

    let (_, bare_holes) = best_start_week(&grid, columns.len(), 1, &columns, &busy, 0).unwrap();
    assert!(
        bare_holes > 0,
        "on a bare graph every quiet day inside the block is a hole"
    );

    let (_, hidden) = best_start_week(&grid, columns.len(), 1, &columns, &busy, 1).unwrap();
    assert_eq!(
        hidden, 0,
        "a level-1 background is where those days belong, so nothing is a hole"
    );
}

#[test]
fn a_backfill_finishes_the_plan_without_moving_the_target() {
    use crate::art::{self, Grid};
    use crate::plan::Plan;
    use std::collections::BTreeMap;

    // The property the whole mode rests on. Topping every short day up to what
    // it needs must finish the plan *in one pass* — and it can, because `need`
    // never exceeds the year's peak: the brightest shade is three quarters of
    // it. A flat `--write` has no such guarantee, which is why adding a uniform
    // count to an active year raises the peak and moves the bar it was aiming
    // at.
    let grid = Grid::new(2026).unwrap();
    let columns = art::bitmap("VYNCINT").unwrap();
    let shades = art::Shades::default();
    let placed = art::place(&columns, &grid, 1, Some(6), shades.commits(4)).unwrap();

    // A year with real activity in it, including one big day that sets the price.
    let mut actual: BTreeMap<NaiveDate, u32> = BTreeMap::new();
    let mut date = grid.first;
    let mut roll = 7u32;
    while date <= grid.last {
        roll = roll.wrapping_mul(31).wrapping_add(17);
        if !roll.is_multiple_of(3) {
            actual.insert(date, 1 + roll % 40);
        }
        date = date.succ_opt().unwrap();
    }
    actual.insert(NaiveDate::from_ymd_opt(2026, 8, 11).unwrap(), 146);

    let before = Plan::build("VYNCINT", &grid, &placed, columns.len(), 1, &actual, shades);
    let need = before.need;
    assert!(before.owing().0 > 0, "there is work to do");

    // Exactly what the backfill would write: each day's shortfall, nothing on a
    // day already there, and nothing at all on a day that must stay dark.
    let owed: BTreeMap<NaiveDate, u32> = before
        .days
        .iter()
        .filter(|day| day.short() > 0)
        .map(|day| (day.date, day.short()))
        .collect();
    assert!(
        owed.keys().all(|date| before
            .on(*date)
            .is_some_and(|day| day.want == crate::plan::Want::Lit)),
        "with no background drawn, only letter days are ever owed anything"
    );

    let mut after_counts = actual.clone();
    for (date, extra) in &owed {
        *after_counts.entry(*date).or_insert(0) += extra;
    }
    let after = Plan::build(
        "VYNCINT",
        &grid,
        &placed,
        columns.len(),
        1,
        &after_counts,
        shades,
    );

    assert_eq!(
        after.need, need,
        "the price did not move, so one pass is enough"
    );
    assert_eq!(after.peak, before.peak, "and neither did the year's peak");
    assert_eq!(after.owing(), (0, 0), "every letter day is bright");
    assert_eq!(
        after.bright(),
        after.letters().count(),
        "all of them, not most"
    );
    // The holes were holes before and are holes still: nothing here pretends to
    // fix the one thing that cannot be fixed.
    assert_eq!(after.holes().len(), before.holes().len());
}

#[test]
fn a_day_inside_the_letters_is_kept_so_it_can_be_warned_about() {
    use crate::art::{self, Grid};
    use crate::plan::{Plan, Want};
    use std::collections::BTreeMap;

    // A clean day inside the text block used to be dropped from the plan for
    // having nothing to say, which left `Plan::on` answering `None` and the
    // report describing it as a day outside the text — free to commit on. It is
    // the opposite of free: it is the only day whose loss is permanent.
    let grid = Grid::new(2027).unwrap();
    let columns = art::bitmap("I").unwrap();
    let shades = art::Shades::default();
    let placed = art::place(&columns, &grid, 1, Some(2), shades.commits(4)).unwrap();
    let plan = Plan::build(
        "I",
        &grid,
        &placed,
        columns.len(),
        1,
        &BTreeMap::new(),
        shades,
    );

    // `I` is five columns of five rows; 13 of those 25 are lit — two full bars
    // and a stem — so 12 must stay dark, and every one of them has to be in the
    // plan for the report to be able to mention it.
    assert_eq!(placed.lit.len(), 13, "the glyph");
    let dark: Vec<_> = plan
        .days
        .iter()
        .filter(|day| day.want == Want::Hole)
        .collect();
    assert_eq!(dark.len(), 12, "the negative space inside it");
    assert!(
        dark.iter()
            .all(|day| day.need == 0 && day.ceiling == Some(0)),
        "nothing is owed on them, and nothing may be spent either"
    );
    assert!(
        dark.iter().all(|day| plan.on(day.date).is_some()),
        "which is only useful if the report can find them"
    );

    // None of that may count as damage, or an untouched year would read as
    // ruined.
    assert_eq!(plan.holes().len(), 0);
    assert_eq!(plan.around(), 0);
    assert_eq!(plan.verdict(), crate::plan::Verdict::Reachable);
    assert_eq!(plan.field().count(), 0, "no background was asked for");

    // A contribution on one of them is what turns it into a hole.
    let spoiled: BTreeMap<NaiveDate, u32> = [(dark[0].date, 1)].into_iter().collect();
    let holed = Plan::build("I", &grid, &placed, columns.len(), 1, &spoiled, shades);
    assert_eq!(holed.holes().len(), 1);
    assert!(matches!(
        holed.verdict(),
        crate::plan::Verdict::Holed { holes: 1 }
    ));
}

#[test]
fn place_refuses_a_start_column_that_would_not_fit() {
    use crate::art::{self, Grid};

    // Past the last column that fits, every pixel falls outside the year: the
    // old answer was a note about dropped pixels and a plan of no days at all,
    // and far enough out — `--start-week -1`, cast to a `usize` — building the
    // date panicked instead.
    let grid = Grid::new(2027).unwrap();
    let columns = art::bitmap("VYNCINT").unwrap();
    let ink = art::Ink { lit: 4, field: 0 };

    let last = grid.weeks - columns.len();
    assert!(
        art::place(&columns, &grid, 1, Some(last), ink).is_ok(),
        "the last column that fits, fits"
    );

    let error =
        art::place(&columns, &grid, 1, Some(last + 1), ink).expect_err("one past it does not");
    assert!(error.contains("past the end of 2027"), "{error}");
    assert!(
        error.contains(&format!("the last one that fits is {last}")),
        "{error}"
    );

    // The value that used to panic, rather than merely draw nothing.
    let error = art::place(&columns, &grid, 1, Some(usize::MAX), ink)
        .expect_err("and neither does usize::MAX");
    assert!(error.contains("past the end of"), "{error}");
}

#[test]
fn a_plan_file_is_input_and_is_bounded_like_one() {
    use crate::plan::Spec;

    // `--plan PATH` names a file, and a file need not have come from your own
    // `--save`, so a plan was the way around every bound the command line
    // enforces. Each of these reached past a different guard.
    let sound = Spec {
        text: "HI".to_string(),
        year: 2027,
        start_week: 10,
        top: 1,
        commits: 4,
        background: 0,
        user: None,
    };
    assert!(sound.validate().is_ok());

    // `usize::MAX + GLYPH_ROWS` wraps to 4, which passes a `> WEEKDAYS` check
    // comfortably, and the rows were then drawn wherever the wrapping landed.
    let mut wild = sound.clone();
    wild.top = usize::MAX;
    let why = wild.validate().expect_err("top");
    assert!(
        why.contains("18446744073709551615"),
        "names the value: {why}"
    );
    assert!(why.contains("between 0 and 2"), "{why}");

    // Four billion commits a day, quoted in full.
    let mut wild = sound.clone();
    wild.commits = u32::MAX;
    assert!(wild
        .validate()
        .expect_err("commits")
        .contains("between 1 and 1000000"));

    // A year no calendar can hold used to panic building the grid, and one
    // merely absurd drew a calendar `cli::YEARS` exists to refuse.
    for year in [-262143, 0, 1999, 2101, 180_000] {
        let mut wild = sound.clone();
        wild.year = year;
        let why = wild.validate().expect_err("year");
        assert!(why.contains("between 2000 and 2100"), "year {year}: {why}");
    }

    let mut wild = sound.clone();
    wild.background = 9;
    assert!(wild
        .validate()
        .expect_err("background")
        .contains("between 0 and 4"));

    // The bound the flag applies loosely, so `place` can name the exact column.
    let mut wild = sound.clone();
    wild.start_week = usize::MAX;
    assert!(wild
        .validate()
        .expect_err("start_week")
        .contains("between 0 and 60"));
}

#[test]
fn a_grid_returns_none_for_a_year_no_calendar_can_hold() {
    use crate::art::Grid;

    // The doc comment promises this: "a year arriving from a command line, a
    // file or a caller is input". It stepped back to a Sunday with unchecked
    // arithmetic, which panics in the first week a calendar can express — so
    // the promise held for every year except the ones that needed it.
    assert!(Grid::new(2027).is_some());
    assert!(Grid::new(2000).is_some(), "the first year the CLI allows");
    assert!(
        Grid::new(-262143).is_none(),
        "a year whose January has no earlier Sunday"
    );
    // And the grid it does build is the one `sunday_of` would have.
    let grid = Grid::new(2027).unwrap();
    assert_eq!(grid.start, crate::art::sunday_of(grid.first));
}

#[test]
fn counts_from_a_file_cannot_wrap_the_price() {
    use crate::art::{self, commits_for_level};
    use std::collections::BTreeMap;

    // The year's peak is the number the whole costing model rests on, and it is
    // summed from calendar counts. Wrapping there reported a peak of 8 for a
    // year whose busiest day held four billion, and quoted a price to match.
    let days: Vec<NaiveDate> = (1..=5)
        .map(|day| NaiveDate::from_ymd_opt(2027, 6, day).unwrap())
        .collect();
    let existing: BTreeMap<NaiveDate, u32> = [(days[0], u32::MAX), (days[1], u32::MAX - 1)]
        .into_iter()
        .collect();

    // Saturating rather than panicking (debug) or wrapping (release).
    let answer = commits_for_level(&days, &existing, 4);
    assert!(answer.is_some_and(|need| need > 0), "{answer:?}");

    // And the shade of an enormous day is still the brightest one, not a
    // wrapped-around dim one.
    assert_eq!(art::level(u32::MAX, u32::MAX), 4);
    assert_eq!(art::level(u32::MAX / 2, u32::MAX), 2);
}
