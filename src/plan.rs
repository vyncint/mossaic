//! Tracking a contribution-art plan against what actually happened.
//!
//! [`crate::art`] answers "what would this text cost". This answers the
//! question you have every day after that: *am I getting there, and what do I
//! owe today?*
//!
//! Two things can go wrong, and only one of them is fixable:
//!
//! - **A letter day that is not bright enough.** Contribute more, today or by
//!   back-dating. Fixable at any time.
//! - **A day that must stay dark and already has contributions.** Nothing takes
//!   those away short of rewriting history, so they are holes in the letters
//!   for good. This is why a busy year cannot be drawn on cleanly, and it is the
//!   answer to "why can't I write VYNCINT in 2026".
//!
//! Both are measured against the year's *peak*, because GitHub's shade is
//! relative to it: one big day anywhere raises the price of every letter day.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, NaiveDate};

use crate::art::{self, Grid, GLYPH_ROWS};
use crate::thousands;

/// What the art wants of a day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Want {
    /// Part of a letter: it has to be bright.
    Lit,
    /// Inside the text's own block but not part of a letter — a day whose
    /// contributions punch a hole in a letter.
    Hole,
    /// Outside the text altogether. Contributions here are noise around the
    /// letters rather than damage to them — unless the plan draws a background,
    /// in which case out here is part of the picture too.
    Around,
}

/// One day, what it should be, and what it is.
///
/// A day is a **band**, not a target. It has a floor — the contributions that
/// get it to the shade the art wants — and, when the art cares what shade it
/// ends at, a ceiling above which it turns into the wrong colour. Letters have
/// no ceiling: brighter than brightest is still brightest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Day {
    /// The date.
    pub date: NaiveDate,
    /// What the art wants of it.
    pub want: Want,
    /// Contributions it has now.
    pub have: u32,
    /// Contributions it must reach. Zero for a day with nothing to reach.
    pub need: u32,
    /// The most it may hold and still be the shade the art wants. `None` for a
    /// day that cannot be too bright.
    ///
    /// Zero means the day must stay dark, which is what a hole in the letters
    /// is when there is no background to fill.
    pub ceiling: Option<u32>,
}

impl Day {
    /// Whether the day is where the art needs it — inside its band, both ends.
    pub fn done(&self) -> bool {
        self.have >= self.need && self.ceiling.is_none_or(|most| self.have <= most)
    }

    /// Contributions still owed to reach the shade the art wants. Zero once the
    /// day is there, and for a day with nothing to reach.
    pub fn short(&self) -> u32 {
        self.need.saturating_sub(self.have)
    }

    /// Contributions past the point where the day stops being the shade the art
    /// wants. Nothing takes these away, so this is damage rather than debt.
    pub fn over(&self) -> u32 {
        match self.ceiling {
            Some(most) => self.have.saturating_sub(most),
            None => 0,
        }
    }
}

/// Whether the text can still be drawn, and what stands in the way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Every letter day is bright and nothing else is.
    Done,
    /// Nothing is in the way; there is work left.
    Reachable,
    /// Days inside the letters already have contributions. The text can still
    /// be brightened, but it will read with holes in it.
    Holed {
        /// How many days inside the letters are lit that should not be.
        holes: usize,
    },
}

/// A plan, and how far along it is.
#[derive(Debug, Clone)]
pub struct Plan {
    /// The text being drawn, uppercased.
    pub text: String,
    /// The year it is being drawn in.
    pub year: i32,
    /// The busiest day in the year, which sets what every letter day costs.
    pub peak: u32,
    /// Which day that is, when the year has one.
    pub peak_day: Option<NaiveDate>,
    /// Contributions a letter day must end at.
    pub need: u32,
    /// The shades the art is drawn in — what the letters are, and what the
    /// background they sit on is.
    pub shades: art::Shades,
    /// Contributions a background day must reach. Zero when the plan draws no
    /// background and the days around the letters are simply left alone.
    pub field_need: u32,
    /// The most a background day may hold before it stops being background.
    pub field_ceiling: Option<u32>,
    /// Every day the plan has an opinion about, in date order.
    pub days: Vec<Day>,
    /// Where the text starts, in calendar columns.
    pub start_week: usize,
    /// How wide it is, in calendar columns.
    pub columns: usize,
}

impl Plan {
    /// Compare a placed text against a year's real contributions.
    pub fn build(
        text: &str,
        grid: &Grid,
        placed: &art::Placed,
        columns: usize,
        top: usize,
        actual: &BTreeMap<NaiveDate, u32>,
        shades: art::Shades,
    ) -> Self {
        let peak = actual.values().copied().max().unwrap_or(0);
        let peak_day = actual
            .iter()
            .filter(|(_, count)| **count == peak && peak > 0)
            .map(|(date, _)| *date)
            .next();
        // What a letter day must reach. The peak it is measured against is the
        // one the finished year will have, which is today's peak unless the
        // letters themselves would exceed it.
        // Never below what the shades need to be separable: a year whose
        // busiest day is 1 cannot hold a background *and* letters.
        let measured = peak.max(shades.min_peak());
        let need = art::commits_to_reach(shades.ink, measured);
        let field_need = shades.commits(measured).field;
        let field_ceiling = shades.ceiling(measured);

        // The text's own block: the columns it spans, on the rows it uses.
        let inside: BTreeSet<NaiveDate> = (placed.start_week..placed.start_week + columns)
            .flat_map(|week| (top..top + GLYPH_ROWS).map(move |row| (week, row)))
            .map(|(week, row)| grid.date_at(week, row))
            .filter(|date| grid.holds(*date))
            .collect();

        let mut days = Vec::new();
        let mut date = grid.first;
        while date <= grid.last {
            let have = actual.get(&date).copied().unwrap_or(0);
            let want = if placed.lit.contains_key(&date) {
                Want::Lit
            } else if inside.contains(&date) {
                Want::Hole
            } else {
                Want::Around
            };
            // What the art asks of this day, as a band.
            let (day_need, ceiling) = match want {
                // Brighter than brightest is still brightest.
                Want::Lit => (need, None),
                // Inside the letters: always answerable to a ceiling, because a
                // day too bright in here is a hole whether or not the plan
                // draws a background.
                Want::Hole => (field_need, field_ceiling),
                // Outside: part of the picture only once a background is drawn.
                // Without one, contributions out here are nobody's business.
                Want::Around if shades.field > 0 => (field_need, field_ceiling),
                Want::Around => (0, None),
            };
            // Days with nothing to say are left out — but which days those are
            // depends on where they sit. Outside the text, a dark day that is
            // dark is not news, and a year with no background has three hundred
            // of them. *Inside* the text it is the news: the plan wants that day
            // to stay dark, nothing takes a contribution back, and this is the
            // only warning there is before the loss is permanent. Dropping them
            // is what made the tracker call tomorrow free when committing on it
            // would have punched a hole in a letter.
            if want != Want::Around || have > 0 || day_need > 0 {
                days.push(Day {
                    date,
                    want,
                    have,
                    need: day_need,
                    ceiling,
                });
            }
            date = date.succ_opt().unwrap_or(date);
            if date == grid.last.succ_opt().unwrap_or(grid.last) {
                break;
            }
        }

        Self {
            text: text.to_uppercase(),
            year: grid.year,
            peak,
            peak_day,
            need,
            shades,
            field_need,
            field_ceiling,
            days,
            start_week: placed.start_week,
            columns,
        }
    }

    /// Compare a placed **canvas** against a year's real contributions.
    ///
    /// The general form of [`Plan::build`]. Text gives every day one of two
    /// shades — a letter day or the background — so it can be described by a
    /// [`Shades`](art::Shades) pair and two prices. A canvas gives every day
    /// its own level, so the price is per day and the pair is only a summary of
    /// the range the picture spans.
    ///
    /// Everything downstream is unchanged, because [`Day`] was already a band
    /// rather than a target: a level-2 day is simply a band with both ends,
    /// exactly as a background day always was.
    ///
    /// `levels` covers **every** day the picture claims, including the dark
    /// ones. That is what makes a canvas different from text: a dark day inside
    /// a picture is part of the picture, and contributing on it is damage.
    pub fn from_levels(
        name: &str,
        grid: &Grid,
        levels: &BTreeMap<NaiveDate, u8>,
        start_week: usize,
        columns: usize,
        actual: &BTreeMap<NaiveDate, u32>,
    ) -> Self {
        let peak = actual.values().copied().max().unwrap_or(0);
        let peak_day = actual
            .iter()
            .filter(|(_, count)| **count == peak && peak > 0)
            .map(|(date, _)| *date)
            .next();

        // The same rule [`art::Shades::min_peak`] applies to two shades, for
        // however many the picture uses: a year whose busiest day is 1 holds
        // only empty and full, so a picture with a level in between needs a
        // peak of at least 4 before its shades are distinguishable at all.
        let intermediate = levels.values().any(|level| (1..4).contains(level));
        let measured = peak.max(if intermediate { 4 } else { 1 });

        // The summary pair: the range the picture spans. `worst()` on it is
        // then an honest answer to "will a reader tell this apart" — the
        // darkest and brightest shades in the drawing are the two furthest
        // apart, so if *they* are faint, everything is.
        let used_low = levels.values().copied().min().unwrap_or(0);
        let used_high = levels.values().copied().max().unwrap_or(0);
        let shades = art::Shades {
            ink: used_high,
            field: used_low,
        };

        let mut days = Vec::new();
        let mut date = grid.first;
        loop {
            let have = actual.get(&date).copied().unwrap_or(0);
            let (want, day_need, ceiling) = match levels.get(&date) {
                // Part of the picture, and meant to be seen.
                Some(level) if *level > 0 => {
                    let (need, ceiling) = art::band(*level, measured);
                    (Want::Lit, need, ceiling)
                }
                // Part of the picture, and meant to stay dark. A contribution
                // here is a hole, exactly as one inside a letter is.
                Some(_) => (Want::Hole, 0, Some(0)),
                // Not covered: the picture has no opinion. Only ever the days
                // in the partial weeks at the ends of the year, for a canvas
                // narrower than the calendar.
                None => (Want::Around, 0, None),
            };
            if want != Want::Around || have > 0 {
                days.push(Day {
                    date,
                    want,
                    have,
                    need: day_need,
                    ceiling,
                });
            }
            let Some(next) = date.succ_opt() else { break };
            if next > grid.last {
                break;
            }
            date = next;
        }

        Self {
            // Not uppercased: this is a template or a file name, not text being
            // drawn, and `DRAGON` is not what anyone called it.
            text: name.to_string(),
            year: grid.year,
            peak,
            peak_day,
            // The most expensive day in the picture, which is what "a letter
            // day costs" means when the days cost different amounts.
            need: art::band(used_high, measured).0,
            shades,
            // A canvas has no separate background: a dark day is dark, and a
            // shaded one is part of the drawing with a price of its own. Saying
            // the darkest day must stay at zero is the honest summary, and it
            // is what `hideable` needs to be for the tracker to call a
            // contribution on a dark day damage.
            field_need: 0,
            field_ceiling: Some(0),
            days,
            start_week,
            columns,
        }
    }

    /// Days that are part of a letter.
    pub fn letters(&self) -> impl Iterator<Item = &Day> {
        self.days.iter().filter(|day| day.want == Want::Lit)
    }

    /// Letter days already bright enough.
    pub fn bright(&self) -> usize {
        self.letters().filter(|day| day.done()).count()
    }

    /// Letter days still short, and what they owe.
    pub fn owing(&self) -> (usize, u32) {
        tally(self.letters())
    }

    /// Days the background covers, when the plan draws one.
    pub fn field(&self) -> impl Iterator<Item = &Day> {
        self.days
            .iter()
            .filter(|day| day.want != Want::Lit && day.need > 0)
    }

    /// Background days already at the shade the art wants.
    pub fn field_bright(&self) -> usize {
        self.field().filter(|day| day.done()).count()
    }

    /// Background days still short, and what they owe.
    ///
    /// Separate from [`Plan::owing`] because the two are different kinds of
    /// work: a letter day is the picture, a background day is the paper it is
    /// printed on. Missing one letter day is worse than missing ten of these.
    pub fn field_owing(&self) -> (usize, u32) {
        tally(self.field())
    }

    /// Days inside the letters that are too bright for what the art wants.
    /// These cannot be undone, which is what makes a year unusable rather than
    /// expensive.
    ///
    /// With no background that means any contribution at all. With one it means
    /// a day that has crept past the background's shade and up towards the
    /// letters' own — the same damage, arrived at differently.
    /// A day drawn at a *middle* shade counts too, and only a canvas has
    /// those. Text gives a letter day no ceiling — brighter than brightest is
    /// still brightest — so [`Day::over`] is always zero for one and this
    /// reads exactly as it did when it only looked at holes.
    pub fn holes(&self) -> Vec<&Day> {
        self.days
            .iter()
            .filter(|day| day.want != Want::Around && day.over() > 0)
            .collect()
    }

    /// Days outside the text holding contributions the art did not ask for.
    ///
    /// Noise around the letters, not damage to them: without a background this
    /// is any active day out there, and with one it is a day bright enough to
    /// be mistaken for a letter.
    pub fn around(&self) -> usize {
        self.days
            .iter()
            .filter(|day| day.want == Want::Around)
            .filter(|day| match day.ceiling {
                Some(most) => day.have > most,
                None => day.have > 0,
            })
            .count()
    }

    /// Whether the text can still be drawn cleanly.
    pub fn verdict(&self) -> Verdict {
        let holes = self.holes().len();
        if holes > 0 {
            return Verdict::Holed { holes };
        }
        // The background is part of the picture, so the plan is not finished
        // until it is laid down too.
        if self.owing().0 == 0 && self.field_owing().0 == 0 {
            return Verdict::Done;
        }
        Verdict::Reachable
    }

    /// What a given day needs. `None` for a day the plan has no opinion about.
    pub fn on(&self, date: NaiveDate) -> Option<&Day> {
        self.days.iter().find(|day| day.date == date)
    }

    /// The next `count` days from `from`, whatever the plan wants of them —
    /// including the dark ones, because "contribute nothing today" is advice.
    ///
    /// Clamped to the year: asked about a year that has not started, it answers
    /// from its first day rather than about days that are not in it.
    pub fn schedule(&self, from: NaiveDate, count: usize) -> Vec<Day> {
        let mut out = Vec::with_capacity(count);
        let mut date = from.max(self.first_day());
        for _ in 0..count {
            if date > self.last_day() {
                break;
            }
            // A day the plan has no opinion about: outside the text, with no
            // background asking anything of it.
            out.push(self.on(date).copied().unwrap_or(Day {
                date,
                want: Want::Around,
                have: 0,
                need: 0,
                ceiling: None,
            }));
            date = match date.succ_opt() {
                Some(next) => next,
                None => break,
            };
        }
        out
    }

    /// Letter days still owing that are in the past — only back-dated commits
    /// can reach them.
    pub fn overdue(&self, today: NaiveDate) -> (usize, u32) {
        tally(self.letters().filter(|day| day.date < today))
    }

    /// Letter days still owing that have not happened yet.
    pub fn ahead(&self, today: NaiveDate) -> (usize, u32) {
        tally(self.letters().filter(|day| day.date >= today))
    }

    fn first_day(&self) -> NaiveDate {
        NaiveDate::from_ymd_opt(self.year, 1, 1).unwrap_or(self.days[0].date)
    }

    fn last_day(&self) -> NaiveDate {
        NaiveDate::from_ymd_opt(self.year, 12, 31).unwrap_or(self.days[0].date)
    }

    /// Whether the year has begun, as of `today`. A plan for a year still ahead
    /// has no overdue days and no advice for this afternoon.
    pub fn under_way(&self, today: NaiveDate) -> bool {
        today >= self.first_day()
    }

    /// Whether `date` falls in the year this plan is drawn in.
    ///
    /// Stronger than [`Plan::under_way`], and the right question to ask before
    /// reporting on a particular day: a plan for 2024 has nothing to say about
    /// a day in 2026, though the year has certainly begun.
    pub fn holds(&self, date: NaiveDate) -> bool {
        self.first_day() <= date && date <= self.last_day()
    }
}

/// How many days still owe something, and how much between them.
///
/// Saturating rather than wrapping: the counts come from a calendar, and a
/// calendar can come from a file.
fn tally<'a>(days: impl Iterator<Item = &'a Day>) -> (usize, u32) {
    days.map(Day::short)
        .filter(|short| *short > 0)
        .fold((0, 0u32), |(count, sum), owed| {
            (count + 1, sum.saturating_add(owed))
        })
}

/// The placement that puts the fewest existing contributions inside the text.
///
/// Shifting the text sideways is free before a single commit is made, and it is
/// the difference between letters with holes in them and letters without: the
/// README's sweep, done by the program rather than by hand.
///
/// `ceiling` is the most a day inside the block may already hold without
/// spoiling the letters — zero when the background is empty and any
/// contribution in there is a hole, higher when the plan draws a background
/// those days can hide in.
pub fn best_start_week(
    grid: &Grid,
    columns: usize,
    top: usize,
    lit_shape: &[[bool; GLYPH_ROWS]],
    actual: &BTreeMap<NaiveDate, u32>,
    ceiling: u32,
) -> Option<(usize, usize)> {
    if columns > grid.weeks {
        return None;
    }
    (0..=grid.weeks - columns)
        .map(|start| {
            let holes = lit_shape
                .iter()
                .enumerate()
                .flat_map(|(offset, column)| {
                    column
                        .iter()
                        .enumerate()
                        .filter(|(_, lit)| !**lit)
                        .map(move |(row, _)| (start + offset, top + row))
                })
                .map(|(week, row)| grid.date_at(week, row))
                .filter(|date| grid.holds(*date))
                .filter(|date| actual.get(date).is_some_and(|count| *count > ceiling))
                .count();
            (start, holes)
        })
        .min_by_key(|(start, holes)| (*holes, *start))
}

/// Contributions a year holds, keyed by date — the shape every function here
/// wants, from the calendar the API returns.
pub fn contributions(calendar: &crate::calendar::Calendar) -> BTreeMap<NaiveDate, u32> {
    calendar
        .days()
        .filter(|day| day.count > 0)
        .map(|day| (day.date, day.count))
        .collect()
}

/// A plan, written down.
///
/// The placement is part of the plan and it used to live only in the command
/// line, so tracking with a different `--start-week` compared against a
/// different plan and said so confidently. Saving it once removes the whole
/// class of mistake: everything here is *resolved*, so a centred text keeps the
/// column it was centred on even if the text later changes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Spec {
    /// What is being drawn.
    pub text: String,
    /// Which year's calendar.
    pub year: i32,
    /// The left edge, in columns — resolved, never "centre it".
    pub start_week: usize,
    /// First calendar row used, 0 = Sunday.
    pub top: usize,
    /// Commits per lit day, for `--write`.
    pub commits: u32,
    /// The level the background sits at, 0 for none. Defaulted rather than
    /// required, so a plan written before backgrounds existed still loads.
    #[serde(default)]
    pub background: u8,
    /// Whose contributions to track. `None` asks gh who you are.
    pub user: Option<String>,
    /// The picture, in the `.art` format, for a plan drawn from a canvas
    /// rather than from text.
    ///
    /// Stored **inline** rather than as a template name or a path, for the
    /// reason everything else here is resolved: a plan is a record of what was
    /// decided, and a name is a pointer to something that can change
    /// underneath it. A template edited next month must not silently
    /// re-target a plan you have been drawing since January.
    ///
    /// `None` for a text plan, and defaulted rather than required so every
    /// plan written before canvases existed still loads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub art: Option<String>,
}

/// Where a plan lives when nobody says otherwise.
pub const DEFAULT_SPEC: &str = "mossaic-plan.json";

impl Spec {
    /// Whether this is a plan the tools can actually draw.
    ///
    /// A plan is **input**: `--plan PATH` names a file, and a file may have come
    /// from somewhere other than your own `--save`. Without this it was the way
    /// around every bound the command line enforces — a `top` of `usize::MAX`
    /// wrapped past [`place`](crate::art::place)'s guard and drew the letters on
    /// scrambled rows, a `commits` of `u32::MAX` quoted four billion commits a
    /// day, and a `year` of -262143 panicked building the calendar. The ranges
    /// are the same ones the flags use, so a saved plan can never mean something
    /// a typed command could not.
    pub fn validate(&self) -> Result<(), String> {
        // `i128`, so that every field's whole range fits and the message names
        // the value that was actually rejected. Narrowing to `i64` first made a
        // `top` of `usize::MAX` report itself as -1, which is a confusing thing
        // to read in a refusal.
        let bounded = |what: &str, value: i128, low: i128, high: i128| {
            if (low..=high).contains(&value) {
                Ok(())
            } else {
                Err(format!(
                    "{what} is {value}, which is not between {low} and {high}"
                ))
            }
        };
        bounded(
            "year",
            i128::from(self.year),
            i128::from(*crate::cli::YEARS.start()),
            i128::from(*crate::cli::YEARS.end()),
        )?;
        // Five rows of glyph have to fit inside a calendar week.
        bounded(
            "top",
            self.top as i128,
            0,
            (crate::art::WEEKDAYS - GLYPH_ROWS) as i128,
        )?;
        bounded("commits", i128::from(self.commits), 1, 1_000_000)?;
        // The tight bound needs the year and the text; `place` applies it.
        bounded("start_week", self.start_week as i128, 0, 60)?;
        bounded("background", i128::from(self.background), 0, 4)?;
        // A canvas is input like everything else here: the file may have come
        // from somewhere other than your own `--save`, so it is parsed and
        // bounded now rather than trusted and drawn later.
        if let Some(art) = &self.art {
            let canvas = crate::art::Canvas::parse(art)
                .map_err(|why| format!("the stored picture is not a canvas: {why}"))?;
            bounded(
                "the stored picture's width",
                canvas.width() as i128,
                1,
                crate::art::CANVAS_COLS as i128,
            )?;
        }
        Ok(())
    }

    /// The picture this plan draws, if it draws one.
    pub fn canvas(&self) -> Result<Option<crate::art::Canvas>, String> {
        self.art
            .as_deref()
            .map(crate::art::Canvas::parse)
            .transpose()
    }

    /// Read a plan, or say why it could not be read.
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let body = std::fs::read_to_string(path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        let spec: Self = serde_json::from_str(&body)
            .map_err(|error| format!("{} is not a mossaic plan: {error}", path.display()))?;
        // Named as a problem with the file, and with the way out of it: 0.2.0
        // let `--commits -1` through, which `--save` then wrote down as four
        // billion, so a plan in the wild can be one this refuses. Saving it
        // again is the fix, and saying so beats leaving someone to work it out.
        spec.validate().map_err(|why| {
            format!(
                "{} is not a plan these tools can use: {why}.\n  \
                 Saving it again replaces it:  mossaic-art TEXT --year YEAR --save",
                path.display()
            )
        })?;
        Ok(spec)
    }

    /// Write a plan where `load` will find it.
    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let body = serde_json::to_string_pretty(self)
            .map_err(|error| format!("could not encode the plan: {error}"))?;
        std::fs::write(path, body + "\n")
            .map_err(|error| format!("could not write {}: {error}", path.display()))
    }
}

/// One day, as a report states it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Standing {
    /// The date, ISO 8601.
    pub date: String,
    /// What the plan makes of the day, as one word:
    ///
    /// | `kind` | what it means |
    /// | --- | --- |
    /// | `letter` | part of a letter; it has to be bright |
    /// | `background` | the plan wants a shade here, in the text block or out |
    /// | `keep-dark` | inside the letters with nothing asked of it — a contribution here becomes a hole |
    /// | `hole` | inside the letters and too bright already; nothing takes that back |
    /// | `outside` | outside the text, and no background asks anything of it |
    ///
    /// `hole` names damage rather than position, so it appears only once a day
    /// has passed its ceiling — a clean day inside the letters is `keep-dark`,
    /// which is an instruction rather than a loss.
    pub kind: &'static str,
    /// What the day must reach; zero for a day with nothing to reach.
    pub need: u32,
    /// What it holds now.
    pub have: u32,
    /// What is still owed on it.
    pub short: u32,
    /// The most it may hold and still be the shade the art wants, when it has
    /// a ceiling at all.
    pub ceiling: Option<u32>,
    /// What it holds past that ceiling — contributions that have already put it
    /// at the wrong shade.
    pub over: u32,
}

impl Standing {
    fn of(day: &Day) -> Self {
        Self {
            date: day.date.to_string(),
            kind: match day.want {
                Want::Lit => "letter",
                // Inside the letters. Too bright is the one loss nothing
                // undoes, so it is named for the damage; short of that the day
                // is either part of a background the plan draws, or a day whose
                // whole job is to stay dark.
                Want::Hole if day.over() > 0 => "hole",
                Want::Hole if day.need > 0 => "background",
                Want::Hole => "keep-dark",
                // Outside the text, but the plan still wants a shade there.
                Want::Around if day.need > 0 => "background",
                Want::Around => "outside",
            },
            need: day.need,
            have: day.have,
            short: day.short(),
            ceiling: day.ceiling,
            over: day.over(),
        }
    }
}

/// The whole standing of a plan, in a shape a machine can read.
///
/// This is what the GitHub Action emits and what a notification is built from:
/// every number the text report prints, and nothing that needs a terminal.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Report {
    /// The text being drawn.
    pub text: String,
    /// The year it is drawn in.
    pub year: i32,
    /// Whose contributions were compared: a GitHub login, or the file read.
    pub source: String,
    /// Where the text sits, in calendar columns and rows.
    pub start_week: usize,
    /// How many columns it spans.
    pub columns: usize,
    /// Contributions the year holds.
    pub year_total: u32,
    /// The busiest day, which sets the price.
    pub peak: u32,
    /// Which day that is, ISO 8601.
    pub peak_day: Option<String>,
    /// What a letter day has to reach.
    pub need_per_day: u32,
    /// The level the letters are drawn at, 1-4.
    pub ink_level: u8,
    /// The level the background is drawn at, or 0 when the plan draws none and
    /// the letters stand against an empty graph.
    pub field_level: u8,
    /// How far apart those two shades look, as CIE76 ΔE, in the worst palette a
    /// reader might have the graph open in.
    pub separation: f32,
    /// `faint`, `readable` or `clear` — what that separation amounts to.
    pub legibility: &'static str,
    /// What a background day has to reach. Zero when there is no background.
    pub field_need_per_day: u32,
    /// The most a background day may hold before it is the wrong shade.
    pub field_ceiling: Option<u32>,
    /// Days the background covers.
    pub field_days: usize,
    /// Background days already at the right shade.
    pub field_bright: usize,
    /// Background days still short.
    pub field_owing_days: usize,
    /// Contributions those days owe between them.
    pub field_owing_commits: u32,
    /// Letter days in the text.
    pub letters: usize,
    /// Letter days already bright enough.
    pub bright: usize,
    /// Letter days still short.
    pub owing_days: usize,
    /// Contributions those days owe between them.
    pub owing_commits: u32,
    /// Days inside the letters that are lit and cannot be unlit.
    pub holes: usize,
    /// Days outside the text with contributions.
    pub around: usize,
    /// `drawn`, `reachable` or `holed`.
    pub verdict: &'static str,
    /// One line fit for a notification title. Carried in the report so that
    /// nothing downstream has to reconstruct it.
    pub headline: String,
    /// A placement with fewer holes, when one exists.
    pub suggested_start_week: Option<usize>,
    /// How many holes that placement would leave.
    pub suggested_holes: Option<usize>,
    /// Today, when the year is under way.
    pub today: Option<Standing>,
    /// Tomorrow, when it is still in the year.
    pub tomorrow: Option<Standing>,
    /// Letter days still ahead, and what they owe.
    pub ahead_days: usize,
    /// Contributions those days owe.
    pub ahead_commits: u32,
    /// Letter days already past and still short — back-dating only.
    pub overdue_days: usize,
    /// Contributions those days owe.
    pub overdue_commits: u32,
}

impl Report {
    /// Everything the text report prints, as data.
    pub fn of(
        plan: &Plan,
        user: &str,
        year_total: u32,
        today: NaiveDate,
        suggestion: Option<(usize, usize)>,
    ) -> Self {
        let (owing_days, owing_commits) = plan.owing();
        let (field_owing_days, field_owing_commits) = plan.field_owing();
        let (legibility, separation) = plan.shades.worst();
        let (overdue_days, overdue_commits) = plan.overdue(today);
        let (ahead_days, ahead_commits) = plan.ahead(today);
        // Only about days the plan's own year holds. Tracking 2024 while it is
        // 2026 used to report on a day in 2026 and call it `outside`, which is
        // true of the wrong calendar; `--today` makes that easy to ask for by
        // accident, so the answer is now "nothing" rather than something
        // confident.
        let standing = |date: NaiveDate| {
            if !plan.holds(date) {
                return None;
            }
            // A day the plan has no entry for is still an answer: it is not
            // part of the text.
            Some(plan.on(date).map_or_else(
                || Standing {
                    date: date.to_string(),
                    kind: "outside",
                    need: 0,
                    have: 0,
                    short: 0,
                    ceiling: None,
                    over: 0,
                },
                Standing::of,
            ))
        };

        let mut report = Self {
            text: plan.text.clone(),
            year: plan.year,
            source: user.to_string(),
            start_week: plan.start_week,
            columns: plan.columns,
            year_total,
            peak: plan.peak,
            peak_day: plan.peak_day.map(|date| date.to_string()),
            need_per_day: plan.need,
            ink_level: plan.shades.ink,
            field_level: plan.shades.field,
            separation,
            legibility: legibility.as_str(),
            field_need_per_day: plan.field_need,
            field_ceiling: plan.field_ceiling,
            field_days: plan.field().count(),
            field_bright: plan.field_bright(),
            field_owing_days,
            field_owing_commits,
            letters: plan.letters().count(),
            bright: plan.bright(),
            owing_days,
            owing_commits,
            holes: plan.holes().len(),
            around: plan.around(),
            verdict: match plan.verdict() {
                Verdict::Done => "drawn",
                Verdict::Reachable => "reachable",
                Verdict::Holed { .. } => "holed",
            },
            headline: String::new(), // filled in below, once the fields it reads exist
            suggested_start_week: suggestion.map(|(week, _)| week),
            suggested_holes: suggestion.map(|(_, holes)| holes),
            today: standing(today),
            tomorrow: today.succ_opt().and_then(standing),
            ahead_days,
            ahead_commits,
            overdue_days,
            overdue_commits,
        };
        report.headline = report.summarise();
        report
    }

    /// One line, for a notification that has room for nothing else.
    fn summarise(&self) -> String {
        match self.verdict {
            "drawn" => format!("{} · {} — drawn", self.text, self.year),
            "holed" => format!(
                "{} · {} — {} of {} bright, {} hole(s) that cannot be unlit",
                self.text, self.year, self.bright, self.letters, self.holes
            ),
            // The background is work too, and saying "0 to go" while three
            // hundred field days are bare is the kind of confidently wrong a
            // notification never recovers from.
            _ => format!(
                "{} · {} — {} of {} bright, {} contributions to go{}",
                self.text,
                self.year,
                self.bright,
                self.letters,
                thousands(self.owing_commits),
                match self.field_owing_commits {
                    0 => String::new(),
                    owed => format!(" (+{} for the background)", thousands(owed)),
                }
            ),
        }
    }

    /// A summary that reads the same in a GitHub step summary, a Slack message,
    /// a Discord embed and an email — the four places this ends up.
    pub fn markdown(&self) -> String {
        let mut out = format!("### {} · {}\n\n", self.text, self.year);
        out.push_str(&match self.verdict {
            "drawn" => format!("**{} is drawn.**\n\n", self.text),
            "holed" => format!(
                "**Cannot be drawn cleanly** — {} day(s) inside the letters are already \
                 lit, and nothing takes those away.\n\n",
                self.holes
            ),
            _ => format!(
                "**On track** — {} contribution(s) to go{}.\n\n",
                thousands(self.owing_commits),
                match self.field_owing_commits {
                    0 => String::new(),
                    owed => format!(", and {} for the background", thousands(owed)),
                }
            ),
        });

        out.push_str("| | |\n| --- | --- |\n");
        out.push_str(&format!(
            "| letters bright | {} of {} |\n",
            self.bright, self.letters
        ));
        if self.owing_days > 0 {
            out.push_str(&format!(
                "| still owing | {} day(s) · {} contributions |\n",
                self.owing_days,
                thousands(self.owing_commits)
            ));
        }
        if self.field_level > 0 {
            out.push_str(&format!(
                "| background | {} of {} at level {} (ΔE {:.0}, {}) |\n",
                self.field_bright,
                self.field_days,
                self.field_level,
                self.separation,
                self.legibility
            ));
            if self.field_owing_days > 0 {
                out.push_str(&format!(
                    "| background owing | {} day(s) · {} contributions |\n",
                    self.field_owing_days,
                    thousands(self.field_owing_commits)
                ));
            }
        }
        out.push_str(&format!(
            "| a letter day costs | {}{} |\n",
            thousands(self.need_per_day),
            match (&self.peak_day, self.peak) {
                (Some(date), peak) if peak > 0 => format!(" (peak {peak} on {date})"),
                _ => String::new(),
            }
        ));
        for (label, day) in [("today", &self.today), ("tomorrow", &self.tomorrow)] {
            if let Some(day) = day {
                out.push_str(&format!(
                    "| {label} | {} |\n",
                    match (day.kind, day.short) {
                        ("letter", 0) => "letter day — already bright enough".to_string(),
                        ("letter", short) => format!(
                            "letter day — {} of {} there, {} to go",
                            thousands(day.have),
                            thousands(day.need),
                            thousands(short)
                        ),
                        // A hole is not work; it is damage, and saying "not part
                        // of the text" about one would be actively misleading.
                        ("hole", _) => {
                            "inside the letters and already lit — a permanent hole".to_string()
                        }
                        // Nothing is owed on it, and that is the whole
                        // instruction: leaving it alone is the work.
                        ("keep-dark", _) => "inside the letters — keep it dark".to_string(),
                        ("background", _) if day.over > 0 => format!(
                            "background — {} contributions, {} too many for level {}",
                            thousands(day.have),
                            thousands(day.over),
                            self.field_level
                        ),
                        ("background", 0) => "background — already the right shade".to_string(),
                        ("background", short) => format!(
                            "background — {} of {} there, {} to go",
                            thousands(day.have),
                            thousands(day.need),
                            thousands(short)
                        ),
                        _ => "not part of the text".to_string(),
                    }
                ));
            }
        }
        if self.overdue_days > 0 {
            out.push_str(&format!(
                "| already past | {} day(s) · {} contributions, back-dating only |\n",
                self.overdue_days,
                thousands(self.overdue_commits)
            ));
        }

        if let (Some(week), Some(holes)) = (self.suggested_start_week, self.suggested_holes) {
            if holes < self.holes {
                out.push_str(&format!(
                    "\n`--start-week {week}` would leave {holes} hole(s) instead of {}.\n",
                    self.holes
                ));
            }
        }
        out
    }
}

/// The year as it stands, with every day coloured by what the plan makes of it:
/// a letter day that is bright, one that is still short, and a day inside the
/// letters that is lit and cannot be unlit.
///
/// The point of drawing it rather than counting it is that holes are a *shape*
/// problem — thirty-eight of them scattered through the text reads very
/// differently from thirty-eight in one corner.
pub fn preview(plan: &Plan, grid: &Grid, palette: Option<&crate::primer::Palette>) -> String {
    const NAMES: [&str; 7] = ["", "Mon", "", "Wed", "", "Fri", ""];

    /// What a day is, for drawing. Ordered by how much it matters: damage
    /// first, because a hole is the one thing that cannot be worked off.
    enum Mark {
        Hole,
        Smudge,
        LetterDone,
        LetterShort,
        FieldDone,
        FieldShort,
        Stray,
        Idle,
    }

    let mark = |day: Option<&Day>| match day {
        // Too bright for its place, and nothing takes contributions away.
        Some(day) if day.want == Want::Hole && day.over() > 0 => Mark::Hole,
        Some(day) if day.over() > 0 => Mark::Smudge,
        Some(day) if day.want == Want::Lit && day.done() => Mark::LetterDone,
        Some(day) if day.want == Want::Lit => Mark::LetterShort,
        // Part of the background the plan draws.
        Some(day) if day.need > 0 && day.done() => Mark::FieldDone,
        Some(day) if day.need > 0 => Mark::FieldShort,
        // A day nothing was asked of, that someone contributed on anyway.
        Some(day) if day.have > 0 => Mark::Stray,
        _ => Mark::Idle,
    };

    let mut out = Vec::new();
    for (row, name) in NAMES.iter().enumerate() {
        let mut line = format!("{name:<4}");
        for week in 0..grid.weeks {
            let date = grid.date_at(week, row);
            if !grid.holds(date) {
                line.push_str("  ");
                continue;
            }
            // Without colour the states still have to be told apart, so each
            // one gets its own pair of characters rather than its own green.
            let (glyph, colour) = match mark(plan.on(date)) {
                Mark::Hole => ("XX", palette.map(|p| p.danger)),
                Mark::Smudge => ("++", palette.map(|p| p.danger)),
                Mark::LetterDone => ("██", palette.map(|p| p.levels[4])),
                Mark::LetterShort => ("▒▒", palette.map(|p| p.levels[1])),
                Mark::FieldDone => ("░░", palette.map(|p| p.levels[2])),
                Mark::FieldShort => ("··", palette.map(|p| p.levels[0])),
                Mark::Stray => ("··", palette.map(|p| p.levels[2])),
                Mark::Idle => ("  ", None),
            };
            match colour {
                // With colour the block carries the state and the glyph is
                // always solid, which is what makes the shape readable at all.
                Some(colour) => line.push_str(&format!(
                    "\x1b[38;2;{};{};{}m{}\x1b[0m",
                    colour.0,
                    colour.1,
                    colour.2,
                    if matches!(mark(plan.on(date)), Mark::Hole | Mark::Smudge) {
                        glyph
                    } else {
                        "██"
                    }
                )),
                None => line.push_str(glyph),
            }
        }
        out.push(line);
    }
    out.join("\n")
}

/// A day's weekday name, for the schedule.
pub fn weekday(date: NaiveDate) -> &'static str {
    const NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    NAMES[date.weekday().num_days_from_sunday() as usize]
}
