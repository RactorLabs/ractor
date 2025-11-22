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
- Host + inference providers/models defined in `~/.tsbx/tsbx.json` (copy `config/tsbx.sample.json`)

## Quick Setup

1. **Install or link the CLI**
   ```bash
   npm install -g ./cli        # from this repo
   # or
   npm install -g @tsbx/cli
   # for local changes
   ./scripts/link.sh
   ```

2. **Configure host + inference providers**
   - Copy `config/tsbx.sample.json` to `~/.tsbx/tsbx.json` (or supply `--config <path>` when running `tsbx start`).
   - Fill in the `host` block plus each provider’s `url`, supported `models`, and (optionally) `default_model`. The first provider marked `"default": true` becomes the default selection for new sandboxes.
   - Individual sandboxes can supply their own inference API key during creation; NL tasks remain disabled for sandboxes that launch without a key.

3. **Start the core services**
   ```bash
   tsbx start
   ```
   Pass component names (e.g., `tsbx start api controller`) if you want to launch a subset.

4. **Visit the Operator UI**  
   Open the `host.url` defined in your `tsbx.json` (defaults to <http://localhost>) to browse sandboxes, launch tasks, and monitor activity. The REST API is available at `<host>/api`.

> If something misbehaves, run `tsbx doctor` or `tsbx fix` from the CLI for guided troubleshooting.

## Configuration File

TSBX reads all branding + inference metadata from a single JSON file (default `~/.tsbx/tsbx.json`, override with `tsbx start --config <path>`). Use `config/tsbx.sample.json` as a starting point:

```json
{
  "host": {
    "name": "TSBX",
    "url": "http://localhost"
  },
  "inference_providers": [
    {
      "name": "Positron",
      "display_name": "Positron",
      "url": "https://api.positron.ai/v1/chat/completions",
      "default": true,
      "default_model": "llama-3.2-3b-instruct-fast-tp2",
      "models": [
        { "name": "llama-3.2-3b-instruct-fast-tp2", "display_name": "Llama 3.2 3B (fast)" },
        { "name": "llama-3.1-8b-instruct-good-tp2", "display_name": "Llama 3.1 8B (quality)" }
      ]
    }
  ]
}
```

- `host.name` / `host.url` drive Operator UI branding and task links.
- Each provider must define at least one model. `default_model` is optional; if omitted, the first model becomes the default.
- The Controller injects provider URL/model into each sandbox; Operator UI fetches the list via `GET /api/v0/inference/providers`.
