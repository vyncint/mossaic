//! Write text into a GitHub contribution graph by dating commits.
//!
//! ```sh
//! cargo run --bin mossaic-art -- VYNCINT --year 2027                    # preview only
//! cargo run --bin mossaic-art -- VYNCINT --year 2027 --snapshot art.json
//! cargo run --bin mossaic-art -- VYNCINT --year 2027 --repo ../art --write
//! ```
//!
//! Nothing is ever pushed. `--write` makes local commits in a directory you
//! name; pushing is a separate command it prints for you to run.

use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::{Datelike, Local, NaiveDate};
use mossaic::art::{self, Grid};
use mossaic::cli::Args;
use mossaic::primer::{Appearance, Palette, Season};
use mossaic::{github, plan, primer, thousands, Colour};

const HELP: &str = "\
mossaic-art — write text into a GitHub contribution graph by dating commits

usage:
  mossaic-art TEXT [options]

  TEXT                what to draw, e.g. VYNCINT (A-Z, 0-9, space, - and .)
  --year YEAR         which year's calendar (default: this one)
  --commits N         commits per lit day (default 4); keep it uniform for one
                      flat shade
  --background LEVEL  (--bg) draw the background as a shade instead of leaving
                      it empty, 0-3 (default 0). The letters stay at level 4, so
                      --background 1 draws them on a light green field rather
                      than on nothing — which is how you draw art without
                      going dark for most of the year. Levels two or more
                      apart are legible in every palette GitHub ships; one
                      apart is not
  --top ROW           first calendar row used, 0 = Sunday (default 1, so Mon-Fri)
  --start-week N      left edge in weeks (default: centred)
  --merge PATH        a saved `gh api graphql` calendar to draw on top of, so the
                      preview accounts for contributions already there
  --snapshot PATH     write a calendar file for `mossaic --file PATH`
  --repo DIR          where --write puts the commits
  --write             actually create the commits in --repo (local only)
  --login NAME        name shown in the snapshot (default: preview)
  --name NAME         commit author name (default: git config)
  --email ADDRESS     commit author email (default: git config)
  --color WHEN        (--colour) auto (default), always or never. auto means
                      colour when stdout is a terminal and NO_COLOR is unset
  --no-color          (--no-colour) the same as --color never
  --format F          text (default), json or markdown — what --track prints.
                      json and markdown are what the GitHub Action reads
  --plan PATH         where the plan is read from and written to
                      (default: mossaic-plan.json)
  --save              write the plan, so later runs need no flags at all
  --font              print every glyph the font has, and exit
  --track [USER]      compare the plan with what has actually been contributed:
                      how far along, what today owes, and whether the text can
                      still be drawn at all. Reads --merge if given, otherwise
                      asks gh for USER (default: whoever gh is)
  -V, --version       print the version
  -h, --help          show this help

examples:
  mossaic-art VYNCINT --year 2027                     what it would look like, and cost
  mossaic-art VYNCINT --year 2027 --background 1      letters on a field, not on nothing
  mossaic-art VYNCINT --year 2027 --track             am I getting there, and what today owes
  mossaic-art VYNCINT --year 2027 --snapshot a.json   then: mossaic --file a.json
  mossaic-art VYNCINT --year 2027 --repo ../art --write   local commits, never pushed
  mossaic-art --font                                  every glyph, side by side

Save the plan once and later runs need no flags at all:

  mossaic-art VYNCINT --year 2027 --start-week 6 --save
  mossaic-art --track          # reads mossaic-plan.json

also installed:
  mossaic          the chart itself
  mossaic-glyphs   what this terminal makes of the fallback cells";

fn main() {
    let Some(options) = parse_args() else {
        return;
    };

    // The CLI has already bounded the year, so this only fails for a caller
    // that reached past it.
    let grid = Grid::new(options.year).unwrap_or_else(|| {
        fail(&format!(
            "{} is not a year a calendar can hold",
            options.year
        ))
    });
    let columns = art::bitmap(&options.text).unwrap_or_else(|error| fail(&error));

    // The letters are always the brightest shade; --background picks what they
    // are drawn *on*. Rejected here rather than rendered, because a background
    // at or above the letters is not faint art, it is a blank graph.
    let shades = art::Shades {
        ink: 4,
        field: options.background,
    };
    shades.check().unwrap_or_else(|error| fail(&error));

    // Loaded before placing, because the background's price is quoted against
    // the year's busiest day and --merge can raise it.
    let existing = match &options.merge {
        Some(path) => load(path, &grid),
        None => BTreeMap::new(),
    };
    let peak = existing
        .values()
        .copied()
        .max()
        .unwrap_or(0)
        .max(options.commits)
        .max(shades.min_peak());
    let ink = art::Ink {
        lit: options.commits,
        field: shades.commits(peak).field,
    };

    // What the shades actually come out as, which is not always what was asked
    // for: a busy year can push the letters below level 4, and a --commits too
    // small to outrun the background makes the whole thing invisible.
    let drawn = art::Shades {
        ink: art::level(ink.lit, peak),
        field: art::level(ink.field, peak),
    };
    if shades.field > 0 && drawn.field >= drawn.ink {
        fail(&format!(
            "--commits {} puts the letters at level {} and the background at level {}, \
             so the letters would not show.\n  \
             In this year a letter day needs at least {} commits to sit above a \
             level-{} background.",
            options.commits,
            drawn.ink,
            drawn.field,
            thousands(art::commits_to_reach(shades.field + 1, peak)),
            shades.field,
        ));
    }

    let placed = art::place(&columns, &grid, options.top, options.start_week, ink)
        .unwrap_or_else(|error| fail(&error));
    if placed.skipped > 0 {
        eprintln!(
            "note: {} pixel(s) fell outside {} and were dropped — the first and \
             last calendar columns are partial weeks, so {} of {} columns hold a \
             whole letter",
            placed.skipped,
            grid.year,
            grid.usable_weeks(),
            grid.weeks
        );
    }

    // Resolved, not as typed: a centred text keeps the column it was centred on,
    // which is the whole point of writing the plan down.
    if options.save {
        let spec = plan::Spec {
            text: options.text.to_uppercase(),
            year: options.year,
            start_week: placed.start_week,
            top: options.top,
            commits: options.commits,
            background: options.background,
            user: options.tracking.clone(),
        };
        spec.save(&options.plan_path)
            .unwrap_or_else(|error| fail(&error));
        println!(
            "saved {} — from now on:\n  mossaic-art --track\n",
            options.plan_path.display()
        );
    }

    if options.track {
        track_progress(&options, &grid, &columns, &placed, shades);
        return;
    }

    let mut combined = existing.clone();
    for (day, count) in &placed.all() {
        *combined.entry(*day).or_insert(0) += count;
    }

    let peak = combined.values().copied().max().unwrap_or(1);
    let art_level = art::level(options.commits, peak);
    let total = placed.total();
    println!(
        "{}  ·  {}  ·  {} of {} columns  ·  {} days  ·  {} commits\n",
        options.text.to_uppercase(),
        grid.year,
        columns.len(),
        grid.weeks,
        placed.lit.len(),
        thousands(total),
    );

    let palette = options
        .colour
        .enabled()
        .then(|| Palette::new(Appearance::Dark, Season::Default, true));
    println!(
        "{}\n",
        art::preview(&art::shading(&placed, shades), &grid, palette.as_ref())
    );

    // What the reader will actually be able to tell apart. Only interesting
    // once there are two shades to compare; against an empty graph the answer
    // is always "obviously".
    if shades.field > 0 {
        let (legibility, delta) = drawn.worst();
        println!(
            "background level {} under letters at level {}  ·  {} background day(s), \
             {} each  ·  ΔE {delta:.0} at worst, {legibility}",
            drawn.field,
            drawn.ink,
            thousands(placed.field.len() as u32),
            ink.field,
        );
        match legibility {
            primer::Legibility::Faint => println!(
                "  -> {} and {} are neighbouring shades. On some themes they are all \
                 but the same colour;\n     leave two levels between them — \
                 --background {} is the safe one against level {}.",
                drawn.field,
                drawn.ink,
                drawn.ink.saturating_sub(2),
                drawn.ink
            ),
            primer::Legibility::Readable => println!(
                "  -> readable, but not at a glance on a small graph. \
                 --background {} reads more clearly.",
                drawn.ink.saturating_sub(3)
            ),
            primer::Legibility::Clear => {}
        }
        println!();
    }

    if !existing.is_empty() {
        // Shading is relative to the year's peak, so say plainly whether the
        // letters will actually stand out from what is already there.
        let rivals = existing
            .iter()
            .filter(|(day, count)| {
                !placed.lit.contains_key(*day) && art::level(**count, peak) >= art_level
            })
            .count();
        println!(
            "drawing over {} day(s) already active (busiest {})",
            existing.len(),
            existing.values().copied().max().unwrap_or(0)
        );
        println!(
            "peak after merge {peak}  ·  letters land at level {art_level}/4  ·  \
             {rivals} existing day(s) as bright or brighter"
        );
        if art_level < 4 {
            let days: Vec<NaiveDate> = placed.lit.keys().copied().collect();
            if let Some(need) = art::commits_for_level(&days, &existing, 4) {
                println!(
                    "  -> for the brightest level, use --commits {need} ({} commits)",
                    thousands(need * placed.lit.len() as u32)
                );
            }
        }
        if rivals > 6 {
            println!("  -> the letters will be competing with a lot of real activity");
        }
    }

    if let Some(path) = &options.snapshot {
        let body = art::snapshot(&combined, &grid, &options.login);
        std::fs::write(path, body)
            .unwrap_or_else(|error| fail(&format!("could not write {path:?}: {error}")));
        println!(
            "\nwrote {} — see it in the real renderer with:",
            path.display()
        );
        println!("  mossaic --file {}", path.display());
    }

    match (&options.repo, options.write) {
        (Some(repo), true) => {
            let (name, email) = art::identity();
            let name = options.name.unwrap_or(name);
            let email = options.email.unwrap_or(email);
            println!(
                "\ncommitting {} commits into {} as {name} <{email}>",
                thousands(total),
                repo.display()
            );
            let label = format!("art: {}", options.text.to_uppercase());
            let made = art::write_commits(&placed.all(), repo, &label, &name, &email)
                .unwrap_or_else(|error| fail(&error));
            println!(
                "made {} commits — nothing has been pushed.\n\n\
                 To publish, create an empty repo on GitHub and:\n\n  \
                 cd {}\n  git remote add origin git@github.com:<you>/<repo>.git\n  \
                 git push -u origin main\n\n\
                 To count towards the graph these must be on the default branch of a \
                 repo you own (not a fork), authored with an email registered to your \
                 GitHub account.",
                thousands(made as u32),
                repo.display()
            );
        }
        (Some(_), false) => println!("\n(add --write to create the commits; this was a dry run)"),
        (None, true) => fail("--write needs --repo DIR"),
        (None, false) => {}
    }
}

/// How far along the plan is, what today owes, and whether the text can still
/// be drawn at all.
fn track_progress(
    options: &Options,
    grid: &Grid,
    columns: &[[bool; art::GLYPH_ROWS]],
    placed: &art::Placed,
    shades: art::Shades,
) {
    let colour = options.colour.enabled();
    let palette = colour.then(|| Palette::new(Appearance::Dark, Season::Default, true));
    let paint = |text: &str, colour: Option<mossaic::primer::Rgb>| match (colour, &palette) {
        (Some(colour), Some(_)) => format!(
            "\x1b[38;2;{};{};{}m{text}\x1b[0m",
            colour.0, colour.1, colour.2
        ),
        _ => text.to_string(),
    };
    let shade = |level: usize| palette.as_ref().map(|palette| palette.levels[level]);
    let danger = || palette.as_ref().map(|palette| palette.danger);

    // The real year: a saved response if one was given, otherwise gh.
    let (who, actual) = match &options.merge {
        Some(path) => (path.display().to_string(), load(path, grid)),
        None => {
            let who = options
                .tracking
                .clone()
                .or_else(github::whoami)
                .unwrap_or_else(|| {
                    fail("could not tell whose contributions to track — pass `--track USER`, or run `gh auth login`")
                });
            let calendar = github::fetch(&who, grid.year).unwrap_or_else(|error| fail(&error));
            (who, plan::contributions(&calendar))
        }
    };

    let plan = plan::Plan::build(
        &options.text,
        grid,
        placed,
        columns.len(),
        options.top,
        &actual,
        shades,
    );
    // A day inside the letters only spoils them if it is brighter than the
    // background they would otherwise be — with no background, that is any
    // contribution at all.
    let hideable = plan.field_ceiling.unwrap_or(0);

    // json and markdown are for machines and for messages; both carry every
    // number the text report prints, so a notification never has to be parsed
    // out of a screen.
    if options.format != Format::Text {
        let today = Local::now().date_naive();
        let suggestion =
            plan::best_start_week(grid, columns.len(), options.top, columns, &actual, hideable);
        let year_total = actual
            .values()
            .fold(0u32, |sum, count| sum.saturating_add(*count));
        let report = plan::Report::of(&plan, &who, year_total, today, suggestion);
        match options.format {
            Format::Json => println!(
                "{}",
                serde_json::to_string_pretty(&report).expect("a tree of numbers and strings")
            ),
            Format::Markdown => print!("{}", report.markdown()),
            Format::Text => unreachable!("handled below"),
        }
        return;
    }
    let (owing_days, owing_commits) = plan.owing();
    let holes = plan.holes();
    let total = actual
        .values()
        .fold(0u32, |sum, count| sum.saturating_add(*count));

    // The placement is part of the plan, and it lives in the command line —
    // tracking with a different --start-week than the text was drawn with
    // compares against a different plan entirely. Printing which one is on
    // screen makes that visible rather than baffling.
    println!("{}  ·  {}  ·  tracking {who}\n", plan.text, plan.year);
    println!(
        "  the plan    {} of {} columns from week {}, on rows {}-{}",
        plan.columns,
        grid.weeks,
        plan.start_week,
        options.top,
        options.top + art::GLYPH_ROWS - 1
    );
    println!(
        "  the year    {} contributions{}",
        thousands(total),
        match plan.peak_day {
            Some(date) if plan.peak > 0 => format!(
                "  ·  busiest {} ({})",
                date.format("%b %-d"),
                thousands(plan.peak)
            ),
            _ => String::new(),
        }
    );
    println!(
        "              a letter day has to reach {} to match it",
        thousands(plan.need)
    );
    if shades.field > 0 {
        let (legibility, delta) = shades.worst();
        println!(
            "  the shades  letters at level {}, background at level {}  ·  \
             ΔE {delta:.0} at worst, {legibility}",
            shades.ink, shades.field
        );
        println!(
            "              a background day has to reach {}{}",
            thousands(plan.field_need),
            match plan.field_ceiling {
                Some(most) => format!(" and stay under {}", thousands(most + 1)),
                None => String::new(),
            }
        );
    }
    println!();

    let bright = plan.bright();
    let letters = plan.letters().count();
    const BAR: usize = 28;
    let filled = (bright * BAR).checked_div(letters).unwrap_or(0);
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(BAR - filled));
    println!(
        "  letters     {}  {} of {} bright",
        paint(&bar, shade(4)),
        bright,
        letters
    );
    if owing_days > 0 {
        println!(
            "  owing       {owing_days} day(s) short, {} contributions between them",
            thousands(owing_commits)
        );
    }
    // The background gets its own bar rather than being folded into the
    // letters': three hundred easy days would drown out seven hard ones, and
    // the letters are the point.
    let field_days = plan.field().count();
    if field_days > 0 {
        let done = plan.field_bright();
        let filled = (done * BAR).checked_div(field_days).unwrap_or(0);
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(BAR - filled));
        println!(
            "  background  {}  {} of {} at level {}",
            paint(&bar, shade(usize::from(shades.field))),
            done,
            field_days,
            shades.field
        );
        let (field_owing_days, field_owing_commits) = plan.field_owing();
        if field_owing_days > 0 {
            println!(
                "  owing       {field_owing_days} background day(s) short, {} contributions",
                thousands(field_owing_commits)
            );
        }
    }
    if !holes.is_empty() {
        println!(
            "  holes       {}",
            paint(
                &format!(
                    "{} day(s) inside the letters are lit and cannot be unlit",
                    holes.len()
                ),
                danger()
            )
        );
    }
    if plan.around() > 0 {
        println!(
            "  around      {} day(s) outside the text have contributions",
            plan.around()
        );
    }

    println!("\n{}\n", plan::preview(&plan, grid, palette.as_ref()));

    match plan.verdict() {
        plan::Verdict::Done => {
            println!("  {}", paint(&format!("{} is drawn.", plan.text), shade(4)))
        }
        plan::Verdict::Reachable => println!(
            "  {} can still be drawn cleanly — {}.",
            plan.text,
            match plan.field_owing().1 {
                0 => format!("{} contributions to go", thousands(owing_commits)),
                owed => format!(
                    "{} for the letters, {} for the field",
                    thousands(owing_commits),
                    thousands(owed)
                ),
            }
        ),
        plan::Verdict::Holed { holes } => {
            println!(
                "  {}",
                paint(
                    &format!("{} cannot be drawn cleanly in {}.", plan.text, plan.year),
                    danger()
                )
            );
            println!(
                "    {holes} day(s) inside the letters already have contributions, and\n    \
                 nothing takes those away — the text would read with holes in it."
            );
            match plan::best_start_week(
                grid,
                columns.len(),
                options.top,
                columns,
                &actual,
                hideable,
            ) {
                Some((week, left)) if left < holes => {
                    println!("    --start-week {week} would leave {left} instead of {holes}.")
                }
                _ => println!(
                    "    Every placement in {} runs into the same problem; an emptier\n    \
                     year is the way out.",
                    plan.year
                ),
            }
        }
    }

    // What to do next, which is the question this is really for.
    let today = Local::now().date_naive();
    if !plan.under_way(today) {
        let first = plan
            .letters()
            .map(|day| day.date)
            .next()
            .unwrap_or(grid.first);
        println!(
            "\n  {} has not started. The first letter day is {} {}.",
            plan.year,
            plan::weekday(first),
            first.format("%b %-d")
        );
        return;
    }
    println!();
    for (label, date) in [
        ("today", today),
        ("tomorrow", today.succ_opt().unwrap_or(today)),
    ] {
        if !grid.holds(date) {
            continue;
        }
        let line = match plan.on(date) {
            Some(day) if day.want == plan::Want::Lit && day.short() > 0 => format!(
                "a letter day — {} needed, {} there, {} to go",
                thousands(day.need),
                thousands(day.have),
                thousands(day.short())
            ),
            Some(day) if day.want == plan::Want::Lit => {
                "a letter day — already bright enough".to_string()
            }
            // Over its ceiling: already the wrong shade, and nothing undoes it.
            Some(day) if day.over() > 0 => format!(
                "background — {} contributions, {} too many for level {}",
                thousands(day.have),
                thousands(day.over()),
                shades.field
            ),
            Some(day) if day.need > 0 && day.short() > 0 => format!(
                "background — {} needed, {} there, {} to go{}",
                thousands(day.need),
                thousands(day.have),
                thousands(day.short()),
                match day.ceiling {
                    Some(most) => format!(" (and no more than {})", thousands(most)),
                    None => String::new(),
                }
            ),
            Some(day) if day.need > 0 => "background — already the right shade".to_string(),
            Some(day) if day.have > 0 => "not part of the text, and already lit".to_string(),
            _ => "not part of the text — anything you commit today shows".to_string(),
        };
        println!(
            "  {label:<11} {} {}  ·  {line}",
            plan::weekday(date),
            date.format("%b %-d")
        );
    }

    let upcoming = plan.schedule(today, 7);
    if upcoming
        .iter()
        .any(|day| day.want == plan::Want::Lit || day.need > 0)
    {
        println!("\n  the next seven days");
        for day in upcoming {
            let what = match day.want {
                plan::Want::Lit if day.short() > 0 => {
                    format!("letter  {} to go", thousands(day.short()))
                }
                plan::Want::Lit => "letter  done".to_string(),
                _ if day.short() > 0 => {
                    format!("field   {} to go", thousands(day.short()))
                }
                _ if day.need > 0 => "field   done".to_string(),
                _ => "—".to_string(),
            };
            println!(
                "    {} {}   {what}",
                plan::weekday(day.date),
                day.date.format("%b %-d")
            );
        }
    }

    let (past, past_commits) = plan.overdue(today);
    let (future, future_commits) = plan.ahead(today);
    if past == 0 && future == 0 {
        return;
    }
    println!("\n  the rest of the year");
    println!(
        "    {future} letter day(s) still to come, {} contributions",
        thousands(future_commits)
    );
    if past > 0 {
        println!(
            "    {past} letter day(s) already past, {} contributions — only back-dated\n    \
             commits reach those:\n    `mossaic-art {} --year {} --repo ../art --write`",
            thousands(past_commits),
            plan.text,
            plan.year
        );
    }
}

/// Every glyph, side by side — what a contributed one actually looks like next
/// to its neighbours, which is the thing a table of quoted strings cannot show.
fn show_font(colour: bool) {
    const PER_ROW: usize = 8;
    let palette = Palette::new(Appearance::Dark, Season::Default, true);
    let paint = |cell: &str, lit: bool| {
        if !colour {
            return if lit {
                cell.to_string()
            } else {
                "  ".to_string()
            };
        }
        let shade = palette.levels[if lit { 4 } else { 0 }];
        format!(
            "\x1b[38;2;{};{};{}m{cell}\x1b[0m",
            shade.0, shade.1, shade.2
        )
    };

    let characters: Vec<char> = art::alphabet().collect();
    println!(
        "{} glyphs, {}x{} each — add one to FONT in src/art.rs\n",
        characters.len(),
        art::GLYPH_COLS,
        art::GLYPH_ROWS
    );
    for chunk in characters.chunks(PER_ROW) {
        let labels: Vec<String> = chunk
            .iter()
            .map(|character| {
                let name = if *character == ' ' {
                    "space".to_string()
                } else {
                    character.to_string()
                };
                format!("{name:<12}")
            })
            .collect();
        println!("  {}", labels.join("").trim_end());
        for row in 0..art::GLYPH_ROWS {
            let mut line = String::from("  ");
            for character in chunk {
                let glyph = art::glyph(*character).expect("from the font itself");
                for lit in glyph[row].chars() {
                    line.push_str(&paint("██", lit == '#'));
                }
                line.push_str("  ");
            }
            println!("{line}");
        }
        println!();
    }
}

/// Existing contributions from a saved `gh api graphql` response.
fn load(path: &PathBuf, grid: &Grid) -> BTreeMap<NaiveDate, u32> {
    let calendar = github::from_file(&path.to_string_lossy())
        .unwrap_or_else(|error| fail(&format!("could not read {path:?}: {error}")));
    calendar
        .days()
        .filter(|day| day.count > 0 && grid.holds(day.date))
        .map(|day| (day.date, day.count))
        .collect()
}

/// What `--track` prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Text,
    Json,
    Markdown,
}

struct Options {
    format: Format,
    track: bool,
    tracking: Option<String>,
    text: String,
    year: i32,
    commits: u32,
    /// The level the background sits at. Zero leaves it empty.
    background: u8,
    top: usize,
    start_week: Option<usize>,
    /// Where the plan is read from and written to.
    plan_path: PathBuf,
    /// Write the plan after resolving it.
    save: bool,
    merge: Option<PathBuf>,
    snapshot: Option<PathBuf>,
    repo: Option<PathBuf>,
    write: bool,
    login: String,
    name: Option<String>,
    email: Option<String>,
    colour: Colour,
}

fn parse_args() -> Option<Options> {
    let mut options = Options {
        format: Format::Text,
        track: false,
        tracking: None,
        text: String::new(),
        plan_path: PathBuf::from(plan::DEFAULT_SPEC),
        save: false,
        year: Local::now().year(),
        commits: 4,
        background: 0,
        top: 1,
        start_week: None,
        merge: None,
        snapshot: None,
        repo: None,
        write: false,
        login: "preview".to_string(),
        name: None,
        email: None,
        colour: Colour::default(),
    };

    let mut font = false;
    let mut track = false;
    let mut tracking: Option<String> = None;
    let mut args = Args::from_env("mossaic-art");

    while let Some(arg) = args.next_arg() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{HELP}");
                return None;
            }
            "-V" | "--version" => {
                println!("mossaic-art {}", env!("CARGO_PKG_VERSION"));
                return None;
            }
            "-y" | "--year" => options.year = args.year("--year"),
            "--commits" => {
                options.commits = args.number("--commits") as u32;
            }
            "--background" | "--bg" => {
                let level = args.number("--background");
                // Bounded here so the error names the flag; `Shades::check`
                // then decides whether the pair can draw anything.
                options.background = match u8::try_from(level) {
                    Ok(level) if level <= 4 => level,
                    _ => fail(&format!(
                        "--background wants a level between 0 and 4, not {level}"
                    )),
                };
            }
            "--top" => {
                options.top = args.number("--top") as usize;
            }
            "--start-week" => {
                options.start_week = Some(args.number("--start-week") as usize);
            }
            "--merge" => options.merge = Some(args.value("--merge").into()),
            "--snapshot" => options.snapshot = Some(args.value("--snapshot").into()),
            "--repo" => options.repo = Some(args.value("--repo").into()),
            "--write" => options.write = true,
            "--login" => options.login = args.value("--login"),
            "--name" => options.name = Some(args.value("--name")),
            "--email" => options.email = Some(args.value("--email")),
            "--color" | "--colour" => {
                let raw = args.value("--color");
                options.colour = Colour::parse(&raw).unwrap_or_else(|| {
                    fail(&format!("--color wants auto, always or never, not {raw:?}"))
                });
            }
            "--no-colour" | "--no-color" => options.colour = Colour::Never,
            "--format" => {
                options.format = match args.value("--format").as_str() {
                    "text" => Format::Text,
                    "json" => Format::Json,
                    "markdown" | "md" => Format::Markdown,
                    other => fail(&format!(
                        "unknown format {other:?} — text, json or markdown"
                    )),
                }
            }
            "--plan" => options.plan_path = args.value("--plan").into(),
            "--save" => options.save = true,
            "--font" => font = true,
            "--track" => {
                track = true;
                // An optional user follows, if the next argument is not a flag.
                if args.peek_value() {
                    tracking = args.next_arg();
                }
            }
            other if other.starts_with('-') => {
                fail(&format!("unknown option {other:?} — try --help"))
            }
            other if options.text.is_empty() => options.text = other.to_string(),
            other => fail(&format!("unexpected argument {other:?}")),
        }
    }

    // After the loop, so that flags on either side of it are all in hand.
    if font {
        show_font(options.colour.enabled());
        return None;
    }
    // A saved plan fills in whatever was not typed. Typed flags always win, so
    // `--year 2028` against a saved 2027 plan is a one-off, not a surprise.
    match (options.text.is_empty(), options.plan_path.exists()) {
        (true, true) => {
            let spec = plan::Spec::load(&options.plan_path).unwrap_or_else(|error| fail(&error));
            options.text = spec.text;
            if !args.was_typed("year") {
                options.year = spec.year;
            }
            if !args.was_typed("top") {
                options.top = spec.top;
            }
            if !args.was_typed("commits") {
                options.commits = spec.commits;
            }
            if !args.was_typed("background") && !args.was_typed("bg") {
                options.background = spec.background;
            }
            if !args.was_typed("start-week") {
                options.start_week = Some(spec.start_week);
            }
            if options.tracking.is_none() {
                options.tracking = spec.user;
            }
        }
        (true, false) => fail(&format!(
            "nothing to draw.\n\n  \
             mossaic-art VYNCINT --year 2027        draw something\n  \
             mossaic-art VYNCINT --year 2027 --save remember it, then just \
             `mossaic-art --track`\n\n\
             (no plan at {})",
            options.plan_path.display()
        )),
        _ => {}
    }
    options.track = track;
    options.tracking = tracking;
    Some(options)
}

fn fail(message: &str) -> ! {
    eprintln!("mossaic-art: {message}");
    std::process::exit(2)
}
