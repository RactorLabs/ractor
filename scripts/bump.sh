#!/usr/bin/env bash
set -euo pipefail

# bump.sh — safely bump repo version across known refs
# Usage:
#   scripts/bump.sh [new_version]
# If new_version is omitted, bumps patch version from Cargo.toml.

root_dir=$(cd "$(dirname "$0")/.." && pwd)
cd "$root_dir"

if ! command -v rg >/dev/null 2>&1; then echo "ripgrep (rg) required" >&2; exit 1; fi

cur=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
if [[ -z "${cur}" ]]; then echo "Could not read current version from Cargo.toml" >&2; exit 1; fi

new="${1:-}"
if [[ -z "${new}" ]]; then
  IFS=. read -r major minor patch <<<"$cur"
  new="$major.$minor.$((patch+1))"
fi

echo "Current: $cur -> New: $new"

# Update Cargo.toml
sed -i "0,/^version = \"[0-9]\+\.[0-9]\+\.[0-9]\+\"$/s//version = \"$new\"/" Cargo.toml

# Update CLI package.json if present
if [[ -f cli/package.json ]]; then
  if command -v jq >/dev/null 2>&1; then
    tmp=$(mktemp)
    jq -r --arg v "$new" '.version=$v' cli/package.json > "$tmp" && mv "$tmp" cli/package.json
  else
    # fallback: simple replace (assumes valid JSON formatting)
    sed -i "0,/\"version\": \"[0-9]\+\.[0-9]\+\.[0-9]\+\"/s//\"version\": \"$new\"/" cli/package.json || true
  fi
fi

# Update Operator docs badge (safe regex)
if [[ -f operator/src/routes/docs/+page.svelte ]]; then
  perl -0777 -pe "s/(const\s+API_VERSION\s*=\s*')\d+\.\d+\.\d+(\s*\(v0\)';)/\1$new\2/" -i operator/src/routes/docs/+page.svelte
fi

echo "Updated refs to $new. Review with: git status && git diff --compact-summary"
echo "To commit: git add -A && git commit -m \"chore: bump version to $new\""

