#!/usr/bin/env bash
# A release always comes from an existing, unchanged stable version tag.
set -euo pipefail
cd "$(dirname "$0")/.."
if [ "${GITHUB_REF_TYPE:-}" != tag ]; then
  echo 'Run the Release workflow on an existing vX.Y.Z tag, including manual runs.' >&2
  exit 1
fi
tag=${GITHUB_REF_NAME:?Missing release tag}
if ! [[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Expected a stable vX.Y.Z tag, got $tag" >&2
  exit 1
fi
version=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' Cargo.toml | head -1)
if [ "$tag" != "v$version" ]; then
  echo "Tag $tag does not match Cargo.toml version $version" >&2
  exit 1
fi
notes="docs/releases/$version.md"
test -s "$notes" || { echo "Missing release notes: $notes" >&2; exit 1; }
commit=$(git rev-parse HEAD)
remote_commit=''
# Annotated tags have a peeled commit; lightweight tags point to it directly.
while read -r object ref; do
  case "$ref" in
    "refs/tags/$tag") if [ -z "$remote_commit" ]; then remote_commit=$object; fi ;;
    "refs/tags/$tag^{}") remote_commit=$object ;;
  esac
done < <(git ls-remote --exit-code origin "refs/tags/$tag" "refs/tags/$tag^{}")
if [ "$commit" != "$remote_commit" ]; then
  echo "Refusing release: remote $tag resolves to $remote_commit, checked out commit is $commit." >&2
  exit 1
fi
printf 'Verified %s at %s using %s\n' "$tag" "$commit" "$notes"
