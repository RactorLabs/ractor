#!/usr/bin/env bash
set -euo pipefail

cat <<'MSG'
TSBX CLI releases are handled by:
  1. Building the Linux binaries: cargo build --release --bin tsbx (repeat per architecture).
  2. Archiving the binary as tsbx-linux-<arch>.tar.gz.
  3. Uploading the archives to the GitHub release referenced by scripts/install.sh.
There is no npm package to publish.
MSG
