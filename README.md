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
- **A window under 112 columns** falls back too, and the chart says so under
  the legend rather than leaving you guessing.
- Either way, `mossaic --png chart.png` renders the same image to a file from
  any terminal at all.

</details>

## Without a terminal that draws pixels

A terminal without either protocol loses the pixels and nothing else — the whole
chart still draws, in block sextants:

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
serves. github.com's own geometry — an 11px square on a 14px pitch — kept as a
ratio and scaled to whatever a character cell measures. Real anti-aliased squares
over the kitty graphics protocol or sixel. Nothing is approximated for the
terminal's convenience: when GitHub restyles the graph, a test in here is what
should fail.

Moss is the metaphor and the mechanic: nothing happens on any one day, and after
a season the wall is green.

## Contents

- [Why](#why) · [Quickstart](#quickstart) · [Usage](#usage) · [Keys](#keys) · [Mouse](#mouse)
- [How it works](#how-it-works) — the probe, the pixels, the colours, the tooltip
- [Writing text into the graph](docs/ART.md) — and tracking it, day by day
- [The GitHub Action](action/README.md) — the tracker, on a schedule
- [Design notes](docs/DESIGN.md) · [Contributing](CONTRIBUTING.md) ·
  [Known limits](#known-limits)

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
mossaic --theme light            # instead of following the terminal's background
mossaic --palette winter         # GitHub's own seasonal colours
mossaic --no-mouse               # leave mouse reporting alone
```

```sh
mossaic-art VYNCINT --year 2027  # draw text into a graph — see docs/ART.md
mossaic-art --track              # how far along the plan is
mossaic-glyphs                   # what this terminal makes of the fallback cells
```

From a checkout, `cargo run -- …` and `cargo run --bin mossaic-art -- …` are
the same commands.

## Keys

| Key | Action |
| --- | --- |
| `←` `→` / `h` `l` | previous / next week |
| `↑` `↓` / `k` `j` | previous / next day |
| `[` `]` | previous / next year **that has contributions** (steps by ±1 when the current year is outside that set, e.g. after `--year 2010`) |
| `t` | jump to today |
| `Home` / `End` | first / last day in range |
| `?` | keys, mouse, and what this terminal can draw |
| `u` | type a different username (`Enter` load, `Esc` cancel) |
| `d` | cycle cell style — auto / pixel / rounded / snug / squares / grid / spaced / blocks / slim / compact |
| `m` | mouse reporting on or off — off gives the terminal its own selection back |
| `r` | reload |
| `q` / `Esc` | quit |

## Mouse

| | |
| --- | --- |
| hover a day | its tooltip, worded the way github.com words it, and a ring around the cell |
| click a day | move the cursor there, for terminals that report clicks but not motion |
| wheel | previous / next year |

## How it works

- **Data** — one GraphQL call per year for `contributionCalendar`. Levels come from
  GitHub's own `contributionLevel` quartiles rather than being re-derived from counts,
  so the shading matches github.com. `contributionYears` drives `[` / `]`, so year
  navigation only visits years that have data.
- **Fetching** — runs on a background thread and is tagged with a sequence number, so
  holding `[` down never lets a stale response overwrite a newer one.
- **The grid** — days are rebuilt into a Sunday-aligned grid keyed off the first day's
  weekday, which handles partial first and last weeks (Jan 1 is rarely a Sunday). The
  cursor is a `NaiveDate`, so moving is date arithmetic clamped to the visible range —
  `←`/`→` are ±7 days, `↑`/`↓` are ±1.
- **The whole year, always** — all 53 weeks are drawn, including days still to come.
  Those are kept but flagged `future`: they render as empty cells and are excluded
  from every statistic, so a half-finished year is not judged as if it were over.
  Without that flag the empty tail of the current year would read as a broken streak,
  and December would be indistinguishable from a quiet Tuesday in March.

### Asking the terminal, rather than guessing

Sniffing `TERM`, `KITTY_WINDOW_ID` and `TERM_PROGRAM` gets this wrong in both
directions: it misses terminals it has never heard of and claims support inside tmux
or ssh where the escape never arrives. So mossaic asks — one write, four questions,
one round trip:

| query | answer | tells us |
| --- | --- | --- |
| `APC _Gi=…,a=q` | `_Gi=…;OK` | it speaks the kitty graphics protocol |
| `OSC 11 ?` | `rgb:rrrr/gggg/bbbb` | the background colour, so light or dark is not a guess |
| `CSI 16 t` | `CSI 6;h;w t` | one character cell in pixels |
| `CSI c` | `CSI ?…;4;… c` | attribute 4 is sixel |

Device attributes come last and every terminal answers them, so that reply doubles
as the "everything that is coming has come" marker: the usual round trip is a
millisecond or two, not the 250 ms timeout. The reply is read from `/dev/tty` with
`O_NONBLOCK`, so a terminal that never answers costs the timeout and nothing else —
no thread left blocked on the keyboard. Only `OK` counts for kitty: a terminal that
knows the protocol but not that transmission medium answers `ENOTSUPPORTED`, and
would then be sent images it cannot draw.

The size of a character cell comes from `TIOCGWINSZ` first, since that is cheapest
and most widely answered, and from `CSI 16 t` or `CSI 14 t` otherwise. Without it an
image cannot be lined up with the labels around it, so mossaic falls back to text
rather than drawing a chart half a column out — or you can measure it yourself and
pass `--cell 10x20`, which is also what makes the pixel layout testable on a harness
that answers no to everything.

### Cells, drawn as pixels

github.com's cell is 11px on a 14px pitch, rounded by 2px, with half a pixel of
`--contribution-default-borderColor-0` around it. Those are the ratios mossaic
draws, scaled to whatever a character cell measures — so the geometry is GitHub's,
at the terminal's resolution.

A day gets **two columns and one row**. A character cell is about twice as tall as it
is wide, so two of them is the nearest thing to a square the grid offers, and it is
the same stride the two-column text styles use — which is what lets the month labels
stay text, the mouse hit-test stay integer arithmetic, and one hovered day be
repainted without redrawing the year. The square itself is `min(2 × cell_w, cell_h)`
across, so it stays square whatever the font's aspect ratio is.

Rounding is anti-aliased from the signed distance to the shape's edge: coverage is
`0.5 − distance`, clamped, one evaluation per pixel and no supersampling. At a 2px
radius on an 11px cell, the corner is a sub-pixel bite — which is exactly why it
needs to be drawn in fractions of a pixel rather than in thirds of a character.

The two protocols differ in what they can be told:

| | kitty | sixel |
| --- | --- | --- |
| transparency | a real alpha channel: corners blend into whatever the terminal's background is | one bit — "leave this pixel alone" — so edges are composited against the background colour the terminal reported |
| colours | 32-bit RGBA | 8-bit palette, and the components are *percentages* |
| placement | pinned to an exact number of columns and rows, so it cannot drift out of step with the labels | wherever the cursor is |
| the year, on the wire | 507 KB of RGBA → **8 KB** zlib'd | **45 KB**, run-length encoded |
| one hovered cell | ~240 bytes | ~490 bytes |
| layering | drawn under text (`z=-1`), so a tooltip can sit over the chart | pixels are pixels; text written over them wins for good |

Both are emitted after ratatui has drawn the frame, straight to the terminal, into
seven rows the renderer deliberately leaves blank. Nothing else ever writes there, so
ratatui's diff never has a reason to erase the image.

**A hovered day costs a cell, not a year.** Moving the pointer re-transmits one
two-column image. Kitty then only has to *delete* the ring to undo it, because the
year is still underneath; sixel has nothing underneath, so the cell is blanked back
to the terminal's own background — which is exact, where repainting the background we
guessed at would not be — and the day drawn again.

### Colour

Every colour is a Primer token, read out of the stylesheets github.com serves rather
than transcribed from memory:

| | level 0 | 1 | 2 | 3 | 4 |
| --- | --- | --- | --- | --- | --- |
| light | `#eff2f5` | `#aceebb` | `#4ac26b` | `#2da44e` | `#116329` |
| dark | `#151b23` | `#033a16` | `#196c2e` | `#2ea043` | `#56d364` |
| dimmed | `#2a313c` | `#1b4721` | `#2b6a30` | `#46954a` | `#6bc46d` |

Which of them applies follows the terminal's own background, the way a browser
follows the OS: `OSC 11` comes back, and anything brighter than half is a light
terminal. `--theme` overrides it.

GitHub also swaps the greens out for a few days a year — `data-holiday` in its
markup, `--contribution-halloween-*` in its CSS — and so does mossaic, on the same
dates. `--palette winter` and `--palette halloween` ask for them out of season;
`--palette default` turns the calendar off.

Without truecolor the five levels are **chosen rather than converted**. Every other
colour is mapped to the nearest of the 6×6×6 cube or the 24-step grey ramp, but the
cube has six steps per channel and GitHub's dark greens fall between two of them:
`#033a16` and `#196c2e` round to the same entry, and the nearest colour to either is
a *grey* — accurate to within a few units, flat on screen, and in the dimmed theme
not even in the right order. A legible ramp beats an accurate one that cannot be
read.

### The tooltip

github.com's wording exactly, ordinals included: `No contributions on August 17th.`,
`1 contribution on …`, `97 contributions on …`. It floats in the two rows above the
grid — a pill closed with half blocks, and a `▼` pointing down the hovered week's
column — rather than over the cells themselves. Over them it would be free in kitty,
where text draws above the image, and permanent damage in sixel, where the pixels a
character cell replaces do not come back. One position that works everywhere beats
two that do not.

The keyboard cursor keeps the detail line and a white ring; the pointer gets the
tooltip and a blue one. Both are on screen at once, because they are answering
different questions.

### Mouse, across terminals

`EnableMouseCapture` turns on button tracking, any-event motion (`1003`) and SGR
coordinates (`1006`). Motion is the mode that makes hovering work and the one most
likely to be missing — Terminal.app, some multiplexers — so a **click** hovers too,
and so does a drag. Where motion never arrives, clicking a day still names it.

Events are drained to the end of the queue every frame rather than one per frame:
motion reports arrive in floods, and answering them one frame at a time leaves the
tooltip trailing several cells behind the pointer.

Mouse reporting takes click-to-select away from the terminal, which is a real cost
for something you may want to copy out of, so `m` turns it off and on. A panic hook
turns it off too, so an unwind cannot leave the shell printing escape codes at every
click.

### Cell styles

`Auto` takes the most faithful that fits, checking both dimensions:

| style | look | needs |
| --- | --- | --- |
| `pixel` | real rounded squares, anti-aliased, gapped both ways | 110 × 16, and a graphics protocol |
| `rounded` | rounded cells, gap between columns, no outline | 163 × 16 |
| `snug` | rounded cells with no gap at all — fits where `rounded` will not | 110 × 16 |
| `squares` | sharp corners, but a gap in both directions | 110 × 16 |
| `grid` | square, shared borders | 164 × 24 |
| `spaced` | square, separated by a blank column | 163 × 16 |
| `blocks` | square, touching | 110 × 16 |
| `slim` | bordered, one column per day (tall rectangles) | 111 × 24 |
| `compact` | one column per day, no gap | 57 × 16 |

`d` overrides the choice and the active style is named beside the legend, with the
protocol when there is one — `pixel cells (sixel)`. `Auto` only ever returns `pixel`,
`rounded`, `squares` or `compact`; the rest are deliberate alternatives you ask for,
`snug` included — at 110 columns it is the same width as `squares` but trades both
gaps for the rounding, and which reads better is a matter of taste. A test sweeps
every width to 260 and asserts `Auto` stays out of that choice.

- **How the corners get rounded without pixels** — a cell is coloured a whole
  character at a time, so rounding needs sub-character resolution. Block sextants
  provide it: each character is a 2×3 grid of sub-blocks, so a pair of them is 4×3
  and all four corners can be shaved.

  ```text
    U+1FB2B  U+1FB1B         .##.
      .#       #.            ####     one day, two characters
      ##       ##            .##.
      .#       #.
  ```

  `squares` is the same idea one step coarser: the upper-half block `▀` is already
  square, since a character is about twice as tall as it is wide, and its unpainted
  lower half is the gap below. Both draw the fill only — github.com has no outline
  around a cell, which is what the earlier bordered styles got wrong.
- **Terminal support for sextants** — they are Unicode 13 and most monospace fonts
  lack them, but terminals that draw box characters themselves render them anyway:
  VTE 0.66+ (GNOME Terminal, Ptyxis), kitty, foot, WezTerm. Run
  `mossaic-glyphs` to see what yours does; if the rounded row looks wrong,
  `d` falls back to `squares`.
- **No text style is all three things** — github.com has rounded corners, a horizontal
  gap and a vertical gap. A sextant fills the whole character height, so anything
  rounded gives up the vertical gap; `▀` keeps both gaps but cannot be rounded,
  because half a character has no sub-rows left to shave. Getting all three from
  characters needs four sub-rows each — the Unicode 16 octants — and nothing here
  draws them. Pixels are the way out, which is the whole reason for `pixel` cells:

  | | rounded corners | gap between columns | gap between rows | exact radius |
  | --- | --- | --- | --- | --- |
  | `pixel` | yes | yes | yes | yes |
  | `rounded` | yes | yes | no | no |
  | `snug` | yes | no | no | no |
  | `squares` | no | yes | yes | — |

## Writing text into the graph

`mossaic-art` draws text as pixels on the calendar, emits the commits that
would light it up, and tracks how far along you are — including the part no
amount of committing fixes:

```
  VYNCINT cannot be drawn cleanly in 2026.
    19 day(s) inside the letters already have contributions, and
    nothing takes those away — the text would read with holes in it.
    --start-week 4 would leave 14 instead of 19.
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
| `tests/art_cli.rs` | the other two binaries, run as a user runs them |
| `action/` | the GitHub Action, and the guide to scheduling it |
| `docs/` | [the art guide](docs/ART.md), [design notes](docs/DESIGN.md), [releasing](docs/RELEASING.md) |

## Tests

```sh
cargo test                                 # everything below, no network
cargo test --lib                           # grid maths, colour, encoders, art, PNG
cargo test --test smoke                    # the real binary in a real pty
cargo test -- --ignored --nocapture        # the two that call the GitHub API
cargo test snapshot -- --nocapture         # print rendered frames as text
```

Two layers, because they catch different things.

**In process.** `cargo test --lib` renders through ratatui's `TestBackend`, which
covers layout, hit-testing and the tooltip, and calls the rasteriser and both
encoders directly. Those are tested against the formats rather than against
themselves: the sixel is decoded back into pixels and compared to what the
rasteriser drew, and the kitty transmission is un-base64'd, inflated and compared
byte for byte. The PNG is parsed back, CRCs checked, and its pixels compared too.

**Out of process.** [termlens] spawns the real binary in a real PTY, renders its
output with a VT emulator and hands back a screen grid — everything `TestBackend`
cannot reach: the event loop, the PTY, mouse encoding, and the escapes mossaic
writes around ratatui rather than through it.

```rust
let mut t = Terminal::builder()
    .size(176, 34)
    .env_clear()                        // hermetic: no host env leaks in
    .background_rgb(0xff, 0xff, 0xff)   // …and OSC 11 answers white
    .args(["--file", "art/vyncint-2027.json"])
    .spawn(env!("CARGO_BIN_EXE_mossaic"))?;

let screen = t.wait_frame(loaded)?;
assert_eq!(legend_colours(&screen), primer_light());  // a light terminal, no flag
```

That is the theme test: nothing is passed on the command line, the terminal simply
answers white to `OSC 11`, and the assertion is the Primer light scale read back off
the rendered screen. The same harness drives the mouse — `drag` reports motion,
which is the event a hover is — and asserts the tooltip lands two rows above the
grid, that `m` hands mouse reporting back, and that a resize re-chooses the cell
style.

**`wait_frame`, not `wait_until`.** mossaic brackets every repaint in DEC 2026
synchronized updates, so the predicate only ever sees complete frames. Waiting on
content alone matches a frame half-applied — the header already carrying the new
total while the legend below it is still the one from the loading screen — and the
resulting test fails three times in four, for reasons that look like magic. The
brackets are worth having anyway: a terminal that understands them shows the text
and the images of a frame together, instead of a chart that arrives without its
cells.

The two tests that need the network are `#[ignore]`d, so `cargo test` is hermetic
and offline. They are the only ones that touch `gh`.

[termlens]: https://crates.io/crates/termlens

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
- Streaks are computed within the displayed year, so one spanning New Year is clipped
  at the year boundary.
- Pixel and square cells need about 112 columns, the bordered grid about 115×26.
  Below that `Auto` drops to a borderless style, where a future day and a day off the
  end of the year both look blank; `d` still forces the others if you would rather
  they clipped.
- Private contributions appear only if the authenticated user can see them.

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
