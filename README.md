# mossaic

**Make your GitHub contribution graph spell something.** Tell mossaic that 2027
should read VYNCINT and it works backwards to a number of commits for *today* —
then tells you each morning whether you are on pace, and says plainly when the
year can no longer be drawn cleanly.

And it shows you the result, in your terminal, as real pixels: GitHub's own
Primer colours at github.com's own geometry, with a tooltip that follows the
mouse.

[![CI](https://github.com/vyncint/mossaic/actions/workflows/ci.yml/badge.svg)](https://github.com/vyncint/mossaic/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/mossaic.svg)](https://crates.io/crates/mossaic)
[![docs.rs](https://img.shields.io/docsrs/mossaic)](https://docs.rs/mossaic)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-blue)](https://github.com/vyncint/mossaic/blob/main/Cargo.toml)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)

![VYNCINT drawn in bright green on a field of light green, a whole year of a GitHub contribution graph](art/pixel-cells.png)

That is 2027, planned and drawn — bright letters on a **light green field**
rather than on an empty graph, so every day of the year stays green. Two
commands, and the second is the one that made the image above:

```sh
mossaic-art VYNCINT --year 2027 --background 1 --snapshot /tmp/art.json
mossaic --file /tmp/art.json --today 2027-12-31 --cell 9x19 --png chart.png
```

Every cell there is an image — anti-aliased rounded squares sent over the kitty
graphics protocol or sixel, lined up exactly with the character cells so the
labels around them stay text.

## Contents

**Start here** · [Quickstart](#quickstart) ·
[Writing text into the graph](docs/ART.md) ·
[The GitHub Action](action/README.md)

**Reference** · [Usage](#usage) · [Keys](#keys) · [Mouse](#mouse) ·
[How it works](#how-it-works) · [Known limits](#known-limits) ·
[Design notes](docs/DESIGN.md) · [Contributing](CONTRIBUTING.md) ·
[Changelog](CHANGELOG.md)

## Quickstart

```sh
cargo install mossaic      # needs Rust 1.88+
```

**Plan the art.** No account, no network — this is arithmetic:

```sh
mossaic-art VYNCINT --year 2027 --background 1 --save
```

**Then, every morning:**

```sh
gh auth login              # once; mossaic never handles a token itself
mossaic-art --track        # how far along, and what today owes
```

```
  letters     ██████████████████░░░░░░░░░░  50 of 75 bright
  owing       25 day(s) short, 100 contributions between them

  VYNCINT can still be drawn cleanly — 100 contributions to go.
```

*(A year part-way through; yours will read differently. The full report, on a
calendar you can reproduce exactly, is in [docs/ART.md](docs/ART.md#tracking-it-day-by-day).)*

**And to look at any graph**, whether or not you are drawing in it:

```sh
mossaic --demo             # a sample year: no account, no network
mossaic                    # yours
```

Press `?` in it for the keys, the mouse, and — the question everything else
depends on — **what your terminal turned out to be able to draw**.

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
- **A window under 112 columns** falls back too, and the chart says so under the
  legend, naming the number it wants.
- Either way, `mossaic --png chart.png` renders the same image to a file from
  any terminal at all.

</details>

## Why

Your contribution graph is probably the chart you look at most often and have
thought about least. It renders in someone else's tab, identically for everyone,
and reports one thing, after the fact: whether you showed up.

**mossaic turns it from a scoreboard into a canvas.** Contribution art is
usually done blind — generate thousands of commits, push, and find out. Here the
year is arithmetic before it is commits: what each lit day costs (every day's
shade is relative to your busiest one), where to place the text so existing days
do not punch holes in it, and whether it is still reachable in September.

Ask it about a year you have already been committing in and it says so, with the
placement that would salvage the most:

```
  VYNCINT cannot be drawn cleanly in 2026.
    61 day(s) inside the letters already have contributions, and
    nothing takes those away — the text would read with holes in it.
    --start-week 1 would leave 23 instead of 61.
```

**A field beats an empty graph.** Letters on nothing means not contributing on
the other 290 days of the year. `--background 1` draws the background as a shade
instead, so the art is one green against another and the year stays alive — the
image at the top of this page, at 590 commits for the whole of 2027.

GitHub's five shades are not evenly spaced, so mossaic measures the pair you
picked in CIELAB across all nine palettes GitHub ships and tells you what a
reader will actually see: `ΔE 35 at worst, clear`. Leave two levels between them
and it is legible everywhere; leave one and it is not.

**And it draws the result honestly**, so you can check the picture before making
a single commit. Primer tokens read from the stylesheets github.com serves.
github.com's own geometry — an 11px square on a 14px pitch, the hairline border
*over* the square's edge rather than inset into it — kept as a ratio and scaled
to whatever a character cell measures. Nothing is approximated for the
terminal's convenience: when GitHub restyles the graph, a test in here is what
should fail.

The whole art guide — placement, cost, saving the plan, running the tracker on a
schedule — is **[docs/ART.md](docs/ART.md)**; the Action that posts it to Slack,
Discord or email is **[action/README.md](action/README.md)**.

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
mossaic-art VYNCINT --year 2027 --background 1   # draw text on a field
mossaic-art --track                              # how far along the plan is
mossaic-art --track --today 2027-06-01           # what a day still to come will owe
mossaic-art --backfill --repo ../art             # commit just what the plan is short
mossaic-glyphs                                   # this terminal's fallback cells
```

From a checkout, `cargo run -- …` and `cargo run --bin mossaic-art -- …` are
the same commands.

## Keys

| Key | Action |
| --- | --- |
| `←` `→` / `h` `l` | previous / next week |
| `↑` `↓` / `k` `j` | previous / next day |
| `[` `]` · `PgUp` `PgDn` | previous / next year **that has contributions** (±1 when the current year is outside that set, e.g. after `--year 2010`) |
| `t` | jump to today |
| `Home` / `End` | first / last day in range |
| `?` | keys, mouse, and what this terminal can draw |
| `u` | type a different username (`Enter` load, `Esc` cancel) |
| `d` | cycle cell style — auto / pixel / rounded / snug / squares / grid / spaced / blocks / slim / compact. The legend names it, and says `auto:` while the chart is still choosing |
| `m` | mouse reporting on or off — off gives the terminal its own selection back |
| `r` | reload |
| `q` | quit — `Esc` cancels the username prompt and closes the help overlay, and does nothing else |

## Mouse

| | |
| --- | --- |
| hover a day | its tooltip, worded the way github.com words it, and a ring around the cell |
| click a day | move the cursor there, for terminals that report clicks but not motion |
| wheel | previous / next year |

## Without a terminal that draws pixels

Nothing is lost but the pixels. The same year as the image above, in block
sextants — same layout, same colours, same tooltip. The field means every day of
2027 is green, which is what the 365-day streak is:

```
┌ mossaic ───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│vyncint  ·  2027  ·  590 contributions in 2027                                                                                                                              │
│                                                                               ▐ 1 contribution on July 28th. ▌                                                             │
│    Jan               Feb         Mar         Apr         May            Jun         Jul       ▼ Aug            Sep         Oct            Nov         Dec                  │
│       🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛          │
│Mon    🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛          │
│       🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛          │
│Wed    🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛          │
│       🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛          │
│Fri 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛          │
│    🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛             │
│                                                                                                                                                                            │
│Fri, Dec 31 2027  ·  1 contribution                                                                                                                                         │
│Less 🬫🬛 🬫🬛 🬫🬛 🬫🬛 🬫🬛 More   ·   auto: rounded cells                                                                                                                          │
│365 active days  ·  365-day streak  ·  longest 365  ·  best Feb 8 (4)                                                                                                       │
│                                                                                                                                                                            │
│←→↑↓ day/week  ·  t today  ·  d cells  ·  m mouse off  ·  r reload  ·  q quit  ·  ? help  ·  preview                                                                        │
└────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

## How it works

Three things decide what you see, and the first is the only one you may need to
act on:

- **What your terminal can draw.** mossaic asks it — one write, five questions,
  one round trip — rather than sniffing `TERM`, which is wrong in both
  directions: it misses terminals it has not heard of and claims support inside
  tmux or ssh where the escape never arrives. `mossaic --capabilities` prints
  the answers; `--graphics` and `--cell` override them.
- **Which colours.** Every value is a Primer token read out of the stylesheets
  github.com serves, not transcribed. Light or dark follows the terminal's own
  background over `OSC 11`, the way a browser follows the OS; `--theme`
  overrides it. GitHub swaps the greens for a few days a year and so does
  mossaic, on the same dates — `--palette` asks for them out of season.
- **How a day is sized.** A day is two character columns wide and one row tall,
  and the square inside it keeps github.com's ratios — an 11px cell on a 14px
  pitch, rounded by 2px — scaled to whatever a character cell measures. That is
  what lets the month labels stay text while the cells are pixels.

**[docs/DESIGN.md](docs/DESIGN.md) is the long answer**: what was traded for
what, why the tooltip sits above the grid rather than over it, why the sixel
palette is degraded on purpose, and what the capability probe actually sends.

## Known limits

- **VTE terminals have sixel switched off.** GNOME Terminal and Ptyxis are built on
  VTE, which does implement sixel — but `VTE_SIXEL_ENABLED_DEFAULT` is `false`, the
  embedding application has to call `vte_terminal_set_enable_sixel`, and neither of
  them does. VTE has no kitty-graphics support at all. So those terminals answer no
  to both and get sextants, correctly.
- **Multiplexers do not pass either graphics protocol through**, so inside tmux or
  screen the probe comes back no and the chart falls to sextants. That is the right
  answer rather than a workaround: the escape would be swallowed either way.
- **Motion reporting is not universal.** Terminals that report clicks but not motion
  (Terminal.app among them) get click-to-select-a-day and no hover.
- **Sixel has no alpha.** Anti-aliased edges are composited against the background
  colour the terminal reported; where it reports none, GitHub's own canvas colour
  stands in, and on a terminal with a background image the corners will show a faint
  halo. Kitty, which takes an alpha channel, does not have the problem.
- **The tooltip sits above the grid, not over the day** — the one placement both
  protocols can live with.
- **Streaks stop at the year boundary**, and the *current* streak is the run ending
  on the day the year is read as of, so a year that has ended has none. `--today`
  moves that day.
- **A 53-week year needs a 112×19 window** for pixel or square cells, 165 columns
  for rounded corners, 166×27 for the bordered grid. Below that `Auto` drops to a
  borderless style, where a future day and a day off the end of the year both look
  blank; `d` still forces the others if you would rather they clipped.
- Private contributions appear only if the authenticated user can see them.

## Development

```sh
cargo test                  # everything, no network
cargo test --test smoke     # the real binary, in a real pty
cargo test --test pixels    # …in a pty that says it can draw pixels
cargo test -- --ignored     # the two that call the GitHub API
```

Three test layers, because they catch different things: in-process for anything
that is a function of its inputs — including the encoders, checked *against the
formats* by decoding sixel back into pixels — and two out-of-process layers where
[termlens] spawns the real binary in a real pty. That pty can be told which
terminal it is simulating, so the pixel path runs its own probe rather than being
handed the answer, and `tests/pixels.rs` asserts what actually goes out on the
wire.

[The file layout, what belongs in which test layer, and the two mistakes that
make tests flaky are in CONTRIBUTING.md](CONTRIBUTING.md).

## Contributing

Bug reports, PRs and — most useful of all — **terminal compatibility reports**
are welcome. A compatibility report starts with two lines:

```sh
mossaic --capabilities         # what your terminal answered
mossaic --png /tmp/chart.png   # the same image, rendered to a file
```

Comparing the file with your screen separates "the rasteriser is wrong" from
"the emission is wrong", which are different bugs.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the dev setup and testing policy, and
[docs/DESIGN.md](docs/DESIGN.md) before changing the pixel path. Security
reports: [SECURITY.md](SECURITY.md).

## License

Dual licensed under [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your
option, the Rust ecosystem's standard. Contributions are dual licensed the same
way unless you say otherwise.

[termlens]: https://crates.io/crates/termlens
