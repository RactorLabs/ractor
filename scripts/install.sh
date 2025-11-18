#!/usr/bin/env bash
set -euo pipefail

OWNER="RactorLabs"
REPO="tsbx"
SOURCE_REF="${TSBX_SOURCE_REF:-main}"
INSTALL_DIR="${HOME}/.local/bin"
CONFIG_DIR="${HOME}/.config/tsbx"
LOG_DIR="${CONFIG_DIR}/logs"
CONFIG_FILE="${CONFIG_DIR}/config.json"
TMP_DIR="$(mktemp -d)"

info() { printf '\033[34m[INFO]\033[0m %s\n' "$1"; }
success() { printf '\033[32m[SUCCESS]\033[0m %s\n' "$1"; }
warn() { printf '\033[33m[WARN]\033[0m %s\n' "$1"; }
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

if [[ "$(uname -s)" != "Linux" ]]; then
  error "Only Linux installations are supported by this installer"
  exit 1
fi

arch="$(uname -m)"
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
asset_url="https://github.com/${OWNER}/${REPO}/releases/latest/download/${asset}"
source_url="https://codeload.github.com/${OWNER}/${REPO}/tar.gz/${SOURCE_REF}"
archive_path="${TMP_DIR}/${asset}"
install_path=""

install_binary_from_archive() {
  info "Extracting binary…"
  tar -xzf "$archive_path" -C "$TMP_DIR"
  if [[ -f "${TMP_DIR}/tsbx" ]]; then
    install_path="${TMP_DIR}/tsbx"
    return
  fi
  install_path="$(find "$TMP_DIR" -type f -name tsbx -print -quit)"
  if [[ -z "$install_path" || ! -f "$install_path" ]]; then
    error "Binary not found in archive"
    exit 1
  fi
}

build_from_source() {
  require cargo

  src_archive="${TMP_DIR}/tsbx-source.tar.gz"
  src_dir="${TMP_DIR}/tsbx-src"
  mkdir -p "$src_dir"

  info "Downloading source (${SOURCE_REF})…"
  curl -fsSL "$source_url" -o "$src_archive"
  info "Extracting source…"
  if ! tar -xzf "$src_archive" -C "$src_dir" --strip-components=1; then
    error "Failed to extract source archive"
    exit 1
  fi

  info "Building tsbx CLI from source (this may take a few minutes)…"
  (cd "$src_dir" && cargo build --release --bin tsbx)

  install_path="${src_dir}/target/release/tsbx"
  if [[ ! -f "$install_path" ]]; then
    error "Cargo build completed but tsbx binary was not found"
    exit 1
  fi
}

if curl -fsSL "$asset_url" -o "$archive_path"; then
  info "Downloaded ${asset}"
  install_binary_from_archive
else
  warn "Prebuilt ${asset} not available (HTTP ${?}); falling back to building from source"
  build_from_source
fi

mkdir -p "$INSTALL_DIR" "$CONFIG_DIR" "$LOG_DIR"
chmod 700 "$CONFIG_DIR" "$LOG_DIR"

install -m 755 "$install_path" "$INSTALL_DIR/tsbx"
success "Installed tsbx to $INSTALL_DIR/tsbx"

first_install=0
if [[ ! -f "$CONFIG_FILE" ]]; then
  first_install=1
  cat >"$CONFIG_FILE" <<'JSON'
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
  if [[ "${TSBX_AUTO_CONFIGURE:-1}" == "1" ]]; then
    info "Launching tsbx configure (set TSBX_AUTO_CONFIGURE=0 to skip)…"
    if [[ -e /dev/tty && -r /dev/tty && -w /dev/tty ]]; then
      if "$INSTALL_DIR/tsbx" configure </dev/tty >/dev/tty 2>/dev/tty; then
        success "Configuration saved"
      else
        warn "Automatic configuration failed; run 'tsbx configure' manually"
      fi
    else
      warn "No interactive TTY detected; run 'tsbx configure' once installation finishes"
    fi
  else
    echo "Run: tsbx configure"
  fi
fi
