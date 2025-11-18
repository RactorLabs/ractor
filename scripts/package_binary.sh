#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist/linux"
ARCH_RAW="$(uname -m)"
case "$ARCH_RAW" in
  x86_64|amd64)
    ARCH="x86_64"
    ;;
  arm64|aarch64)
    ARCH="aarch64"
    ;;
  *)
    echo "Unsupported architecture: $ARCH_RAW" >&2
    exit 1
    ;;
esac

cd "$ROOT_DIR"
rm -rf "$DIST_DIR/tsbx-linux-$ARCH" "$DIST_DIR/tsbx-linux-$ARCH.tar.gz"
mkdir -p "$DIST_DIR/tsbx-linux-$ARCH"

echo "Building tsbx (release)..."
cargo build --release --bin tsbx

cp target/release/tsbx "$DIST_DIR/tsbx-linux-$ARCH/tsbx"
chmod 755 "$DIST_DIR/tsbx-linux-$ARCH/tsbx"

tar -C "$DIST_DIR/tsbx-linux-$ARCH" -czf "$DIST_DIR/tsbx-linux-$ARCH.tar.gz" tsbx

echo "Created $DIST_DIR/tsbx-linux-$ARCH.tar.gz"
