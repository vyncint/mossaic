# Releasing mossaic

The pipeline does the work; this is the human side of it.

## Prerequisites

- crates.io **Trusted Publishing** linked to this repository (Settings →
  Trusted Publishing on crates.io, pointing at `release.yml` and the `release`
  environment). No `CARGO_REGISTRY_TOKEN` secret is stored anywhere.

  0.1.0 was the exception and could not have been otherwise: Trusted
  Publishing is configured *on a crate*, so it cannot be set up for one that
  has never been published. That release used a stored token, which was
  deleted immediately afterwards. A new crate from this template will have to
  do the same once, then switch back.
- A `release` GitHub environment restricted to `v*` tags, so an OIDC publish
  token cannot be minted from a branch.
- `v*.*.*` tags protected by a ruleset, so only a maintainer can push one.
- Two repository secrets, both personal access tokens and both optional in the
  sense that a release without them goes **green** — which is exactly why they
  are listed here:
  - `HOMEBREW_TAP_TOKEN`, with `contents: write` on `vyncint/homebrew-tap`.
    Without it the formula job prints a notice and exits 0, and
    `brew install vyncint/tap/mossaic` keeps serving the previous version.
  - `TESTING_REPO_DISPATCH_TOKEN`, with `contents: write` on
    `vyncint/contribution-art`. Without it that repository finds the release
    on its own schedule instead of at once.

## Cutting vX.Y.Z

```sh
# 0. Green main, and no flakes: run the stress workflow and wait for it.
gh workflow run stress.yml
gh run watch

# 1. Bump `version` in Cargo.toml, and refresh the lockfile entry.
cargo check                      # rewrites Cargo.lock's mossaic version

# 2. Move the CHANGELOG section: [Unreleased] -> [X.Y.Z] - YYYY-MM-DD, leaving an
#    empty [Unreleased] above it — and repoint the link definitions at the foot of
#    the file, or the new heading renders as literal `[X.Y.Z]` text:
#      [Unreleased]: …/compare/vX.Y.Z...HEAD
#      [X.Y.Z]:      …/compare/vPREV...vX.Y.Z

# 2b. Bump every version the docs name. Two kinds, and both go stale: the
#     `action@vPREV` refs, which point at a release and are the lines people
#     copy (0.3.0 shipped with three of them a version behind), and the
#     illustrative "pin a number" examples, which a reader reasonably reads as
#     the current one. This catches both:
grep -rEn "mossaic/action@v|[0-9]+\.[0-9]+\.[0-9]+" docs action | grep -v CHANGELOG

# 3. Land it.
git switch -c release/vX.Y.Z && git commit -am "release: vX.Y.Z" && gh pr create

# 4. Tag the squash-merged commit on main.
git switch main && git pull
git tag vX.Y.Z && git push origin vX.Y.Z
```

The tag triggers `release.yml`, which runs these jobs:

1. **tag matches crate version** — fails unless the tag is the version in
   `Cargo.toml`, and unless `extract-changelog.sh` finds non-empty notes for it
   in `CHANGELOG.md`. Everything below depends on it, so nothing publishes
   without notes.
2. **ci** — the full CI gates again (`workflow_call` into `ci.yml`).
3. **cargo-semver-checks** — against the release on crates.io. It is skipped
   only when crates.io answers 404 for the crate, which is the first release;
   any other answer fails the job, since a check that could not run has not
   passed.
4. **publish to crates.io** — `cargo publish --locked` via Trusted Publishing,
   in the `release` environment. Needs 2 and 3. The irreversible step.
5. **GitHub release** — created from the tag with the extracted notes.
6. **binaries** (`binaries.yml`, called) — builds the five targets
   (`x86_64`/`aarch64-unknown-linux-musl`, `x86_64`/`aarch64-apple-darwin`,
   `x86_64-pc-windows-msvc`) and uploads each archive under a versioned and an
   unversioned name, each with a `.sha256`. Then **homebrew formula**
   regenerates `Formula/mossaic.rb` from those checksums and pushes it to the
   tap — or, with no `HOMEBREW_TAP_TOKEN`, says so and leaves the tap alone.
7. **install** (`install.yml`, called) — `cargo install mossaic` from
   crates.io on Linux and macOS, on stable and on the MSRV, and runs what it
   installed. Needs 6.
8. **notify-testing-repo** — dispatches `tool-published` to
   `vyncint/contribution-art`. Best-effort: it never fails the release.

## After the run

Green is not the whole answer — two of the jobs above succeed by design when
their token is missing. Check what a user would get:

```sh
# 20 assets: five archives, each under two names, each with a .sha256.
gh release view vX.Y.Z --json assets --jq '.assets | length'

# The tap names the new version.
gh api repos/vyncint/homebrew-tap/contents/Formula/mossaic.rb \
  -H "Accept: application/vnd.github.raw" | grep -m1 'releases/download/'
```

## What a version number means here

mossaic ships a library *and* three binaries, and they version together.

- **Breaking** (minor, pre-1.0): a removed or renamed public item; a changed
  CLI flag; a change to what the chart draws that a user would have to relearn.
- **Not breaking**: new flags, new cell styles, a terminal newly supported, a
  palette corrected to match what github.com actually serves.
- **MSRV bumps are minor**, never patch.

## If something fails mid-release

- **Before publish**: fix, delete the tag (`git push --delete origin vX.Y.Z`),
  re-tag.
- **After publish**: crates.io releases are permanent. Yank
  (`cargo yank --version X.Y.Z`) and ship a patch. Do not delete the tag — the
  published crate points at it.
- **Binaries or formula missing** (a failed `binaries` leg, a tap token set
  after the fact): dispatch `binaries.yml` with the tag. Never re-tag — the
  crate is already published.

  ```sh
  gh workflow run binaries.yml -f tag=vX.Y.Z
  ```

  It uploads only the assets that are absent — a published archive is never
  replaced, since its checksum is already in the formula and in people's notes
  — and regenerates the formula. `install` did not run if `binaries` failed;
  `gh workflow run install.yml -f version=X.Y.Z` runs it on its own.

## First release checklist

- [ ] `cargo package --list` contains what it should and nothing more
- [ ] README renders on crates.io (relative image links resolve to the repo)
- [ ] `docs.rs` builds: `cargo doc --no-deps --all-features` with
      `RUSTDOCFLAGS=-D warnings`
- [ ] `mossaic --capabilities` checked on at least one terminal of each kind:
      one that draws kitty graphics, one that draws sixel, one that draws
      neither
- [ ] the repository's **About** box is filled in — it is the only description
      a visitor sees before scrolling, and it lives in GitHub's settings rather
      than in this tree:

      > Your GitHub contribution graph, pixel-exact in the terminal — and a
      > planner that turns "draw my name in 2027" into a number for today.

      Topics: `rust`, `cli`, `tui`, `ratatui`, `terminal`, `github-contributions`,
      `contribution-graph`, `kitty-graphics-protocol`, `sixel`, `github-actions`.
      The name does not say what this is, so the topics are how it is found.
- [ ] website field set to `https://docs.rs/mossaic`
