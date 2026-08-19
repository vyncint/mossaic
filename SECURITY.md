# Security Policy

`mossaic` is a terminal application that reads one thing from the network —
your contribution calendar, through the `gh` CLI — and writes escape sequences
to your terminal. It handles no credentials of its own and executes nothing it
downloads. Bugs that let fetched data escape that boundary (escape sequences
reaching the terminal unfiltered, or a path being written outside the file you
named) are treated as security issues.

## Posture

What the project does continuously, enforced by required CI on every change:

- **No `unsafe` code.** `unsafe_code = "forbid"` in `Cargo.toml`; the one libc
  dependency is used for a single constant (`O_NONBLOCK`), not a call.
- **No token handling.** Authentication is `gh`'s: mossaic shells out to
  `gh api graphql` and never sees, stores or logs a token.
- **The API response is parsed, not trusted.** Dates, counts and levels are
  deserialised into typed fields; nothing from the response is printed to the
  terminal without going through the renderer, which writes cells, not bytes.
- **Dependency policy** (`cargo-deny` job): RUSTSEC advisories, yanked crates,
  a license allowlist and crates.io-only sources, on every PR, with weekly
  grouped Dependabot updates.
- **Workflow security** (`zizmor` job at `--persona=pedantic`): every GitHub
  Action pinned to a full commit SHA, checkouts that do not persist
  credentials, a read-only workflow token by default. Accepted findings are
  documented in `.github/zizmor.yml`.

## What has been found and fixed

The 0.1.0 review, in the order the findings mattered. Each has a regression
test named after it in `src/render_tests.rs`.

- **Escape sequences from a calendar reached the terminal.** A crafted
  `--file` snapshot, or a hostile API response, could put `ESC` into the login
  or an error message. The renderer was never the problem — ratatui drops
  control characters when it fills a cell — but the paths that print straight
  to stdout (`--png`, `mossaic-art --track`, error messages) passed them through, so a
  file could set the terminal title, request a cursor-position report the
  application would then read as input, or write the clipboard through
  `OSC 52`. Untrusted text is now stripped of control characters where it
  enters, in `github::parse`, rather than at each place that prints it.
- **A calendar spanning millennia allocated gigabytes.** The grid is sized by
  the distance between the first and last day, so two dates far enough apart
  in a few hundred bytes of JSON aborted the process on a 4.6 GB allocation.
  A calendar that spans more than 400 days is now refused, and the grid
  constructor caps its own growth as well.
- **Counts from a file overflowed the shading arithmetic.** `count * 4` on a
  crafted `u32::MAX` wrapped in release and panicked in debug. The shading is
  computed in 64 bits now — widened rather than saturated, because saturating
  it would report a day equal to the peak as level 1 instead of 4, which the
  test for this caught.
- **A reported character-cell size sized an allocation unchecked.**
  `--cell 20000x20000`, or a terminal answering nonsense to `CSI 16 t`, asked
  for a 1.19 TB image. Cell sizes above 64 pixels are refused from both
  sources.
- **The git identity went into a fast-import stream unvalidated.** An identity
  is written as a line of its own, so a newline in `--name` or `--email` would
  have been a command. git refuses the malformed result today — verified, no
  branch was created — but the check is ours now, and fails with a sentence
  instead of a crash report.

Not affected, and checked: the `gh` invocation builds an argument vector and
never a shell string; the Action passes every input through `env:` rather than
interpolating it into `run:`, and audits clean under `zizmor --persona=pedantic`
including the composite action itself; every advisory that has ever touched a
crate in this tree is patched in the version the lockfile pins.

## Scope

In scope:

- Escape sequences from fetched data reaching the terminal (an injected
  sequence in a login or error message, for instance).
- `mossaic-art --write` touching anything outside the `--repo` directory it was given.
- `--png` or `--snapshot` writing outside the path they were given.
- Anything that makes mossaic execute a command it was not asked to.

Out of scope:

- The contents of your contribution graph.
- `gh` itself — report those to [cli/cli](https://github.com/cli/cli).
- A terminal mis-rendering a valid escape sequence: that is a compatibility
  bug, and a very welcome ordinary issue.

## Reporting

Report privately through
[GitHub Security Advisories](https://github.com/vyncint/mossaic/security/advisories/new),
or by email to <vyncint@icloud.com>. Please include the version, the terminal,
and a reproduction.

Expect an acknowledgement within a few days. Fixes are released as soon as they
are ready; credit is given in the changelog unless you would rather not have it.
