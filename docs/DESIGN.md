# Design

Why mossaic is shaped the way it is. The README says what it does; this says
what was traded for what, so that changing it is a decision rather than a
discovery.

## 1. The north star

The chart should look like the one on github.com. Not "like a contribution
graph" — like *that* one: its colours, its geometry, its wording. Everything
below follows from taking that literally.

Two consequences worth stating:

- **Colours are read, not chosen.** Every value in `primer.rs` comes from the
  stylesheets github.com serves (`--contribution-default-bgColor-0..4` and the
  tokens around them). When GitHub restyles the graph, the palette test is what
  should fail.
- **Geometry is a ratio, not a pixel count.** github.com draws an 11px square
  on a 14px pitch with a 2px radius and half a pixel of border. mossaic keeps
  the ratios and scales them to whatever a character cell measures, so the
  proportions survive any font size.

## 2. Layers

```
github.rs   one GraphQL call            ->  Calendar
calendar.rs Sunday-aligned grid, stats  ->  Calendar
primer.rs   which colours               ->  Palette
term.rs     what the terminal can do    ->  Caps
ui.rs       what goes where             ->  Layout + text
graphics.rs what the pixels are         ->  Image -> kitty | sixel
app.rs      state, keys, mouse          ->  Scene
```

`ui.rs` draws text through ratatui and hands back a [`Layout`]. `app.rs` turns
that into a `Scene`, and `graphics.rs` makes the screen match it. Nothing in
`graphics.rs` knows about years or keys; nothing in `ui.rs` knows about
protocols.

Three decisions in the data layer are worth stating, because none of them is
visible from the diagram:

- **Levels come from GitHub, not from arithmetic.** The API returns a
  `contributionLevel` per day and that is what is drawn. Re-deriving it from
  counts would be one more place to disagree with github.com.
- **Fetches carry a sequence number.** They run on a background thread, and
  holding `[` down starts several; without the number a slow early response
  could overwrite a newer one and show the wrong year.
- **Days still to come are kept, and flagged.** They draw as empty cells and
  are excluded from every statistic. Without the flag, the empty tail of the
  current year would read as a broken streak, and December would be
  indistinguishable from a quiet Tuesday in March.

## 3. The cell contract

**A day is two columns wide and one row tall.** Everything else falls out of
that:

- A character cell is about twice as tall as it is wide, so two of them is the
  nearest thing to a square the grid offers.
- The image therefore lands on exact character boundaries, which is what lets
  the month labels and weekday gutter stay *text* while the cells are pixels.
- The mouse hit-test is integer division, shared by every style — pixel cells
  and the two-column text styles have the same stride.
- One day can be repainted on its own, because one day is one addressable
  rectangle of character cells.

The square inside that pitch is `min(2 × cell_w, cell_h) × 11/14`, centred: the
pitch is only square when a cell is exactly twice as tall as it is wide, and
taking the smaller side keeps the day square on fonts where it is not.

Rounding is anti-aliased from the signed distance to the shape's edge —
coverage is `0.5 − distance`, clamped, one evaluation per pixel, no
supersampling. At a 2px radius on an 11px cell the corner is a sub-pixel bite,
which is exactly why it has to be drawn in fractions of a pixel rather than in
thirds of a character.

## 4. Two protocols, one image

The rasteriser produces straight-alpha RGBA once. What differs is the wire:

| | kitty | sixel |
| --- | --- | --- |
| transparency | a real alpha channel; corners blend into any background | one bit — "leave this pixel alone" — so edges are composited against the background the terminal reported |
| colour | 32-bit RGBA | 8-bit palette, components in *percent* |
| placement | pinned to an exact number of columns and rows (`c`/`r`), so it cannot drift out of step with the labels | wherever the cursor is |
| a year | 507 KB raw → **8 KB** zlib'd | **45 KB**, run-length encoded |
| one cell | ~240 bytes | ~490 bytes |
| layering | `z=-1`: under text, over the background | pixels are pixels |

Sixel's palette is the one place the image is degraded on purpose: anti-aliasing
a year produces far more shades than 256 registers hold, so blends are snapped
to a coarser grid until they fit, starting at full precision. See `indexed()`.

## 5. The painter is a diff

Redrawing a year for every hovered day would be 45 KB of sixel at 12 frames a
second. So `Painter` holds what is on screen — the base image's identity, and
at most two marked cells — and writes only what changed:

- **Base**: re-transmitted when the year, user, palette, position or cell size
  changes. Keyed by a hash, so an unchanged frame writes nothing at all.
- **Marks**: the keyboard cursor and the hovered day, one image each, addressed
  by their own ring so a caller cannot get the slots out of order.

Undoing a mark differs by protocol, and this is the asymmetry that shapes the
code: **kitty deletes**, because the year is still underneath; **sixel
repaints**, because nothing is. Sixel's repaint blanks the two character cells
first — the ring is drawn half in the gap between cells, so repainting only the
square would leave its outer edge behind, and blanking restores the terminal's
real background rather than the one we guessed at.

## 6. Asking the terminal

`TERM` sniffing gets this wrong in both directions: it misses terminals nobody
has heard of, and claims support inside tmux or ssh where the escape never
arrives. So `term::probe` asks — one write, four questions, one round trip:

| query | answer | tells us |
| --- | --- | --- |
| `APC _Gi=…,a=q` | `_Gi=…;OK` | it speaks the kitty graphics protocol |
| `OSC 11 ?` | `rgb:…` | the background, so light or dark is not a guess |
| `CSI 16 t` | `CSI 6;h;w t` | one character cell in pixels |
| `CSI c` | `CSI ?…;4;… c` | attribute 4 is sixel |

Device attributes come last and **every** terminal answers them, so that reply
doubles as the "everything that is coming has come" marker. The read is from
`/dev/tty` with `O_NONBLOCK`, so a terminal that never answers costs the
timeout and nothing else — no thread left blocked holding the keyboard, which
is what a naive blocking read would do to the next keypress.

Only `OK` counts for kitty. A terminal that knows the protocol but not that
transmission medium answers `ENOTSUPPORTED`, and would then be sent images it
cannot draw.

**Without a cell size there are no pixels.** An image that cannot be lined up
with the labels around it is worse than no image, so a terminal that will not
say gets text cells — or you measure it yourself and pass `--cell 10x20`.

## 7. The tooltip sits above the grid

github.com floats its tooltip over the cells. mossaic puts it in the two rows
*above* them, and this is a deliberate loss.

Over the cells is free on kitty: images are drawn under text, so a tooltip
covers the chart and moving away reveals it again. On sixel it is permanent
damage — a character cell written over pixels does not give them back, and
repairing it means re-emitting a slice of the image on a frame boundary that
does not exist. One placement that works everywhere beat two that did not.

What is kept: the wording is github.com's exactly, ordinals included, and a `▼`
points at the hovered week's column so the association is unambiguous.

## 8. Two pointers, not one

github.com has hover. mossaic has hover *and* a keyboard cursor, because a
terminal chart that needs a mouse is a worse chart. They are different things
and are drawn differently:

- the **cursor** gets `--fgColor-default` and owns the detail line,
- the **pointer** gets `--fgColor-accent` and owns the tooltip,
- and when they land on the same day the pointer wins, because two rings on one
  cell would draw over each other and erasing one would take the other with it.

Motion, drag and click all hover. `1003` motion reporting is the mode most
likely to be missing (Terminal.app, some multiplexers), and a click is the
event those terminals do send.

Events are drained to the end of the queue every frame rather than one per
frame: motion reports arrive in floods, and answering them one at a time
leaves the tooltip trailing several cells behind the pointer.

Mouse reporting takes click-to-select away from the terminal, which is a real
cost for something you may want to copy out of — so `m` turns it off and on,
and a panic hook turns it off as well, because an unwind must not leave the
shell printing escape codes at every click.

## 9. Colour, degraded

The five shades, as github.com serves them:

| | level 0 | 1 | 2 | 3 | 4 |
| --- | --- | --- | --- | --- | --- |
| light | `#eff2f5` | `#aceebb` | `#4ac26b` | `#2da44e` | `#116329` |
| dark | `#151b23` | `#033a16` | `#196c2e` | `#2ea043` | `#56d364` |
| dimmed | `#2a313c` | `#1b4721` | `#2b6a30` | `#46954a` | `#6bc46d` |

The five levels are the one place a converted colour is not good enough. The
256-colour cube has six steps per channel, and GitHub's dark greens fall between
two of them: `#033a16` and `#196c2e` round to the same entry, and the nearest
colour to either is a *grey* — accurate to within a few units, flat on screen,
and in the dimmed theme not even monotonic. So the levels are **chosen** for
that mode and everything else is converted. A legible ramp beats an accurate one
that cannot be read.

## 10. Art is two shades, and the gap between them is measured

Contribution art started as "lit or not", which quietly costs you the rest of
the year: keeping the letters visible means **not contributing** on the other
290 days. So a background can be drawn as a shade instead of as nothing, and
the art becomes one green against another.

That turns a boolean into a colour comparison, and colour comparisons need a
metric. RGB distance is not one — `#033a16` and `#196c2e` differ by 22 in a
single channel and look nearly identical — so `Rgb::separation` works in CIELAB
and reports CIE76 ΔE.

The threshold is measured, not chosen. Across the nine palettes GitHub ships
(three appearances × three seasons):

- **adjacent levels fall as low as ΔE 10.8** — light theme, levels 2 and 3,
- **levels two or more apart never fall below ΔE 35.4.**

Hence the rule the CLI enforces and the docs repeat: leave two levels between
the field and the ink. `Shades::worst` reports the worst palette rather than
the current one, because art is drawn once and read by everyone — and for a
few weeks a year GitHub switches everybody to a seasonal ramp regardless.

Two failures are refused rather than drawn, because neither produces art:
a field at or above the ink, and a commit count too small for the year to hold
two shades at all. That second one is not obvious: a shade is a fraction of the
year's *peak*, so a year whose busiest day is 1 has exactly two shades in it.
`Shades::min_peak` is why a background forces the peak to at least 4, where the
counts 1–4 land on levels 1–4 exactly.

A background day is then a **band** rather than a target — a floor to reach and
a ceiling to stay under — and the two ends fail differently. Below the floor is
debt, which contributing fixes. Above the ceiling is damage, which nothing
does: it is the same loss as a lit day inside a letter, arrived at by being the
wrong colour instead of by being lit at all.

## 11. Frames

Every repaint is bracketed in DEC 2026 synchronized updates. mossaic writes a
frame in two parts — ratatui's text, then the images straight after it — and a
terminal that understands the brackets shows them together instead of a chart
that arrives without its cells.

It also makes the tests exact: `wait_frame` sees only complete repaints. Waiting
on content alone matches a frame half-applied, which fails about three runs in
four for reasons that look like magic.

## 12. Testing

Two layers, because they catch different things.

**In process** covers what is a function of its inputs: layout, hit-testing,
palettes, the art font, and both encoders — tested against the formats rather
than against themselves. The sixel is decoded back into pixels and compared to
what the rasteriser drew; the kitty transmission is un-base64'd, inflated and
compared byte for byte; the PNG is re-parsed and its CRCs checked.

**Out of process** ([termlens]) spawns the real binary in a real PTY and asserts
on the rendered screen: the event loop, mouse encoding, and the escapes written
around ratatui rather than through it. The theme test is the shape to copy — no
flags, the terminal simply answers white to `OSC 11`, and the assertion reads
Primer's light scale back off the screen.

What the harness cannot reach is the image *bytes*: its emulator consumes DCS
and APC strings, as a terminal that draws no pixels would. That is why the
encoders are covered in process, and why `--png` exists — it renders the same
image to a file, which separates "the rasteriser is wrong" from "the emission is
wrong".

## 13. Non-goals

- **A general image library.** `png.rs` writes one colour type with one filter,
  because that is what a chart needs.
- **Terminals that draw pixels but will not say so.** `--graphics` and `--cell`
  are the escape hatch; guessing is not.
- **Writing to GitHub.** `mossaic-art` emits commits into a local repository and prints
  the push command. It never pushes.

[`Layout`]: ../src/ui.rs
[termlens]: https://crates.io/crates/termlens
