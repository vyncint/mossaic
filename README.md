# mossaic

Make your GitHub contribution graph spell something. mossaic turns *"2027 should
read VYNCINT"* into a number of commits for **today**, tells you each morning
whether you are on pace, and says plainly when a year can no longer be drawn
cleanly.

And it shows you the result: the whole year in your terminal, drawn as real
rounded squares where the terminal can draw pixels, in GitHub's own Primer
colours, with a tooltip that follows the mouse.

[![CI](https://github.com/vyncint/mossaic/actions/workflows/ci.yml/badge.svg)](https://github.com/vyncint/mossaic/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/mossaic.svg)](https://crates.io/crates/mossaic)
[![docs.rs](https://img.shields.io/docsrs/mossaic)](https://docs.rs/mossaic)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-blue)](https://github.com/vyncint/mossaic/blob/main/Cargo.toml)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)

![a year of contributions drawn as rounded squares, spelling VYNCINT](art/pixel-cells.png)

That is a real year, drawn in a real terminal. Every cell is an image:
anti-aliased rounded squares at github.com's own geometry, sent over the kitty
graphics protocol or sixel, lined up exactly with the character cells so the
labels around them stay text.

## Contents

**Start here**

- [Quickstart](#quickstart) — plan the art, track it, look at the graph
- [Writing text into the graph](docs/ART.md) — the whole guide: placement, what
  it costs, drawing on a background, saving a plan
- [The GitHub Action](action/README.md) — track on a schedule, into an issue,
  Slack, Discord or email

**Reference**

- [Usage](#usage) · [Keys](#keys) · [Mouse](#mouse) · [Known limits](#known-limits)
- [Why](#why) · [How it works](#how-it-works) · [Design notes](docs/DESIGN.md)
- [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md) ·
  [Changelog](CHANGELOG.md)

## Quickstart

```sh
cargo install mossaic      # needs Rust 1.88+
```

**Plan the art.** No account and no network needed — this is arithmetic:

```sh
mossaic-art VYNCINT --year 2027            # what it looks like, and what it costs
mossaic-art VYNCINT --year 2027 --save     # remember the plan
```

**Then, every morning:**

```sh
gh auth login                              # once; mossaic never handles a token itself
mossaic-art --track                        # how far along, and what today owes
```

```
  letters     ██████████████████░░░░░░░░░░  50 of 75 bright
  owing       25 day(s) short, 100 contributions between them

  VYNCINT can still be drawn cleanly — 100 contributions to go.

  the rest of the year
    25 letter day(s) still to come, 100 contributions
```

*(A year part-way through; yours will read differently. The full report, on a
calendar you can reproduce exactly, is in [docs/ART.md](docs/ART.md#tracking-it-day-by-day).)*

It also answers the question no amount of committing fixes — whether days
already inside the letters have spoiled them, and which placement would spoil
the fewest.

**And to look at the graph itself**, whether or not you are drawing in it:

```sh
mossaic --demo             # a sample year: no account, no network
mossaic                    # yours
```

Press `?` in it for the keys, the mouse, and — the question everything else
depends on — **what your terminal turned out to be able to draw**.

That is the whole setup. Everything below is optional.

<details>
<summary>Not seeing rounded pixel cells?</summary>

Two things decide it, and `mossaic --capabilities` tells you both:

```
terminal   TERM=xterm-kitty
kitty      yes          <- the protocol
sixel      no
cell       9x19 px      <- the size, needed to line an image up with the labels
background #0d1117  (Dark theme)
cells      kitty
```

- **`cells text`** means the terminal draws neither protocol. kitty, Ghostty,
  WezTerm, foot, Konsole, iTerm2, mlterm, Windows Terminal and
  `xterm -ti vt340` all draw one of them; GNOME Terminal and Ptyxis draw
  neither, because VTE ships sixel switched off. Nothing is lost but the
  pixels — the chart still draws in block sextants.
- **A window under 110 columns** falls back too, and the chart says so under
  the legend rather than leaving you guessing — naming the number it wants.
- Either way, `mossaic --png chart.png` renders the same image to a file from
  any terminal at all.

</details>

## Why

Your contribution graph is probably the chart you look at most often and have
thought about least. It renders in someone else's tab, identically for everyone,
and reports one thing, after the fact: whether you showed up.

**mossaic turns it from a scoreboard into a canvas.** Decide that 2027 should
read VYNCINT and it works backwards — which days must be bright, at what shade,
and, because every day's shade is relative to your busiest one, what that costs
*today*. A year-long intention becomes a number you can act on this morning. Put
it in Actions and it will tell you each day whether you are on pace, behind, or
past the point where the word can still be finished.

Contribution art is usually done blind: generate thousands of commits, push, and
find out. Here the year is arithmetic before it is commits — what it costs, where
to place it so existing days do not punch holes in the letters, and whether it is
still reachable in September.

**And it draws the result honestly**, so you can check the picture before making
a single commit. GitHub's own Primer tokens, read from the stylesheets github.com
serves. github.com's own geometry — an 11px square on a 14px pitch, with the
hairline border drawn *over* the square's edge rather than inset into it — kept as
a ratio and scaled to whatever a character cell measures. Real anti-aliased squares
over the kitty graphics protocol or sixel. Nothing is approximated for the
terminal's convenience: when GitHub restyles the graph, a test in here is what
should fail.

Moss is the metaphor and the mechanic: nothing happens on any one day, and after
a season the wall is green.

## Writing text into the graph

`mossaic-art` draws text as pixels on the calendar, emits the commits that
would light it up, and tracks how far along you are — including the part no
amount of committing fixes:

```
  VYNCINT cannot be drawn cleanly in 2026.
    61 day(s) inside the letters already have contributions, and
    nothing takes those away — the text would read with holes in it.
    --start-week 1 would leave 23 instead of 61.
```

```sh
mossaic-art VYNCINT --year 2027                 # what it would look like, and cost
mossaic-art VYNCINT --year 2027 --save          # remember the plan
mossaic-art --track                             # am I getting there?
```

Drawn that way the letters stand against an **empty** graph, which means not
contributing on the other 290 days of the year. `--background` draws the
background as a colour instead, so the art is one shade against another and
every day of the year stays green:

```sh
mossaic-art VYNCINT --year 2027 --background 1  # letters on a field, not on nothing
```

GitHub's five shades are not evenly spaced, so mossaic measures the two you
picked in CIELAB across all nine palettes GitHub ships and tells you what a
reader will actually see — `ΔE 35 at worst, clear`. Leave two levels between
them and it is legible everywhere; leave one and it is not.

The whole guide — placement, cost, saving the plan, and running the tracker on
a schedule — is in **[docs/ART.md](docs/ART.md)**, and the Action that posts it
to Slack, Discord or email is in **[action/README.md](action/README.md)**.

## Usage

Three binaries, and every option has a default — the first line of each block is
the whole command you need.

```sh
mossaic                          # you, this year
mossaic --demo                   # a sample year: no account, no network
mossaic octocat --year 2024      # someone else, some year
mossaic --file saved.json        # a saved calendar, no network
```

```sh
mossaic --capabilities           # what your terminal can draw
mossaic --png chart.png          # the chart as an image, from any terminal
mossaic --graphics sixel         # force a protocol, or `text` for no pixels
mossaic --cell 10x20             # when the terminal will not say how big a cell is
mossaic --today 2027-06-30       # read the year as of a day, not the clock
mossaic --theme light            # instead of following the terminal's background
mossaic --palette winter         # GitHub's own seasonal colours
mossaic --no-mouse               # leave mouse reporting alone
```

```sh
mossaic-art VYNCINT --year 2027  # draw text into a graph — see docs/ART.md
mossaic-art --track              # how far along the plan is
mossaic-art --track --today 2027-06-01   # what a day that has not arrived will owe
mossaic-art --backfill --repo ../art     # commit just what the plan is short
mossaic-glyphs                   # what this terminal makes of the fallback cells
```

From a checkout, `cargo run -- …` and `cargo run --bin mossaic-art -- …` are
the same commands.

## Keys

| Key | Action |
| --- | --- |
| `←` `→` / `h` `l` | previous / next week |
| `↑` `↓` / `k` `j` | previous / next day |
| `[` `]` · `PgUp` `PgDn` | previous / next year **that has contributions** (steps by ±1 when the current year is outside that set, e.g. after `--year 2010`) |
| `t` | jump to today |
| `Home` / `End` | first / last day in range |
| `?` | keys, mouse, and what this terminal can draw |
| `u` | type a different username (`Enter` load, `Esc` cancel) |
| `d` | cycle cell style — auto / pixel / rounded / snug / squares / grid / spaced / blocks / slim / compact. The legend names it, and says `auto:` while the chart is still choosing for you |
| `m` | mouse reporting on or off — off gives the terminal its own selection back |
| `r` | reload |
| `q` / `Esc` | quit |

## Mouse

| | |
| --- | --- |
| hover a day | its tooltip, worded the way github.com words it, and a ring around the cell |
| click a day | move the cursor there, for terminals that report clicks but not motion |
| wheel | previous / next year |

## Without a terminal that draws pixels

A terminal without either protocol loses the pixels and nothing else. The whole
chart still draws, in block sextants — same layout, same colours, same tooltip:

```
┌ mossaic ───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│vyncint  ·  2026  ·  9,527 contributions in 2026                                                                                                                              │
│                                                                                      ▐ 113 contributions on August 19th. ▌                                                   │
│    Jan            Feb         Mar            Apr         May            Jun         Jul         Aug    ▼       Sep         Oct         Nov            Dec                    │
│       🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛            │
│Mon    🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛            │
│       🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛            │
│Wed    🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛            │
│    🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛            │
│Fri 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛               │
│    🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛               │
│                                                                                                                                                                              │
│Wed, Aug 19 2026  ·  113 contributions                                                                                                                                        │
│Less 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 More   ·   rounded cells                                                                                                                                  │
│110 active days  ·  longest 31  ·  best Aug 11 (146)                                                                                                                          │
│                                                                                                                                                                              │
│←→↑↓ day/week  ·  t today  ·  d cells  ·  m mouse off  ·  r reload  ·  q quit  ·  preview                                                                                     │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

## How it works

Three things decide what you see, and the first is the only one you may need to
act on:

- **What your terminal can draw.** mossaic asks it — one write, four questions,
  one round trip — rather than sniffing `TERM`, which is wrong in both
  directions: it misses terminals it has not heard of and claims support inside
  tmux or ssh where the escape never arrives. `mossaic --capabilities` prints
  the answers. `--graphics` and `--cell` override them.
- **Which colours.** Every value is a Primer token read out of the stylesheets
  github.com serves, not transcribed. Light or dark follows the terminal's own
  background over `OSC 11`, the way a browser follows the OS; `--theme`
  overrides it. GitHub swaps the greens for a few days a year and so does
  mossaic, on the same dates — `--palette` asks for them out of season.
- **How a day is sized.** A day is two character columns wide and one row tall,
  and the square inside it keeps github.com's own ratios — an 11px cell on a
  14px pitch, rounded by 2px — scaled to whatever a character cell measures.
  That is what lets the month labels stay text while the cells are pixels.

**[docs/DESIGN.md](docs/DESIGN.md) is the long answer**: what was traded for
what, why the tooltip sits above the grid rather than over it, why the sixel
palette is degraded on purpose, and what the capability probe actually sends.

## Known limits

- **VTE terminals have sixel switched off.** GNOME Terminal and Ptyxis are built on
  VTE, which does implement sixel — but `VTE_SIXEL_ENABLED_DEFAULT` is `false`, the
  embedding application has to call `vte_terminal_set_enable_sixel`, and neither of
  them does. VTE has no kitty-graphics support at all. So those terminals answer no
  to both and get sextants, correctly. kitty, Ghostty, WezTerm, foot, Konsole, iTerm2,
  mlterm, Windows Terminal and `xterm -ti vt340` all draw one protocol or the other.
- **Multiplexers do not pass either graphics protocol through**, so inside tmux or
  screen the probe comes back no and the chart falls to sextants. That is the right
  answer rather than a workaround: the escape would be swallowed either way.
- **Motion reporting is not universal.** Terminals that report clicks but not motion
  (Terminal.app among them) get click-to-select-a-day and no hover.
- **Sixel has no alpha.** Anti-aliased edges are composited against the background
  colour the terminal reported; where it reports none, GitHub's own canvas colour
  stands in, and on a terminal with a background image the corners will show a faint
  halo. Kitty, which takes an alpha channel, does not have the problem.
- **The tooltip sits above the grid, not over the day.** See above — it is the one
  placement both protocols can live with.
- Streaks are computed within the displayed year, so one spanning New Year is
  clipped at the year boundary — and the *current* streak is the run ending on
  the day the year is read as of, so a year that has ended has none. `--today`
  moves that day.
- A 53-week year needs 110 columns and 17 rows for pixel or square cells, 163 for
  rounded corners, and 164×25 for the bordered grid. Below that `Auto` drops to a
  borderless style, where a future day and a day off the end of the year both look
  blank; `d` still forces the others if you would rather they clipped.
- Private contributions appear only if the authenticated user can see them.

## Layout

| File | Purpose |
| --- | --- |
| `src/lib.rs` | the library the three binaries share |
| `src/main.rs` | `mossaic`: argument parsing, terminal setup, event loop |
| `src/app.rs` | state, key and mouse handling, background fetches |
| `src/calendar.rs` | the day grid, month labels, streak stats |
| `src/github.rs` | GraphQL query and response parsing |
| `src/primer.rs` | Primer colours, themes, seasons, 256-colour fallback |
| `src/term.rs` | what the terminal can do, asked rather than guessed |
| `src/graphics.rs` | the rasteriser, the kitty and sixel encoders, the painter |
| `src/ui.rs` | rendering, cell sizing, the tooltip |
| `src/art.rs` | the 5×5 font, calendar placement, and what shading costs |
| `src/plan.rs` | comparing a plan with what was actually contributed |
| `src/png.rs` | a small PNG encoder, for `--png` |
| `src/cli.rs` | the argument parsing all three binaries share |
| `src/bin/mossaic-art.rs` | draws text into a graph, and tracks the plan |
| `src/bin/mossaic-glyphs.rs` | what a terminal makes of the fallback cells |
| `src/render_tests.rs` | in-process tests: layout, colour, encoders, art, PNG |
| `tests/smoke.rs` | the chart in a real pty, through [termlens] |
| `tests/pixels.rs` | the pixel path in a pty that answers the probe |
| `tests/art_cli.rs` | the other two binaries, run as a user runs them |
| `action/` | the GitHub Action, and the guide to scheduling it |
| `docs/` | [the art guide](docs/ART.md), [design notes](docs/DESIGN.md), [releasing](docs/RELEASING.md) |

## Tests

```sh
cargo test                          # everything, no network
cargo test --lib                    # grid maths, colour, the encoders, the art font
cargo test --test smoke             # the real binary, in a real pty
cargo test --test pixels            # …in a pty that says it can draw pixels
cargo test -- --ignored             # the two that call the GitHub API
```

Three layers, because they catch different things. In-process tests check what is
a function of its inputs, and the encoders are checked *against the formats* —
the sixel is decoded back into pixels and compared to what the rasteriser drew.
Out of process, [termlens] spawns the real binary in a real PTY. And because that
PTY can be told which terminal it is simulating, the pixel path runs its own
probe rather than being handed the answer: `tests/pixels.rs` asserts what goes out
on the wire, including that a year over sixel really is an order of magnitude more
bytes than the same year over kitty.

[What belongs in which layer, and the two mistakes that make tests flaky, is in
CONTRIBUTING.md §3](CONTRIBUTING.md#3-testing-policy).

## Contributing

Bug reports, terminal compatibility reports and PRs are welcome — see
[CONTRIBUTING.md](CONTRIBUTING.md) for the dev setup and the testing policy, and
[docs/DESIGN.md](docs/DESIGN.md) before changing anything about the pixel path or
the wait semantics the tests rely on.

A compatibility report is the single most useful thing an outside contributor can
send, and it starts with two lines:

```sh
mossaic --capabilities     # what your terminal answered
mossaic --png /tmp/chart.png   # the same image, rendered to a file
```

Comparing the file with your screen separates "the rasteriser is wrong" from "the
emission is wrong", which are different bugs.

Security reports: [SECURITY.md](SECURITY.md).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option — the Rust ecosystem's standard dual
license. Apache-2.0 carries an express patent grant; MIT is maximally simple and
GPLv2-compatible. Offering both lets every downstream user pick whichever their
project or policy needs. Unless you explicitly state otherwise, any contribution
intentionally submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual licensed as above, without any additional terms
or conditions.
