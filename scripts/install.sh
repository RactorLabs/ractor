#!/usr/bin/env bash
set -euo pipefail

OWNER="RactorLabs"
REPO="tsbx"
INSTALL_DIR="${HOME}/.local/bin"
CONFIG_DIR="${HOME}/.config/tsbx"
LOG_DIR="${CONFIG_DIR}/logs"
CONFIG_FILE="${CONFIG_DIR}/config.json"
TMP_DIR="$(mktemp -d)"

info() { printf '\033[34m[INFO]\033[0m %s\n' "$1"; }
success() { printf '\033[32m[SUCCESS]\033[0m %s\n' "$1"; }
error() { printf '\033[31m[ERROR]\033[0m %s\n' "$1" >&2; }

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

require() {
  if ! command -v "$1" >/dev/null 2>&1; then
    error "'$1' is required for installation"
    exit 1
  fi
}

require curl
require tar

arch=$(uname -m)
case "$arch" in
  x86_64|amd64)
    asset_arch="x86_64"
    ;;
  arm64|aarch64)
    asset_arch="aarch64"
    ;;
  *)
    error "Unsupported architecture: $arch"
    exit 1
    ;;
esac

asset="tsbx-linux-${asset_arch}.tar.gz"
url="https://github.com/${OWNER}/${REPO}/releases/latest/download/${asset}"
archive_path="${TMP_DIR}/${asset}"

info "Downloading ${asset}…"
curl -fsSL "$url" -o "$archive_path"
info "Extracting binary…"
tar -xzf "$archive_path" -C "$TMP_DIR"

mkdir -p "$INSTALL_DIR" "$CONFIG_DIR" "$LOG_DIR"
chmod 700 "$CONFIG_DIR" "$LOG_DIR"

if [[ -f "${TMP_DIR}/tsbx" ]]; then
  install_path="${TMP_DIR}/tsbx"
else
  install_path="$(find "$TMP_DIR" -type f -name tsbx -print -quit)"
fi

if [[ -z "$install_path" || ! -f "$install_path" ]]; then
  error "Binary not found in archive"
  exit 1
fi

install -m 755 "$install_path" "$INSTALL_DIR/tsbx"
success "Installed tsbx to $INSTALL_DIR/tsbx"

first_install=0
if [[ ! -f "$CONFIG_FILE" ]]; then
  first_install=1
  cat >"$CONFIG_FILE" <<JSON
{
  "provider_name": "",
  "inference_url": "",
  "default_model": "",
  "api_key": "",
  "created_at": "",
  "updated_at": ""
}
JSON
  chmod 600 "$CONFIG_FILE"
fi

if ! grep -q "${INSTALL_DIR}" <<<"$PATH"; then
  info "Add ${INSTALL_DIR} to your PATH to invoke tsbx without a full path"
fi

success "Installation complete"
if [[ $first_install -eq 1 ]]; then
  echo "Run: tsbx configure"
fi
