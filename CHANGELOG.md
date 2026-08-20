# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Until 1.0, minor versions (0.x) may contain breaking changes; they are always
listed under a **Changed** or **Removed** heading.

## [Unreleased]

## [0.4.0] - 2026-08-20

### Fixed

- `Esc` no longer quits the chart in normal mode; it cancels the username
  prompt and dismisses the help overlay, as the UI implies. `q` remains the
  only quit key.
  

### Added

- **`--today DATE` on the chart, not just the tracker.** 0.3.0 made time an input
  for `mossaic-art` and left `mossaic` reading the clock, so the two disagreed
  about whether time is an input at all — and the still-to-come half of the
  rendering was reachable only from a live fetch, which is why the cursor could
  vanish there unnoticed. With `--file` it also makes a saved year show its future
  days, which is what a plan snapshot for next year actually is. (#38)
- **`--palette auto` can pick winter.** `Season::on` matched only Halloween, so a
  scale that is fully implemented in all three appearances, measured among the nine
  palettes `Shades::worst` reads, and askable with `--palette winter` could never
  appear by itself. GitHub does not publish the windows; December 19–25 is what it
  has been observed using, and the code says so rather than claiming a
  specification. (#22)
- **The chart says when a protocol was asked for and could not be used.**
  `--graphics kitty` without a cell size drew characters silently; `--capabilities`
  explained it and the chart did not. (#37)

### Changed

- **Cells are the size github.com draws them.** The 5% border was drawn first and
  the fill inset by it, so the coloured square measured 14px in a 20px pitch —
  0.700, where github.com's is 11 on 14, or 0.786. Every cell was about a ninth too
  small with a dark rim where a hairline belonged, `--png` included, which is where
  the README's own screenshots come from. The square is now drawn at full size with
  the hairline over its edge, and the offsets are rounded to whole pixels: at a 9x19
  cell the opaque extent was 12x11 where 12x12 was drawn, soft on one axis and crisp
  on the other. (#20)
- **A current streak is the run ending on a given day**, so `Calendar::stats` takes
  one. Both loops compared positions in a list rather than dates, so any gap read as
  an unbroken run — ten non-adjacent Mondays reported a ten-day streak — and the
  "quiet today" tolerance was applied to the last elapsed day of *any* year, so a
  year that ended a decade ago reported a current streak of forty. (#18)
- **`mossaic-glyphs` uses the shared argument parser**, so all three binaries agree
  about what a user notices: `--color=never` is honoured rather than ignored, an
  unknown option or a stray argument exits 2 with a message naming the binary, and a
  missing value says so instead of quietly defaulting. (#25)
- **`Rgb::luminance` is now `Rgb::brightness`.** It applies the luminance
  coefficients to gamma-encoded sRGB with no linearisation, so it was not the
  quantity its name promised — mid grey came out 0.502 where relative luminance is
  0.216. The threshold it produces is the more useful one for choosing a terminal
  theme; only the name was wrong. (#35)
- **A flag that would do nothing is refused rather than ignored.** `--track` with
  `--snapshot`, `--write` or `--repo` wrote nothing and said nothing; `--demo` with
  `--file` silently picked the demo. Distinct from `--commits`, which tracking has no
  use for but which is a tuning parameter rather than a side effect somebody asked
  for. (#26)
- **The legend says `auto:` while the chart is choosing.** Cycling with `d` past the
  narrowest style lands back on `Auto`, which resolves to whatever fits — so the
  press looked like it had done nothing. (#34)

### Fixed

- **Pixel images are bounded by columns as well as rows.** Sixel clears the
  character cells it is about to cover by writing spaces, and did so with no idea
  how wide the terminal was: a 53-week grid wrote 106 of them from column 6, so on
  an 80-column terminal seven rows wrapped over the weekday gutter, the right border
  and the rows below — and stayed wrapped, because ratatui believed it had written
  those cells. The legend image was never bounded at all, and was clamped onto the
  frame's last row. (#17)
- **A shade GitHub names and this version does not is derived, not dropped.**
  `level_of` mapped anything unrecognised to 0, and nothing downstream re-derives
  one — so a day with forty contributions was painted exactly like an empty one
  while the header reported a full year. The fallback is `art::level`, GitHub's own
  rule, which the art costing already used. (#19)
- **The keyboard cursor is visible on a day that has not happened.** `patch` drew
  nothing at all without a level, the ring included, and `filled_rows` blanked a
  future day before consulting the mark — so it vanished in pixel, rounded, snug and
  squares cells while the five bordered styles showed it, and the detail line went on
  naming the day it was on. (#21)
- **The capability probe reads a reply that shares its buffer.** Only the first
  `CSI ?` was tried, so a `CSI ?2026;2$y` — the DECRQM answer for synchronized
  update, the mode this program itself uses — left in the tty queue by whatever ran
  before us hid sixel *and* the sentinel, making every start-up pay the whole 250 ms
  deadline. `OSC 11` now also accepts the `#rrggbb` form of an X colour spec, which
  was read as no answer at all, so a light terminal replying that way got the dark
  palette. (#23)
- **`[` and `]` stay inside the years `--year` accepts.** The fallback for a year
  outside the contribution set stepped without a bound, one `gh` subprocess per
  keypress, and the header named years the CLI refuses to be given. (#24)
- **At 80x24 the chart still says how to get out.** The footer was one 111-character
  line, hard-truncated, so neither `q quit` nor `? help` was on screen; the note row
  was not counted in the chrome budget, so at one particular height it evicted the
  footer; and the help overlay truncated from the bottom, dropping what the terminal
  can draw and `any key closes this` with it. The footer now drops the guessable
  hints first and the overlay drops its cheapest lines. (#27)
- **A click that misses a day puts the tooltip away**, instead of leaving it naming
  an unrelated date — which on a terminal that reports clicks but not motion nothing
  later corrected. And the mouse no longer acts behind the help overlay, where the
  wheel changed year and started a fetch for a year the reader could not see. (#28)
- **`--png` refuses an empty year** rather than writing a zero-width PNG, which the
  format forbids and no viewer opens, and reporting success. (#29)
- **The help overlay is opaque on kitty.** An image at `z=-2` draws over a cell
  background rather than under it, and the panel sets one, so the chart showed
  through its text. The painter now stands down while it is up. (#30)
- **The advice line never points at a smaller style that does not exist.** From
  `Auto` at its narrowest it said "press d for a smaller style", where `d` goes to
  pixels — three times wider. It now names the columns a year needs. (#31)
- **`--capabilities` honours `--cell`**, so the one command whose job is explaining
  the pixel decision no longer reports the opposite of what the run would do. (#34)
- **A calendar naming a day twice is refused** rather than keeping whichever record
  an unstable sort put last, and a GraphQL error without a `message` surfaces as an
  error rather than as "unexpected response from gh: missing field `message`".
  `calendar::demo` no longer panics on a year no calendar can hold. (#35)
- **The character cell is measured again on a resize**, which is also when a font
  size changes — `docs/DESIGN.md` promises the geometry is scaled to whatever a cell
  measures, and it was measured once at startup. (#37)
- **Eight measured claims in the design notes, and five in the source.** The
  tightest adjacent ΔE is 9.1 (light + halloween, levels 1 and 2), not the 10.8 that
  five places quoted for a different pair (`src/art.rs` among them); the sixel coarsening never triggers for a
  chart, which needs 20–27 of 256 registers; the kitty layering is two layers at
  `z=-2` and `z=-1`, not one at `z=-1`; the probe asks five questions, not four, and
  has a window-size fallback; the two-level gap is warned about rather than enforced;
  the byte figures are ~5 KB and 44 KB at a 9x19 cell; the xterm-256 collision is
  between levels 0 and 1 rather than 1 and 2, and the dimmed ramp ties rather than
  inverts; and the layers block was missing four modules. The load-bearing claim —
  two levels apart never below ΔE 35.4 — holds, and is now pinned by a test. (#33)
- **Release hygiene.** `.claude/` no longer ships inside the crate,
  `extract-changelog.sh` runs from anywhere rather than only the repository root, and
  `docs/RELEASING.md` gained the two steps whose absence let three `action@v0.2.0`
  references go stale: bumping them, and repointing the changelog's own link
  definitions. (#32)

### Documented

- **The advice line named a window size that would not have worked.** It quoted the
  drawable area inside mossaic's own border, so a reader with a 17-row window was
  told it "has 15" — resizing to the 17 it asked for still did not fit. Both numbers
  are now the ones a terminal reports. The README's figures were wrong in the other
  direction ("about 115×26" for the bordered grid, against 166×27), and are now
  measured in a pty: 112×19 for pixel or square cells, 165 columns for rounded
  corners. (#31, #33)
- **The README is a quarter shorter and both its pictures are 2027.** The headline
  claim is *"2027 should read VYNCINT"* and the screenshot was a 2026 calendar, so
  the first thing a reader saw disagreed with the first thing they read. Both
  images are now the same planned 2027 year — once in pixels, once in sextants —
  and the screenshot draws the letters on a **level-1 field** rather than on an
  empty graph, which is the option the guide recommends and the old picture never
  showed. Its caption is the two commands that reproduce it byte for byte.
  It was also rendered with the pre-#20 geometry, so the one image whose entire
  job is showing that mossaic draws github.com's cell size was showing cells a
  ninth too small; and the terminal mock was several versions stale — no `? help`
  in the footer, no streak in the summary, no `auto:` on the legend, and an
  active-day count from before `--today`. Both are regenerated from the binary.
  The file-layout and test-layer sections, which duplicated `CONTRIBUTING.md`,
  are now links to it. (#20, #33)
- **Every byte figure in `docs/DESIGN.md` §4 is now the measured one**, and the
  three places that quoted it agree: 4.9 KB on the wire for a year over kitty
  against 38 KB of sixel, and 243 against 591 bytes for a cursor move. The table
  said 5 KB/44 KB, §13 said 43 KB, and the tests' own comments said 8 KB/45 KB.
  `tests/pixels.rs` prints what it measured, so the next reader does not have to
  guess which was right. (#33)
- **§3 says how the border is drawn**, which is the whole of #20: over the square's
  edge, not inset into it, with the offset rounded to whole pixels. (#20, #33)
- **Which palette the legibility numbers describe.** `Palette::separation` measures
  the 24-bit values whatever the terminal can show, and art is read in a browser, so
  those are the right numbers for the decision — but `docs/ART.md` now says so
  rather than leaving a reader on a 256-colour terminal to assume the figures
  describe what is in front of them. (#36)
- `Esc` quits, which nothing said. (#34)

## [0.3.1] - 2026-08-20

### Added

- **`tests/pixels.rs`: the pixel path, end to end.** Eight tests over the half of
  the program that was not merely unasserted but *unreachable*. mossaic decides
  whether to draw pixels by asking the terminal, so a harness that answered no to
  every question could only be driven down the text path — the suite worked around
  that with `--graphics sixel --cell 10x20`, which forces the protocol and hands
  over the cell size, skipping the probe, the fallbacks and the auto choice.
  termlens 0.5 states which terminal is being simulated, so the real decision runs.

  What that buys, beyond the probe itself: the byte budgets in `docs/DESIGN.md` §4
  are now checked against the wire (a year is ~5 KB over kitty against ~44 KB of
  sixel, and the test holds the order of magnitude the design decision rests on);
  the diffing in §5 is checked the same way, since one cursor move costs under a
  tenth of a year rather than re-sending it; the cell contract in §3 is asserted
  through a terminal for the first time, with all seven grid rows left to the
  painter while the month labels stay text; `--graphics text` is checked to send no
  image at all on a terminal that could draw one; `Auto` is checked never to place
  an image too wide to fit; and `--capabilities`, which had no test at all because
  no harness could answer the questions it asks, is checked to agree with what the
  chart then does.
- **Two tests for behaviour no content assertion can see.** An idle chart repaints
  on an 80 ms timer so the loading spinner can turn, and every one of those frames
  shows exactly the right content — `printable_chars` is what shows that a settled
  chart writes nothing at all. And mossaic ignores keys it does not bind silently,
  which used to be the same screen as refusing them with a bell; `bells()` tells
  the two apart.

### Changed

- **termlens 0.4.2 → 0.5.0** (dev-dependency). `send`, `send_str` and `paste`
  return `Result` rather than panicking, so every input call in the suite now
  propagates with `?` — 24 call sites, which is the whole migration.

## [0.3.0] - 2026-08-19

### Added

- **`--today DATE` makes "today" an input.** The tracker read the clock, so its
  answer to "what does today owe" was a fact about when it ran rather than about
  a calendar and a date. Now it can be told, which is what lets a report be
  reproduced, lets a day that has not arrived be asked what it will owe, and
  lets the tests assert on a report at all. (#2, #3)
- **`--backfill` commits what the days already past are short of, and nothing
  else.** A plain `--write` puts the same flat count on every lit day, including
  the ones already bright — and adding to the busiest of those raises the year's
  peak, which is the very thing every letter day is measured against. A shortfall
  cannot do that: `need` is at most the peak already, so what every other day
  owes is unchanged afterwards and one pass of arithmetic is enough. Days still to
  come are deliberately left for an ordinary commit on the day; back-dating is
  only how you reach the ones that have gone. `--today` is what "past" means.
  Needs `--repo`, and `--write` to commit. (#7)
- **`keep-dark`, a day the report now warns about.** A day inside the text block
  that is not part of a letter has to stay empty, and a contribution on it is
  the one loss nothing takes back. The Action gains `today-kind` and
  `tomorrow-kind` outputs, so *do not commit tomorrow* can arrive a day early.
  (#4)
- **A Windows job in CI.** The `cfg(not(unix))` probe was never compiled by any
  gate, and had drifted out of date in two ways that `-D warnings` would have
  caught. (#11)

### Changed

- **`fail-on: behind` counts letter days only.** It tested `today.short`, which
  is also non-zero for a short *background* day — so with `background` set, each
  of the year's ~290 field days failed the job, blaming a letter day for it.
  (#9)
- **A report says nothing about a day its own year does not hold.** Tracking
  2024 while it is 2026 reported on a day in 2026 and called it `outside`, which
  is true of the wrong calendar; `today` and `tomorrow` are now `null` there.
  (#4)
- **`hole` names damage rather than position**, in the JSON report and in the
  markdown one. A clean day inside the letters is `keep-dark`; `hole` is what it
  becomes once it is too bright. The old value was a *position*, and the markdown
  report read it as damage — so with a background drawn, a day inside the letters
  holding **no contributions at all** was reported as "inside the letters and
  already lit — a permanent hole". It now reads as the background day it is.
  (#4)
- **A failed fetch names whose year it was reading.** Whose contributions to
  track can come from a saved plan, and "gh was not found" gave no hint which
  login it had resolved. The login is stripped of control characters where it is
  resolved rather than at each of the places that print it, which is the rule the
  calendar already followed — a plan file is a file someone may have sent you.

### Fixed

- **A calendar has to sit somewhere a calendar can sit.** `github::parse` bounded
  how *wide* a saved calendar was (400 days) but not where it *was*, and the grid
  reaches outside the days it is handed: back to the Sunday before the first, and
  a week forward from the cursor. A file whose first day was within three of the
  earliest date a `NaiveDate` can express panicked in release as well as debug —
  and in the chart that was the worst of it, because the fetch runs on its own
  thread: the thread died and the screen sat on "loading" for ever, with no error
  and no way to retry. `Calendar::build` degrades to an empty grid rather than
  panicking, for the same reason `art::Grid::new` returns `None`.
- **The cursor cannot be walked off the end of the calendar.** `move_cursor`
  added to the date and clamped afterwards, so the clamp could not save it: with
  a calendar ending within a week of the last representable date, one arrow key
  ended the process.
- **A saved plan is validated when it is read.** `--plan PATH` names a file, and
  a file need not have come from your own `--save` — so it was the way around
  every bound the command line enforces. A `top` of `usize::MAX` wrapped past
  `place`'s guard (`usize::MAX + 5` is 4, which is comfortably "inside" a
  seven-row week) and drew the letters on scrambled rows; a `commits` of
  `u32::MAX` quoted four billion commits a day; a `year` of -262143 panicked
  building the calendar, and one of 180000 drew a calendar `cli::YEARS` exists to
  refuse. `Spec::validate` applies the flags' own ranges, so a saved plan can
  never mean something a typed command could not.
- **`art::Grid::new` keeps the promise in its own doc comment.** It said it
  returned `None` rather than panicking "because this is a library: a year
  arriving from a command line, a file or a caller is input" — and then stepped
  back to a Sunday with unchecked arithmetic, which panics in the first week a
  calendar can express. Two guards for one hole, because the second is what the
  library owes a caller that is not this CLI.
- **Three counts from a `--merge` calendar no longer wrap.** The year's peak — the
  number the entire costing model rests on — was accumulated with `+=`. A
  calendar whose busiest day held four billion reported a peak of 8 and quoted a
  price to match it, in release builds; in debug it panicked. Everything around
  them already saturated or widened.
- **Numeric options are bounded where they are read.** `--year` was
  range-checked because a binary once passed 999999 through to a panic; every
  other number took the unguarded path, and `as` is not a check. `--commits -1`
  came out as 4,294,967,295 — which `--write` would then try to build a
  fast-import stream for — `--commits 0` drew bright letters for no commits at
  all, `--top -1` wrapped past the guard meant to catch it, and
  `--start-week -1` reached `usize::MAX`, where building a date from it panicked.
  A start column too far right is now refused with the last one that would have
  fitted, rather than silently drawing a plan of no days. (#5)
- **A saved plan's `user` is no longer discarded.** It was read out of the file
  and then overwritten with the nothing a bare `--track` carries, so
  `mossaic-art --track` ignored the login the plan named — and with `gh`
  authenticated it tracked the wrong person rather than saying so. (#6)
- **A `--merge` calendar from another year says so.** Every day of a 2026
  calendar falls outside a 2027 grid, so it filtered down to nothing and
  everything downstream was correct about an empty year: 9,527 contributions
  read as none, and a plan that cannot be drawn read as reachable at one commit
  a day. (#8)
- **`--today` outside the plan's year says so.** The flag carries no year of its
  own, so `--today 2027-06-01` against a plan whose year defaulted to this one
  asked about a calendar the plan does not cover — and the entire "what to do
  next" half of the report silently vanished. Still allowed, because every letter
  day of an ended year is overdue and that is a retrospective worth asking for.
- **Test scratch files no longer share a fixed path.** Fourteen of them named a
  constant under the temp directory, so two test processes in one checkout — a
  stress loop beside an ordinary `cargo test` — raced and failed in ways that
  looked like the code. `CONTRIBUTING.md` §3 asks for hermetic tests, and a fixed
  global path is not one.
- **Two tests no longer assert on the wall clock.** One claimed whatever day it
  ran on was a lit day inside the letters — true when it was written, false the
  morning after — and another that 2027 had not started. Both now pin `--today`,
  and CI runs the CLI suite a second time from a timezone that is already
  tomorrow. (#3)
- **The command `--track` suggests for back-dating now does what the sentence
  above it promises.** It printed a plain `--write` with the text typed out,
  which ignored the saved plan, defaulted to 4 commits a day where the plan
  needed 110, and wrote to every lit day rather than the short ones. (#7)
- **Sixel uses all 256 colour registers.** The capacity check bailed at a full
  palette rather than a full-plus-one, and it ran after an index had been handed
  out — one step later and `seen.len() as u8` would have wrapped to 0 and
  aliased a colour. Restructured so no such index is ever constructed. (#14)
- **Rustdoc**: `term::probe` carried doc comments on both sides of its
  `#[cfg]` and rendered them concatenated; `calendar::demo`'s summary line
  belonged to `Stats`; the non-unix `probe` had no documentation at all. (#10)
- **`calendar::demo` no longer re-implements GitHub's shading.** It called
  `art::level`'s formula a second time, in `u32` where the original widens to
  `u64` — a second place to change when GitHub restyles the graph, in a crate
  whose whole point is agreeing with github.com. (#14)

### Removed

- **`y` and `Y` no longer change year.** They were undocumented, they duplicated
  `[`, `]`, `PageUp` and `PageDown`, and in a chart that binds `h j k l` a `y`
  that silently moves the year is the one binding with a real chance of
  surprising a vim-handed user, to whom it means *yank*. `PageUp` and `PageDown`
  are documented now, in the README and in the `?` overlay. (#12)

## [0.2.0] - 2026-08-19

### Changed

- **The GitHub Action installs the tracker from crates.io** instead of building
  the checked-out repository, and gains a `version` input to say which release:
  the default, `latest`, follows crates.io, and a pinned number changes only
  when you change it. Nothing is cached — a fresh `--locked` install per run
  costs a minute or two and can never be stale, poisoned, or the wrong
  version. The ref you pin (`@v0.2.0`) now chooses only the action's own
  steps.

### Fixed

- **Tracking is no longer refused because of a flag it does not use.** Over a
  busy year, `--track --merge --background 1` failed with "the letters would
  not show" — a check meant for runs that *draw*, where `--commits` is what
  each lit day gets. Tracking never writes a commit; it works out what a
  letter day needs from the year's real peak. The GitHub Action passes no
  `--merge` and so never hit it, which meant the Action and the CLI disagreed
  about the same plan. Found by running the Action against a real year and
  comparing.
- **A hole is called a hole again.** With no background drawn, a lit day inside
  the letters reported as "background — 113 contributions, 113 too many for
  level 0", naming a shade the plan never asked for. The markdown report always
  got this right, so the text report was the one disagreeing with it.

## [0.1.1] - 2026-08-19

No functional changes — the crate is byte-for-byte the same tool as 0.1.0.
This release exists to prove the publishing pipeline works the way it is
supposed to.

### Changed

- **Releases publish through crates.io Trusted Publishing.** The workflow
  exchanges the job's OIDC token for a credential that lives for minutes, so
  no registry token is stored on the repository at all. 0.1.0 could not do
  this — Trusted Publishing is configured *on a crate*, and the crate did not
  exist until 0.1.0 published — so it went out with a stored token, which has
  since been deleted along with the zizmor exception that covered it.

## [0.1.0] - 2026-08-19

First release. The **Changed** and **Fixed** sections below record decisions
taken during development rather than changes against a previous release —
there was none.

### Added

- **The chart.** A whole year of GitHub contributions in the terminal: every
  week drawn, including the days still to come, with a cursor that moves by day
  and by week, year navigation that visits only years with contributions, and
  streak statistics computed over the days that have actually happened.
- **Primer colours**, read out of the stylesheets github.com serves rather than
  transcribed: the light, dark and dark-dimmed scales, plus GitHub's seasonal
  `winter` and `halloween` palettes on the dates GitHub itself shows them.
  Light or dark follows the terminal's own background over `OSC 11`.
- **Pixel cells**, at github.com's geometry — an 11px square on a 14px pitch,
  2px corner radius, half a pixel of border — anti-aliased from a signed
  distance field and square whatever the font's aspect ratio. Sent over the
  **kitty graphics protocol** (RGBA, zlib'd: 507 KB of image becomes 8 KB on
  the wire) or **sixel** (palette, run-length encoded), with block sextants as
  the fallback where the terminal draws neither.
- **A capability probe** that asks the terminal instead of sniffing `TERM`:
  the kitty graphics query, `OSC 11`, `CSI 16 t`/`CSI 14 t` and device
  attributes in one round trip, read from `/dev/tty` with `O_NONBLOCK` so a
  terminal that never answers costs a timeout and nothing else.
  `--capabilities` prints what yours said.
- **Mouse hover**, with github.com's own tooltip wording (`97 contributions on
  August 19th.`) floating above the grid. Motion, drag and click all hover,
  because `1003` motion reporting is the mode most likely to be missing; the
  wheel changes year; `m` hands mouse reporting back to the terminal. A hovered
  day repaints one cell, not the year.
- **`--png`**, which writes the chart as an image using the same rasteriser the
  protocols feed from — the chart, from a terminal that draws no pixels.
- **`--cell WxH`**, for terminals that will not report their cell size.
- **`mossaic-art`**, a binary that writes text into a contribution graph by dating
  commits: a 5×5 font, placement on Mon–Fri, a preview, snapshots the chart can
  render, and `git fast-import` for the tens of thousands of commits an active
  year can cost. It never pushes.
- **`mossaic-glyphs`**, which shows the fallback cells so a terminal's rendering of
  block sextants can be checked at a glance.
- **Synchronized frame updates** (DEC 2026) around every repaint, so a terminal
  that understands them shows a frame's text and images together.
- **`--demo`**, a deterministic sample year that needs no account, no `gh` and
  no network — so the chart can be seen before anything is authenticated.
- **`?` in the chart**: an overlay with the keys, the mouse, and what this
  terminal can actually draw. The question "does my terminal do pixels?" is
  answerable without leaving the chart or knowing that `--capabilities` exists.
- **`-V` / `--version`**, and a `--help` organised around what people come to
  do rather than around the list of switches.
- **`mossaic-art --track`**, which compares the plan with what has actually been
  contributed: how many letter days are bright, what today and tomorrow owe, how
  much is left, and — the part no amount of committing fixes — how many days
  inside the letters are already lit and never can be unlit. It sweeps
  `--start-week` to suggest the placement that runs into fewest of them, and
  draws the year with each day coloured by what the plan makes of it.
- **A GitHub Action** (`vyncint/mossaic/action`), so a plan can be tracked on
  a schedule instead of by remembering to ask. It runs the tracker, writes the
  report to the job summary, and hands back `verdict`, `headline`, `markdown`,
  `json` and the scalars — `bright`, `owing-commits`, `holes`, `today-short`,
  `tomorrow-need`. `fail-on: behind` turns "today owes something" into a failed
  job. Sending the report to Slack, Discord, email or an issue is a step you add
  after it; `action/README.md` carries one for each and a repository setup guide,
  and `action/track.example.yml` is the file to copy.
- **`mossaic-art --track --format json|markdown`**, which is what the Action reads: every
  number the text report prints, in a shape a machine and a message can carry.
- **`mossaic-art --font`**, which prints every glyph side by side — the view a new one
  has to be judged in, for whoever writes it and whoever reviews it.

- **A commit policy, enforced.** AI assistance is welcome; AI attribution is
  not. `commit-policy.yml` runs `check-no-ai-attribution.sh` over every commit
  in a pull request and rejects `Co-Authored-By` trailers naming an assistant,
  "Generated with" watermarks, and bot identities. The check is shared verbatim
  with [termlens], which runs the same rule — two repositories enforcing one
  policy should not drift.

### Changed

- **`mossaic-art --background LEVEL`**, which draws the background as a shade
  instead of leaving it empty. Contribution art used to cost you the rest of
  the year — keeping the letters visible meant *not contributing* on the other
  290 days. Now the letters are one green against another and every day stays
  green. Because GitHub's five shades are not evenly spaced, the two you pick
  are measured in CIELAB across all nine palettes GitHub ships and reported as
  `ΔE 35 at worst, clear`: adjacent levels fall as low as ΔE 10.8, levels two
  apart never below 35.4, so the tool asks for a gap of two and says so when it
  does not get one. A background day is tracked as a band — a floor to reach
  and a ceiling to stay under — and the Action gains `background`, `legibility`,
  `separation` and the `field-*` counts.
- **The extra binaries are namespaced**: `art` and `glyphs` are now
  `mossaic-art` and `mossaic-glyphs`. Both were about to go onto everyone's
  `PATH` under names nobody could guess belonged to this, and `art` in
  particular is a name someone else's tool is entitled to.
- **A plan can be saved**, so tracking needs no flags at all:
  `mossaic-art VYNCINT --year 2027 --save` writes `mossaic-plan.json`, and
  `mossaic-art --track` reads it. The placement is stored *resolved*, which
  closes the one trap in the tracker — comparing against a different plan
  because a flag was typed differently. Typed flags still win over saved ones.
- **`--color auto|always|never` on the text tools**, honouring
  [`NO_COLOR`](https://no-color.org); `--no-colour` is now a spelling of
  `--color never`. `-V`/`--version` and `-y`/`--year` work everywhere they read
  as though they should.
- **One argument parser** behind all three binaries, so they agree about the
  things a user notices: `--year=2027` and `--year 2027`, what a missing value
  says, and that every error is `binary: message` with exit code 2.
- **The README leads with a quickstart** rather than with prerequisites, and the
  contribution-art material moved to `docs/ART.md` — a front page and a manual
  are different documents.

### Fixed

- **A contributed glyph can no longer break the build's silence.** A row a
  character short used to index past the end and panic for whoever drew it
  first; a stray character silently rendered as a dark day. Both are now
  compile-time failures naming the rule they broke, alongside the same for a
  character listed twice. Two further rules — a glyph must draw something and
  not everything, and no two characters may draw the same pixels — are covered
  by tests, because a compiler cannot judge them.
- **Five findings from a security review of untrusted input**, each with a
  regression test: escape sequences from a crafted calendar reaching the
  terminal through the non-renderer output paths; a calendar spanning millennia
  allocating gigabytes; counts from a file overflowing the shading arithmetic;
  a character-cell size sizing an image allocation unchecked; and a git identity
  going into a `fast-import` stream unvalidated. See SECURITY.md for what each
  one was.

- **`--font` and `--capabilities` no longer ignore the flags written after
  them**: both act once the whole command line is parsed.
- **`--help` now lists every spelling the parser accepts.** The test that keeps
  help and parser in step read only the first flag of each match arm, so an arm
  like `"-h" | "--help"` was half-checked — `--help`, `--colour` and
  `--no-color` all worked and none of them were documented.
- **`mossaic-art --year 999999` panicked**, and `--year -5` drew a calendar
  for the year minus five. Both binaries now check the year against the same
  range, and `art::Grid::new` returns `None` for a year no calendar can hold
  rather than ending in an `expect` — it is a library, and a year is input.
- **One cost model, not two.** `commits_for_level` took a day's shade from the
  commits *added* to it but the year's peak from the day's *total* — the same
  day counted two ways, which under-priced art landing on days that already had
  contributions. Shade and peak now both come from the total, the answer is a
  fixed point rather than a 20,000-step search, and `commits_to_reach` states
  the arithmetic in one line that every quoted price goes through. The README's
  worked example (a peak of 112 costing 85 a day) is unchanged; its sweep figure
  moved, and has been re-measured.

[termlens]: https://github.com/vyncint/termlens

[Unreleased]: https://github.com/vyncint/mossaic/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/vyncint/mossaic/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/vyncint/mossaic/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/vyncint/mossaic/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/vyncint/mossaic/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/vyncint/mossaic/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/vyncint/mossaic/releases/tag/v0.1.0
