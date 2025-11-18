#!/usr/bin/env bash
set -euo pipefail

SOURCE_REF="${TSBX_SOURCE_REF:-tsbx-installation}"
INSTALL_URL="https://raw.githubusercontent.com/RactorLabs/tsbx/${SOURCE_REF}/scripts/install.sh"
REPO_DIR="${TSBX_REPO_DIR:-${HOME}/repos/tsbx}"
CONFIG_DIR="${HOME}/.config/tsbx"
CONFIG_FILE="${CONFIG_DIR}/config.json"
PROMPT_CONFIG="${TSBX_PROMPT_CONFIG:-1}"
SKIP_LINK="${TSBX_SKIP_LINK:-0}"
AUTO_START="${TSBX_AUTO_START:-1}"
AUTO_CONFIG="${TSBX_AUTO_CONFIGURE:-1}"
CONFIG_PRESEEDED=0

info() { printf '\033[34m[INFO]\033[0m %s\n' "$1"; }
success() { printf '\033[32m[SUCCESS]\033[0m %s\n' "$1"; }
warn() { printf '\033[33m[WARN]\033[0m %s\n' "$1"; }
error() { printf '\033[31m[ERROR]\033[0m %s\n' "$1" >&2; }

ensure_tty() {
  if [[ -n "${BOOTSTRAP_TTY_IN:-}" && -n "${BOOTSTRAP_TTY_OUT:-}" ]]; then
    return
  fi
  if [[ -r /dev/tty && -w /dev/tty ]]; then
    BOOTSTRAP_TTY_IN="/dev/tty"
    BOOTSTRAP_TTY_OUT="/dev/tty"
    return
  fi
  if [[ -t 0 && -t 1 ]]; then
    BOOTSTRAP_TTY_IN="/dev/stdin"
    BOOTSTRAP_TTY_OUT="/dev/stdout"
    return
  fi
  error "No interactive TTY detected. Run this script directly from a terminal."
  exit 1
}

prompt_value() {
  ensure_tty
  local __var="$1"
  local __label="$2"
  local __default="${3:-}"
  local __required="${4:-0}"
  local __value=""
  while true; do
    if [[ -n "$__default" ]]; then
      printf "%s [%s]: " "$__label" "$__default" >"$BOOTSTRAP_TTY_OUT"
    else
      printf "%s: " "$__label" >"$BOOTSTRAP_TTY_OUT"
    fi
    if ! IFS= read -r __value <"$BOOTSTRAP_TTY_IN"; then
      error "Failed to read input from terminal"
      exit 1
    fi
    if [[ -z "$__value" ]]; then
      __value="$__default"
    fi
    if [[ "$__required" == "1" && -z "$__value" ]]; then
      printf "Value is required.\n" >"$BOOTSTRAP_TTY_OUT"
      continue
    fi
    printf -v "$__var" '%s' "$__value"
    break
  done
}

normalize_path() {
  local input="$1"
  python3 - "$input" <<'PY'
import os
import sys
path = sys.argv[1]
print(os.path.abspath(os.path.expanduser(path)))
PY
}

generate_uuid() {
  python3 - <<'PY'
import uuid
print(uuid.uuid4())
PY
}

load_existing_config() {
  if [[ ! -f "$CONFIG_FILE" ]]; then
    return
  fi
  eval "$(
    python3 - "$CONFIG_FILE" <<'PY'
import json, shlex, sys
from pathlib import Path
path = Path(sys.argv[1])
if not path.exists():
    sys.exit(0)
try:
    data = json.loads(path.read_text())
except json.JSONDecodeError:
    sys.exit(0)
def emit(key, env_var):
    value = data.get(key, "")
    print(f'{env_var}={shlex.quote(value)}')
emit("provider_name", "CURRENT_PROVIDER_NAME")
emit("inference_url", "CURRENT_INFERENCE_URL")
emit("default_model", "CURRENT_DEFAULT_MODEL")
emit("api_key", "CURRENT_API_KEY")
emit("sandbox_dir", "CURRENT_SANDBOX_DIR")
emit("tsbx_api_url", "CURRENT_TSBX_API_URL")
emit("sandbox_id", "CURRENT_SANDBOX_ID")
emit("tsbx_token", "CURRENT_TSBX_TOKEN")
emit("created_at", "CURRENT_CREATED_AT")
PY
  )"
}

write_config_file() {
  mkdir -p "$CONFIG_DIR"
  local now
  now="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  local created="${CURRENT_CREATED_AT:-$now}"
  BOOT_PROVIDER_NAME="$PROVIDER_NAME" \
  BOOT_INFERENCE_URL="$INFERENCE_URL" \
  BOOT_DEFAULT_MODEL="$DEFAULT_MODEL" \
  BOOT_API_KEY="$API_KEY" \
  BOOT_SANDBOX_DIR="$SANDBOX_DIR" \
  BOOT_TSBX_API_URL="$TSBX_API_URL" \
  BOOT_SANDBOX_ID="$SANDBOX_ID" \
  BOOT_TSBX_TOKEN="$TSBX_TOKEN" \
  BOOT_CREATED_AT="$created" \
  BOOT_UPDATED_AT="$now" \
  python3 - "$CONFIG_FILE" <<'PY'
import json
import os
import sys

path = sys.argv[1]
cfg = {
    "provider_name": os.environ["BOOT_PROVIDER_NAME"],
    "inference_url": os.environ["BOOT_INFERENCE_URL"],
    "default_model": os.environ["BOOT_DEFAULT_MODEL"],
    "api_key": os.environ["BOOT_API_KEY"],
    "created_at": os.environ["BOOT_CREATED_AT"],
    "updated_at": os.environ["BOOT_UPDATED_AT"],
    "sandbox_dir": os.environ["BOOT_SANDBOX_DIR"],
    "tsbx_api_url": os.environ["BOOT_TSBX_API_URL"],
    "sandbox_id": os.environ["BOOT_SANDBOX_ID"],
    "tsbx_token": os.environ["BOOT_TSBX_TOKEN"],
}
with open(path, "w", encoding="utf-8") as fh:
    json.dump(cfg, fh, indent=2)
os.chmod(path, 0o600)
PY
  info "Wrote configuration to ${CONFIG_FILE}"
}

collect_config() {
  if [[ "$PROMPT_CONFIG" != "1" ]]; then
    info "Skipping interactive config prompts (TSBX_PROMPT_CONFIG=0)"
    return
  fi

  command -v python3 >/dev/null 2>&1 || { error "python3 is required for interactive prompts"; exit 1; }
  load_existing_config

  info "Let's capture your TSBX configuration before installing the CLI."
  local default_workspace="${CURRENT_SANDBOX_DIR:-$REPO_DIR}"
  local default_api_url="${CURRENT_TSBX_API_URL:-http://localhost:9000}"
  local default_sandbox_id="${CURRENT_SANDBOX_ID:-$(generate_uuid)}"

  prompt_value PROVIDER_NAME "Provider name" "${CURRENT_PROVIDER_NAME:-}" 0
  prompt_value INFERENCE_URL "Inference API URL" "${CURRENT_INFERENCE_URL:-}" 1
  prompt_value DEFAULT_MODEL "Default model" "${CURRENT_DEFAULT_MODEL:-}" 0
  prompt_value API_KEY "Provider API key" "${CURRENT_API_KEY:-}" 1
  prompt_value SANDBOX_DIR_RAW "Sandbox workspace directory" "${default_workspace}" 1
  prompt_value TSBX_API_URL "TSBX API URL" "${default_api_url}" 1
  prompt_value SANDBOX_ID "Sandbox ID" "${default_sandbox_id}" 1
  prompt_value TSBX_TOKEN "TSBX API token (TSBX_TOKEN)" "${CURRENT_TSBX_TOKEN:-}" 1

  SANDBOX_DIR="$(normalize_path "$SANDBOX_DIR_RAW")"
  write_config_file
  CONFIG_PRESEEDED=1
}

run_installer() {
  info "Downloading installer (${SOURCE_REF})…"
  local tmp_file
  tmp_file="$(mktemp)"
  curl -fsSL "$INSTALL_URL" -o "$tmp_file"
  chmod +x "$tmp_file"

  local installer_auto_config="$AUTO_CONFIG"
  if [[ "${CONFIG_PRESEEDED:-0}" == "1" ]]; then
    installer_auto_config=0
  fi

  info "Installing tsbx CLI (auto-configure=${installer_auto_config})…"
  if ! TSBX_SOURCE_REF="$SOURCE_REF" TSBX_AUTO_CONFIGURE="$installer_auto_config" "$tmp_file"; then
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

start_sandbox() {
  if [[ "$AUTO_START" != "1" ]]; then
    info "Skipping automatic sandbox start (TSBX_AUTO_START=0)"
    return
  fi
  info "Starting sandbox via 'tsbx start'"
  if ! tsbx start; then
    warn "tsbx start failed; check ~/.config/tsbx/logs for details and rerun manually"
  fi
}

main() {
  command -v curl >/dev/null 2>&1 || { error "curl is required"; exit 1; }
  collect_config

  export PATH="${HOME}/.local/bin:${PATH}"
  run_installer

  if ! command -v tsbx >/dev/null 2>&1; then
    error "tsbx was not found on PATH after installation"
    exit 1
  fi

  info "tsbx version: $(tsbx --version)"
  link_repo
  start_sandbox
  success "Bootstrap complete"
}

main "$@"
