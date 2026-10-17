#!/usr/bin/env bash
# What a release adds: a line per commit since the release before it, the version commits left out.
set -euo pipefail
commit="$1"
tag="${2:-beta}"
since=$(git describe --tags --abbrev=0 --match 'v[0-9]*' --exclude "$tag" "$commit^" 2> /dev/null || true)
[ -n "$since" ] || exit 0
echo "Changes since $since:"
echo
git log --no-merges --format='- %s' --invert-grep --grep='^RimStack [0-9]' "$since..$commit"
