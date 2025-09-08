#!/usr/bin/env bash
set -euo pipefail

# release.sh — tag and push the current version, after building
# Usage:
#   scripts/release.sh [--skip-build]
#
# Behavior:
# - Reads version from Cargo.toml
# - Optionally builds Rust (release) and Operator (npm build) unless --skip-build is set
# - Commits any pending changes as "chore: prepare release <version>"
# - Creates a git tag <version> if not already present
# - Pushes main and the tag to origin (triggers GitHub release workflow)

root_dir=$(cd "$(dirname "$0")/.." && pwd)
cd "$root_dir"

skip_build=false
for arg in "$@"; do
  case "$arg" in
    --skip-build) skip_build=true ;;
    -h|--help)
      echo "Usage: $0 [--skip-build]"; exit 0 ;;
    *) echo "Unknown option: $arg" >&2; exit 1 ;;
  esac
done

need() { command -v "$1" >/dev/null 2>&1 || { echo "Missing required command: $1" >&2; exit 1; }; }
need rg

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
[[ -n "$version" ]] || { echo "Could not read version from Cargo.toml" >&2; exit 1; }
echo "Releasing version: $version"

if [[ "$skip_build" != true ]]; then
  echo "Building Rust (cargo build --release)"
  cargo build --release

  if [[ -f operator/package.json ]]; then
    echo "Building Operator (npm ci|install && npm run build)"
    (cd operator && (npm ci || npm install) >/dev/null && npm run -s build)
  fi
fi

# Stage and commit pending changes if any
if ! git diff --quiet; then
  git add -A
  git commit -m "chore: prepare release $version"
fi

# Tag and push
if git rev-parse -q --verify "refs/tags/$version" >/dev/null; then
  echo "Tag $version already exists; skipping tag creation"
else
  git tag "$version"
fi

git push origin main
git push origin "$version"

echo "Release $version pushed. GitHub Actions should handle publishing."

