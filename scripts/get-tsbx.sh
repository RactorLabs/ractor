#!/usr/bin/env bash
set -euo pipefail

SOURCE_REF="${TSBX_SOURCE_REF:-tsbx-installation}"
INSTALL_SCRIPT_URL="https://raw.githubusercontent.com/RactorLabs/tsbx/${SOURCE_REF}/scripts/install.sh"

curl -fsSL "$INSTALL_SCRIPT_URL" | TSBX_SOURCE_REF="$SOURCE_REF" bash -s -- "$@"
