#!/bin/sh
# Release notes from local git only. The previous tag is the nearest ancestor
# whose name is exactly vX.Y.Z, not the tag being released and not any other tag.
set -eu

if [ "$#" -ne 1 ]; then
  echo "usage: release-notes.sh <version>" >&2
  exit 1
fi

version=$1
current=v${version}
prev=
best=
tags=$(mktemp)
trap 'rm -f "$tags"' EXIT
git tag --merged HEAD > "$tags"
while IFS= read -r candidate || [ -n "$candidate" ]; do
  [ -n "$candidate" ] || continue
  if [ "$candidate" = "$current" ]; then
    continue
  fi
  printf '%s\n' "$candidate" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$' || continue
  count=$(git rev-list --count "${candidate}..HEAD")
  if [ -z "$best" ] || [ "$count" -lt "$best" ]; then
    best=$count
    prev=$candidate
  fi
done < "$tags"

printf '## %s\n' "$version"
if [ -n "$prev" ]; then
  git --no-pager log --reverse --format=%s "${prev}..HEAD"
else
  git --no-pager log --reverse --format=%s HEAD
fi
