# Working on mossaic

Instructions for coding agents — and useful to humans. **mossaic** is your GitHub contribution graph drawn pixel-exactly in the terminal, plus a planner that turns "draw my name in 2027" into a number for today.

This file is the canonical brief; `CLAUDE.md` points here. `CONTRIBUTING.md`
is the full contributor document and wins wherever the two disagree.

## Layout

- `src/` — the library and three binaries: `mossaic` (the chart), `mossaic-art`
  (the planner), `mossaic-glyphs`. `graphics.rs` is the rasteriser, `primer.rs`
  the GitHub colour tokens, `art.rs` the 5×5 font.
- `tests/` — `smoke.rs` and `pixels.rs` drive the real binary in a real PTY
  through termlens; `art_cli.rs` drives the planner as a shell would.
- `docs/DESIGN.md` — what was traded for what in the pixel path. **Read it
  before changing anything that emits kitty or sixel.**

## Build and test

```sh
cargo test                     # everything, no network
cargo test --test smoke        # the real binary, in a real PTY
cargo test --test pixels       # …in a PTY that says it can draw pixels
cargo test -- --ignored        # the two that call the GitHub API
```

Stable toolchain, MSRV 1.88 (set by ratatui 0.30).

## Things that will bite you here

- **Three test layers, and they catch different things.** In-process for
  anything that is a function of its inputs; two out-of-process layers where
  termlens spawns the real binary. Put a test in the layer that can actually
  see the bug.
- **`art/font.png` is generated, not drawn.** Regenerate with
  `cargo run --bin mossaic-art -- --font --png art/font.png`; a test compares
  it byte for byte and the README's glyph list against the font itself.
- **The colours are read from github.com's stylesheets, not transcribed.**
  When GitHub restyles the graph, a test here should fail. Do not "fix" it by
  editing the expected value without checking what GitHub now serves.

## The rules that will fail CI

Three, and they are the same in every one of these repositories.

1. **Conventional Commits.** `feat:`, `fix:`, `docs:`, `test:`, `ci:`,
   `chore:`, `refactor:`, `perf:` — imperative mood, subject line under 72
   characters, scope optional (`fix(screen): …`).
2. **DCO sign-off.** `git commit -s`, and the `Signed-off-by:` email must
   match the commit author's. Forgot? `git commit --amend -s --no-edit`, or
   `git rebase --signoff main` for a branch.
3. **No AI attribution.** See below — this one is about you, and it is the
   rule most likely to catch an agent out.

Run them yourself before pushing; both scripts take a commit range:

```sh
.github/scripts/check-dco.sh main..HEAD
.github/scripts/check-no-ai-attribution.sh main..HEAD
```

## Using AI here

**You are welcome.** Every one of these projects was built with AI assistance
and says so in its CONTRIBUTING. Use whatever helps.

**You are not a contributor.** Do not add yourself to the history:

- no `Co-Authored-By:` trailer naming an assistant, a model, or a vendor,
- no "Generated with …" footer, no robot emoji,
- no bot account as author or committer.

The human who opens the pull request is the author of record and takes
responsibility for the change under the DCO. That is what the sign-off
certifies, and it cannot be certified by a tool. `.claude/settings.json`
turns co-author trailers off for agents that read it; the check in CI is the
boundary, and it reads every commit in the range.

If CI catches one, the fix is to rewrite the message, not to argue with it:

```sh
git commit --amend            # the last commit
git rebase -i main            # several, marking each `reword`
git push --force-with-lease
```

## What good work looks like here

These repositories share a house style, and it is stricter than most:

- **Evidence over assertion.** A bug report says what was measured against
  which released version. "Reproduced against 0.4.0" is the standard; "the
  code looks wrong" is not. Issues in these repos read *Today / Why it is
  worth fixing / Fix / Done when*, with a concrete reproduction.
- **Every change lands with a test**, and the test must be able to fail. If
  you add a guard, prove it catches the thing — break it once and watch it go
  red before you commit.
- **Comments say *why*, never *what*.** The diff shows what. A comment earns
  its place by recording the reason, the alternative rejected, or the failure
  that motivated the line.
- **Say what you did not do.** A pull request that lists what it left out and
  why is worth more than one that implies completeness. If something is
  unverified, say so — an honest gap is cheap and a false claim is expensive.
- **Documentation is checked, not maintained.** Where a README states a fact
  the code owns, there is usually a test asserting the two agree. Do not
  break that pattern by hand-editing the doc.

## Pull requests

Branch from `main` (`feat/…`, `fix/…`, `docs/…`, `ci/…`). PRs are
**squash-merged**, so the PR title becomes the commit subject on `main` —
write it as a Conventional Commit. Update `CHANGELOG.md` under
`[Unreleased]` for anything user-facing.

Direct pushes to `main` are blocked by a ruleset; everything goes through a
pull request, including releases.

## Releasing

Tag `vX.Y.Z` on `main`; `release.yml` gates, publishes via Trusted Publishing, and cuts the GitHub Release. See `docs/RELEASING.md`.
