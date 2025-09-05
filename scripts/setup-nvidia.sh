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
BLUE='\033[0,34m'
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

  local distribution
  distribution=$(. /etc/os-release; echo ${ID}${VERSION_ID})
  curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | \
    gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg

  curl -s -L "https://nvidia.github.io/libnvidia-container/stable/${distribution}/libnvidia-container.list" | \
    sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#' | \
    tee /etc/apt/sources.list.d/nvidia-container-toolkit.list >/dev/null

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

