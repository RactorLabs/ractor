<p align="center">
  <img src="assets/logo.png" alt="TSBX logo" width="140" />
</p>
<h1 align="center">TSBX</h1>

## Overview

TSBX orchestrates long-lived, Docker-backed sandboxes for agent workflows. It bundles a Rust service stack, a minimal Linux CLI, and an Operator UI so teams can provision, monitor, and control persistent workspaces connected to an OpenAI-compatible inference endpoint.

## Requirements

- Linux host with Docker 20.10+
- `bash`, `curl`, and `tar` (for the installer)
- Rust 1.82+ only if you plan to build the server binaries locally
- Inference endpoint exposed over HTTPS with a valid API key

## One-Command Bootstrap (Linux)

For a fully automated flow (pre-configure → install → link → start a sandbox), run:

```bash
curl -fsSL https://raw.githubusercontent.com/RactorLabs/tsbx/tsbx-installation/scripts/bootstrap.sh | bash
```

- The script assumes the repo lives at `~/repos/tsbx` (override with `TSBX_REPO_DIR=/path/to/tsbx`) and launches `./scripts/link.sh` plus a `tsbx start`. Before downloading anything it prompts for:
  - Provider info (name, inference URL, default model, API key)
  - Local workspace directory
  - TSBX API URL, sandbox ID, and TSBX API token (`TSBX_TOKEN`)
- Customize behavior with:
  - `TSBX_PROMPT_CONFIG=0` – skip pre-install prompts (the installer will fall back to `tsbx configure`).
  - `TSBX_AUTO_START=0` – skip the final `tsbx start`.
  - `TSBX_SKIP_LINK=1` – skip `./scripts/link.sh` (useful in CI).
  - `TSBX_SOURCE_REF=branch-or-tag` – pin to a specific installer ref.

If you prefer to run each step yourself, follow the manual quick-setup below.

## Quick Setup (Linux)

1. **Install the CLI**
   ```bash
   curl -fsSL https://raw.githubusercontent.com/RactorLabs/tsbx/tsbx-installation/scripts/get-tsbx.sh | bash
   ```
   The script downloads the latest `tsbx` binary to `~/.local/bin/tsbx`, creates `~/.config/tsbx/`, and immediately launches the interactive `tsbx configure` wizard (set `TSBX_AUTO_CONFIGURE=0` to skip).
   If a GitHub release asset is not available, it falls back to building the CLI from source; set `TSBX_SOURCE_REF=<branch>` before running the command if you need it to build from a specific branch.

2. **Capture provider + sandbox settings**
   ```bash
   tsbx configure
   ```
   Follow the prompts for provider info, the path to your local sandbox workspace, the TSBX API URL, sandbox ID, and the TSBX API token. The CLI writes `~/.config/tsbx/config.json` with `0600` permissions, and future `tsbx start` commands will `cd` into that workspace automatically while passing `--api-url`/`--sandbox-id` to `tsbx-sandbox`.

3. **Start a sandbox**
   ```bash
   tsbx start
   ```
   The CLI prints “Starting a new TSBX sandbox…” and launches the runtime (by default it runs `cargo run --release --bin tsbx-sandbox`). Boot logs land in `~/.config/tsbx/logs/`.

4. **Check the CLI version**
   ```bash
   tsbx version
   ```

### Commands

| Command          | Description                                                         |
| ---------------- | ------------------------------------------------------------------- |
| `tsbx start`     | Launches a sandbox and streams logs to `~/.config/tsbx/logs/`.      |
| `tsbx configure` | Interactive prompt that validates and stores provider credentials.  |
| `tsbx version`   | Prints the CLI version string.                                      |

Set `TSBX_SANDBOX_COMMAND` if you need to override the process that actually boots a sandbox (for example, `export TSBX_SANDBOX_COMMAND='./scripts/run_sandbox.sh'`).

### Building release binaries

Run the helper script to produce the archive expected by `scripts/install.sh`:

```bash
./scripts/package_binary.sh
```

It builds `cargo build --release --bin tsbx`, places the binary in `dist/linux/tsbx-linux-<arch>/`, and creates `tsbx-linux-<arch>.tar.gz`. Upload that tarball to your GitHub release (repeat on each architecture you plan to support, e.g., x86_64 and aarch64).
