//! Fetches the contribution calendar through the `gh` CLI, so authentication is
//! whatever `gh auth login` already set up — no token handling here.

use std::io::ErrorKind;
use std::process::Command;

use chrono::{Datelike, Local, NaiveDate};
use serde::Deserialize;

use crate::calendar::{Calendar, Day};

const QUERY: &str = "\
query($login: String!, $from: DateTime!, $to: DateTime!) {
  user(login: $login) {
    login
    contributionsCollection(from: $from, to: $to) {
      contributionYears
      contributionCalendar {
        totalContributions
        weeks { contributionDays { date contributionCount contributionLevel } }
      }
    }
  }
}";

const NO_GH: &str = "`gh` (GitHub CLI) was not found on PATH. \
Install it from https://cli.github.com, then run `gh auth login`.";

/// Fetch one calendar year. Every day of the year is kept, including ones still to
/// come; those are flagged `future` so they can be drawn as empty cells.
pub fn fetch(login: &str, year: i32) -> Result<Calendar, String> {
    let from = format!("{year}-01-01T00:00:00Z");
    let to = format!("{year}-12-31T23:59:59Z");
    let body = run_query(&[("login", login), ("from", &from), ("to", &to)])?;
    parse(year, &body, Some(Local::now().date_naive()))
}

/// Load a calendar from a saved response instead of calling gh. A snapshot has no
/// notion of "now", so every day in it counts as elapsed and is drawn — which is
/// what makes it useful for previewing a year that has not happened.
pub fn from_file(path: &str) -> Result<Calendar, String> {
    let body =
        std::fs::read_to_string(path).map_err(|error| format!("could not read {path}: {error}"))?;
    parse(Local::now().year(), &body, None)
}

/// The authenticated user, used as the default when no username is given.
pub fn whoami() -> Option<String> {
    let out = Command::new("gh")
        .args(["api", "user", "--jq", ".login"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let login = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!login.is_empty()).then_some(login)
}

fn run_query(vars: &[(&str, &str)]) -> Result<String, String> {
    let mut cmd = Command::new("gh");
    cmd.args(["api", "graphql", "-f", &format!("query={QUERY}")]);
    for (key, value) in vars {
        cmd.arg("-f").arg(format!("{key}={value}"));
    }

    let out = cmd.output().map_err(|e| match e.kind() {
        ErrorKind::NotFound => NO_GH.to_string(),
        _ => format!("could not run gh: {e}"),
    })?;

    // gh exits non-zero on GraphQL errors but still prints the response body, which
    // carries the useful message. Only fall back to stderr when there is no body.
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    if stdout.trim().is_empty() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("gh exited with {} and no output", out.status)
        } else {
            crate::printable(&stderr)
        });
    }
    Ok(stdout)
}

/// `now` decides which days count as still to come; `None` means none of them do.
fn parse(fallback_year: i32, body: &str, now: Option<NaiveDate>) -> Result<Calendar, String> {
    let resp: Response =
        serde_json::from_str(body).map_err(|e| format!("unexpected response from gh: {e}"))?;

    // Everything below this line came from somewhere else, so it is stripped of
    // control characters before it can reach a terminal.
    if let Some(message) = resp.errors.into_iter().flatten().next().map(|e| e.message) {
        return Err(crate::printable(&message));
    }
    let user = resp
        .data
        .and_then(|d| d.user)
        .ok_or_else(|| "GitHub returned no user for that login".to_string())?;

    let calendar = &user.contributions.calendar;
    let mut days = Vec::with_capacity(371);
    for week in &calendar.weeks {
        for raw in &week.days {
            let date = NaiveDate::parse_from_str(&raw.date, "%Y-%m-%d")
                .map_err(|e| format!("bad date {:?}: {e}", raw.date))?;
            days.push(Day {
                date,
                count: raw.count,
                level: level_of(&raw.level),
                future: now.is_some_and(|today| date > today),
            });
        }
    }
    days.sort_unstable_by_key(|d| d.date);
    // A calendar is one year plus the partial weeks at its ends. Without this,
    // a file naming two dates millennia apart makes the grid below allocate a
    // column for every week between them — gigabytes, from a few hundred bytes
    // of JSON.
    if let (Some(first), Some(last)) = (days.first(), days.last()) {
        let span = (last.date - first.date).num_days();
        if span > 400 {
            return Err(format!(
                "that calendar spans {span} days from {} to {}; a year is at most 366",
                first.date, last.date
            ));
        }
    }
    // GitHub returns exactly the requested year, so the first day names it.
    let year = days.first().map_or(fallback_year, |day| day.date.year());

    Ok(Calendar::build(
        crate::printable(&user.login),
        year,
        calendar.total,
        user.contributions.years,
        days,
    ))
}

fn level_of(level: &str) -> u8 {
    match level {
        "FIRST_QUARTILE" => 1,
        "SECOND_QUARTILE" => 2,
        "THIRD_QUARTILE" => 3,
        "FOURTH_QUARTILE" => 4,
        _ => 0,
    }
}

#[derive(Deserialize)]
struct Response {
    data: Option<Data>,
    /// Present but explicitly null on success, so this cannot be a plain `Vec`.
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Deserialize)]
struct Data {
    user: Option<User>,
}

#[derive(Deserialize)]
struct User {
    login: String,
    #[serde(rename = "contributionsCollection")]
    contributions: Contributions,
}

#[derive(Deserialize)]
struct Contributions {
    #[serde(rename = "contributionYears")]
    years: Vec<i32>,
    #[serde(rename = "contributionCalendar")]
    calendar: CalendarData,
}

#[derive(Deserialize)]
struct CalendarData {
    #[serde(rename = "totalContributions")]
    total: u32,
    weeks: Vec<RawWeek>,
}

#[derive(Deserialize)]
struct RawWeek {
    #[serde(rename = "contributionDays")]
    days: Vec<RawDay>,
}

#[derive(Deserialize)]
struct RawDay {
    date: String,
    #[serde(rename = "contributionCount")]
    count: u32,
    #[serde(rename = "contributionLevel")]
    level: String,
}

#[derive(Deserialize)]
struct GraphQlError {
    message: String,
}
