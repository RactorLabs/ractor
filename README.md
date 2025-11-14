<p align="center">
  <img src="assets/logo.png" alt="TSBX logo" width="140" />
</p>
<h1 align="center">TSBX</h1>

## Overview

TSBX orchestrates long-lived, Docker-backed sandboxes for agent workflows. It bundles a Rust service stack, a Node.js CLI, and an Operator UI so teams can provision, monitor, and control persistent workspaces connected to an OpenAI-compatible inference endpoint.

## Requirements

- Linux host with Docker 20.10+
- Node.js 18+ and npm (for the CLI)
- Rust 1.82+ (only if you plan to build the Rust services locally)
- Inference endpoint exposed at `TSBX_INFERENCE_URL` with a valid API key and model name

## Quick Setup

1. **Install or link the CLI**
   ```bash
   npm install -g ./cli        # from this repo
   # or
   npm install -g @tsbx/cli
   # for local changes
   ./scripts/link.sh
   ```

2. **Provide inference credentials**
   ```bash
   export TSBX_INFERENCE_URL="https://api.positron.ai/v1/chat/completions"
   export TSBX_INFERENCE_API_KEY="replace-with-your-api-key"
   export TSBX_INFERENCE_MODEL="llama-3.2-3b-instruct-fast-tp2"
   ```

3. **Start the core services**
   ```bash
   tsbx start
   ```
   Pass component names (e.g., `tsbx start api controller`) if you want to launch a subset.

4. **Visit the Operator UI**  
   Open <http://localhost> (or your configured `TSBX_HOST_URL`) to browse sandboxes, launch tasks, and monitor activity. The REST API is available at `<host>/api`.
