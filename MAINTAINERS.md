# Maintainers

## Current maintainers

| Name       | GitHub                                 | Role            |
| ---------- | -------------------------------------- | --------------- |
| Vyncint Ng | [@vyncint](https://github.com/vyncint) | Lead maintainer |

## Governance model

`mossaic` uses a **single-maintainer model**: the lead maintainer has final
say on design, scope and releases. This is written down so expectations are
clear, not because it is a goal.

What maintainers do:

- review and merge PRs (squash merges through required CI),
- triage issues, and label terminal-compatibility reports so they are easy to
  find,
- cut releases,
- handle security reports per [SECURITY.md](SECURITY.md),
- enforce the [Code of Conduct](CODE_OF_CONDUCT.md).

## Becoming a maintainer

Sustained, high-quality contributions over a few months are the path.
Maintainers are added by consensus of the existing maintainer(s) and get an
entry here and in `.github/CODEOWNERS`.

## Decision making

Day-to-day decisions happen in issues and PRs. Anything that changes what the
chart looks like, the cell-style set, or the contracts in
[docs/DESIGN.md](docs/DESIGN.md) needs a maintainer's explicit approval —
fidelity to github.com is the project's north star, and drifting from it is a
decision, not an implementation detail.
