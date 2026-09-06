#!/usr/bin/env bash
# Test extract-changelog.sh against a fixture changelog.
#
# This is the only piece of the release path with no test at all, which is
# why three behaviours could sit in it unnoticed: a section present and empty
# reported as absent, the oldest section absorbing the link block, and the
# runbook attributing the failure to a job that never opened CHANGELOG.md.
#
# Portable shell: this runs on the macOS leg too, where bash is 3.2.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
SCRIPT="$ROOT/.github/scripts/extract-changelog.sh"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# A fixture with every shape: an empty [Unreleased], a filled release, an
# empty one, the oldest, and the link block that used to be swallowed.
mkdir -p "$WORK/.github/scripts"
cp "$SCRIPT" "$WORK/.github/scripts/"
cat > "$WORK/CHANGELOG.md" <<'CHANGELOG'
# Changelog

## [Unreleased]

## [0.7.0] - 2026-09-15

### Added

- something real.

## [0.6.9] - 2026-09-01

## [0.1.0] - 2026-01-31

### Added

- the first one.

[0.7.0]: https://example.invalid/0.7.0
[0.1.0]: https://example.invalid/0.1.0
CHANGELOG

run() { ( cd "$WORK" && ./.github/scripts/extract-changelog.sh "$@" ) 2>&1; }
status=0
fail() { echo "FAIL: $1" >&2; status=1; }

# A filled section, by bare version and by tag.
for version in 0.7.0 v0.7.0; do
  out=$(run "$version") || { fail "$version should succeed"; continue; }
  case "$out" in
    *"something real"*) ;;
    *) fail "$version: notes missing: $out" ;;
  esac
done

# Present but empty is its own message, and is not "not found".
for version in Unreleased 0.6.9; do
  if out=$(run "$version"); then
    fail "$version: an empty section must fail"
  else
    case "$out" in
      *"but it is empty"*) ;;
      *) fail "$version: must say the section is empty, got: $out" ;;
    esac
  fi
done

# Absent is the other message.
if out=$(run 9.9.9); then
  fail "9.9.9: an absent section must fail"
else
  case "$out" in
    *"No CHANGELOG.md section found"*) ;;
    *) fail "9.9.9: wrong message: $out" ;;
  esac
fi

# The oldest section stops at the link block rather than running to EOF.
out=$(run 0.1.0)
case "$out" in
  *"example.invalid"*) fail "the oldest section absorbed the link block: $out" ;;
  *"the first one"*) ;;
  *) fail "0.1.0: notes missing: $out" ;;
esac

[ "$status" -eq 0 ] && echo "extract-changelog.sh: every shape behaves"
exit "$status"
