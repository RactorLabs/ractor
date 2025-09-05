#!/usr/bin/env bash

# Raworc Doctor
# - Verifies host readiness for Raworc + GPU workloads
# - Prints status for: OS, GPU, drivers, Docker, NVIDIA runtime, container GPU test

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

ok()   { echo -e "${GREEN:-}✓${NC:-} $*"; }
bad()  { echo -e "${RED:-}✗${NC:-} $*"; }
warn() { echo -e "${YELLOW:-}!${NC:-} $*"; }
info() { echo -e "${BLUE:-}i${NC:-} $*"; }

detect_os() {
  if [ -f /etc/os-release ]; then
    . /etc/os-release
    OS_ID=$ID
    OS_VERSION_ID=$VERSION_ID
    ok "OS detected: ${PRETTY_NAME:-$ID $VERSION_ID}"
  else
    warn "/etc/os-release not found; unknown OS"
  fi
}

check_gpu() {
  if lspci | grep -i nvidia >/dev/null 2>&1; then
    ok "NVIDIA GPU: present"
  else
    warn "NVIDIA GPU: not detected (CPU mode only)"
  fi
}

check_nvidia_smi() {
  if command -v nvidia-smi >/dev/null 2>&1; then
    if nvidia-smi >/dev/null 2>&1; then
      ok "nvidia-smi: working"
    else
      warn "nvidia-smi: found but not working (driver may need reboot)"
    fi
  else
    warn "nvidia-smi: not found in PATH"
  fi
}

check_docker() {
  if command -v docker >/dev/null 2>&1; then
    ok "Docker: installed ($(docker --version 2>/dev/null || echo unknown))"
  else
    bad "Docker: not installed"
    return 1
  fi
}

check_nvidia_runtime() {
  if docker info 2>/dev/null | grep -qi "Runtimes:"; then
    if docker info 2>/dev/null | grep -qi "nvidia"; then
      ok "Docker runtime: nvidia available"
    else
      warn "Docker runtime: nvidia NOT available"
    fi
  else
    warn "Docker info not accessible (daemon down?)"
  fi
}

test_cuda_container() {
  info "Testing CUDA container access (this may take a moment)..."
  if docker run --rm --gpus all nvidia/cuda:12.4.1-base-ubuntu22.04 nvidia-smi >/dev/null 2>&1; then
    ok "CUDA container test: success (GPU accessible)"
  else
    warn "CUDA container test: failed (GPU not accessible to containers)"
  fi
}

main() {
  detect_os || true
  check_gpu || true
  check_nvidia_smi || true
  local docker_ok=0
  if check_docker; then docker_ok=1; else docker_ok=0; fi
  if [ "$docker_ok" = 1 ]; then
    check_nvidia_runtime || true
    test_cuda_container || true
  fi

  echo
}

main "$@"
