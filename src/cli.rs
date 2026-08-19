//! The small amount of argument parsing every binary here needs.
//!
//! Shared so that the tools agree about the things a user notices: that
//! `--year 2027` and `--year=2027` are the same, that a missing value says so
//! rather than being read as the next flag, and that every error looks like
//! `mossaic-art: …` and exits 2.
//!
//! Deliberately not a parser library. There are four kinds of option here —
//! flag, value, number, positional — and a dependency that handles the other
//! forty would be a dependency to keep current.

use std::collections::VecDeque;
use std::ffi::OsString;
use std::ops::RangeInclusive;

/// Years the tools will work in. GitHub's graph starts in 2008 and a plan for
/// 2101 is not a plan; the range mostly exists so that nothing downstream has
/// to wonder whether a date can be constructed.
pub const YEARS: RangeInclusive<i32> = 2000..=2100;

/// A command line, part-way through being read.
#[derive(Debug)]
pub struct Args {
    program: &'static str,
    rest: VecDeque<String>,
    /// Long options that were actually typed, so a saved plan or a config can
    /// fill in the rest without overriding them.
    typed: Vec<String>,
}

impl Args {
    /// The process's own arguments, with `--key=value` split in two.
    pub fn from_env(program: &'static str) -> Self {
        Self::new(program, std::env::args_os().skip(1))
    }

    /// The same, from anywhere — which is what makes it testable.
    pub fn new(program: &'static str, args: impl Iterator<Item = OsString>) -> Self {
        let rest = args
            .map(|arg| arg.to_string_lossy().into_owned())
            .flat_map(|arg| match arg.split_once('=') {
                // Only long options split, so a path or a name containing `=`
                // is left alone.
                Some((key, value)) if key.starts_with("--") => {
                    vec![key.to_string(), value.to_string()]
                }
                _ => vec![arg],
            })
            .collect();
        Self {
            program,
            rest,
            typed: Vec::new(),
        }
    }

    /// The next argument, whatever it is. Not `next`: this is not an iterator,
    /// and reading it as one would be a subtle way to lose an argument.
    pub fn next_arg(&mut self) -> Option<String> {
        self.rest.pop_front()
    }

    /// Whether an argument follows that is not itself an option — for the flags
    /// that take an optional value, like `--track [USER]`.
    pub fn peek_value(&self) -> bool {
        self.rest.front().is_some_and(|next| !next.starts_with('-'))
    }

    /// The value belonging to `flag`, or a readable exit.
    pub fn value(&mut self, flag: &str) -> String {
        self.remember(flag);
        self.rest
            .pop_front()
            .unwrap_or_else(|| self.fail(&format!("{flag} needs a value")))
    }

    /// The same, as a number.
    ///
    /// Unbounded, so a caller that narrows the result has to cope with whatever
    /// arrives. Prefer [`Args::number_in`]: every numeric option here feeds a
    /// `usize` or a `u32` in the end, and `as` is not a check.
    pub fn number(&mut self, flag: &str) -> i64 {
        let raw = self.value(flag);
        raw.parse()
            .unwrap_or_else(|_| self.fail(&format!("{flag} needs a number, not {raw:?}")))
    }

    /// A number inside `range`, or a readable exit.
    ///
    /// This is the one that should be reached for. Every numeric option in
    /// these tools ends up as a `usize` or a `u32`, and casting an unchecked
    /// `i64` with `as` is not a conversion but a reinterpretation:
    /// `--commits -1` came out as 4,294,967,295 and `--start-week -1` reached
    /// `usize::MAX`, where building a date from it panicked. Bounding the value
    /// where it is read makes the cast that follows infallible.
    ///
    /// `noun` names the thing in the error, the way `year` does — "wants a
    /// level between 0 and 4" reads better than "wants a number".
    pub fn number_in(&mut self, flag: &str, noun: &str, range: RangeInclusive<i64>) -> i64 {
        let raw = self.value(flag);
        raw.parse::<i64>()
            .ok()
            .filter(|value| range.contains(value))
            .unwrap_or_else(|| {
                self.fail(&format!(
                    "{flag} wants a {noun} between {} and {}, not {raw:?}",
                    range.start(),
                    range.end()
                ))
            })
    }

    /// A calendar year, checked against the range the calendar model can hold.
    ///
    /// Shared because the two binaries used to disagree: one refused a year
    /// outside 2000-2100 and the other passed 999999 through to a panic.
    pub fn year(&mut self, flag: &str) -> i32 {
        self.number_in(
            flag,
            "year",
            i64::from(*YEARS.start())..=i64::from(*YEARS.end()),
        ) as i32
    }

    /// Note that a flag was typed, for the ones that carry no value.
    pub fn remember(&mut self, flag: &str) {
        let name = flag.trim_start_matches('-').to_string();
        if !self.typed.contains(&name) {
            self.typed.push(name);
        }
    }

    /// Whether `flag` was typed on this command line.
    pub fn was_typed(&self, flag: &str) -> bool {
        self.typed
            .iter()
            .any(|seen| seen == flag.trim_start_matches('-'))
    }

    /// Stop, with a message and the exit code a shell expects for misuse.
    pub fn fail(&self, message: &str) -> ! {
        eprintln!("{}: {message}", self.program);
        std::process::exit(2)
    }
}

#[cfg(test)]
mod tests {
    use super::Args;

    fn args(items: &[&str]) -> Args {
        Args::new("test", items.iter().map(|item| (*item).into()))
    }

    #[test]
    fn long_options_split_on_equals_and_nothing_else_does() {
        let mut parsed = args(&["--year=2027", "-y", "2026", "some=path"]);
        assert_eq!(parsed.next_arg().as_deref(), Some("--year"));
        assert_eq!(parsed.value("--year"), "2027");
        assert_eq!(parsed.next_arg().as_deref(), Some("-y"));
        assert_eq!(parsed.value("-y"), "2026");
        // A positional keeps its `=`: it may be a path, or a name.
        assert_eq!(parsed.next_arg().as_deref(), Some("some=path"));
    }

    #[test]
    fn an_optional_value_is_told_from_the_next_flag() {
        let mut parsed = args(&["--track", "octocat", "--year", "2027"]);
        parsed.next_arg();
        assert!(parsed.peek_value(), "a login follows");
        assert_eq!(parsed.value("--track"), "octocat");
        parsed.next_arg();
        assert!(parsed.peek_value(), "a year follows");

        let mut bare = args(&["--track", "--year", "2027"]);
        bare.next_arg();
        assert!(!bare.peek_value(), "a flag is not a value");
    }

    #[test]
    fn a_bounded_number_takes_its_ends_and_the_year_rides_on_it() {
        let mut parsed = args(&["--top", "0", "--commits", "1", "--year", "2100"]);
        parsed.next_arg();
        assert_eq!(parsed.number_in("--top", "row", 0..=2), 0, "the low end");
        parsed.next_arg();
        assert_eq!(parsed.number_in("--commits", "count", 1..=9), 1);
        parsed.next_arg();
        // `year` is `number_in` with the calendar's own range, so the two
        // cannot drift apart.
        assert_eq!(parsed.year("--year"), 2100, "the high end");
    }

    #[test]
    fn typed_flags_are_remembered_so_defaults_know_to_stay_out() {
        let mut parsed = args(&["--year", "2027"]);
        parsed.next_arg();
        parsed.value("--year");
        assert!(parsed.was_typed("--year"));
        assert!(parsed.was_typed("year"), "with or without the dashes");
        assert!(!parsed.was_typed("--top"));
    }
}
