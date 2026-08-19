# Contributing to mossaic

Thanks for looking. Bug reports, terminal compatibility reports and PRs are all
welcome — the compatibility ones especially, since mossaic's whole job is
getting along with terminals none of us has.

## 1. Dev setup

```sh
git clone https://github.com/vyncint/mossaic
cd mossaic
cargo test          # 50-odd tests, hermetic and offline
cargo run           # the chart, for whoever `gh` is logged in as
```

You need [the GitHub CLI](https://cli.github.com) authenticated (`gh auth
login`) to run the chart against a real account, but **not** to run the tests:
the two that call the API are `#[ignore]`d.

The toolchain is pinned to stable in `rust-toolchain.toml`, so `cargo fmt` and
`cargo clippy` give the same answers here as in CI. That file is not the MSRV —
that is `rust-version` in `Cargo.toml`, verified by the `msrv` job.

## 2. Project layout

| Path | What lives there |
| --- | --- |
| `src/lib.rs` | the library the three binaries share |
| `src/main.rs` | `mossaic`: arguments, terminal setup, event loop |
| `src/bin/` | `mossaic-art` and `mossaic-glyphs` |
| `src/plan.rs` | comparing a plan with what was actually contributed |
| `src/{calendar,github}.rs` | the day grid and the GraphQL call behind it |
| `src/{primer,term}.rs` | Primer's colours; what the terminal says it can do |
| `src/graphics.rs` | the rasteriser, the kitty and sixel encoders, the painter |
| `src/{ui,app}.rs` | rendering and layout; state, keys and mouse |
| `src/{art,png}.rs` | the 5×5 font and its costing; a small PNG encoder |
| `src/render_tests.rs` | in-process tests: layout, colour, encoders, art, PNG |
| `tests/smoke.rs` | out-of-process tests: the real binary in a real PTY |
| `docs/ART.md` | drawing text into a graph, and tracking the plan |
| `docs/DESIGN.md` | why the pixel path is shaped the way it is |
| `docs/RELEASING.md` | how a version gets cut |

## 3. Testing policy

Every behavioural change needs a test, and which of the two layers it belongs in
is usually obvious:

- **In process** (`src/render_tests.rs`) for anything that is a function of
  inputs: layout maths, hit-testing, palettes, the encoders, the art font.
  Encoders are tested against the formats, not against themselves — the sixel is
  decoded back into pixels and compared to what the rasteriser drew.
- **Out of process** (`tests/smoke.rs`, through
  [termlens](https://crates.io/crates/termlens)) for anything that involves the
  event loop, the PTY, or escapes written around ratatui rather than through it.

Two rules that are easy to get wrong:

- **Wait on frames, not on content.** mossaic brackets repaints in DEC 2026
  synchronized updates, so `wait_frame` sees only complete ones. `wait_until`
  re-checks on every chunk and will happily match a frame half-applied — a test
  written that way fails about three runs in four, for reasons that look like
  magic.
- **Keep tests hermetic.** `env_clear()`, `--file art/vyncint-2027.json`, no
  network. If a change genuinely needs the API, mark it `#[ignore]` and say so.
- **Pin `--today` in anything that reads a report.** The clock is an input to
  `mossaic-art --track`, and an assertion that does not pin it is an assertion
  about the day it was written. Two of them were: one claimed whatever day CI
  saw was a lit day inside the letters, and it passed for exactly as long as
  that was true. `--today 2026-08-19` against `art/vyncint-2026.json` is the
  fixture pair to reach for. The `test` job runs the CLI suite a second time
  under `TZ=Pacific/Kiritimati`, which is a backstop rather than the rule.

Anything user-visible in the terminal deserves a look in a real one too. The
harness is a VT emulator; it agrees with terminals about text and knows nothing
about pixels.

If a change touches frames, waits or the event loop, run the flake hunter before
asking for review — it is the whole suite, a hundred times, on both OSes:

```sh
gh workflow run stress.yml            # or, locally:
for i in $(seq 20); do cargo test --release || break; done
```

## 4. Commit conventions

Short imperative subject, body explaining *why*. Reference issues as
`Closes #123`. Squash merges, so the PR title becomes the commit subject.

**No AI attribution.** AI assistance is welcome here — use whatever helps. AI
*attribution* is not: no `Co-Authored-By` trailer naming an assistant, no
"Generated with" watermark, no bot identity as author or committer. Whoever
opens the pull request is the author of record, and the history should say so.

This is enforced, not requested: [`commit-policy.yml`](.github/workflows/commit-policy.yml)
runs [`check-no-ai-attribution.sh`](.github/scripts/check-no-ai-attribution.sh)
over every commit in a pull request. If it fails, drop the trailer and
force-push:

```sh
git commit --amend        # the last commit
git rebase -i main        # several, marking each `reword`
git push --force-with-lease
```

Agents that read repository settings are also told up front: `.claude/settings.json`
turns co-author trailers off. That is a courtesy, not the boundary — settings
files are advisory and CI is what actually holds.

## 5. Pull requests

A pull request is the only way anything lands: `main` rejects direct pushes —
the maintainer's too — and merges are squashes that need the `required-green`
and `commit-policy` checks green first.

1. Open an issue first for anything larger than a fix — especially anything
   that changes what the chart looks like.
2. `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`
   and `cargo test` before pushing.
3. Update `CHANGELOG.md` under `[Unreleased]` for user-facing changes.
4. Fill in the PR checklist.

## 6. Terminal compatibility reports

These are worth their own note, because they are the most useful thing an
outside contributor can send. If the chart looks wrong on your terminal:

1. Run `mossaic --capabilities` and include its output verbatim.
2. Say which terminal and version, and whether `--graphics text` looks right.
3. If pixels are involved, `mossaic --png /tmp/chart.png` renders the same
   image to a file — comparing that with your screen separates "the rasteriser
   is wrong" from "the protocol emission is wrong", which are different bugs.

## 7. Adding a glyph to the font

The `mossaic-art` binary draws text with a 5×5 font in `src/art.rs`. It has A–Z, 0–9,
space, `-` and `.` — everything else is a gap someone can fill, and filling one
is a single table entry:

```rust
('!', ["..#..", "..#..", "..#..", ".....", "..#.."]),
```

See what you made:

```sh
cargo run --bin mossaic-art -- --font          # every glyph, side by side
cargo run --bin mossaic-art -- "HI!" --year 2027   # in a year, at the real size
```

Three rules are checked **when the crate compiles**, so a mistake is a build
failure with the reason rather than a panic for whoever draws it first:

1. exactly five rows of exactly five characters,
2. only `#` and `.`,
3. no character twice.

Two more are checked by `cargo test --lib`, because a compiler cannot judge
them: a glyph must draw *something* and not everything, and no two characters
may draw the same pixels — two that do are indistinguishable once they are on
the graph.

Beyond that it is a judgement call, and the bar is legibility at five pixels
square next to its neighbours. `--font` is the argument to make in the PR: paste
what it prints.

Two things to know before proposing something bigger:

- **Width is uniform on purpose.** `6N - 1` describes the width of any text, and
  the placement, the centring and the eight-character limit all rest on it. A
  variable-width font is a real design change, not a glyph.
- **The table is uppercase**, and lookup tries the character as written before
  folding — so lowercase glyphs can be added later without touching anything
  else.

## 8. Code style

The code is commented for the reader who wonders *why*, not *what*. Comments
that explain a trade-off, a protocol quirk, or a decision that looks arbitrary
earn their place; comments that restate the line below them do not. Public items
carry documentation — `missing_docs` is a warning and CI runs clippy with
`-D warnings`.
