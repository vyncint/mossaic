# Writing text into a contribution graph

`mossaic-art` renders text as pixels on the calendar, emits the commits that
would light them up, and then tracks how far along you are. It never pushes:
`--write` makes local commits in a directory you name, and prints the push
command for you to run.

- [Drawing it](#drawing-it)
- [Drawing on a background, not on nothing](#drawing-on-a-background-not-on-nothing)
- [Tracking it, day by day](#tracking-it-day-by-day)
- [Catching up on the days already past](#catching-up-on-the-days-already-past)
- [Tracking it from a schedule](#tracking-it-from-a-schedule)
- [How GitHub shades a day, and why it decides the cost](#how-github-shades-a-day-and-why-it-decides-the-cost)

## Drawing it

```sh
mossaic-art VYNCINT --year 2027                   # preview only
mossaic-art VYNCINT --year 2027 \
    --snapshot art/vyncint-2027.json                          # then: mossaic --file …
mossaic-art VYNCINT --year 2027 \
    --repo ../vyncint-art --write                             # local commits
```

Letters are a 5×5 font (A–Z, 0–9, space, `-`, `.`), one blank column between them,
placed on rows Mon–Fri so Sunday and Saturday stay clear. `mossaic-art --font` prints the
whole set; adding to it is one table entry in `src/art.rs`, with the shape rules
checked when the crate compiles — see
[CONTRIBUTING.md §7](../CONTRIBUTING.md#7-adding-a-glyph-to-the-font). `VYNCINT` is 41 of the
year's 53 columns and is centred by default; `--start-week` and `--top` move it,
`--commits` sets how many commits each lit day gets.

**Eight characters is the limit**, whatever the year. Nine fit on paper — five
columns a letter plus one between is `6N − 1`, and nine of those is 53, exactly a
year — but the first and last calendar columns are *partial weeks*. A year begins
and ends mid-column, so text that fills it loses the part of its leading letter that
falls in December of the year before. `mossaic-art` says so rather than drawing a broken
letter:

```
note: 3 pixel(s) fell outside 2027 and were dropped — the first and last calendar
columns are partial weeks, so 52 of 53 columns hold a whole letter
```

Preview it in the real renderer before committing anything — `--snapshot` writes a
file in GitHub's own response shape and `mossaic --file` draws it, treating every day
as elapsed so a future year still shows.

## Drawing on a background, not on nothing

Contribution art has an awkward cost nobody mentions: to keep the letters
visible you have to keep the rest of the year **dark**. Drawing `VYNCINT` in
2027 the classic way means 75 bright days and 290 days on which you must not
contribute at all. For a tool about contributing, that is a strange thing to
ask.

`--background LEVEL` draws the background as a *colour* instead of as nothing.
The letters stay at level 4; the rest of the year sits at the level you pick.
The art becomes the difference between two greens, and the year stays busy.

```sh
mossaic-art VYNCINT --year 2027 --background 1
```

```
VYNCINT  ·  2027  ·  41 of 53 columns  ·  75 days  ·  590 commits

      ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
Mon   ░░░░░░░░░░██░░░░░░██░░██░░░░░░██░░██░░░░░░██░░░░██████░░░░██████████░░
      ░░░░░░░░░░██░░░░░░██░░░░██░░██░░░░████░░░░██░░██░░░░░░██░░░░░░██░░░░░░
Wed   ░░░░░░░░░░██░░░░░░██░░░░░░██░░░░░░██░░██░░██░░██░░░░░░░░░░░░░░██░░░░░░

background level 1 under letters at level 4  ·  290 background day(s), 1 each
  ·  ΔE 35 at worst, clear
```

75 letter days at 4 commits and 290 background days at 1 — 590 commits for a
year in which **every single day is green**.

### Leave two levels between them

GitHub's five shades are not evenly spaced, and how far apart two of them look
depends on which theme the reader has. mossaic measures it in
[CIELAB](https://en.wikipedia.org/wiki/CIELAB_color_space) — ΔE, where under 2
is invisible, 10 is "you would have to be told", and 35 reads as two different
colours at a glance — across all nine palettes GitHub ships (light, dark and
dimmed, each with its winter and halloween variants).

These are the colours github.com serves a browser, which is where art is read, so
they are the right ones for this decision. They are *not* the 256-colour ramp the
chart falls back to in a terminal without truecolour: that ramp is chosen for
legibility rather than accuracy, and its own separations differ. The numbers below
describe the picture your readers see, not the one in your terminal.

| `--background` | worst ΔE against level 4 | | where it is worst |
| --- | --- | --- | --- |
| `0` (default) | 70.3 | clear | dimmed + winter |
| `1` | 35.5 | clear | dark + halloween |
| `2` | 35.4 | clear | dimmed |
| `3` | 17.5 | **faint** | dimmed |

The rule falls straight out of the table: **leave at least two levels between
the background and the letters.** Adjacent shades fall as low as ΔE 9.1 — on the
light halloween palette, levels 1 and 2 are all but the same colour — while any
gap of two or more never drops below 35.4. `--background 1` and `--background 2` are
both safe; `--background 3` is drawn, with a warning:

```
  -> 3 and 4 are neighbouring shades. On some themes they are all but the same
     colour;
     leave two levels between them — --background 2 is the safe one against
     level 4.
```

Two combinations are refused outright rather than drawn, because neither
produces art at all:

```sh
mossaic-art VYNCINT --background 4              # the background is the letters
# mossaic-art: the background (level 4) must be darker than the letters
#              (level 4), or there is nothing to see

mossaic-art VYNCINT --background 1 --commits 1  # too few commits to hold two shades
# mossaic-art: --commits 1 puts the letters at level 1 and the background at
#              level 1, so the letters would not show.
#   In this year a letter day needs at least 2 commits to sit above a level-1
#   background.
```

That second one is worth understanding, because it is the one that surprises
people. A shade is a *fraction of the year's busiest day*, so a year whose peak
is 1 has exactly two shades in it — empty and full — and cannot hold a
background at all. At a peak of 4 the counts 1, 2, 3, 4 land on levels 1, 2, 3,
4 exactly, which is as small as a five-shade year gets.

### What tracking does with it

A background day is a **band**, not a target: it has a floor (contribute enough
to reach the shade) and a ceiling (contribute more and it stops being
background). Both matter, and they fail differently:

- **Below the floor** is debt. Contribute more, today or by back-dating.
- **Above the ceiling** is damage. Nothing takes contributions away, so a
  background day that ran to level 4 is a bright dot in the picture for good —
  the same kind of loss a lit day inside a letter has always been.

`--track` reports the two shades separately, because they are different kinds
of work — three hundred easy days would otherwise drown out seven hard ones:

```
  the shades  letters at level 4, background at level 1  ·  ΔE 35 at worst, clear
              a background day has to reach 1 and stay under 2

  letters     ████████████████████████████  75 of 75 bright
  background  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░  0 of 290 at level 1
  owing       290 background day(s) short, 290 contributions
```

The preview marks each state: `██` a letter that is bright enough, `▒▒` one
still short, `░░` background at the right shade, `··` background still to do,
`XX` a hole inside the letters, `++` a day outside them that has run too
bright.

One thing a background changes for the better: on a bare graph, *any*
contribution inside the text block is a hole. With a background, a quiet day in
there is exactly what the field wants — so a year that could not be drawn
cleanly at all often can be, once the background gives those days somewhere to
belong.

## Tracking it, day by day

Drawing the art is one command. Getting there while also living a normal year is
a hundred small decisions, and `--track` is the one that answers them:

```sh
# Against your own year. Add `--merge art/vyncint-2026.json --today 2026-08-19`
# to reproduce the output below exactly — that is the calendar this repository
# ships, read on the day this was written.
mossaic-art VYNCINT --year 2026 --track
```

```
VYNCINT  ·  2026  ·  tracking art/vyncint-2026.json

  the plan    41 of 53 columns from week 6, on rows 1-5
  the year    9,527 contributions  ·  busiest Aug 11 (146)
              a letter day has to reach 110 to match it

  letters     ██████░░░░░░░░░░░░░░░░░░░░░░  18 of 75 bright
  owing       57 day(s) short, 5,994 contributions between them
  holes       61 day(s) inside the letters are lit and cannot be unlit
  around      23 day(s) outside the text have contributions

  <the year, drawn: bright where a letter is done, dim where it is owed,
   red where a day inside the letters is lit and cannot be unlit>

  VYNCINT cannot be drawn cleanly in 2026.
    61 day(s) inside the letters already have contributions, and
    nothing takes those away — the text would read with holes in it.
    --start-week 1 would leave 23 instead of 61.

  today       Wed Aug 19  ·  inside the letters and already lit (113) — a permanent hole
  tomorrow    Thu Aug 20  ·  inside the letters — keep it dark, or it becomes a permanent hole

  the next seven days
    Wed Aug 19   hole
    Thu Aug 20   keep dark
    Fri Aug 21   letter  110 to go
    Sat Aug 22   —
    Sun Aug 23   —
    Mon Aug 24   letter  110 to go
    Tue Aug 25   keep dark

  the rest of the year
    23 letter day(s) still to come, 2,530 contributions
    34 letter day(s) already past, 3,464 contributions — only back-dated
    commits reach those:

      mossaic-art VYNCINT --year 2026 --start-week 6 --top 1 --backfill --repo ../art --write
```

Four kinds of answer, and only two of them are work:

- **A letter day that is short.** Contribute more, today or by back-dating.
  Fixable whenever.
- **A day inside the letters that must stay dark.** The gaps inside and between
  the letters are part of the picture, and a contribution landing on one punches
  a hole that nothing takes back. This is the only kind of day where the
  instruction is to do *nothing*, and the only one worth being told about a day
  early — the report says `keep it dark`, the seven-day schedule says
  `keep dark`, and the Action's `tomorrow-kind` output says `keep-dark`.
- **A day inside the letters that is already lit.** The same day, after the fact.
  Nothing takes contributions away, so it is a hole in the text for good. This is
  the honest answer to "why can't I write VYNCINT in 2026": not that it is
  expensive, but that the year has already been written on. `--track` counts the
  holes, and sweeps `--start-week` to find the placement that runs into fewest.
- **A day outside the text with contributions.** Noise around the letters rather
  than damage to them; reported, not warned about.

### Asking about a day that is not today

`--today DATE` is what the report measures against, and it defaults to the
clock rather than replacing it. Two things it is for:

```sh
mossaic-art --track --today 2027-06-01   # what will that day owe?
mossaic-art --track --today 2026-08-19   # the same answer, next year and forever
```

The first is planning; the second is why anything here can be tested or
documented at all. A report that reads the clock is a report whose output
changes overnight, which is no use in a README and no use in an assertion.

It reads `--merge PATH` if you have a saved calendar, and otherwise asks `gh` —
`--track USER` for someone else's year. Run it whenever you like: it holds no
state, so the answer is a fact about today's data rather than about the last
time you ran it.

### Catching up on the days already past

Nagging cannot reach a day that has gone by, and `--backfill` is what does. It
reads the plan, asks what the year actually holds, and commits **each day's
shortfall** — nothing on a day that is already bright, and nothing at all on a
day whose job is to stay dark:

```sh
mossaic-art --backfill --repo ../art            # what it would write
mossaic-art --backfill --repo ../art --write    # write it, locally
```

```
# with --merge art/vyncint-2026.json --today 2026-08-19, to reproduce this exactly
VYNCINT  ·  2026  ·  backfilling against art/vyncint-2026.json

  letters     57 day(s) short, 5,994 commits
  a day gets  what it is short of 110, never a flat count
  reaching    days before 2026-08-19, which are the ones only back-dating reaches
              23 day(s) from 2026-08-19 on are short too, and left alone — contribute on those as they come

  warning: VYNCINT cannot be drawn cleanly in 2026 — 61 day(s) inside the
  letters are already lit, and nothing takes those away. Backfilling will
  brighten the letters, and the text will still read with holes in it.
  `mossaic-art --track` sweeps --start-week for a placement with fewer.

  3,464 commit(s) across 34 day(s), earliest 2026-02-09, latest 2026-08-14

(add --write to create them; this was a dry run)
```

With no background drawn, the 34 days and 3,464 commits are exactly what
`--track` reports as "already past": the two answers come from the same plan and
the same date, so they agree by construction rather than by coincidence. With a
background they will not match, and should not — `--track` counts "already past"
in *letter* days, while a backfill also lays down every past day of the field.

**Only days already past.** A day still to come needs no back-dating — you
contribute on it when it arrives — and whether GitHub counts a future-dated
commit at all is unverified (see below). `--today` is what "past" is measured
against, so the 34 days here are exactly the ones `--track` reports as "already
past".

A shortfall rather than a flat `--commits`, and the reason is the same
arithmetic the rest of this page turns on: a shade is a fraction of the year's
*peak*, so putting the same count on every lit day adds to the busiest of them
and raises the peak — which raises what every other letter day needs. The bar
moves as you walk towards it. A shortfall cannot, because what a day needs is
never more than the peak it is measured against: the brightest shade is three
quarters of it. Topping a day up to `need` therefore leaves the peak where the
scale already stood, so what every *other* day owes is the same afterwards as
before — one pass, and no second round of arithmetic to chase. (On an *empty*
year the peak does move, from nothing to whatever the art puts there, which is
the easy direction; `Shades::min_peak` is what keeps even that expressible.)
There is a test that proves the fixed point.

It finishes the *past*, not the year: the days still to come are deliberately
left for you to contribute on as they arrive.

Like `--write`, it never pushes; it prints the command for you to run.

**Why the price moves.** Everything above is measured against the year's peak,
because that is what GitHub shades against. One big day anywhere — a merge queue
that landed 112 commits — raises what *every* letter day costs, which is why the
report leads with the busiest day and what it did to the bar.

## Tracking it from a schedule

The tracker is also a [GitHub Action](../action/README.md), so the report can
arrive rather than be asked for:

```yaml
- id: art
  uses: vyncint/mossaic/action@v0.3.1
  with:
    text: VYNCINT
    year: "2027"
    start-week: "6"
    timezone: Asia/Ho_Chi_Minh
```

It hands back `verdict`, `headline`, `markdown` and `json`, plus the scalars —
`bright`, `owing-commits`, `holes`, `today-short`, `tomorrow-need` — and writes
the report to the job summary. Sending it on is a step you add after it, because
Slack, Discord, email and issue comments all have maintained actions already and
none of them belong inside this one; `action/README.md` has a workflow for each,
and `action/track.example.yml` is the file to copy into a repository of your own.

`fail-on: behind` turns "today is a letter day and it is short" into a failed
job, so the reminder arrives through the notifications you already have.

## How GitHub shades a day, and why it decides the cost

Verified against a real calendar, 365 of 365 days matching:

```
level = 0                          when count == 0
level = min(4, ceil(count * 4 / peak))   otherwise
```

`peak` is the busiest day of that year. The buckets are equal slices of `[0, peak]`,
**not** rank quartiles — so a day only reaches the brightest level once it passes
three quarters of the year's peak. Two consequences:

- **An empty year is cheap.** With nothing else in it the art sets the peak itself, so
  any uniform count lights every letter at the top level. A few commits per day is enough.
- **An active year is expensive**, and the price is set by your busiest day. Drawing
  over a year whose peak is 112 needs ~85 commits per lit day just to match it — and
  more still, because art landing on an already-busy day *sums* with it and pushes the
  peak up again. `--merge` solves for the real figure rather than estimating it, and
  reports how many existing days would be as bright as the letters.

Placement matters as much as count: which days the letters happen to cover changes the
peak, and so the cost. Sweeping `--start-week` over one real year moved the bill from
25,275 commits to 12,900 for the same text — and `--track` does that sweep for you.

```sh
gh api graphql -f query='...' > real.json    # your actual year
mossaic-art VYNCINT --year 2026 --start-week 1 \
    --commits 113 --merge real.json --snapshot art/vyncint-2026.json
```

Commits are written with `git fast-import`, so tens of thousands take under a second
rather than the minutes a `git commit` per commit would cost.

For commits to count towards the graph they must be on the **default branch** of a
**repo you own** (not a fork), authored with an **email registered to your GitHub
account** — `git config user.email` must be one GitHub knows.

**Whether a future-dated commit counts is unverified.** Git will happily date one
ahead of today, but GitHub may not count it; no account has a future year in
`contributionYears`, so there is nothing to check against. Push a single future-dated
commit and look before generating thousands. Dates inside the current year are the
safer bet either way: they become ordinary past dates as the year runs on.

## Saving the plan

The placement is part of the plan, and passing it every time is a mistake
waiting to happen: tracking with a different `--start-week` compares against a
different plan and says so confidently. Save it once instead.

```sh
mossaic-art VYNCINT --year 2027 --start-week 6 --save
mossaic-art --track          # reads mossaic-plan.json, no flags needed
```

The file stores the placement *resolved*, so a text that was centred keeps the
column it was centred on. Typed flags still win over the saved ones, so
`--year 2028` is a one-off rather than a surprise. `--plan PATH` puts the file
somewhere else.
