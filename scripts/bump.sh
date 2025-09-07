#!/usr/bin/env bash
set -euo pipefail

# bump.sh — safely bump repo version across known refs and build artifacts
# Usage:
#   scripts/bump.sh [new_version]
# If new_version is omitted, bumps patch version from Cargo.toml.

root_dir=$(cd "$(dirname "$0")/.." && pwd)
cd "$root_dir"

need() { command -v "$1" >/dev/null 2>&1 || { echo "Missing required command: $1" >&2; exit 1; }; }
need rg

cur=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
[[ -n "$cur" ]] || { echo "Could not read current version from Cargo.toml" >&2; exit 1; }

new="${1:-}"
if [[ -z "$new" ]]; then
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
    sed -i "0,/\"version\": \"[0-9]\+\.[0-9]\+\.[0-9]\+\"/s//\"version\": \"$new\"/" cli/package.json || true
  fi
fi

# Update Operator docs badge (safe regex)
if [[ -f operator/src/routes/docs/+page.svelte ]]; then
  perl -0777 -pe "s/(const\s+API_VERSION\s*=\s*')\d+\.\d+\.\d+(\s*\(v0\)';)/\1$new\2/" -i operator/src/routes/docs/+page.svelte || true
  # Fallback: if corrupted, replace the line following the version comment
  if ! rg -n "const\\s+API_VERSION" operator/src/routes/docs/+page.svelte >/dev/null; then
    python3 - "$new" <<'PY'
import sys
from pathlib import Path
new=sys.argv[1]
p=Path('operator/src/routes/docs/+page.svelte')
s=p.read_text()
needle='// Hard-coded docs version; update during version bumps'
if needle in s:
    parts=s.split(needle)
    head=parts[0]+needle+"\n  const API_VERSION = '%s (v0)';\n"%new
    rest=parts[1].split('\n',1)[1] if '\n' in parts[1] else ''
    s=head+rest
    p.write_text(s)
PY
  fi
fi

echo "Building Rust (cargo build --release)"
cargo build --release

if [[ -f cli/package.json ]]; then
  echo "Installing CLI deps (cli/)"
  (cd cli && (npm ci || npm install) >/dev/null)
fi

if [[ -f operator/package.json ]]; then
  echo "Installing Operator deps and building (operator/)"
  (cd operator && (npm ci || npm install) >/dev/null && npm run -s build)
fi

echo "Staging and committing bump"
git add -A
if ! git diff --cached --quiet; then
  git commit -m "chore: bump version to $new"
  git push origin main
else
  echo "No changes to commit"
fi
