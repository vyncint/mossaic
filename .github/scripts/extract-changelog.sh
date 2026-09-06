#!/usr/bin/env bash
# extract-changelog.sh — print the CHANGELOG.md section for one version.
#
# Used by release.yml to build GitHub Release notes; never hand-write notes.
#
# Usage:
#   .github/scripts/extract-changelog.sh 0.1.0        # bare version
#   .github/scripts/extract-changelog.sh v0.1.0       # tag form also fine
#
# tests:
#   .github/scripts/extract-changelog.sh 0.3.1        # a released section
set -euo pipefail

# Run from anywhere: the changelog lives beside this script, two levels up.
cd "$(dirname "$0")/../.."

version="${1:?usage: extract-changelog.sh <version|vX.Y.Z|Unreleased>}"
version="${version#v}"

# Did the header exist at all? Asked separately, because a section that is
# present and *empty* is a different accident from one that is absent — and
# the empty one is the likely accident, since RELEASING.md step 2 is a hand
# edit and step 2b's grep only checks version strings. Reporting both as
# "no section found" sent a reader looking for a heading that was there.
if grep -q "^## \[${version}\]" CHANGELOG.md; then
  present=1
else
  present=0
fi

out="$(awk -v ver="$version" '
  # Section headers look like "## [0.1.0] - 2026-01-31" or "## [Unreleased]".
  /^## \[/ {
    if (found) exit
    if (index($0, "[" ver "]") > 0) { found = 1; next }
  }
  # The link block at the foot of the file ends the last section. Without
  # this the oldest section ran to EOF and absorbed every link definition:
  # `extract-changelog.sh 0.1.0` printed 157 lines ending in a URL.
  found && /^\[.*\]:/ { exit }
  found { lines[++n] = $0 }
  END {
    start = 1; while (start <= n && lines[start] ~ /^[[:space:]]*$/) start++
    end = n;   while (end >= start && lines[end]   ~ /^[[:space:]]*$/) end--
    for (i = start; i <= end; i++) print lines[i]
  }
' CHANGELOG.md)"

if [ -z "$out" ]; then
  if [ "$present" -eq 1 ]; then
    echo "::error::CHANGELOG.md has a '${version}' section but it is empty — write the notes before tagging." >&2
  else
    echo "::error::No CHANGELOG.md section found for version '${version}'." >&2
  fi
  exit 1
fi
printf '%s\n' "$out"
