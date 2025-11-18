#!/usr/bin/env bash
set -euo pipefail

SOURCE_REF="${TSBX_SOURCE_REF:-tsbx-installation}"
INSTALL_URL="https://raw.githubusercontent.com/RactorLabs/tsbx/${SOURCE_REF}/scripts/install.sh"
REPO_DIR="${TSBX_REPO_DIR:-${HOME}/repos/tsbx}"
SERVICES="${TSBX_SERVICES:-mysql api controller}"
AUTO_CONFIG="${TSBX_AUTO_CONFIGURE:-1}"
SKIP_LINK="${TSBX_SKIP_LINK:-0}"

info() { printf '\033[34m[INFO]\033[0m %s\n' "$1"; }
success() { printf '\033[32m[SUCCESS]\033[0m %s\n' "$1"; }
warn() { printf '\033[33m[WARN]\033[0m %s\n' "$1"; }
error() { printf '\033[31m[ERROR]\033[0m %s\n' "$1" >&2; }

run_installer() {
  info "Downloading installer (${SOURCE_REF})…"
  local tmp_file
  tmp_file="$(mktemp)"
  curl -fsSL "$INSTALL_URL" -o "$tmp_file"
  chmod +x "$tmp_file"

  info "Installing tsbx CLI (auto-configure=${AUTO_CONFIG})…"
  if ! TSBX_SOURCE_REF="$SOURCE_REF" TSBX_AUTO_CONFIGURE="$AUTO_CONFIG" "$tmp_file"; then
    rm -f "$tmp_file"
    error "Installer failed"
    exit 1
  fi
  rm -f "$tmp_file"
}

link_repo() {
  if [[ "$SKIP_LINK" == "1" ]]; then
    info "Skipping ./scripts/link.sh (TSBX_SKIP_LINK=1)"
    return
  fi

  if [[ ! -d "$REPO_DIR" ]]; then
    warn "Repo directory ${REPO_DIR} not found; skipping link step"
    return
  fi

  if [[ ! -x "${REPO_DIR}/scripts/link.sh" ]]; then
    warn "link.sh not executable in ${REPO_DIR}/scripts; skipping link step"
    return
  fi

  info "Linking CLI to repo at ${REPO_DIR}"
  (cd "$REPO_DIR" && ./scripts/link.sh)
}

start_services() {
  local trimmed="${SERVICES// }"
  if [[ -z "$trimmed" ]]; then
    info "TSBX_SERVICES is empty; skipping tsbx start"
    return
  fi

  info "Starting services with: tsbx start ${SERVICES}"
  tsbx start $SERVICES
}

main() {
  command -v curl >/dev/null 2>&1 || { error "curl is required"; exit 1; }

  export PATH="${HOME}/.local/bin:${PATH}"
  run_installer

  if ! command -v tsbx >/dev/null 2>&1; then
    error "tsbx was not found on PATH after installation"
    exit 1
  fi

  info "tsbx version: $(tsbx --version)"
  link_repo
  start_services
  success "Bootstrap complete"
}

main "$@"
