#!/usr/bin/env bash

# Raworc Host Setup
# - Installs NVIDIA GPU driver (Ubuntu/Debian)
# - Installs NVIDIA Container Toolkit and configures Docker GPU runtime
# - Safe repo/keyring handling and fallbacks for supported distributions
#
# Usage:
#   sudo ./scripts/setup.sh                 # Install drivers + toolkit
#   sudo ./scripts/setup.sh --driver-only   # Drivers only
#   sudo ./scripts/setup.sh --toolkit-only  # Toolkit only
#   ./scripts/setup.sh --help               # Help

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()    { echo -e "${BLUE:-}[INFO]${NC:-} $*"; }
warn()    { echo -e "${YELLOW:-}[WARN]${NC:-} $*"; }
ok()      { echo -e "${GREEN:-}[OK]${NC:-} $*"; }
err()     { echo -e "${RED:-}[ERROR]${NC:-} $*" 1>&2; }

require_root() {
  if [ "${EUID:-$(id -u)}" -ne 0 ]; then
    err "This script must be run as root (try: sudo $0 ...)"
    exit 1
  fi
}

detect_os() {
  if [ -f /etc/os-release ]; then
    . /etc/os-release
    OS_ID=$ID
    OS_VERSION_ID=$VERSION_ID
  else
    err "/etc/os-release not found; unsupported OS"
    exit 2
  fi
}

repair_nvidia_list_if_needed() {
  local list_path=/etc/apt/sources.list.d/nvidia-container-toolkit.list
  if [ -f "$list_path" ]; then
    if grep -qiE '^\s*<!doctype|^\s*<html' "$list_path" || ! grep -qE '^\s*deb\s' "$list_path"; then
      warn "Found malformed NVIDIA apt list at $list_path; backing up and removing before proceeding."
      mv "$list_path" "${list_path}.bak.$(date +%s)" || rm -f "$list_path"
    fi
  fi
}

check_gpu_presence() {
  info "Checking for NVIDIA GPU..."
  if lspci | grep -i nvidia >/dev/null 2>&1; then
    ok "NVIDIA GPU detected"
    return 0
  else
    warn "No NVIDIA GPU detected (OK for CPU-only)"
    return 1
  fi
}

install_driver_ubuntu() {
  info "Installing NVIDIA driver (Ubuntu)"
  apt-get update
  DEBIAN_FRONTEND=noninteractive apt-get install -y ubuntu-drivers-common
  ubuntu-drivers autoinstall || true
  ok "Driver installation attempted (reboot may be required)"
}

install_driver_debian() {
  info "Installing NVIDIA driver (Debian)"
  apt-get update
  DEBIAN_FRONTEND=noninteractive apt-get install -y nvidia-driver firmware-misc-nonfree || true
  ok "Driver installation attempted (reboot may be required)"
}

install_nvidia_container_toolkit_deb() {
  info "Installing NVIDIA Container Toolkit"

  apt-get update
  apt-get install -y ca-certificates curl gnupg >/dev/null 2>&1 || \
    apt-get install -y ca-certificates curl gnupg

  # Repair any malformed list from prior runs
  repair_nvidia_list_if_needed || true

  local distribution
  if [ -n "${NVIDIA_DISTRIBUTION:-}" ]; then
    distribution="$NVIDIA_DISTRIBUTION"
  else
    distribution=$(. /etc/os-release; echo ${ID}${VERSION_ID})
  fi

  # Prepare keyring safely
  local tmp_keyring
  tmp_keyring=$(mktemp)
  if ! curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | gpg --dearmor --batch --yes -o "$tmp_keyring"; then
    rm -f "$tmp_keyring"
    err "Failed to fetch or dearmor NVIDIA GPG key"
    exit 1
  fi
  install -Dm644 "$tmp_keyring" /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg
  rm -f "$tmp_keyring"

  # Resolve supported distro list if exact not found
  local resolved_distribution
  resolved_distribution="$(
    bash -c '
      set -euo pipefail
      want="'"${distribution}"'"
      id=$(echo "$want" | sed -E "s/([a-zA-Z]+).*/\1/")
      ver=$(echo "$want" | sed -E "s/[a-zA-Z]+(.*)/\1/")
      candidates=("$id$ver")
      case "$id" in
        ubuntu) known=(24.04 22.04 20.04);;
        debian) known=(12 11);;
        *) known=();;
      esac
      for k in "${known[@]}"; do
        candidates+=("$id$k")
      done
      uniq=()
      for c in "${candidates[@]}"; do
        skip=
        for u in "${uniq[@]}"; do [ "$u" = "$c" ] && { skip=1; break; }; done
        [ -z "$skip" ] && uniq+=("$c")
      done
      for c in "${uniq[@]}"; do
        code=$(curl -fsIL -o /dev/null -w "%{http_code}" "https://nvidia.github.io/libnvidia-container/stable/${c}/libnvidia-container.list" || true)
        if [ "$code" = "200" ]; then echo "$c"; exit 0; fi
      done
      exit 1
    ' 2>/dev/null || true
  )"

  if [ -z "$resolved_distribution" ]; then
    err "Failed to find a supported NVIDIA apt list for '${distribution}'"
    exit 1
  fi
  info "Using NVIDIA repo distribution: ${resolved_distribution} (requested: ${distribution})"

  local tmp_list
  tmp_list=$(mktemp)
  if ! curl -fsSL "https://nvidia.github.io/libnvidia-container/stable/${resolved_distribution}/libnvidia-container.list" -o "$tmp_list"; then
    err "Failed to download NVIDIA apt list for '${resolved_distribution}'"
    rm -f "$tmp_list"
    exit 1
  fi
  if grep -qiE '^\s*<!doctype|^\s*<html' "$tmp_list" || ! grep -qE '^\s*deb\s' "$tmp_list"; then
    err "Downloaded NVIDIA apt list appears invalid for '${resolved_distribution}'"
    rm -f "$tmp_list"
    exit 1
  fi

  sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#' "$tmp_list" > /etc/apt/sources.list.d/nvidia-container-toolkit.list
  rm -f "$tmp_list"

  apt-get update
  apt-get install -y nvidia-container-toolkit

  # Configure Docker runtime
  if command -v nvidia-ctk >/dev/null 2>&1; then
    nvidia-ctk runtime configure --runtime=docker
  else
    warn "nvidia-ctk not found; attempting legacy config via daemon.json"
    local DAEMON_JSON=/etc/docker/daemon.json
    mkdir -p /etc/docker
    cat > "$DAEMON_JSON" << 'JSON'
{
  "default-runtime": "nvidia",
  "runtimes": {
    "nvidia": {
      "path": "/usr/bin/nvidia-container-runtime",
      "runtimeArgs": []
    }
  }
}
JSON
  fi

  systemctl restart docker || true
  ok "NVIDIA Container Toolkit installed and Docker configured"
}

main() {
  if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    cat << USAGE
Usage: sudo $0 [--driver-only|--toolkit-only]

Without flags installs both GPU driver (Ubuntu/Debian) and NVIDIA Container Toolkit.
USAGE
    exit 0
  fi

  require_root
  detect_os
  info "Detected OS: ${OS_ID:-unknown} ${OS_VERSION_ID:-}"

  local DO_DRIVER=true
  local DO_TOOLKIT=true
  case "${1:-}" in
    --driver-only) DO_TOOLKIT=false ;;
    --toolkit-only) DO_DRIVER=false ;;
    "") : ;;
    *) warn "Unknown option '$1' ignored" ;;
  esac

  repair_nvidia_list_if_needed || true
  check_gpu_presence || true

  if [ "$DO_DRIVER" = true ]; then
    case "$OS_ID" in
      ubuntu) install_driver_ubuntu ;;
      debian) install_driver_debian ;;
      *) warn "Unsupported OS for automated driver install: $OS_ID" ;;
    esac
    warn "A reboot is usually required after driver installation."
  fi

  if [ "$DO_TOOLKIT" = true ]; then
    install_nvidia_container_toolkit_deb
  fi

  ok "Setup completed"
}

main "$@"

