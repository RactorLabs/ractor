#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <component> [component ...]" >&2
  echo "Buildable components: api controller agent operator content gateway" >&2
  exit 1
fi

BUILDABLE_SET="api controller agent operator content gateway"

for COMPONENT in "$@"; do
  if ! grep -qw "$COMPONENT" <<< "$BUILDABLE_SET"; then
    echo "Error: '$COMPONENT' is not a buildable component." >&2
    echo "Buildable: $BUILDABLE_SET" >&2
    exit 1
  fi
done

for COMPONENT in "$@"; do
  echo "[INFO] Rebuilding component: $COMPONENT"

  echo "[INFO] Stopping $COMPONENT..."
  raworc stop "$COMPONENT" || true

  echo "[INFO] Building $COMPONENT..."
  bash "$(dirname "$0")/build.sh" "$COMPONENT"

  echo "[INFO] Starting $COMPONENT..."
  raworc start "$COMPONENT"

  echo "[SUCCESS] Rebuilt $COMPONENT"
done
