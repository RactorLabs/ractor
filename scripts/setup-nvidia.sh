#!/usr/bin/env bash

set -euo pipefail

# NVIDIA GPU + Container Toolkit setup helper (Debian/Ubuntu)
# - Installs NVIDIA GPU driver (Ubuntu: ubuntu-drivers autoinstall)
# - Installs NVIDIA Container Toolkit for Docker
# - Validates with nvidia-smi and test CUDA container
#
# Usage examples:
#   bash scripts/setup-nvidia.sh --check
#   sudo bash scripts/setup-nvidia.sh --install
#   sudo bash scripts/setup-nvidia.sh --driver-only
#   sudo bash scripts/setup-nvidia.sh --toolkit-only
#   bash scripts/setup-nvidia.sh --validate

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()    { echo -e "${BLUE:-}[INFO]${NC:-} $*"; }
ok()      { echo -e "${GREEN:-}[OK]${NC:-}   $*"; }
warn()    { echo -e "${YELLOW:-}[WARN]${NC:-} $*"; }
err()     { echo -e "${RED:-}[ERR]${NC:-}  $*" >&2; }

need_root() {
  if [ "${EUID:-$(id -u)}" -ne 0 ]; then
    err "This action requires root. Re-run with: sudo $0 $*"
    exit 1
  fi
}

detect_os() {
  if [ -f /etc/os-release ]; then
    # shellcheck disable=SC1091
    . /etc/os-release
    OS_ID=${ID:-unknown}
    OS_VERSION_ID=${VERSION_ID:-unknown}
  else
    OS_ID=unknown
    OS_VERSION_ID=unknown
  fi
}

repair_nvidia_list_if_needed() {
  # Remove/backup malformed NVIDIA apt list that could break apt-get update
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
    warn "No NVIDIA GPU detected by lspci"
    return 1
  fi
}

check_current_state() {
  echo "== GPU Hardware =="
  lspci -nn | grep -E "VGA|3D|Display" || true
  echo
  echo "== NVIDIA Userspace =="
  if command -v nvidia-smi >/dev/null 2>&1; then
    ok "nvidia-smi found"
    nvidia-smi -L || true
    nvidia-smi --query-gpu=name,driver_version --format=csv,noheader || true
  else
    warn "nvidia-smi not found"
  fi
  echo
  echo "== Kernel Modules =="
  lsmod | grep -i nvidia || echo "nvidia kernel module not loaded"
  [ -f /proc/driver/nvidia/version ] && cat /proc/driver/nvidia/version || echo "/proc/driver/nvidia/version not present"
  echo
  echo "== /dev/dri =="
  ls -l /dev/dri || echo "/dev/dri not present"
}

install_driver_ubuntu() {
  need_root "$@"
  info "Installing NVIDIA driver on Ubuntu via ubuntu-drivers..."
  apt-get update
  apt-get install -y ubuntu-drivers-common
  ubuntu-drivers autoinstall
  ok "Driver installation command completed. A reboot is typically required."
}

install_driver_debian() {
  need_root "$@"
  info "Installing NVIDIA driver on Debian..."
  apt-get update
  apt-get install -y linux-headers-$(uname -r) firmware-misc-nonfree
  apt-get install -y nvidia-driver
  ok "Driver installation completed. A reboot is typically required."
}

install_nvidia_container_toolkit_deb() {
  need_root "$@"
  info "Installing NVIDIA Container Toolkit for Docker..."
  if ! command -v docker >/dev/null 2>&1; then
    err "Docker is not installed. Install Docker first and re-run."
    exit 1
  fi

  # Ensure required tools for keyring/repo setup are present
  apt-get update
  apt-get install -y ca-certificates curl gnupg >/dev/null 2>&1 || \
    apt-get install -y ca-certificates curl gnupg

  # Repair any previously malformed NVIDIA list
  local list_path
  list_path=/etc/apt/sources.list.d/nvidia-container-toolkit.list
  if [ -f "$list_path" ]; then
    if grep -qiE '^\s*<!doctype|^\s*<html' "$list_path" || ! grep -qE '^\s*deb\s' "$list_path"; then
      warn "Existing NVIDIA apt list at $list_path looks invalid; backing up and replacing."
      mv "$list_path" "${list_path}.bak.$(date +%s)" || true
    fi
  fi

  local distribution
  # Allow manual override (e.g., NVIDIA_DISTRIBUTION=ubuntu24.04)
  if [ -n "${NVIDIA_DISTRIBUTION:-}" ]; then
    distribution="$NVIDIA_DISTRIBUTION"
  else
    distribution=$(. /etc/os-release; echo ${ID}${VERSION_ID})
  fi

  # Prepare keyring (write to temp then move into place to avoid prompts)
  local tmp_keyring
  tmp_keyring=$(mktemp)
  if ! curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | \
      gpg --dearmor --batch --yes -o "$tmp_keyring"; then
    rm -f "$tmp_keyring"
    err "Failed to fetch or dearmor NVIDIA GPG key."
    exit 1
  fi
  install -Dm644 "$tmp_keyring" /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg
  rm -f "$tmp_keyring"

  # Resolve a supported NVIDIA repo distribution if the exact one is unavailable
  local resolved_distribution
  resolved_distribution="$(
    bash -c '
      set -euo pipefail
      want="'"${distribution}"'"
      id=$(echo "$want" | sed -E "s/([a-zA-Z]+).*/\1/")
      ver=$(echo "$want" | sed -E "s/[a-zA-Z]+(.*)/\1/")
      # Candidate list to try (exact first)
      candidates=("$id$ver")
      case "$id" in
        ubuntu)
          # Try known supported Ubuntu versions in descending order up to requested
          # Update this list periodically as NVIDIA adds support
          known=(24.04 22.04 20.04)
          for k in "${known[@]}"; do
            # Only include versions <= requested, if requested is numeric
            if echo "$ver" | grep -Eq "^[0-9]+\.[0-9]+$"; then
              # compare as sort -V order
              if printf "%s\n%s\n" "$k" "$ver" | sort -V | head -n1 | grep -qx "$k"; then
                candidates+=("ubuntu$k")
              fi
            else
              candidates+=("ubuntu$k")
            fi
          done
          ;;
        debian)
          # Try major versions
          known=(12 11)
          for k in "${known[@]}"; do
            candidates+=("debian$k")
          done
          ;;
      esac
      # De-duplicate candidates while preserving order
      uniq=()
      for c in "${candidates[@]}"; do
        skip=
        for u in "${uniq[@]}"; do [ "$u" = "$c" ] && { skip=1; break; }; done
        [ -z "$skip" ] && uniq+=("$c")
      done
      # Probe each candidate URL for availability
      for c in "${uniq[@]}"; do
        code=$(curl -fsIL -o /dev/null -w "%{http_code}" "https://nvidia.github.io/libnvidia-container/stable/${c}/libnvidia-container.list" || true)
        if [ "$code" = "200" ]; then echo "$c"; exit 0; fi
      done
      exit 1
    ' 2>/dev/null || true
  )"

  if [ -z "$resolved_distribution" ]; then
    err "Failed to find a supported NVIDIA apt list for '${distribution}'."
    err "You can set NVIDIA_DISTRIBUTION=ubuntu24.04 or similar and retry."
    exit 1
  fi
  info "Using NVIDIA repo distribution: ${resolved_distribution} (requested: ${distribution})"

  # Download the apt list safely to a temp file and validate before installing
  local tmp_list
  tmp_list=$(mktemp)
  if ! curl -fsSL "https://nvidia.github.io/libnvidia-container/stable/${resolved_distribution}/libnvidia-container.list" -o "$tmp_list"; then
    err "Failed to download NVIDIA apt list for '${resolved_distribution}'."
    rm -f "$tmp_list"
    exit 1
  fi
  if grep -qiE '^\s*<!doctype|^\s*<html' "$tmp_list" || ! grep -qE '^\s*deb\s' "$tmp_list"; then
    err "Downloaded NVIDIA apt list appears invalid for '${resolved_distribution}'."
    rm -f "$tmp_list"
    exit 1
  fi

  # Inject signed-by attribute and install to sources.list.d
  sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#' "$tmp_list" > /etc/apt/sources.list.d/nvidia-container-toolkit.list
  rm -f "$tmp_list"
  if [ "$resolved_distribution" != "$distribution" ]; then
    warn "Using NVIDIA repo for '${resolved_distribution}' (fallback from '${distribution}')."
  fi

  apt-get update
  apt-get install -y nvidia-container-toolkit

  # Configure Docker runtime
  if command -v nvidia-ctk >/dev/null 2>&1; then
    nvidia-ctk runtime configure --runtime=docker
  else
    warn "nvidia-ctk not found; attempting legacy config via daemon.json"
    jq_installed=true
    if ! command -v jq >/dev/null 2>&1; then jq_installed=false; fi
    DAEMON_JSON=/etc/docker/daemon.json
    mkdir -p /etc/docker
    if [ -f "$DAEMON_JSON" ] && [ "$jq_installed" = true ]; then
      tmp=$(mktemp)
      jq '."default-runtime" = "nvidia" | .runtimes.nvidia.path = "/usr/bin/nvidia-container-runtime" | .runtimes.nvidia.runtimeArgs = []' "$DAEMON_JSON" > "$tmp" || true
      mv "$tmp" "$DAEMON_JSON"
    else
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
  fi

  systemctl restart docker || true
  ok "NVIDIA Container Toolkit installed and Docker configured"
}

validate() {
  info "Validating NVIDIA setup..."
  if command -v nvidia-smi >/dev/null 2>&1; then
    nvidia-smi || true
  else
    warn "nvidia-smi not found in PATH (driver may require reboot)"
  fi
  if command -v docker >/dev/null 2>&1; then
    info "Running CUDA base image to test GPU access..."
    docker run --rm --gpus all nvidia/cuda:12.4.1-base-ubuntu22.04 nvidia-smi || warn "CUDA container test failed"
  else
    warn "Docker not installed; skipping container validation"
  fi
}

main() {
  local ACTION=""
  if [ $# -eq 0 ]; then
    echo "Usage: $0 [--check|--install|--driver-only|--toolkit-only|--validate]"
    exit 1
  fi
  while [ $# -gt 0 ]; do
    case "$1" in
      --check) ACTION="check" ; shift ;;
      --install) ACTION="install" ; shift ;;
      --driver-only) ACTION="driver" ; shift ;;
      --toolkit-only) ACTION="toolkit" ; shift ;;
      --validate) ACTION="validate" ; shift ;;
      *) err "Unknown option: $1" ; exit 1 ;;
    esac
  done

  detect_os
  info "Detected OS: ${OS_ID:-unknown} ${OS_VERSION_ID:-}"

  case "$ACTION" in
    check)
      check_current_state
      ;;
    install)
      repair_nvidia_list_if_needed || true
      check_gpu_presence || warn "Continuing anyway (you may be on a VM)"
      case "$OS_ID" in
        ubuntu) install_driver_ubuntu ;;
        debian) install_driver_debian ;;
        *) err "Unsupported OS for automated driver install: $OS_ID"; exit 2 ;;
      esac
      install_nvidia_container_toolkit_deb
      warn "A reboot is usually required after driver installation. Please reboot, then run: $0 --validate"
      ;;
    driver)
      repair_nvidia_list_if_needed || true
      check_gpu_presence || warn "Continuing anyway"
      case "$OS_ID" in
        ubuntu) install_driver_ubuntu ;;
        debian) install_driver_debian ;;
        *) err "Unsupported OS for automated driver install: $OS_ID"; exit 2 ;;
      esac
      warn "Reboot is recommended before using the GPU."
      ;;
    toolkit)
      install_nvidia_container_toolkit_deb
      ;;
    validate)
      validate
      ;;
  esac
}

main "$@"
