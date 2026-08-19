## What & why

<!-- What does this PR change, and what problem does it solve?
     Link the issue: "Closes #123". -->

## Checklist

- [ ] Linked an issue (or explained above why none exists)
- [ ] Tests added/updated for the change
- [ ] `cargo fmt --all` and `cargo clippy --all-targets --all-features -- -D warnings` are clean
- [ ] `cargo test` passes (the network tests are `#[ignore]`d; run them if the change touches `github.rs`)
- [ ] Anything user-visible in the terminal was checked against a real one, not only the harness
- [ ] `CHANGELOG.md` updated under `[Unreleased]` (user-facing changes only)
- [ ] No AI attribution in any commit — no `Co-Authored-By` naming an assistant,
      no "Generated with" watermark. You are the author of record; CI checks this.
