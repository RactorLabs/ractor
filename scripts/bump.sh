#!/usr/bin/env bash
set -euo pipefail

# bump.sh — safely bump repo version across known refs and build artifacts
# Usage:
#   scripts/bump.sh [new_version] [--no-build] [--no-push]
# If new_version is omitted, bumps patch version from Cargo.toml (when SemVer).

root_dir=$(cd "$(dirname "$0")/.." && pwd)
cd "$root_dir"

# Parse args
DO_BUILD=1
DO_PUSH=1
DO_COMMIT=1
new=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-build)
      DO_BUILD=0; shift ;;
    --no-push)
      DO_PUSH=0; shift ;;
    --no-commit)
      DO_COMMIT=0; shift ;;
    *)
      if [[ -z "$new" ]]; then new="$1"; else echo "Unexpected arg: $1" >&2; exit 2; fi
      shift ;;
  esac
done

need() { command -v "$1" >/dev/null 2>&1 || { echo "Missing required command: $1" >&2; exit 1; }; }
need rg

cur=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n1)
[[ -n "$cur" ]] || { echo "Could not read current version from Cargo.toml" >&2; exit 1; }

is_semver() {
  [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]
}

if [[ -z "$new" ]]; then
  if is_semver "$cur"; then
    IFS=. read -r major minor patch <<<"$cur"
    new="$major.$minor.$((patch+1))"
  else
    echo "Current version in Cargo.toml ('$cur') is not SemVer (x.y.z)." >&2
    echo "Provide an explicit new version: scripts/bump.sh 0.X.Y" >&2
    exit 1
  fi
fi

if ! is_semver "$new"; then
  echo "New version '$new' is not SemVer (x.y.z)" >&2
  exit 1
fi

echo "Current: $cur -> New: $new"

# Update Cargo.toml (first standalone version line in [package])
sed -i "0,/^version = \".*\"$/s//version = \"$new\"/" Cargo.toml

# Update Operator docs badge (safe regex)
if [[ -f operator/src/routes/docs/+page.svelte ]]; then
  sed -i "s/const API_VERSION = '[0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\([^']*\)'/const API_VERSION = '$new\1'/" operator/src/routes/docs/+page.svelte || true
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

if [[ $DO_BUILD -eq 1 ]]; then
  echo "Building Rust (cargo build --release)"
  cargo build --release
else
  echo "Skipping Rust build (--no-build)"
fi

if [[ -f operator/package.json && $DO_BUILD -eq 1 ]]; then
  echo "Installing Operator deps and building (operator/)"
  (cd operator && (npm ci || npm install) >/dev/null && npm run -s build)
fi

if [[ $DO_COMMIT -eq 1 ]]; then
  echo "Staging and committing bump"
  git add -A
  if ! git diff --cached --quiet; then
    git commit -m "chore: bump version to $new"
    if [[ $DO_PUSH -eq 1 ]]; then
      # Push only if 'origin' exists and branch 'main' is configured
      if git remote get-url origin >/dev/null 2>&1; then
        if git rev-parse --abbrev-ref HEAD | grep -q '^main$'; then
          if ! git push origin main; then
            echo "Warning: push to origin/main failed. Please push manually." >&2
          fi
        else
          echo "Not on 'main' branch; skipping automatic push." >&2
        fi
      else
        echo "No 'origin' remote configured; skipping automatic push." >&2
      fi
    else
      echo "Skipping push (--no-push)"
    fi
  else
    echo "No changes to commit"
  fi
else
  echo "Skipping commit (--no-commit). Files modified (not staged):"
  git status --porcelain
fi
