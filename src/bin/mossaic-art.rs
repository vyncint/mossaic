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
use std::path::{Path, PathBuf};

use chrono::{Datelike, Local, NaiveDate};
use mossaic::art::{self, Grid};
use mossaic::cli::{Args, YEARS};
use mossaic::primer::{Appearance, Palette, Season};
use mossaic::{github, graphics, plan, png, primer, templates, thousands, Colour};

const HELP: &str = "\
mossaic-art — write text into a GitHub contribution graph by dating commits

usage:
  mossaic-art TEXT [options]
  mossaic-art --template NAME [options]
  mossaic-art --matrix FILE [options]

  TEXT                what to draw, e.g. VYNCINT. Letters, digits, punctuation
                      and shapes: `I :heart: RUST`. A shape is written between
                      colons, or pasted as the symbol or emoji it depicts.
                      --font lists everything

  Text is drawn through a 5x5 font on Mon-Fri. The three flags below draw a
  *picture* instead: seven rows by up to 53 columns, any of GitHub's five
  shades per day, using the whole week and the whole year.

  --template NAME     draw a template from the catalogue. --list-templates
                      shows what there is
  --list-templates    list the templates, with a thumbnail of each, and exit
  --matrix FILE       (--file) draw a .art file: 7 rows of the shades 0-4, or
                      of the blocks ░ ▒ ▓ █ with a space for 0.
                      Lines starting with # are comments, and `# name:`,
                      `# author:` and `# description:` are read as the
                      picture's own. With --commits N a level-4 day costs N
                      and every darker level is priced against it
  --output FILE       (-o) where a drawing is written
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
  --backfill          commit what each day already past is still short of the
                      plan, measured against the real calendar — the flag for
                      catching up. Unlike a plain --write it never adds to a day
                      that is already bright, which over an active year is what
                      stops the art raising the very peak it is measured
                      against, and it leaves days still to come alone. --today
                      is what \"past\" means. Needs --repo, and --write to commit
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
  --png PATH          with --font, write the glyphs to a PNG instead of the
                      terminal — the sheet the README shows
  --track [USER]      compare the plan with what has actually been contributed:
                      how far along, what today owes, and whether the text can
                      still be drawn at all. Reads --merge if given, otherwise
                      asks gh for USER (default: whoever gh is)
  --today DATE        what \"today\" means, as YYYY-MM-DD (default: the clock).
                      A report is then reproducible, and a day that has not
                      arrived can be asked what it will owe
  -V, --version       print the version
  -h, --help          show this help

examples:
  mossaic-art --list-templates                        see the pictures on offer
  mossaic-art --template dragon --year 2027           draw one across the year
  mossaic-art --matrix mine.art --year 2027 --save    draw your own, and keep it
  mossaic-art VYNCINT --year 2027                     what it would look like, and cost
  mossaic-art VYNCINT --year 2027 --background 1      letters on a field, not on nothing
  mossaic-art VYNCINT --year 2027 --track             am I getting there, and what today owes
  mossaic-art VYNCINT --year 2027 --snapshot a.json   then: mossaic --file a.json
  mossaic-art VYNCINT --year 2027 --repo ../art --write   local commits, never pushed
  mossaic-art --backfill --repo ../art --write        commit just what the plan is short
  mossaic-art --track --today 2027-06-01              what that day will owe
  mossaic-art \"I :heart: RUST\" --year 2027            a shape among the letters
  mossaic-art --font                                  every glyph, side by side
  mossaic-art --font --png art/font.png               the same sheet, as an image

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
    // A picture takes a path of its own, before a single line of the text
    // machinery runs. That is what makes "no regressions for text" a property
    // of the control flow rather than a claim about the diff.
    if let Some((name, canvas)) = options.canvas.clone() {
        run_canvas(&options, &grid, &name, &canvas);
        return;
    }

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
    // the year's busiest day and --merge can raise it. Only a run that draws
    // needs it: tracking and backfilling read the calendar themselves, and
    // loading it here as well would parse the file twice and say anything it
    // has to say about it twice.
    let existing = match (&options.merge, options.track || options.backfill) {
        (Some(path), false) => load(path, &grid),
        _ => BTreeMap::new(),
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
    // Only for a run that would draw *a flat count*. `--commits` is what a
    // plain `--write` puts on a lit day; neither tracking nor backfilling uses
    // it — both work out what a day needs from the year's real peak. Refusing
    // `--track --merge --background` over a busy year told the user their plan
    // was impossible when the only thing wrong was a flag that run does not
    // use, and `--backfill` ignores it for exactly the same reason.
    if !options.track && !options.backfill && shades.field > 0 && drawn.field >= drawn.ink {
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
            art: None,
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
    if options.backfill {
        backfill(&options, &grid, &columns, &placed, shades);
        return;
    }

    let mut combined = existing.clone();
    for (day, count) in &placed.all() {
        // Saturating: the peak the whole costing model rests on is computed from
        // this map, and a `--merge` calendar is a file. Wrapping here reported a
        // peak of 8 for a year whose busiest day held four billion, and then
        // quoted a price to match it.
        let total = combined.entry(*day).or_insert(0);
        *total = total.saturating_add(*count);
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
                    thousands(need.saturating_mul(placed.lit.len() as u32))
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

/// `--list-templates`: the catalogue, with where each one came from.
fn show_templates(colour: bool) {
    let catalogue = templates::catalogue();
    if catalogue.is_empty() {
        println!("no templates installed");
        return;
    }
    let palette = colour.then(|| Palette::new(Appearance::Dark, Season::Default, true));
    let width = catalogue
        .iter()
        .map(|template| template.name.len())
        .max()
        .unwrap_or(0);

    println!("{} template(s):\n", catalogue.len());
    for template in &catalogue {
        let author = template
            .author()
            .map(|who| format!("  by {who}"))
            .unwrap_or_default();
        println!(
            "  {:<width$}  {}{author}",
            template.name,
            template.title(),
            width = width
        );
        if let Some(description) = template.description() {
            println!("  {:<width$}  {description}", "", width = width);
        }
        println!(
            "  {:<width$}  {} columns, {}\n",
            "",
            template.canvas.width(),
            template.origin,
            width = width
        );
        // A thumbnail, because a name and a sentence are not a picture and the
        // whole point of choosing one is what it looks like.
        for row in 0..art::CANVAS_ROWS {
            let mut line = String::from("    ");
            for week in 0..template.canvas.width() {
                let level = template.canvas.at(week, row);
                let block = art::SHADE_CHARS[usize::from(level).min(4)].1;
                match (&palette, level) {
                    (Some(palette), level) if level > 0 => {
                        let rgb = palette.levels[usize::from(level).min(4)];
                        line.push_str(&format!(
                            "\x1b[38;2;{};{};{}m{block}\x1b[0m",
                            rgb.0, rgb.1, rgb.2
                        ));
                    }
                    _ => line.push(block),
                }
            }
            println!("{line}");
        }
        println!();
    }
    println!("draw one:  mossaic-art --template {}", catalogue[0].name);
}

/// Everything a run that draws a **picture** does, from preview to commits.
///
/// Deliberately a path of its own rather than a widening of the text one. The
/// two share the plan, the pricing and every report format that a machine
/// reads; what they do not share is language. A canvas has no letters and no
/// background, so a report that says "letters at level 4, background at level
/// 1" would be describing something that is not there.
fn run_canvas(options: &Options, grid: &Grid, name: &str, canvas: &art::Canvas) {
    let start = options.start_week.unwrap_or_else(|| canvas.centred(grid));
    if start >= grid.weeks {
        fail(&format!(
            "--start-week {start} is past the {} columns {} has",
            grid.weeks, grid.year
        ));
    }
    let (levels, skipped) = canvas.place(grid, start);
    if levels.is_empty() {
        fail(&format!(
            "nothing of the picture lands inside {} from week {start}",
            grid.year
        ));
    }
    if skipped > 0 {
        eprintln!(
            "note: {skipped} cell(s) fell outside {} and were dropped — the first \
             and last calendar columns are partial weeks",
            grid.year
        );
    }

    // Resolved, exactly as a text plan is: the picture is stored inline, so a
    // template edited next month cannot re-target a plan drawn from it today.
    if options.save {
        let spec = plan::Spec {
            text: name.to_string(),
            year: options.year,
            start_week: start,
            top: 0,
            commits: options.commits,
            background: 0,
            user: options.tracking.clone(),
            art: Some(canvas.to_art()),
        };
        spec.save(&options.plan_path)
            .unwrap_or_else(|error| fail(&error));
        println!(
            "saved {} — from now on:\n  mossaic-art --track\n",
            options.plan_path.display()
        );
    }

    if options.track {
        track_canvas(options, grid, name, canvas, &levels, start);
        return;
    }

    let existing = match &options.merge {
        Some(path) => load(path, grid),
        None => BTreeMap::new(),
    };
    // `--commits` prices the *brightest* day, and every darker level is priced
    // against it by the same formula GitHub shades with. At the default of 4
    // that is 1, 2, 3, 4 commits for levels 1 to 4 — the smallest year in which
    // all five shades are distinct.
    let peak = existing
        .values()
        .copied()
        .max()
        .unwrap_or(0)
        .max(options.commits)
        .max(canvas.min_peak());
    let commits: BTreeMap<NaiveDate, u32> = levels
        .iter()
        .filter(|(_, level)| **level > 0)
        .map(|(date, level)| (*date, art::commits_to_reach(*level, peak)))
        .collect();
    let total = commits
        .values()
        .fold(0u32, |sum, count| sum.saturating_add(*count));

    if options.backfill {
        backfill_canvas(options, grid, &levels);
        return;
    }

    let histogram = canvas.histogram();
    println!(
        "{name}  ·  {}  ·  {} of {} columns  ·  {} days  ·  {} commits\n",
        grid.year,
        canvas.width(),
        grid.weeks,
        commits.len(),
        thousands(total),
    );

    let palette = options
        .colour
        .enabled()
        .then(|| Palette::new(Appearance::Dark, Season::Default, true));
    println!("{}\n", art::preview(&levels, grid, palette.as_ref()));

    println!("  level  days   commits each");
    for level in (1..=4u8).rev() {
        if histogram[usize::from(level)] == 0 {
            continue;
        }
        println!(
            "  {level:>5}  {:>4}   {}",
            histogram[usize::from(level)],
            thousands(art::commits_to_reach(level, peak)),
        );
    }
    println!("  {:>5}  {:>4}   must stay dark", 0, histogram[0]);

    // Whether a reader will see a picture or a smudge. The pair measured is the
    // darkest and brightest the drawing actually uses: if those two are faint,
    // everything between them is worse.
    if let Some((low, high)) = canvas.range() {
        let shades = art::Shades {
            ink: high,
            field: low,
        };
        let (legibility, delta) = shades.worst();
        println!("\n  shades {low} to {high}  ·  ΔE {delta:.0} at worst, {legibility}",);
        if legibility != primer::Legibility::Clear {
            println!(
                "  -> neighbouring shades are all but the same colour on some themes. \
                 A picture\n     drawn in levels {low} and {high} will read as one shade for \
                 some of its audience."
            );
        }
    }
    println!();

    if let Some(path) = &options.snapshot {
        let mut combined = existing.clone();
        for (day, count) in &commits {
            let total = combined.entry(*day).or_insert(0);
            *total = total.saturating_add(*count);
        }
        let body = art::snapshot(&combined, grid, &options.login);
        std::fs::write(path, body)
            .unwrap_or_else(|error| fail(&format!("could not write {path:?}: {error}")));
        println!(
            "wrote {} — see it in the real renderer with:",
            path.display()
        );
        println!("  mossaic --file {}\n", path.display());
    }

    match (&options.repo, options.write) {
        (Some(repo), true) => {
            let (default_name, default_email) = art::identity();
            let who = options.name.clone().unwrap_or(default_name);
            let email = options.email.clone().unwrap_or(default_email);
            println!(
                "committing {} commits into {} as {who} <{email}>",
                thousands(total),
                repo.display()
            );
            let label = format!("art: {name}");
            let made = art::write_commits(&commits, repo, &label, &who, &email)
                .unwrap_or_else(|error| fail(&error));
            println!(
                "made {} commits — nothing has been pushed.\n\n\
                 To publish, create an empty repo on GitHub and:\n\n  \
                 cd {}\n  git remote add origin git@github.com:<you>/<repo>.git\n  \
                 git push -u origin main",
                thousands(made as u32),
                repo.display()
            );
        }
        (Some(_), false) => println!("(add --write to create the commits; this was a dry run)"),
        (None, true) => fail("--write needs --repo DIR"),
        (None, false) => {}
    }
}

/// `--track` for a picture: how far along it is, and what today owes.
fn track_canvas(
    options: &Options,
    grid: &Grid,
    name: &str,
    canvas: &art::Canvas,
    levels: &BTreeMap<NaiveDate, u8>,
    start: usize,
) {
    let colour = options.colour.enabled();
    let palette = colour.then(|| Palette::new(Appearance::Dark, Season::Default, true));
    let (who, actual) = observed(options, grid);
    let plan = plan::Plan::from_levels(name, grid, levels, start, canvas.width(), &actual);
    let today = options.now();

    // Every machine-read format is generic over the plan already, so a picture
    // reports through exactly the same code a text plan does. Only the human
    // one below needed writing, because only it uses words like "letters".
    if options.format != Format::Text {
        let year_total = actual
            .values()
            .fold(0u32, |sum, count| sum.saturating_add(*count));
        // No suggestion: a picture is usually the full width of the year, so
        // there is no column to move it to, and offering one that does not fit
        // is worse than offering none.
        let report = plan::Report::of(&plan, &who, year_total, today, None);
        match options.format {
            Format::Json => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .unwrap_or_else(|error| fail(&format!("could not encode the report: {error}")))
            ),
            Format::Markdown => println!("{}", report.markdown()),
            Format::Text => unreachable!("handled above"),
        }
        return;
    }

    println!("{name} · {} — tracking {who}\n", grid.year);
    println!("{}\n", art::preview(levels, grid, palette.as_ref()));

    let (owing_days, owing_commits) = plan.owing();
    let drawn = plan.bright();
    let wanted = plan.letters().count();
    match plan.verdict() {
        plan::Verdict::Done => println!("  Drawn. Every day is the shade the picture asks for."),
        plan::Verdict::Reachable => println!(
            "  On track — {drawn} of {wanted} shaded day(s) are right, \
             {owing_days} to go."
        ),
        plan::Verdict::Holed { holes } => println!(
            "  Cannot be drawn cleanly — {holes} day(s) are brighter than the \
             picture wants,\n  and nothing takes a contribution away."
        ),
    }

    println!("\n  level  days   done   owing   each");
    let histogram = canvas.histogram();
    for level in (1..=4u8).rev() {
        if histogram[usize::from(level)] == 0 {
            continue;
        }
        let at_level: Vec<&plan::Day> = plan
            .days
            .iter()
            .filter(|day| levels.get(&day.date).copied() == Some(level))
            .collect();
        let done = at_level.iter().filter(|day| day.done()).count();
        let short: u32 = at_level
            .iter()
            .fold(0u32, |sum, day| sum.saturating_add(day.short()));
        let each = at_level.first().map(|day| day.need).unwrap_or(0);
        println!(
            "  {level:>5}  {:>4}   {done:>4}   {:>5}   {}",
            at_level.len(),
            thousands(short),
            thousands(each),
        );
    }
    let dark: Vec<&plan::Day> = plan
        .days
        .iter()
        .filter(|day| day.want == plan::Want::Hole)
        .collect();
    println!(
        "  {:>5}  {:>4}   {:>4}   {:>5}   must stay dark",
        0,
        dark.len(),
        dark.iter().filter(|day| day.done()).count(),
        "-",
    );

    println!(
        "\n  still owing  {} day(s) · {} contributions",
        thousands(owing_days as u32),
        thousands(owing_commits)
    );

    if plan.holds(today) {
        match plan.on(today) {
            Some(day) if day.short() > 0 => println!(
                "  today        {} of {} there, {} to go",
                thousands(day.have),
                thousands(day.need),
                thousands(day.short())
            ),
            Some(day) if day.need == 0 => println!("  today        must stay dark"),
            Some(_) => println!("  today        already the right shade"),
            None => println!("  today        the picture does not reach"),
        }
        if let Some(tomorrow) = today.succ_opt().and_then(|next| plan.on(next)) {
            println!(
                "  tomorrow     {}",
                if tomorrow.need == 0 {
                    "must stay dark".to_string()
                } else {
                    format!(
                        "{} of {} there",
                        thousands(tomorrow.have),
                        thousands(tomorrow.need)
                    )
                }
            );
        }
    }

    let holes = plan.holes();
    if !holes.is_empty() {
        println!("\n  {} day(s) already too bright:", holes.len());
        for day in holes.iter().take(8) {
            println!(
                "    {}  has {}, wants at most {}",
                day.date,
                thousands(day.have),
                thousands(day.ceiling.unwrap_or(0))
            );
        }
        if holes.len() > 8 {
            println!("    ... and {} more", holes.len() - 8);
        }
    }
    println!();
}

/// `--backfill` for a picture: commit what each day already past still owes.
fn backfill_canvas(options: &Options, grid: &Grid, levels: &BTreeMap<NaiveDate, u8>) {
    let (_, actual) = observed(options, grid);
    let plan = plan::Plan::from_levels("picture", grid, levels, 0, 0, &actual);
    let today = options.now();
    let owed: BTreeMap<NaiveDate, u32> = plan
        .days
        .iter()
        .filter(|day| day.date <= today && day.short() > 0)
        .map(|day| (day.date, day.short()))
        .collect();
    let total = owed
        .values()
        .fold(0u32, |sum, count| sum.saturating_add(*count));

    if owed.is_empty() {
        println!("nothing to catch up on — every day up to {today} is already right.");
        return;
    }
    println!(
        "{} day(s) up to {today} are short, {} contributions in total",
        owed.len(),
        thousands(total)
    );
    match (&options.repo, options.write) {
        (Some(repo), true) => {
            let (default_name, default_email) = art::identity();
            let who = options.name.clone().unwrap_or(default_name);
            let email = options.email.clone().unwrap_or(default_email);
            let made = art::write_commits(&owed, repo, "art: catch up", &who, &email)
                .unwrap_or_else(|error| fail(&error));
            println!(
                "made {} commits — nothing has been pushed.",
                thousands(made as u32)
            );
        }
        (Some(_), false) => println!("(add --write to create them; this was a dry run)"),
        (None, true) => fail("--write needs --repo DIR"),
        (None, false) => println!("(--repo DIR --write would create them)"),
    }
}

/// The year as it actually stands: a saved response if one was given, otherwise
/// whatever `gh` reports. Shared by `--track` and `--backfill`, which ask the
/// same question of the same year and must never get different answers.
fn observed(options: &Options, grid: &Grid) -> (String, BTreeMap<NaiveDate, u32>) {
    match &options.merge {
        Some(path) => (path.display().to_string(), load(path, grid)),
        None => {
            // Cleaned where it enters, which is the rule the calendar already
            // follows: a login can arrive from a plan file, a plan file is a
            // file someone may have sent you, and every path below this prints
            // it — a header, an error, the report's `source`. A terminal
            // executes what it is written.
            let who = options
                .tracking
                .clone()
                .or_else(github::whoami)
                .map(|who| mossaic::printable(&who))
                .unwrap_or_else(|| {
                    fail("could not tell whose contributions to track — pass `--track USER`, or run `gh auth login`")
                });
            // Named, because whose year this is can come from a saved plan
            // rather than from the command line, and "gh was not found" gives no
            // hint about which login it was going to ask for.
            let calendar = github::fetch(&who, grid.year, options.now()).unwrap_or_else(|error| {
                fail(&format!(
                    "could not read {who}'s {} contributions: {error}",
                    grid.year
                ))
            });
            (who, plan::contributions(&calendar))
        }
    }
}

/// Commit what the plan is short, and nothing else.
///
/// A plain `--write` puts the same count on every lit day, which over an active
/// year is both wasteful and self-defeating: adding to the busiest day raises
/// the year's peak, and the peak is what every letter day is measured against.
/// A shortfall cannot do that. `need` is at most the peak already — the
/// brightest shade is three quarters of it — so topping a day up to `need`
/// never exceeds the busiest day and never moves the target. One pass, with no
/// fixed point to chase.
fn backfill(
    options: &Options,
    grid: &Grid,
    columns: &[[bool; art::GLYPH_ROWS]],
    placed: &art::Placed,
    shades: art::Shades,
) {
    let repo = options.repo.clone().unwrap_or_else(|| {
        fail("--backfill needs --repo DIR — the directory to make the commits in")
    });
    let (who, actual) = observed(options, grid);
    let plan = plan::Plan::build(
        &options.text,
        grid,
        placed,
        columns.len(),
        options.top,
        &actual,
        shades,
    );

    // Days already past, and only those. A day at or past what it needs
    // contributes nothing here, and neither does one whose job is to stay dark —
    // `short()` is zero for both.
    //
    // The date bound is the point of the mode. Back-dating is the only way to
    // reach a day that has gone by; a day still to come needs no help, and
    // whether GitHub even counts a future-dated commit is unverified (see
    // docs/ART.md). Writing the whole year would also mean pre-filling a
    // background onto days that have not happened.
    let today = options.now();
    let owed: BTreeMap<NaiveDate, u32> = plan
        .days
        .iter()
        .filter(|day| day.date < today && day.short() > 0)
        .map(|day| (day.date, day.short()))
        .collect();
    let total = owed.values().fold(0u32, |sum, n| sum.saturating_add(*n));
    let ahead = plan
        .days
        .iter()
        .filter(|day| day.date >= today && day.short() > 0)
        .count();

    println!(
        "{}  ·  {}  ·  backfilling against {who}\n",
        plan.text, plan.year
    );
    let (letter_days, letter_commits) = plan.owing();
    println!(
        "  letters     {letter_days} day(s) short, {} commits",
        thousands(letter_commits)
    );
    if shades.field > 0 {
        let (field_days, field_commits) = plan.field_owing();
        println!(
            "  background  {field_days} day(s) short, {} commits",
            thousands(field_commits)
        );
    }
    println!(
        "  a day gets  what it is short of {}, never a flat count",
        thousands(plan.need)
    );
    // Only when there is something past to reach: before the year starts, "days
    // before 2026-08-19" for a 2027 plan names a date the plan has never heard
    // of, and the line below says the useful thing anyway.
    if !owed.is_empty() {
        println!("  reaching    days before {today}, which are the ones only back-dating reaches");
        if ahead > 0 {
            println!(
                "              {ahead} day(s) from {today} on are short too, and left alone — \
                 contribute on those as they come"
            );
        }
    }
    // An irreversible write is the worst place to leave this unsaid: the letters
    // can be finished and still read with holes in them, and no number of
    // commits changes that.
    if let plan::Verdict::Holed { holes } = plan.verdict() {
        println!(
            "\n  warning: {} cannot be drawn cleanly in {} — {holes} day(s) inside the\n  \
             letters are already lit, and nothing takes those away. Backfilling will\n  \
             brighten the letters, and the text will still read with holes in it.\n  \
             `mossaic-art --track` sweeps --start-week for a placement with fewer.",
            plan.text, plan.year
        );
    }

    if owed.is_empty() {
        match ahead {
            0 => println!("\nnothing to backfill — every day the plan wants is already there."),
            _ => {
                println!("\nnothing to backfill — every day the plan is short of is still to come.")
            }
        }
        return;
    }
    println!(
        "\n  {} commit(s) across {} day(s), earliest {}, latest {}",
        thousands(total),
        owed.len(),
        owed.keys().next().expect("not empty"),
        owed.keys().next_back().expect("not empty"),
    );

    if !options.write {
        println!("\n(add --write to create them; this was a dry run)");
        return;
    }

    let (name, email) = art::identity();
    let name = options.name.clone().unwrap_or(name);
    let email = options.email.clone().unwrap_or(email);
    println!(
        "\ncommitting {} commits into {} as {name} <{email}>",
        thousands(total),
        repo.display()
    );
    let label = format!("art: {}", plan.text);
    let made = art::write_commits(&owed, &repo, &label, &name, &email)
        .unwrap_or_else(|error| fail(&error));
    println!(
        "made {} commits — nothing has been pushed.\n\n\
         To publish, from {}:\n\n  \
         git push -u origin main\n\n\
         To count towards the graph these must be on the default branch of a repo \
         you own (not a fork), authored with an email registered to your GitHub \
         account. Run --track again afterwards: GitHub takes a few minutes to \
         recount.",
        thousands(made as u32),
        repo.display()
    );
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

    let (who, actual) = observed(options, grid);

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

    // Resolved once, and said out loud only when the plan's year has already
    // ended: `under_way` and not `holds` together mean today is past the last
    // day of it. There the today, tomorrow and next-seven-days lines are simply
    // absent with nothing to explain them, which reads like a fault.
    //
    // Deliberately silent for a year that has *not started*, which is the other
    // way to fall outside it. That is the first example in the help, the README
    // and docs/ART.md, and the report already answers it in full — "2027 has not
    // started. The first letter day is Mon Feb 8." A note there told people
    // their own year was wrong.
    let today = options.now();
    if plan.under_way(today) && !plan.holds(today) {
        eprintln!(
            "note: {today} is after {}, so this report has no today or tomorrow in \
             it — those lines cover the plan's own year.",
            plan.year
        );
    }

    // json and markdown are for machines and for messages; both carry every
    // number the text report prints, so a notification never has to be parsed
    // out of a screen.
    if options.format != Format::Text {
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
            // Over its ceiling, and nothing undoes it. Which loss it is
            // depends on where the day sits: inside the letters it is a hole,
            // and calling that "too many for level 0" when no background was
            // asked for describes a shade that does not exist.
            Some(day) if day.over() > 0 && day.want == plan::Want::Hole => {
                format!(
                    "inside the letters and already lit ({}) — a permanent hole",
                    thousands(day.have)
                )
            }
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
            // Inside the letters with nothing asked of it. Committing here is
            // the one mistake that cannot be worked off afterwards, so saying
            // so before the day arrives is the whole point of reading this.
            Some(day) if day.want == plan::Want::Hole => {
                "inside the letters — keep it dark, or it becomes a permanent hole".to_string()
            }
            Some(day) if day.have > 0 => "not part of the text, and already lit".to_string(),
            _ => "not part of the text — anything committed on it shows".to_string(),
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
        .any(|day| day.want != plan::Want::Around || day.need > 0)
    {
        println!("\n  the next seven days");
        for day in upcoming {
            let what = match day.want {
                plan::Want::Lit if day.short() > 0 => {
                    format!("letter  {} to go", thousands(day.short()))
                }
                plan::Want::Lit => "letter  done".to_string(),
                // Already past its ceiling, before the advice arrives. Telling
                // someone to keep a day dark that is already lit is worse than
                // saying nothing: inside the letters it is a hole and permanent,
                // and outside it is a field day that has run too bright.
                plan::Want::Hole if day.over() > 0 => "hole".to_string(),
                _ if day.over() > 0 => "too bright".to_string(),
                _ if day.short() > 0 => {
                    format!("field   {} to go", thousands(day.short()))
                }
                _ if day.need > 0 => "field   done".to_string(),
                // Nothing to do, which is not the same as nothing to say: this
                // is a day inside the letters that has to stay empty.
                plan::Want::Hole => "keep dark".to_string(),
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
        // The command has to be the one that does what the sentence above it
        // says. A plain `--write` would put a flat `--commits` on every lit day,
        // including the ones already bright — and with the text typed out it
        // would not even read the plan, so the placement could differ from the
        // one just reported on.
        println!(
            "    {past} letter day(s) already past, {} contributions — only back-dated\n    \
             commits reach those:\n\n      mossaic-art {}--backfill --repo ../art --write\n",
            thousands(past_commits),
            match options.plan_loaded {
                true => String::new(),
                // No saved plan, so the placement has to be spelled out or the
                // command would draw a differently-placed text.
                false => format!(
                    "{} --year {} --start-week {} --top {} {}",
                    plan.text,
                    plan.year,
                    plan.start_week,
                    options.top,
                    match shades.field {
                        0 => String::new(),
                        level => format!("--background {level} "),
                    }
                ),
            }
        );
        if !options.plan_loaded {
            println!(
                "    (or `--save` the plan once, and it is just \
                 `mossaic-art --backfill …`)"
            );
        }
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
            .map(|character| format!("{:<12}", art::label(*character)))
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

/// Every glyph the font has, drawn as contribution cells and written to a PNG.
///
/// The font view above is for a terminal; this is for everywhere else. A
/// README cannot show block sextants — too few systems have a font for them —
/// and it cannot show the symbols either, since half of them are
/// ambiguous-width. An image of the real rasteriser's output has neither
/// problem, and it is what the glyphs will actually look like on a graph.
fn write_font_sheet(path: &Path) {
    /// Glyphs to a row. Thirteen is what makes the sheet about twice as wide
    /// as it is tall, which is the shape a README column wants.
    const PER_ROW: usize = 13;
    /// Bright glyphs on a light-green field, which is how the art at the top
    /// of the README is drawn — and the arrangement that keeps every day of a
    /// year green rather than leaving 300 of them dark.
    const INK: u8 = 4;
    const FIELD: u8 = 1;

    // Space draws nothing, so a blank block in the middle of the sheet would
    // read as a mistake rather than as a glyph.
    let characters: Vec<char> = art::alphabet().filter(|c| !c.is_whitespace()).collect();
    /// Field between glyphs, and around them. One cell was not enough: with
    /// the gutter the same shade as the field, neighbouring glyphs had no
    /// boundary and a row of letters read as one long smear.
    const GAP: usize = 2;
    let rows = characters.len().div_ceil(PER_ROW);
    let width = PER_ROW * (art::GLYPH_COLS + GAP) + GAP;
    let height = rows * (art::GLYPH_ROWS + GAP) + GAP;

    // The field first, so the gaps between glyphs are the field rather than
    // holes in it, then each glyph's lit days over the top.
    let mut levels: Vec<Vec<Option<u8>>> = vec![vec![Some(FIELD); height]; width];
    for (index, character) in characters.iter().enumerate() {
        let glyph = art::glyph(*character).expect("from the font itself");
        let left = GAP + (index % PER_ROW) * (art::GLYPH_COLS + GAP);
        let top = GAP + (index / PER_ROW) * (art::GLYPH_ROWS + GAP);
        for (row, line) in glyph.iter().enumerate() {
            for (column, lit) in line.chars().enumerate() {
                if lit == '#' {
                    levels[left + column][top + row] = Some(INK);
                }
            }
        }
    }

    let palette = Palette::new(Appearance::Dark, Season::Default, true);
    let image = graphics::sheet(&levels, &palette, (9, 19));
    png::write(path, &image, palette.canvas)
        .unwrap_or_else(|error| fail(&format!("could not write {}: {error}", path.display())));
    println!(
        "wrote {} — {}x{} px, {} glyphs",
        path.display(),
        image.width,
        image.height,
        characters.len()
    );
}

/// Existing contributions from a saved `gh api graphql` response.
fn load(path: &PathBuf, grid: &Grid) -> BTreeMap<NaiveDate, u32> {
    let calendar = github::from_file(&path.to_string_lossy(), None)
        .unwrap_or_else(|error| fail(&format!("could not read {path:?}: {error}")));
    let kept: BTreeMap<NaiveDate, u32> = calendar
        .days()
        .filter(|day| day.count > 0 && grid.holds(day.date))
        .map(|day| (day.date, day.count))
        .collect();
    // A calendar for another year filters down to nothing, and everything
    // downstream is then correct about an empty year: 9,527 contributions read
    // as none, and an impossible plan reads as trivially reachable. `--year` is
    // the flag most likely to be typed as a one-off over a saved plan, so the
    // mismatch is easy to reach and silent when it happens.
    if kept.is_empty() && calendar.days().any(|day| day.count > 0) {
        eprintln!(
            "note: {} holds no contributions in {} — it covers {}. Tracking \
             against an empty year.",
            path.display(),
            grid.year,
            calendar.year
        );
    }
    kept
}

/// What `--track` prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Text,
    Json,
    Markdown,
}

impl Options {
    /// What "today" means for this run: `--today` when it was given, otherwise
    /// the clock.
    ///
    /// An input rather than an ambient fact, which is what makes a report
    /// reproducible — and what lets a plan be asked about a day that has not
    /// arrived yet.
    fn now(&self) -> NaiveDate {
        self.today.unwrap_or_else(|| Local::now().date_naive())
    }
}

struct Options {
    format: Format,
    track: bool,
    tracking: Option<String>,
    /// Commit each day's shortfall rather than a flat count.
    backfill: bool,
    /// What "today" means. `None` reads the clock.
    today: Option<NaiveDate>,
    /// Whether the plan was read from a file, which decides how the advice at
    /// the end of `--track` spells the command it suggests.
    plan_loaded: bool,
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
    /// The picture being drawn, and what to call it, for a run that draws a
    /// canvas rather than text.
    ///
    /// Beside `text` rather than replacing it, so that every path that reads
    /// `text` is exactly the code it was: a canvas run branches away at the
    /// top of `main` and never reaches them.
    canvas: Option<(String, art::Canvas)>,
    /// Where `--draw` writes the picture it made.
    output: Option<PathBuf>,
}

fn parse_args() -> Option<Options> {
    let mut options = Options {
        format: Format::Text,
        track: false,
        tracking: None,
        backfill: false,
        today: None,
        plan_loaded: false,
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
        canvas: None,
        output: None,
    };

    let mut template: Option<String> = None;
    let mut matrix: Option<PathBuf> = None;
    let mut list_templates = false;
    let mut font = false;
    let mut png_path: Option<PathBuf> = None;
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
            // Every one of these is bounded where it is read, so the cast that
            // follows cannot misrepresent it. `--commits -1` used to come out
            // as four billion, and `--start-week -1` as `usize::MAX`, where
            // building a date from it panicked.
            "--commits" => {
                // A million a day is far past any real need — the brightest
                // shade is three quarters of the year's peak — and it keeps a
                // whole year of them inside a u32.
                options.commits = args.number_in("--commits", "count", 1..=1_000_000) as u32;
            }
            "--background" | "--bg" => {
                // Bounded here so the error names the flag; `Shades::check`
                // then decides whether the pair can draw anything.
                options.background = args.number_in("--background", "level", 0..=4) as u8;
            }
            "--top" => {
                let rows = (art::WEEKDAYS - art::GLYPH_ROWS) as i64;
                options.top = args.number_in("--top", "row", 0..=rows) as usize;
            }
            "--start-week" => {
                // A loose bound here, because the tight one depends on the year
                // and the text: `place` knows both and refuses what will not
                // fit, naming the last column that would.
                options.start_week =
                    Some(args.number_in("--start-week", "column", 0..=60) as usize);
            }
            "--backfill" => options.backfill = true,
            "--today" => {
                let raw = args.value("--today");
                let date = NaiveDate::parse_from_str(&raw, "%Y-%m-%d").unwrap_or_else(|_| {
                    fail(&format!("--today wants a date as YYYY-MM-DD, not {raw:?}"))
                });
                if !YEARS.contains(&date.year()) {
                    fail(&format!(
                        "--today {raw} is outside the years these tools work in, \
                         {} to {}",
                        YEARS.start(),
                        YEARS.end()
                    ));
                }
                options.today = Some(date);
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
            "--template" => template = Some(args.value("--template")),
            "--matrix" | "--file" => matrix = Some(args.value("--matrix").into()),
            "--list-templates" => list_templates = true,
            "--output" | "-o" => options.output = Some(args.value("--output").into()),
            "--font" => font = true,
            "--png" => png_path = Some(PathBuf::from(args.value("--png"))),
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
            other if options.text.is_empty() => {
                // Expanded here rather than at every use: the plan is saved
                // uppercased and read back by the tracker, and `:star:` has to
                // survive both. A character does.
                options.text = art::canonical(other).unwrap_or_else(|error| fail(&error));
            }
            other => fail(&format!("unexpected argument {other:?}")),
        }
    }

    if list_templates {
        show_templates(options.colour.enabled());
        return None;
    }

    // One picture, from one place. Two sources on one command line would mean
    // guessing which was meant, and the guess would be silent.
    let sources = [
        ("--template", template.is_some()),
        ("--matrix", matrix.is_some()),
        ("a TEXT argument", !options.text.is_empty()),
    ];
    let given: Vec<&str> = sources
        .iter()
        .filter(|(_, present)| *present)
        .map(|(name, _)| *name)
        .collect();
    if given.len() > 1 {
        fail(&format!(
            "{} both draw the picture — pick one",
            given.join(" and ")
        ));
    }

    if let Some(name) = &template {
        let found = templates::find(name).unwrap_or_else(|error| fail(&error));
        options.canvas = Some((found.title().to_string(), found.canvas));
    }
    if let Some(path) = &matrix {
        let body = std::fs::read_to_string(path)
            .unwrap_or_else(|error| fail(&format!("could not read {}: {error}", path.display())));
        let canvas = art::Canvas::parse(&body)
            .unwrap_or_else(|error| fail(&format!("{}: {error}", path.display())));
        let name = canvas.meta().name.clone().unwrap_or_else(|| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("picture")
                .to_string()
        });
        options.canvas = Some((name, canvas));
    }

    // After the loop, so that flags on either side of it are all in hand.
    if font {
        match &png_path {
            Some(path) => write_font_sheet(path),
            None => show_font(options.colour.enabled()),
        }
        return None;
    }
    // A saved plan fills in whatever was not typed. Typed flags always win, so
    // `--year 2028` against a saved 2027 plan is a one-off, not a surprise.
    // Set before the plan is read, so that the `is_none` check below sees a
    // user typed on this command line. Assigning them afterwards is what threw
    // the saved one away: `tracking` is `None` for a bare `--track`, so a plan
    // that named a user was loaded and then overwritten with nothing.
    options.track = track;
    options.tracking = tracking;
    match (
        options.text.is_empty() && options.canvas.is_none(),
        options.plan_path.exists(),
    ) {
        (true, true) => {
            let spec = plan::Spec::load(&options.plan_path).unwrap_or_else(|error| fail(&error));
            options.plan_loaded = true;
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
            // A saved picture is the picture. `text` is then its name rather
            // than something to run through the font, so the two are swapped
            // over here and the text path is never entered.
            if let Some(canvas) = spec.art.as_deref() {
                let canvas = art::Canvas::parse(canvas).unwrap_or_else(|error| {
                    fail(&format!("{}: {error}", options.plan_path.display()))
                });
                options.canvas = Some((options.text.clone(), canvas));
                options.text = String::new();
            }
        }
        (true, false) => fail(&format!(
            "nothing to draw.\n\n  \
             mossaic-art VYNCINT --year 2027        draw something\n  \
             mossaic-art --template dragon          draw a picture instead\n  \
             mossaic-art VYNCINT --year 2027 --save remember it, then just \
             `mossaic-art --track`\n\n\
             (no plan at {})",
            options.plan_path.display()
        )),
        _ => {}
    }
    // One reports and the other writes; running both from one command line
    // would mean guessing which was meant.
    if options.track && options.backfill {
        fail("--track reports on the plan and --backfill commits to it — pick one");
    }
    // Tracking writes nothing, so a flag asking it to was doing nothing at all —
    // silently, which is worse than refusing. Distinct from `--commits`, a tuning
    // parameter tracking genuinely has no use for; these ask for a side effect.
    if options.track {
        for (flag, asked) in [
            ("--snapshot", options.snapshot.is_some()),
            ("--write", options.write),
            ("--repo", options.repo.is_some()),
        ] {
            if asked {
                fail(&format!(
                    "--track reports on the plan and writes nothing, so {flag} would \
                     do nothing. Drop one of them."
                ));
            }
        }
    }
    Some(options)
}

fn fail(message: &str) -> ! {
    eprintln!("mossaic-art: {message}");
    std::process::exit(2)
}
