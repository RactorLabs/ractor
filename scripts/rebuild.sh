#!/usr/bin/env bash
set -euo pipefail

COMPONENT=${1:-}

if [[ -z "$COMPONENT" ]]; then
  echo "Usage: $0 <component>" >&2
  echo "Buildable components: server controller agent operator content gateway" >&2
  exit 1
fi

case "$COMPONENT" in
  server|controller|agent|operator|content|gateway)
    ;;
  *)
    echo "Error: '$COMPONENT' is not a buildable component." >&2
    echo "Buildable: server controller agent operator content gateway" >&2
    exit 1
    ;;
esac

echo "[INFO] Rebuilding component: $COMPONENT"

echo "[INFO] Stopping $COMPONENT..."
raworc stop "$COMPONENT" || true

echo "[INFO] Building $COMPONENT..."
bash "$(dirname "$0")/build.sh" "$COMPONENT"

echo "[INFO] Starting $COMPONENT..."
raworc start "$COMPONENT"

echo "[SUCCESS] Rebuilt $COMPONENT"

