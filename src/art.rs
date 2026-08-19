//! Writing text into a contribution graph by dating commits.
//!
//! Each letter is drawn as pixels on the year's 7×53 calendar, every lit pixel
//! is mapped to a date, and the commits that would light it up are emitted.
//! Nothing here pushes anything: [`write_commits`] makes local commits in a
//! directory you name, and the caller prints the push command.
//!
//! The costing is the interesting part. GitHub's shade for a day is
//! `min(4, ceil(count * 4 / peak))` where `peak` is the busiest day of that
//! year — equal slices of `[0, peak]`, not rank quartiles — so drawing over an
//! active year is priced by its busiest day, and raising the count raises the
//! peak along with it. [`commits_for_level`] solves for the figure rather than
//! estimating it.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use chrono::{Datelike, Days, NaiveDate};

use crate::primer::{Appearance, Legibility, Palette, Season};

/// Rows in a glyph. Letters are five tall, placed on Mon-Fri, which leaves
/// Sunday and Saturday clear.
pub const GLYPH_ROWS: usize = 5;
/// Columns in a glyph. Uniform, so letters line up and `6N - 1` describes the
/// width of any text — the placement, the centring and the eight-character
/// limit all rest on it.
pub const GLYPH_COLS: usize = 5;
/// Rows in a calendar week, Sunday first. Public because it is what bounds
/// `--top`: five rows of glyph have to fit inside it.
pub const WEEKDAYS: usize = 7;

/// The font: `#` lights a day, `.` leaves it dark.
///
/// # Adding a glyph
///
/// Add a row to this table and nothing else — [`alphabet`], the previews and
/// the error message for unknown characters all read from it. Three rules,
/// every one of them checked when the crate compiles rather than when someone
/// runs it:
///
/// 1. exactly [`GLYPH_ROWS`] rows of exactly [`GLYPH_COLS`] characters,
/// 2. only `#` and `.`,
/// 3. no character twice.
///
/// Break one and the build stops with the reason. `mossaic-art --font` prints the whole
/// set, which is the quickest way to see what a new glyph actually looks like
/// next to its neighbours.
///
/// The table is uppercase because [`bitmap`] folds its input, but it looks up
/// the character as written first — so lowercase glyphs can be added later
/// without touching anything else.
const FONT: &[(char, [&str; GLYPH_ROWS])] = &[
    ('A', [".###.", "#...#", "#####", "#...#", "#...#"]),
    ('B', ["####.", "#...#", "####.", "#...#", "####."]),
    ('C', [".###.", "#...#", "#....", "#...#", ".###."]),
    ('D', ["####.", "#...#", "#...#", "#...#", "####."]),
    ('E', ["#####", "#....", "####.", "#....", "#####"]),
    ('F', ["#####", "#....", "####.", "#....", "#...."]),
    ('G', [".###.", "#....", "#..##", "#...#", ".###."]),
    ('H', ["#...#", "#...#", "#####", "#...#", "#...#"]),
    ('I', ["#####", "..#..", "..#..", "..#..", "#####"]),
    ('J', ["....#", "....#", "....#", "#...#", ".###."]),
    ('K', ["#...#", "#..#.", "###..", "#..#.", "#...#"]),
    ('L', ["#....", "#....", "#....", "#....", "#####"]),
    ('M', ["#...#", "##.##", "#.#.#", "#...#", "#...#"]),
    ('N', ["#...#", "##..#", "#.#.#", "#..##", "#...#"]),
    ('O', [".###.", "#...#", "#...#", "#...#", ".###."]),
    ('P', ["####.", "#...#", "####.", "#....", "#...."]),
    ('Q', [".###.", "#...#", "#.#.#", "#..#.", ".##.#"]),
    ('R', ["####.", "#...#", "####.", "#..#.", "#...#"]),
    ('S', [".####", "#....", ".###.", "....#", "####."]),
    ('T', ["#####", "..#..", "..#..", "..#..", "..#.."]),
    ('U', ["#...#", "#...#", "#...#", "#...#", ".###."]),
    ('V', ["#...#", "#...#", "#...#", ".#.#.", "..#.."]),
    ('W', ["#...#", "#...#", "#.#.#", "##.##", "#...#"]),
    ('X', ["#...#", ".#.#.", "..#..", ".#.#.", "#...#"]),
    ('Y', ["#...#", ".#.#.", "..#..", "..#..", "..#.."]),
    ('Z', ["#####", "...#.", "..#..", ".#...", "#####"]),
    ('0', [".###.", "#..##", "#.#.#", "##..#", ".###."]),
    ('1', ["..#..", ".##..", "..#..", "..#..", ".###."]),
    ('2', [".###.", "#...#", "..##.", ".#...", "#####"]),
    ('3', ["####.", "....#", "..##.", "....#", "####."]),
    ('4', ["#..#.", "#..#.", "#####", "...#.", "...#."]),
    ('5', ["#####", "#....", "####.", "....#", "####."]),
    ('6', [".###.", "#....", "####.", "#...#", ".###."]),
    ('7', ["#####", "....#", "...#.", "..#..", ".#..."]),
    ('8', [".###.", "#...#", ".###.", "#...#", ".###."]),
    ('9', [".###.", "#...#", ".####", "....#", ".###."]),
    (' ', [".....", ".....", ".....", ".....", "....."]),
    ('-', [".....", ".....", "#####", ".....", "....."]),
    ('.', [".....", ".....", ".....", ".....", "..#.."]),
];

/// The font's rules, enforced at compile time. A glyph contributed with a row a
/// character short used to be a panic at the first person to draw it; now it is
/// a build failure with the reason, before it can be merged.
const _: () = {
    let mut index = 0;
    while index < FONT.len() {
        let (character, rows) = FONT[index];

        let mut row = 0;
        while row < GLYPH_ROWS {
            let bytes = rows[row].as_bytes();
            assert!(
                bytes.len() == GLYPH_COLS,
                "every glyph row must be GLYPH_COLS characters wide"
            );
            let mut column = 0;
            while column < bytes.len() {
                assert!(
                    bytes[column] == b'#' || bytes[column] == b'.',
                    "glyph rows are made of '#' and '.' only"
                );
                column += 1;
            }
            row += 1;
        }

        let mut other = 0;
        while other < index {
            assert!(
                FONT[other].0 as u32 != character as u32,
                "the same character is in the font twice"
            );
            other += 1;
        }
        index += 1;
    }
};

/// The glyph for a character, if the font has one.
///
/// The character as written wins, so a lowercase glyph added to the font table
/// in `src/art.rs` would be used for lowercase text; failing that, its uppercase
/// form is tried, which is what makes `vyncint` and `VYNCINT` draw the same
/// thing today.
pub fn glyph(character: char) -> Option<[&'static str; GLYPH_ROWS]> {
    let exact = |wanted: char| {
        FONT.iter()
            .find(|(candidate, _)| *candidate == wanted)
            .map(|(_, rows)| *rows)
    };
    exact(character).or_else(|| character.to_uppercase().find_map(exact))
}

/// Every character the font can draw, in order.
pub fn alphabet() -> impl Iterator<Item = char> {
    FONT.iter().map(|(character, _)| *character)
}

/// The Sunday that starts `day`'s calendar column. Sunday is row 0.
///
/// Panics within six days of the first date a [`NaiveDate`] can hold, where
/// there is no earlier Sunday to name. [`Grid::new`] does this arithmetic
/// checked, because a year reaching it can come from a file.
pub fn sunday_of(day: NaiveDate) -> NaiveDate {
    day - Days::new(u64::from(day.weekday().num_days_from_sunday()))
}

/// The calendar for one year: seven rows by however many Sunday-aligned weeks.
#[derive(Debug, Clone, Copy)]
pub struct Grid {
    /// The calendar year this grid covers.
    pub year: i32,
    /// January 1st.
    pub first: NaiveDate,
    /// December 31st.
    pub last: NaiveDate,
    /// The Sunday that starts column 0, which is usually in the year before.
    pub start: NaiveDate,
    /// Columns, counting the partial weeks at both ends.
    pub weeks: usize,
}

impl Grid {
    /// The calendar for `year`, or `None` for one no calendar can hold.
    ///
    /// Returns rather than panics because this is a library: a year arriving
    /// from a command line, a file or a caller is input, and `--year 999999`
    /// used to end in an `expect`.
    pub fn new(year: i32) -> Option<Self> {
        let first = NaiveDate::from_ymd_opt(year, 1, 1)?;
        let last = NaiveDate::from_ymd_opt(year, 12, 31)?;
        // Checked, not [`sunday_of`]: the first year a calendar can express has
        // no room before it, so stepping back to a Sunday runs off the end and
        // panics. This function promises `None` for a year no calendar can hold,
        // and a year can arrive from a plan file — so it has to keep that
        // promise rather than nearly keep it.
        let start =
            first.checked_sub_days(Days::new(u64::from(first.weekday().num_days_from_sunday())))?;
        Some(Self {
            year,
            first,
            last,
            start,
            weeks: ((last - start).num_days() / 7 + 1) as usize,
        })
    }

    /// The date at a grid position, which may fall outside the year.
    ///
    /// `week` must be less than [`Grid::weeks`] and `row` less than 7 — that is
    /// what "a position on this grid" means, and every caller here walks a
    /// bounded range to satisfy it. Far outside that, adding the offset to a
    /// date runs off the end of the calendar and panics, which is why
    /// [`place`] refuses a start column that would not fit rather than
    /// building dates from it.
    pub fn date_at(&self, week: usize, row: usize) -> NaiveDate {
        self.start + Days::new((week * WEEKDAYS + row) as u64)
    }

    /// Whether `day` is inside the year this grid covers.
    pub fn holds(&self, day: NaiveDate) -> bool {
        self.first <= day && day <= self.last
    }

    /// Columns whose Mon–Fri all fall inside the year. The first and last are
    /// partial weeks, so a letter placed on one loses whatever spills over.
    pub fn usable_weeks(&self) -> usize {
        (0..self.weeks)
            .filter(|week| (1..=5).all(|row| self.holds(self.date_at(*week, row))))
            .count()
    }
}

/// Columns of the rendered text, each `GLYPH_ROWS` tall, with one blank column
/// between letters.
pub fn bitmap(text: &str) -> Result<Vec<[bool; GLYPH_ROWS]>, String> {
    let unknown: Vec<char> = text.chars().filter(|c| glyph(*c).is_none()).collect();
    if !unknown.is_empty() {
        let names: Vec<String> = unknown.iter().map(|c| format!("{c:?}")).collect();
        return Err(format!(
            "no glyph for: {} — the font has {}",
            names.join(" "),
            describe_alphabet()
        ));
    }

    let mut columns = Vec::new();
    for (index, character) in text.chars().enumerate() {
        let rows = glyph(character).expect("checked above");
        if index > 0 {
            columns.push([false; GLYPH_ROWS]);
        }
        // GLYPH_COLS rather than the first row's length: the width is the same
        // for every glyph, and the compile-time check above is what guarantees it.
        for column in 0..GLYPH_COLS {
            let mut lit = [false; GLYPH_ROWS];
            for (row, line) in rows.iter().enumerate() {
                lit[row] = line.as_bytes()[column] == b'#';
            }
            columns.push(lit);
        }
    }
    Ok(columns)
}

/// The font's contents, for the message someone sees after a typo.
fn describe_alphabet() -> String {
    let printable: String = alphabet()
        .filter(|c| !c.is_whitespace())
        .collect::<Vec<char>>()
        .chunks(36)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect::<Vec<String>>()
        .join(" ");
    format!("{printable} and space")
}

/// The two shades contribution art is drawn in.
///
/// The classic look is `ink: 4, field: 0` — letters against an empty graph. It
/// reads perfectly and it costs you every other day of the year: to keep the
/// background dark you have to *stop contributing* on roughly three hundred
/// days, which is a strange thing for a tool about contributing to ask.
///
/// Raising `field` above zero draws the background as a colour rather than as
/// nothing. The letters are then the difference between two greens instead of
/// the difference between green and empty, and a daily contributor can draw art
/// without going dark for most of the year.
///
/// The catch is that the difference has to be visible, and GitHub's five shades
/// are not evenly spaced. Adjacent levels come as close as ΔE 10.8; levels two
/// or more apart never fall below ΔE 35.4. [`Shades::worst`] measures it across
/// every palette a reader might be looking at.
///
/// ```
/// # use mossaic::art::Shades;
/// # use mossaic::primer::Legibility;
/// // Level 1 under level 4: clear in every palette GitHub ships.
/// assert_eq!(Shades { ink: 4, field: 1 }.worst().0, Legibility::Clear);
/// // One level apart is a gamble that depends on the reader's theme.
/// assert_eq!(Shades { ink: 4, field: 3 }.worst().0, Legibility::Faint);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shades {
    /// The level a letter day must reach.
    pub ink: u8,
    /// The level every other day should sit at. Zero leaves the background
    /// empty, which is the classic look.
    pub field: u8,
}

impl Default for Shades {
    /// The brightest letters on an empty background.
    fn default() -> Self {
        Self { ink: 4, field: 0 }
    }
}

impl Shades {
    /// Every palette a reader might have the graph open in: three appearances
    /// times three seasons, and GitHub picks the season by date, not by choice.
    const READERS: [(Appearance, Season); 9] = [
        (Appearance::Light, Season::Default),
        (Appearance::Light, Season::Winter),
        (Appearance::Light, Season::Halloween),
        (Appearance::Dark, Season::Default),
        (Appearance::Dark, Season::Winter),
        (Appearance::Dark, Season::Halloween),
        (Appearance::Dimmed, Season::Default),
        (Appearance::Dimmed, Season::Winter),
        (Appearance::Dimmed, Season::Halloween),
    ];

    /// Whether these two shades can draw anything at all.
    ///
    /// This is the one rule that cannot be relaxed: a background at or above
    /// the letters does not make faint art, it makes a blank graph.
    pub fn check(self) -> Result<(), String> {
        if self.ink > 4 || self.field > 4 {
            return Err(format!(
                "shades run 0 to 4; got ink {} and field {}",
                self.ink, self.field
            ));
        }
        if self.ink == 0 {
            return Err("letters cannot be drawn at level 0 — that is an empty day".to_string());
        }
        if self.field >= self.ink {
            return Err(format!(
                "the background (level {}) must be darker than the letters (level {}), \
                 or there is nothing to see",
                self.field, self.ink
            ));
        }
        Ok(())
    }

    /// How far apart the two shades look in one palette, as CIE76 ΔE.
    pub fn separation(self, palette: &Palette) -> f32 {
        palette.separation(self.field, self.ink)
    }

    /// The worst these shades look in any palette a reader might have, and the
    /// separation that earned it.
    ///
    /// Art is drawn once and read by everyone: someone on the light theme,
    /// someone on dark, and — for a few weeks each year — everyone at once on
    /// GitHub's seasonal palette. The honest number is the worst of them.
    pub fn worst(self) -> (Legibility, f32) {
        let worst = Self::READERS
            .iter()
            .map(|(appearance, season)| self.separation(&Palette::new(*appearance, *season, true)))
            .fold(f32::INFINITY, f32::min);
        (Legibility::of(worst), worst)
    }

    /// The smallest busiest-day these shades can be told apart in.
    ///
    /// GitHub's scale has four steps, so a year whose busiest day is 1 holds
    /// exactly two shades: empty and full. Asking for a level-1 background in
    /// one is asking for a colour the scale cannot express — both shades round
    /// to the same commit count and the letters vanish. At a peak of 4 the
    /// counts 1, 2, 3, 4 land on levels 1, 2, 3, 4 exactly, which is as small
    /// as a five-shade year gets.
    pub fn min_peak(self) -> u32 {
        if self.field > 0 {
            4
        } else {
            1
        }
    }

    /// The commits a day of each kind needs, in a year whose busiest day ends
    /// at `peak`.
    pub fn commits(self, peak: u32) -> Ink {
        Ink {
            lit: commits_to_reach(self.ink, peak),
            field: match self.field {
                0 => 0,
                level => commits_to_reach(level, peak),
            },
        }
    }

    /// The most a background day may hold before it stops being background.
    ///
    /// Zero-field art has a ceiling of zero: the day must stay dark. Otherwise
    /// it is one below the count that would reach the next shade up, and that is
    /// always a number — [`commits_to_reach`] never returns less than 1, so the
    /// `Option` here is a shape the caller wanted rather than a case that
    /// happens. [`crate::plan::Day::ceiling`] is the one that is genuinely
    /// `None`, for a day that cannot be too bright at all.
    pub fn ceiling(self, peak: u32) -> Option<u32> {
        if self.field == 0 {
            return Some(0);
        }
        // Saturating, because `Shades` is public and need not have been checked:
        // a field of 255 would otherwise overflow the level rather than clamp.
        commits_to_reach(self.field.saturating_add(1), peak).checked_sub(1)
    }
}

/// How many commits each kind of day gets.
///
/// Separate from [`Shades`] because a shade is what a reader sees and a commit
/// count is what you have to do — the map between them depends on the year's
/// busiest day, which changes as the art is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ink {
    /// Commits on a day that is part of a letter.
    pub lit: u32,
    /// Commits on every other day. Zero leaves the background empty.
    pub field: u32,
}

/// Where the text landed, and what it cost.
#[derive(Debug, Clone, Default)]
pub struct Placed {
    /// Commits per lit day, in date order.
    pub lit: BTreeMap<NaiveDate, u32>,
    /// Commits per background day, in date order. Empty unless a field shade
    /// was asked for.
    pub field: BTreeMap<NaiveDate, u32>,
    /// Pixels that fell outside the year — the first and last calendar columns
    /// are partial weeks, so text that fills the year loses its edges.
    pub skipped: usize,
    /// The column the text starts at, centred unless one was given.
    pub start_week: usize,
}

impl Placed {
    /// Every day the art writes to, letters and background together.
    pub fn all(&self) -> BTreeMap<NaiveDate, u32> {
        let mut all = self.field.clone();
        all.extend(self.lit.iter().map(|(date, count)| (*date, *count)));
        all
    }

    /// Every commit the art would make.
    pub fn total(&self) -> u32 {
        self.all()
            .values()
            .fold(0u32, |sum, count| sum.saturating_add(*count))
    }
}

/// Map lit pixels onto dates. `top` is the first calendar row used, 0 = Sunday.
///
/// When `ink.field` is non-zero every other day of the year is filled too, so
/// the art is a contrast between two shades rather than between something and
/// nothing. Those days land in [`Placed::field`], kept apart from the letters
/// because they are priced, tracked and reported differently.
pub fn place(
    columns: &[[bool; GLYPH_ROWS]],
    grid: &Grid,
    top: usize,
    start: Option<usize>,
    ink: Ink,
) -> Result<Placed, String> {
    // Subtraction, so the guard cannot be wrapped past: `usize::MAX + GLYPH_ROWS`
    // comes out as 4, which is comfortably "inside" a seven-row week, and the
    // rows were then drawn wherever the wrapping took them.
    if top > WEEKDAYS - GLYPH_ROWS {
        return Err(format!("--top {top} would push the text past row 6"));
    }
    if columns.len() > grid.weeks {
        return Err(format!(
            "{} columns needed but {} has only {}; use shorter text",
            columns.len(),
            grid.year,
            grid.weeks
        ));
    }
    // Centred by default, which is also what keeps a short text off the ragged
    // first and last columns.
    let start_week = start.unwrap_or((grid.weeks - columns.len()) / 2);
    // Refused rather than drawn. Past the last column that fits, every pixel
    // falls outside the year: the old answer was a note about dropped pixels
    // and a plan of nought days, and far enough out — `--start-week -1`, cast
    // to a usize — building the date panicked.
    // Subtraction rather than `start_week + columns.len() > grid.weeks`: the
    // check above has already ruled out the underflow, and the addition would
    // itself overflow for the very value that made this check necessary.
    if start_week > grid.weeks - columns.len() {
        return Err(format!(
            "--start-week {start_week} puts {} columns past the end of {}, \
             which has {}; the last one that fits is {}",
            columns.len(),
            grid.year,
            grid.weeks,
            grid.weeks - columns.len()
        ));
    }

    let mut lit = BTreeMap::new();
    let mut skipped = 0;
    for (offset, column) in columns.iter().enumerate() {
        for (row, on) in column.iter().enumerate() {
            if !on {
                continue;
            }
            let day = grid.date_at(start_week + offset, top + row);
            if grid.holds(day) {
                lit.insert(day, ink.lit);
            } else {
                skipped += 1;
            }
        }
    }

    // The background is every day of the year the letters did not claim. Built
    // after the letters so a lit day is never also a field day, whatever order
    // the glyphs were walked in.
    let mut field = BTreeMap::new();
    if ink.field > 0 {
        let mut date = grid.first;
        loop {
            if !lit.contains_key(&date) {
                field.insert(date, ink.field);
            }
            match date.succ_opt() {
                Some(next) if next <= grid.last => date = next,
                _ => break,
            }
        }
    }

    Ok(Placed {
        lit,
        field,
        skipped,
        start_week,
    })
}

/// GitHub's shade for a day, 0-4. Verified against a real calendar, 365 of 365
/// days matching.
pub fn level(count: u32, peak: u32) -> u8 {
    if count == 0 || peak == 0 {
        return 0;
    }
    // Widened, not saturated: a count comes from a calendar, and a calendar can
    // come from a file, so `count * 4` must not wrap — but saturating it would
    // be worse than wrong, because a day equal to the peak would then divide to
    // level 1 instead of 4.
    (u64::from(count) * 4).div_ceil(u64::from(peak)).min(4) as u8
}

/// The contributions a day needs to reach `level`, in a year whose busiest day
/// is `peak`.
///
/// GitHub's shade is `min(4, ceil(count * 4 / peak))`, so a day reaches `level`
/// exactly when `count * 4 > (level - 1) * peak`. That inverts to one line — no
/// searching, and it is the same arithmetic everywhere a price is quoted here.
///
/// ```
/// # use mossaic::art::commits_to_reach;
/// // A year whose busiest day is 112 sells its brightest shade for 85 a day.
/// assert_eq!(commits_to_reach(4, 112), 85);
/// assert_eq!(commits_to_reach(1, 112), 1);   // any contribution shows
/// ```
pub fn commits_to_reach(level: u8, peak: u32) -> u32 {
    let wanted = u64::from(level.saturating_sub(1)) * u64::from(peak) / 4 + 1;
    wanted.clamp(1, u64::from(u32::MAX)) as u32
}

/// The smallest uniform number of *extra* commits per lit day that puts the art
/// at `target`, given what those days already hold.
///
/// A day's shade comes from its total, and so does the year's peak — adding to
/// an already-busy day raises the bar for every other day, which is why this is
/// a fixed point rather than a formula. It converges in a handful of rounds:
/// each pass can only raise the peak by the spread between the busiest and
/// quietest lit day, and that spread does not grow.
pub fn commits_for_level(
    days: &[NaiveDate],
    existing: &BTreeMap<NaiveDate, u32>,
    target: u8,
) -> Option<u32> {
    if days.is_empty() {
        return None;
    }
    let held = |day: &NaiveDate| existing.get(day).copied().unwrap_or(0);
    let elsewhere = existing
        .iter()
        .filter(|(day, _)| !days.contains(day))
        .map(|(_, count)| *count)
        .max()
        .unwrap_or(0);
    let quietest = days.iter().map(held).min().unwrap_or(0);

    let mut added = 0;
    for _ in 0..64 {
        let peak = days
            .iter()
            // Saturating, like every other sum here: the counts come from a
            // calendar and a calendar can come from a file.
            .map(|day| held(day).saturating_add(added))
            .chain([elsewhere])
            .max()
            .unwrap_or(added)
            .max(1);
        // The quietest lit day is the binding one: every other ends up brighter.
        let wanted = commits_to_reach(target, peak).saturating_sub(quietest);
        if wanted <= added {
            return Some(added.max(1));
        }
        added = wanted;
    }
    None
}

/// A GraphQL-shaped response, so `mossaic --file` can render this for real.
pub fn snapshot(counts: &BTreeMap<NaiveDate, u32>, grid: &Grid, login: &str) -> String {
    const NAMES: [&str; 5] = [
        "NONE",
        "FIRST_QUARTILE",
        "SECOND_QUARTILE",
        "THIRD_QUARTILE",
        "FOURTH_QUARTILE",
    ];
    let peak = counts.values().copied().max().unwrap_or(1);
    let mut weeks = Vec::with_capacity(grid.weeks);
    for week in 0..grid.weeks {
        let days: Vec<serde_json::Value> = (0..WEEKDAYS)
            .map(|row| grid.date_at(week, row))
            .filter(|day| grid.holds(*day))
            .map(|day| {
                let count = counts.get(&day).copied().unwrap_or(0);
                serde_json::json!({
                    "date": day.to_string(),
                    "contributionCount": count,
                    "contributionLevel": NAMES[level(count, peak) as usize],
                })
            })
            .collect();
        if !days.is_empty() {
            weeks.push(serde_json::json!({ "contributionDays": days }));
        }
    }

    let total = counts
        .values()
        .fold(0u32, |sum, count| sum.saturating_add(*count));
    let payload = serde_json::json!({
        "data": { "user": {
            "login": login,
            "contributionsCollection": {
                "contributionYears": [grid.year],
                "contributionCalendar": { "totalContributions": total, "weeks": weeks },
            },
        }},
        "errors": serde_json::Value::Null,
    });
    serde_json::to_string_pretty(&payload).expect("a tree of numbers and strings")
}

/// The shade each day of the art would end at, which is what a reader sees.
///
/// Lit days take the ink shade, everything the background covers takes the
/// field shade, and any day neither claims stays at 0.
pub fn shading(placed: &Placed, shades: Shades) -> BTreeMap<NaiveDate, u8> {
    let mut out: BTreeMap<NaiveDate, u8> = placed
        .field
        .keys()
        .map(|date| (*date, shades.field))
        .collect();
    out.extend(placed.lit.keys().map(|date| (*date, shades.ink)));
    out
}

/// The whole year as the chart would draw it, so the text can be checked before
/// a single commit is made.
///
/// Takes shades rather than commit counts because that is what is being
/// checked: whether the letters stand out from the background they sit on.
pub fn preview(levels: &BTreeMap<NaiveDate, u8>, grid: &Grid, palette: Option<&Palette>) -> String {
    const NAMES: [&str; WEEKDAYS] = ["", "Mon", "", "Wed", "", "Fri", ""];
    // Without colour the five shades still have to be told apart, so the ramp
    // carries the level rather than just "on" or "off" — which is the whole
    // point once the background is a shade instead of nothing.
    const RAMP: [&str; 5] = ["  ", "░░", "▒▒", "▓▓", "██"];
    let paint = |level: u8| {
        let level = usize::from(level).min(4);
        match palette {
            Some(palette) => {
                let colour = palette.levels[level];
                format!("\x1b[38;2;{};{};{}m██\x1b[0m", colour.0, colour.1, colour.2)
            }
            None => RAMP[level].to_string(),
        }
    };

    let mut label = " ".repeat(4);
    let mut seen = Vec::new();
    for week in 0..grid.weeks {
        let Some(first) = (0..WEEKDAYS)
            .map(|row| grid.date_at(week, row))
            .find(|day| grid.holds(*day))
        else {
            continue;
        };
        if seen.contains(&first.month()) {
            continue;
        }
        seen.push(first.month());
        let column = 4 + week * 2;
        if column >= label.chars().count() {
            label.push_str(&" ".repeat(column - label.chars().count()));
            label.push_str(&first.format("%b").to_string());
        }
    }

    let mut out = vec![label];
    for (row, name) in NAMES.iter().enumerate() {
        let mut line = format!("{name:<4}");
        for week in 0..grid.weeks {
            let day = grid.date_at(week, row);
            if !grid.holds(day) {
                line.push_str("  ");
            } else {
                line.push_str(&paint(levels.get(&day).copied().unwrap_or(0)));
            }
        }
        out.push(line);
    }
    out.join("\n")
}

/// Whoever git is configured as. The email must be one GitHub knows, or the
/// commits will exist but never reach the contribution graph.
pub fn identity() -> (String, String) {
    let config = |key: &str| {
        Command::new("git")
            .args(["config", "--get", key])
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
            .filter(|value| !value.is_empty())
    };
    (
        config("user.name").unwrap_or_else(|| "art".to_string()),
        config("user.email").unwrap_or_else(|| "art@example.invalid".to_string()),
    )
}

/// Create the commits locally with `git fast-import`. Never pushes.
///
/// fast-import because the counts get large: shading is relative to the year's
/// peak, so standing out in an active year can need thousands of commits, and a
/// `git commit` process each would take minutes.
pub fn write_commits(
    lit: &BTreeMap<NaiveDate, u32>,
    repo: &Path,
    label: &str,
    name: &str,
    email: &str,
) -> Result<usize, String> {
    // An identity is written into the fast-import stream as a line of its own,
    // so a newline in it would be a command. git refuses the malformed result
    // today; refusing it here makes that a clear error rather than a crash
    // report, and does not depend on git continuing to notice.
    for (what, value) in [("name", name), ("email", email)] {
        if value.chars().any(|c| c.is_control()) || value.contains(['<', '>']) {
            return Err(format!(
                "the commit {what} may not contain control characters, '<' or '>': {value:?}"
            ));
        }
    }

    if !repo.join(".git").is_dir() {
        std::fs::create_dir_all(repo).map_err(|e| format!("could not create {repo:?}: {e}"))?;
        run(repo, &["init", "-q", "-b", "main"])?;
    }

    let mut stream = String::new();
    let mut index = 0usize;
    for (day, count) in lit {
        let stamp = day
            .and_hms_opt(12, 0, 0)
            .expect("noon exists")
            .and_utc()
            .timestamp();
        for _ in 0..*count {
            index += 1;
            let message = format!("{label} {day} #{index}\n");
            let body = format!("{index}\n");
            stream.push_str("commit refs/heads/main\n");
            stream.push_str(&format!("mark :{index}\n"));
            stream.push_str(&format!("author {name} <{email}> {stamp} +0000\n"));
            stream.push_str(&format!("committer {name} <{email}> {stamp} +0000\n"));
            stream.push_str(&format!("data {}\n{message}", message.len()));
            if index > 1 {
                stream.push_str(&format!("from :{}\n", index - 1));
            }
            stream.push_str(&format!(
                "M 100644 inline count.txt\ndata {}\n{body}\n",
                body.len()
            ));
        }
    }

    let mut child = Command::new("git")
        .args(["fast-import", "--quiet"])
        .current_dir(repo)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run git fast-import: {e}"))?;
    child
        .stdin
        .take()
        .expect("piped")
        .write_all(stream.as_bytes())
        .map_err(|e| format!("could not feed git fast-import: {e}"))?;
    let status = child
        .wait()
        .map_err(|e| format!("git fast-import failed: {e}"))?;
    if !status.success() {
        return Err(format!("git fast-import exited with {status}"));
    }
    run(repo, &["reset", "--hard", "main"])?;
    Ok(index)
}

fn run(repo: &Path, args: &[&str]) -> Result<(), String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|e| format!("could not run git {}: {e}", args[0]))?;
    if out.status.success() {
        return Ok(());
    }
    Err(format!(
        "git {} failed: {}",
        args[0],
        String::from_utf8_lossy(&out.stderr).trim()
    ))
}
