# Contributing to mossaic

Thanks for looking. Bug reports, terminal compatibility reports and PRs are all
welcome — the compatibility ones especially, since mossaic's whole job is
getting along with terminals none of us has.

> **These four projects share one contributor pattern** — the same commit
> rules, the same DCO, the same AI policy, the same CI and release shape:
> [termlens](https://github.com/vyncint/termlens),
> [mossaic](https://github.com/vyncint/mossaic),
> [launchbound](https://github.com/vyncint/launchbound),
> [reconverge](https://github.com/vyncint/reconverge). Learn it once.

## 1. Dev setup

```sh
git clone https://github.com/vyncint/mossaic
cd mossaic
cargo test          # 150-odd tests, hermetic and offline
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
| `tests/pixels.rs` | the same, in a PTY that answers the graphics probe |
| `docs/ART.md` | drawing text into a graph, and tracking the plan |
| `docs/DESIGN.md` | why the pixel path is shaped the way it is |
| `docs/RELEASING.md` | how a version gets cut |

## 3. Testing policy

Every behavioural change needs a test, and which of the three layers it belongs
in is usually obvious:

- **In process** (`src/render_tests.rs`) for anything that is a function of
  inputs: layout maths, hit-testing, palettes, the encoders, the art font.
  Encoders are tested against the formats, not against themselves — the sixel is
  decoded back into pixels and compared to what the rasteriser drew.
- **Out of process** (`tests/smoke.rs`, through
  [termlens](https://crates.io/crates/termlens)) for anything that involves the
  event loop, the PTY, or escapes written around ratatui rather than through it.
- **Out of process, with pixels** (`tests/pixels.rs`) for anything that depends on
  the terminal *answering* the capability probe. Declare what is being simulated —
  `.graphics(Graphics::Kitty).cell_size(9, 19)` — rather than forcing the outcome
  with `--graphics`/`--cell`, so the probe, the fallbacks and the auto choice all
  run. `Screen::graphics()` then reports what went out, by protocol and in bytes,
  which is how a claim about the wire gets a check behind it. What it cannot tell
  you is what the image *looks* like; that stays with the in-process encoder tests
  and `--png`.

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
harness is a VT emulator: it agrees with terminals about text, and about *whether*
an image was sent and how large it was, but it does not draw one — so nothing in
it can tell you the picture is right.

If a change touches frames, waits or the event loop, run the flake hunter before
asking for review — it is the whole suite, a hundred times, on both OSes:

```sh
gh workflow run stress.yml            # or, locally:
for i in $(seq 20); do cargo test --release || break; done
```

## 4. Commit conventions

We use [Conventional Commits](https://www.conventionalcommits.org/):
`feat:`, `fix:`, `docs:`, `test:`, `ci:`, `chore:`, `refactor:`, `perf:` —
scope optional (`feat(ui): …`). Subject line: imperative mood,
≤ 72 characters.

## 5. Developer Certificate of Origin (DCO)

Every commit must be signed off:

```sh
git commit -s
```

This appends `Signed-off-by: Your Name <you@example.com>` and certifies you
wrote the change or otherwise have the right to submit it under the project
license — the [Developer Certificate of Origin](https://developercertificate.org),
the same lightweight model the Linux kernel uses. The sign-off email must
match the commit author email; CI enforces this on every commit in a PR.

**There is no CLA. DCO only.** You keep your copyright.

Forgot to sign off? `git commit --amend -s` for the last commit, or
`git rebase --signoff main` for a whole branch, then force-push.

One exception, and it is GitHub's rather than ours: a pull request
**squash-merged through the web UI** has its author email rewritten by GitHub
*after* the sign-off was written, so an exact match is impossible by
construction. Such a commit must carry a sign-off, but is not matched against
an author it did not choose. The commits that went into the PR were already
checked, address and all, on the branch.

GitHub also *writes* that message, and it drops the trailers of the commits it
squashed whenever the branch contained a merge commit — pressing **Update
branch** is enough to cause it. The merge then lands on main carrying no
sign-off, and main is linear and non-fast-forward, so it cannot be repaired.
The check therefore exempts exactly one commit — the tip of a push to main,
which can only get there through a pull request that was already checked
strictly. **Keep your branch up to date by rebasing, not merging:**

```sh
git fetch origin && git rebase origin/main
git push --force-with-lease
```

That also matches what main requires: linear history, so a merge commit on
your branch is only ever going to be squashed away.

## 6. AI tooling policy

**AI assistance is welcome here — use whatever helps.** Every one of these
projects was built with it. There is an [AGENTS.md](AGENTS.md) briefing coding
agents on the layout, the commands, and the house style.

**AI attribution is not welcome.** No `Co-Authored-By` trailer naming an
assistant, model or vendor; no "Generated with …" footer; no robot emoji; no
bot identity as author or committer. Whoever opens the pull request is the
author of record, takes responsibility under the DCO, and the history should
say so — a tool cannot certify the DCO, which is the whole point of it.

This is enforced, not requested: `commit-policy.yml` runs
[`check-no-ai-attribution.sh`](.github/scripts/check-no-ai-attribution.sh) and
[`check-dco.sh`](.github/scripts/check-dco.sh) over every commit in a pull
request. Run them yourself first — both take a range:

```sh
.github/scripts/check-dco.sh main..HEAD
.github/scripts/check-no-ai-attribution.sh main..HEAD
```

If a check fails, rewrite the message rather than arguing with it:

```sh
git commit --amend            # the last commit
git rebase -i main            # several, marking each `reword`
git push --force-with-lease
```

`.claude/settings.json` turns co-author trailers off for agents that read
repository settings. That is a courtesy; the check in CI is the boundary.
Contributions authored *by* an autonomous account are not accepted.

## 7. PR flow

- Branch from `main`; name branches `feat/…`, `fix/…`, `docs/…`, `ci/…`.
- PRs are **squash-merged** — keep the PR title in Conventional Commit form,
  since it becomes the commit subject on `main`. Branches are deleted on merge.
- Required checks: `required-green` (fmt, clippy, tests on Ubuntu + macOS + Windows, MSRV, docs, cargo-deny, zizmor), plus `commit-policy` (DCO + attribution). All
  must pass before merge; direct pushes to `main` are blocked by a ruleset.
- **Every change lands with a test, and the test must be able to fail.** If
  you add a guard, break it once and watch it go red before you commit.
- **Say what you did not do.** A PR that lists what it left out and why is
  worth more than one implying completeness. An honest gap is cheap; a false
  claim is expensive.
- **Contributing from a fork?** Two things are normal. On your first PR the
  workflows wait for a maintainer to approve them — GitHub's standard
  first-time-contributor safeguard, nothing you did wrong. And when
  `commit-policy` fails on a fork PR it cannot post its explanatory comment
  (fork PRs get a read-only token); the job log carries the full explanation,
  including the offending commit and the command that fixes it.
- Review: expect actionable review within a few days. Small, focused PRs get
  reviewed faster. Update `CHANGELOG.md` under `[Unreleased]` for any
  user-facing change.

## 8. Release process

Releases are cut by maintainers only; the checklist lives in
[docs/RELEASING.md](docs/RELEASING.md).

## 9. Terminal compatibility reports

These are worth their own note, because they are the most useful thing an
outside contributor can send. If the chart looks wrong on your terminal:

1. Run `mossaic --capabilities` and include its output verbatim.
2. Say which terminal and version, and whether `--graphics text` looks right.
3. If pixels are involved, `mossaic --png /tmp/chart.png` renders the same
   image to a file — comparing that with your screen separates "the rasteriser
   is wrong" from "the protocol emission is wrong", which are different bugs.

## 10. Adding a glyph to the font

The `mossaic-art` binary draws text with a 5×5 font in `src/art.rs`. It has A–Z,
0–9, punctuation, and a set of named shapes — everything else is a gap someone
can fill, and filling one is a single table entry:

```rust
('^', ["..#..", ".#.#.", ".....", ".....", "....."]),
```

A **shape** is two entries, because a symbol nobody can type is a shape nobody
can draw. The glyph goes in `FONT` keyed by its character, and its name — or
names — go in `SHAPES` beside it:

```rust
('\u{2601}', ["..##.", ".####", "#####", ".....", "....."]),   // FONT
("cloud", '\u{2601}'),                                          // SHAPES
```

That is what makes `mossaic-art ":cloud:"` work. If the shape has a common
emoji spelling, one line in `FOLD` points it at the same glyph, so someone who
pastes 🌥 gets what they meant rather than an error about a codepoint.

See what you made:

```sh
cargo run --bin mossaic-art -- --font                # every glyph, side by side
cargo run --bin mossaic-art -- "HI!" --year 2027     # in a year, at the real size
cargo run --bin mossaic-art -- ":cloud:" --year 2027 # a shape, at the real size
```

Six rules are checked **when the crate compiles**, so a mistake is a build
failure with the reason rather than a panic for whoever draws it first:

1. exactly five rows of exactly five characters,
2. only `#` and `.`,
3. no character twice,
4. a shape name names a character the font actually has,
5. shape names are lowercase ASCII, and no name twice — `:STAR:` has to fold to
   one thing, because every plan is stored uppercased,
6. nothing folds to a character the font lacks, and a folded character is not
   itself in the font — one bitmap per shape.

Two more rules are checked by the tests rather than the compiler, because they
are judgement rather than shape: no glyph may be blank or a solid block (either
reads as no character at all), and no two glyphs may draw the same pixels. The
README's list of what the font can draw is checked against the font too, so a
new glyph that is not documented fails the suite.

## 11. Adding a pixel-art template

> [#57](https://github.com/vyncint/mossaic/issues/57) is the same thing written
> as an invitation, with a worked example and a list of ideas to claim. This
> section is the reference; that issue is the walkthrough.

A **template** is a whole-year picture: seven rows by up to 53 columns, in any
of GitHub's five shades. Contributing one is dropping a file into
`art/templates/` — there is no list to edit, because `build.rs` finds every
`.art` file in that directory and embeds it when the crate compiles. If your
file is there and the tests pass, `--template <stem>` works.

The file is a header and seven rows:

```
# name: Dragon
# author: @vyncint
# description: A serpentine dragon coiling across the whole year, in all five shades

00000000000000000000033333333333333000003300000000000
00000000000000000333344444444444444333344400000000000
...
```

- **Seven rows**, one per weekday, Sunday first. Not five: a template uses the
  weekend, which is the whole difference between a picture and text.
- **Up to 53 columns.** A row shorter than the widest is padded with level 0 on
  the right, so an editor that strips trailing whitespace cannot corrupt a
  picture whose last column is dark.
- **Shades are `0` to `4`**, or the blocks ` ░▒▓█` if you would rather read the
  file as a picture. The two may be mixed; they mean the same thing.
- **Lines starting with `#` are comments.** `name`, `author` and `description`
  are read; anything else is ignored, so a `# note:` line is fine.
- **A line of length zero is skipped**, so the header can breathe. A row of
  *spaces* is not empty — it is seven dark days, and it is kept.

The file name is what people type after `--template`, so it must be lowercase
letters, digits and dashes.

Draw it by hand rather than by counting characters:

```sh
cargo run --bin mossaic-art -- --draw -o art/templates/mine.art
cargo run --bin mossaic-art -- --template mine --year 2027   # in a year, priced
cargo run --bin mossaic-art -- --list-templates              # as others will see it
```

`--draw` is the editor: arrows or `hjkl` to move, `0`–`4` to paint, space to
cycle a cell, the mouse to paint directly, `u` to undo, `s` to save. It shows
what the picture would cost in commits as you draw it, and how well its
shades will separate for a reader on any palette GitHub ships.

Five rules are checked by the test suite, so a mistake is a failing test with
the reason rather than a broken `--template` for whoever tries it first:

1. exactly seven rows, and a width between 1 and 53,
2. only shade characters,
3. a `# name:` and a `# description:` — a template with neither is one nobody
   can tell apart in a listing,
4. no two templates with the same title,
5. more than one shade, since a picture drawn in one is a blank graph.

The suite reads the directory rather than the embedded copies, deliberately:
`build.rs` is as much under test as the files are, and a template that never
got embedded is invisible to a test that only looks at what was embedded.

**Use `0`, `2` and `4`, and nothing else.** This is the one rule worth learning
before you draw anything. GitHub's five greens are not evenly spaced — every
*adjacent* pair is 9 to 20 ΔE apart in the worst palette it ships, which is
close enough to read as a single colour. `{0, 2, 4}` is the **only** set of
three with no faint pair in it, and there is no clear set of four, so a picture
using all five cannot avoid putting two near-identical greens beside each
other. It will look thorough in your terminal and read as a smudge on the
graph.

The reference template learned this the hard way: `dragon.art` was drawn in all
five shades first, and the rendered chart is what showed it up.
`zero_two_four_is_the_largest_palette_with_no_faint_pair` proves the rule by
enumeration, and `the_reference_template_reads_clearly` holds the reference to
it. `--list-templates`, `--draw` and `--template` all report the **closest**
pair a picture uses rather than the widest, because the widest always flatters.

**On shape.** Seven by fifty-three is a very wide, very short canvas — about
7.5:1 — so forms that stretch along it read far better than ones that want to
be square. A serpent works; a portrait does not.

## 12. Code style

The code is commented for the reader who wonders *why*, not *what*. Comments
that explain a trade-off, a protocol quirk, or a decision that looks arbitrary
earn their place; comments that restate the line below them do not. Public items
carry documentation — `missing_docs` is a warning and CI runs clippy with
`-D warnings`.
