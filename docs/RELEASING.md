# Releasing mossaic

The pipeline does the work; this is the human side of it.

## Prerequisites

- A publish credential. These are not interchangeable, and which one you have
  depends on whether the crate exists yet:
  - **Before the first release** there is no crate on crates.io, and Trusted
    Publishing is configured *on a crate* — so there is nothing to configure.
    The first release uses a stored `CARGO_REGISTRY_TOKEN` repository secret,
    scoped to this crate.
  - **After it**, switch: set up Trusted Publishing on crates.io (Settings →
    Trusted Publishing, pointing at `release.yml` and the `release`
    environment), change the publish step in `release.yml` back to
    `rust-lang/crates-io-auth-action` with `id-token: write`, and delete the
    secret. Short-lived tokens beat one sitting at rest.
- A `release` GitHub environment restricted to `v*` tags, so an OIDC publish
  token cannot be minted from a branch.
- `v*.*.*` tags protected by a ruleset, so only a maintainer can push one.

## Cutting vX.Y.Z

```sh
# 0. Green main, and no flakes: run the stress workflow and wait for it.
gh workflow run stress.yml
gh run watch

# 1. Bump `version` in Cargo.toml, and refresh the lockfile entry.
cargo check                      # rewrites Cargo.lock's mossaic version

# 2. Move the CHANGELOG section: [Unreleased] -> [X.Y.Z] - YYYY-MM-DD,
#    leaving an empty [Unreleased] above it.

# 3. Land it.
git switch -c release/vX.Y.Z && git commit -am "release: vX.Y.Z" && gh pr create

# 4. Tag the squash-merged commit on main.
git switch main && git pull
git tag vX.Y.Z && git push origin vX.Y.Z
```

The tag triggers `release.yml`, which:

1. fails unless the tag matches the crate version,
2. re-runs the full CI gates (`workflow_call` into `ci.yml`),
3. runs `cargo-semver-checks` against the last published release — skipped
   gracefully on the first one, since there is no baseline,
4. `cargo publish --locked` via Trusted Publishing,
5. creates the GitHub Release with notes extracted from `CHANGELOG.md`.

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
