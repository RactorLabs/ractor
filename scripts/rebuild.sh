#!/usr/bin/env bash
set -euo pipefail

BUILDABLE_SET=(api controller agent operator content gateway)

# If no components specified, rebuild all (like build.sh default)
if [[ $# -eq 0 ]]; then
  set -- "${BUILDABLE_SET[@]}"
fi

# Validate components
for COMPONENT in "$@"; do
  if ! printf '%s\n' "${BUILDABLE_SET[@]}" | grep -qx "$COMPONENT"; then
    echo "Error: '$COMPONENT' is not a rebuildable component." >&2
    echo "Rebuildable: ${BUILDABLE_SET[*]}" >&2
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
