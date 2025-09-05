#!/usr/bin/env bash
# Wrapper script for Raworc CLI to call repo doctor script when installed via npm link
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_SCRIPT="$SCRIPT_DIR/../../scripts/doctor.sh"
if [ -f "$REPO_SCRIPT" ]; then
  exec bash "$REPO_SCRIPT" "$@"
fi
echo "Unable to locate repo scripts/doctor.sh from CLI. Please run from the repository root: ./scripts/doctor.sh" >&2
exit 1

