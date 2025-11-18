#!/usr/bin/env bash
set -euo pipefail

cat <<'MSG'
The tsbx CLI is a Rust binary and no longer supports npm linking.
Run `cargo build --bin tsbx` to build a local binary, or `cargo install --path . --bin tsbx` to install it globally during development.
MSG
