//! Contribution calendar model: a Sunday-aligned grid of days plus derived stats.

use chrono::{Datelike, Duration, NaiveDate};

/// A single day in the calendar. `level` is GitHub's quartile bucket, 0 (none) to 4 (most).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Day {
    /// The calendar date.
    pub date: NaiveDate,
    /// Contributions GitHub counted for it.
    pub count: u32,
    /// GitHub's own shade, 0 (none) to 4 (most).
    pub level: u8,
    /// The day has not happened yet. Drawn as an empty cell and left out of every
    /// statistic, so a half-finished year is not judged as if it were over.
    pub future: bool,
}

/// One column of the chart. Indexed by weekday, 0 = Sunday.
/// Slots are `None` only where a week falls outside the year, which is the head of
/// the first week and the tail of the last.
#[derive(Debug, Clone, Default)]
pub struct Week {
    /// Sunday first. `None` where the week falls outside the year.
    pub days: [Option<Day>; 7],
}

/// One year of contributions, as a grid the chart can draw and the mouse can
/// index into.
#[derive(Debug, Clone)]
pub struct Calendar {
    /// The GitHub login this belongs to.
    pub login: String,
    /// The year it covers.
    pub year: i32,
    /// Total reported by GitHub for the range, which can exceed the sum of visible days.
    pub total: u32,
    /// Years the user has contributions in, ascending. Drives year navigation.
    pub years: Vec<i32>,
    /// The grid itself, one entry per column.
    pub weeks: Vec<Week>,
    /// Sunday of week 0, so date <-> grid position is arithmetic rather than a search.
    grid_start: Option<NaiveDate>,
}

impl Calendar {
    /// Build the grid from a chronologically sorted, contiguous run of days.
    pub fn build(
        login: String,
        year: i32,
        total: u32,
        mut years: Vec<i32>,
        days: Vec<Day>,
    ) -> Self {
        years.sort_unstable();
        // Checked, for the same reason `art::Grid::new` is: this is a public
        // constructor, and the Sunday before a day need not exist — at the very
        // first date a `NaiveDate` can hold there is nothing behind it. A
        // calendar with no start has no columns, which draws as "no contribution
        // data" rather than taking the process with it.
        let grid_start = days.first().and_then(|d| {
            d.date.checked_sub_signed(Duration::days(i64::from(
                d.date.weekday().num_days_from_sunday(),
            )))
        });

        // A year is 53 columns, 54 when it starts on a Saturday in a leap year.
        // The cap is defensive: this is a public constructor, and a day far out
        // of range would otherwise size the grid by the distance to it.
        const MAX_WEEKS: usize = 60;
        let mut weeks: Vec<Week> = Vec::new();
        if let Some(start) = grid_start {
            for day in &days {
                let offset = (day.date - start).num_days();
                if offset < 0 || offset >= (MAX_WEEKS * 7) as i64 {
                    continue;
                }
                let (w, wd) = ((offset / 7) as usize, (offset % 7) as usize);
                if weeks.len() <= w {
                    weeks.resize(w + 1, Week::default());
                }
                weeks[w].days[wd] = Some(*day);
            }
        }

        Self {
            login,
            year,
            total,
            years,
            weeks,
            grid_start,
        }
    }

    /// Grid coordinates as (week column, weekday row), or `None` if off-grid.
    pub fn position(&self, date: NaiveDate) -> Option<(usize, usize)> {
        let offset = (date - self.grid_start?).num_days();
        if offset < 0 {
            return None;
        }
        let (w, wd) = ((offset / 7) as usize, (offset % 7) as usize);
        (w < self.weeks.len()).then_some((w, wd))
    }

    /// The day at `date`, if the grid holds it.
    pub fn day(&self, date: NaiveDate) -> Option<Day> {
        let (w, wd) = self.position(date)?;
        self.weeks[w].days[wd]
    }

    /// Days in chronological order. The grid is Sunday-aligned, so row-major order is date order.
    pub fn days(&self) -> impl Iterator<Item = Day> + '_ {
        self.weeks
            .iter()
            .flat_map(|w| w.days.iter().flatten().copied())
    }

    /// Days that have actually happened, which is what statistics are drawn from.
    pub fn elapsed(&self) -> impl Iterator<Item = Day> + '_ {
        self.days().filter(|day| !day.future)
    }

    /// Whether any day in the range has actually happened yet.
    pub fn has_elapsed_days(&self) -> bool {
        self.elapsed().next().is_some()
    }

    /// The first day drawn, which is January 1st for a fetched year.
    pub fn first_date(&self) -> Option<NaiveDate> {
        self.days().next().map(|d| d.date)
    }

    /// The last day drawn, December 31st included even if it has not happened.
    pub fn last_date(&self) -> Option<NaiveDate> {
        self.days().last().map(|d| d.date)
    }

    /// True when the requested year had not begun as of `today`. Such a year is
    /// empty because none of it has happened, not because nothing was done in it.
    pub fn starts_after(&self, today: NaiveDate) -> bool {
        self.year > today.year()
    }

    /// Month labels as (week column, short name), placed on the first week of each month.
    pub fn month_labels(&self) -> Vec<(usize, &'static str)> {
        const NAMES: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let mut labels = Vec::new();
        let mut prev = 0;
        for (w, week) in self.weeks.iter().enumerate() {
            let Some(day) = week.days.iter().flatten().next() else {
                continue;
            };
            let month = day.date.month();
            if month != prev {
                labels.push((w, NAMES[(month - 1) as usize]));
                prev = month;
            }
        }
        labels
    }

    /// Streaks and totals, counted over the days that have happened.
    ///
    /// `today` is which day "current" means. A streak is a run of *consecutive
    /// dates*, so both loops compare dates rather than positions in a list: a
    /// calendar that lists only its busy days — a legal saved response, and what
    /// a hand-rolled `gh api` snapshot looks like — used to report ten
    /// non-adjacent Mondays as a ten-day streak.
    pub fn stats(&self, today: NaiveDate) -> Stats {
        let mut stats = Stats::default();

        // The longest run, and the totals. A run continues only into the very
        // next date; a gap in the calendar breaks it, exactly as a quiet day does.
        let mut run = 0u32;
        let mut previous: Option<NaiveDate> = None;
        for day in self.elapsed() {
            if day.count == 0 {
                run = 0;
                previous = None;
                continue;
            }
            stats.active_days += 1;
            run = match previous {
                Some(prev) if prev.succ_opt() == Some(day.date) => run + 1,
                _ => 1,
            };
            previous = Some(day.date);
            if run > stats.longest_streak {
                stats.longest_streak = run;
            }
            if stats.best.is_none_or(|(_, best)| day.count > best) {
                stats.best = Some((day.date, day.count));
            }
        }

        // The run ending today, walked back a date at a time. A quiet *today* is
        // tolerated so "today, so far" does not read as a broken streak — which
        // is a statement about today and not about the last day of any year: a
        // finished year's last elapsed day is December 31st, and dropping that
        // one unconditionally reported a decade-old year as a current streak.
        let mut walk = today;
        if self
            .day(today)
            .is_some_and(|day| !day.future && day.count == 0)
        {
            walk = today.pred_opt().unwrap_or(today);
        }
        while let Some(day) = self.day(walk) {
            if day.future || day.count == 0 {
                break;
            }
            stats.current_streak += 1;
            match walk.pred_opt() {
                Some(previous) => walk = previous,
                None => break,
            }
        }

        stats
    }
}

/// A sample year, for trying the chart with no account and no network.
///
/// Deterministic — the same year every time, so what `--demo` shows is what the
/// tests assert on. Weekdays are busier than weekends and the counts cluster the
/// way real ones do, because a demo of a contribution chart that looks nothing
/// like a contribution chart demonstrates the wrong thing.
pub fn demo(year: i32) -> Calendar {
    // A year no calendar can hold gets an empty one rather than a panic. Not
    // reachable from either CLI — `cli::year` bounds it — but this is a public
    // constructor, and the others in this module stopped panicking on their
    // arguments in 0.3.0.
    let (Some(first), Some(last)) = (
        NaiveDate::from_ymd_opt(year, 1, 1),
        NaiveDate::from_ymd_opt(year, 12, 31),
    ) else {
        return Calendar::build("demo".to_string(), year, 0, vec![year], Vec::new());
    };

    let mut seed: u32 = 0x9e37_79b9 ^ (year as u32);
    let mut next = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        seed >> 16
    };

    let mut counts = Vec::new();
    let mut date = first;
    while date <= last {
        let weekend = matches!(date.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun);
        let roll = next() % 100;
        let count = match (weekend, roll) {
            (true, 0..=64) | (false, 0..=24) => 0,
            (_, 25..=79) => 1 + next() % 6,
            (_, 80..=95) => 6 + next() % 12,
            _ => 18 + next() % 30,
        };
        counts.push((date, count));
        date += Duration::days(1);
    }

    let peak = counts
        .iter()
        .map(|(_, count)| *count)
        .max()
        .unwrap_or(1)
        .max(1);
    let total = counts
        .iter()
        .fold(0u32, |sum, (_, count)| sum.saturating_add(*count));
    let days = counts
        .into_iter()
        .map(|(date, count)| Day {
            date,
            count,
            // GitHub's own shading, from the one place that implements it. A
            // second copy here would be a second thing to change when GitHub
            // restyles the graph, in a crate whose whole point is agreeing
            // with github.com.
            level: crate::art::level(count, peak),
            future: false,
        })
        .collect();
    Calendar::build("demo".to_string(), year, total, vec![year], days)
}

/// What the summary line reports, counted over elapsed days only.
#[derive(Debug, Clone, Copy, Default)]
pub struct Stats {
    /// Days with at least one contribution.
    pub active_days: u32,
    /// The busiest day and its count.
    pub best: Option<(NaiveDate, u32)>,
    /// The run ending at the last elapsed day, tolerating a quiet today.
    pub current_streak: u32,
    /// The longest run of active days anywhere in the year.
    pub longest_streak: u32,
}
