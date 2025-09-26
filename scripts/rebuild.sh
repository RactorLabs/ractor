#!/usr/bin/env bash
set -euo pipefail

BUILDABLE_SET=(api agent controller operator content gateway githex)
DEFAULT_SET=(api agent controller operator content gateway githex)

process_args() {
  local input=("$@")
  local unique=()
  # dedupe while preserving order (no associative arrays)
  local c i exists
  for c in "${input[@]}"; do
    exists=0
    for i in "${!unique[@]}"; do
      if [[ "${unique[$i]}" == "$c" ]]; then exists=1; break; fi
    done
    [[ $exists -eq 0 ]] && unique+=("$c")
  done
  # ensure 'agent' precedes 'controller' when both present
  local i_agent=-1 i_controller=-1
  for i in "${!unique[@]}"; do
    [[ "${unique[$i]}" == "agent" ]] && i_agent=$i
    [[ "${unique[$i]}" == "controller" ]] && i_controller=$i
  done
  if [[ $i_agent -ge 0 && $i_controller -ge 0 && $i_agent -gt $i_controller ]]; then
    local tmp=( )
    for i in "${!unique[@]}"; do
      if [[ $i -eq $i_agent ]]; then continue; fi
      if [[ $i -eq $i_controller ]]; then tmp+=("agent"); fi
      tmp+=("${unique[$i]}")
    done
    unique=("${tmp[@]}")
  fi
  printf '%s\n' "${unique[@]}"
}

# If no components specified, rebuild all (like build.sh default)
if [[ $# -eq 0 ]]; then
  set -- "${DEFAULT_SET[@]}"
fi

# Validate components and compute ordered list
ORDERED=()
for COMPONENT in "$@"; do
  if ! printf '%s\n' "${BUILDABLE_SET[@]}" | grep -qx "$COMPONENT"; then
    echo "Error: '$COMPONENT' is not a rebuildable component." >&2
    echo "Rebuildable: ${BUILDABLE_SET[*]}" >&2
    exit 1
  fi
  ORDERED+=("$COMPONENT")
done

mapfile -t ORDERED < <(process_args "${ORDERED[@]}")

for COMPONENT in "${ORDERED[@]}"; do
  echo "[INFO] Rebuilding component: $COMPONENT"

  # 1) Build first to minimize downtime
  echo "[INFO] Building $COMPONENT..."
  bash "$(dirname "$0")/build.sh" "$COMPONENT"

  # 2) Stop the running container (when applicable)
  if [[ "$COMPONENT" != "agent" && "$COMPONENT" != "githex" ]]; then
    echo "[INFO] Stopping $COMPONENT..."
    if command -v raworc >/dev/null 2>&1; then
      raworc stop "$COMPONENT" || true
    else
      echo "[WARNING] raworc CLI not found; skipping stop for $COMPONENT" >&2
    fi
  else
    if [[ "$COMPONENT" == "agent" ]]; then
      echo "[INFO] Skipping stop for agent (no standalone agent container)"
    else
      echo "[INFO] Skipping stop for githex (not auto-managed)"
    fi
  fi

  # 3) Start the container so it picks up the freshly built image
  if [[ "$COMPONENT" != "agent" && "$COMPONENT" != "githex" ]]; then
    echo "[INFO] Starting $COMPONENT..."
    if command -v raworc >/dev/null 2>&1; then
      raworc start "$COMPONENT" || true
    else
      echo "[WARNING] raworc CLI not found; skipping start for $COMPONENT" >&2
    fi
  else
    if [[ "$COMPONENT" == "agent" ]]; then
      echo "[INFO] Skipping start for agent (controller uses agent image)"
    else
      echo "[INFO] Skipping start for githex (never auto-started)"
    fi
  fi

  echo "[SUCCESS] Rebuilt $COMPONENT"
done
