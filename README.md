<p align="center">
  <img src="assets/logo.png" alt="TaskSandbox logo" width="140" />
</p>
<h1 align="center">TaskSandbox</h1>

<p align="center">
  <a href="https://ractorlabs.com/"><img src="https://img.shields.io/badge/website-ractorlabs.com-0A66C2?logo=google-chrome&logoColor=white" alt="Website" /></a>
  <a href="https://x.com/ractorlabs"><img src="https://img.shields.io/badge/Follow-@ractorlabs-000000?logo=x&logoColor=white" alt="Follow on X" /></a>
  <a href="https://discord.gg/jTpP6PgZtt"><img src="https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white" alt="Discord" /></a>
  <a href="https://github.com/ractorlabs/tsbx/releases"><img src="https://img.shields.io/github/v/release/ractorlabs/tsbx?display_name=tag&sort=semver" alt="Release" /></a>
  <a href="https://github.com/ractorlabs/tsbx/actions/workflows/build.yml"><img src="https://github.com/ractorlabs/tsbx/actions/workflows/build.yml/badge.svg" alt="Build" /></a>

</p>

## What is TaskSandbox

TaskSandbox is a Rust-first platform for orchestrating long-lived, stateful agent sandboxes. It provisions Docker-isolated workspaces with persistent storage, wires them to Ollama-powered tooling, and exposes a CLI, REST API, and Operator UI so teams can automate and supervise computer-use workflows.

## Why TaskSandbox

- Sandbox isolation & persistence — Each sandbox runs inside a managed Docker container with a dedicated `/sandbox` volume and private `.env`, created by the controller and reused across stop/restart cycles.
- Built-in agent tooling — The sandbox runtime ships a tool registry (bash execution, file editing, plan management, publish/stop helpers, etc.) so agents can automate real workflows safely.
- Observability & lifecycle control — Controller and sandbox services emit structured tracing logs, while the Operator UI surfaces status, timers, and lifecycle actions (stop, restart, remix, publish) for operators.
- API coverage — The Rust API service exposes REST endpoints for sandboxes, tasks, operators, files, and auth, enabling external orchestration or integration.
- LLM integration — Sessions talk to Ollama via `OLLAMA_HOST` and the configurable `TSBX_DEFAULT_MODEL`, with GPU/CPU toggles and model pre-pull support driven by the CLI.
- Unified CLI workflow — The Node.js `tsbx` CLI manages MySQL, Ollama, API, Controller, Operator, Content, and Gateway containers with consistent branding and environment defaults.
- Portable dev→prod — Docker images built via `./scripts/build.sh` are the same ones the CLI pulls or runs in CI/CD, keeping local and production stacks aligned.
- Rust-first core — API, controller, sandbox, and content services are Rust 2021 binaries with structured logging and consistent error handling.

## Requirements

- Docker (20.10+)
- Node.js 16+ and npm (Node 20 recommended)
- Rust 1.82+ (only required for contributors building the Rust services locally)
- OS: Linux only (Ubuntu 22.04 LTS recommended at the moment)
- GPU: NVIDIA H100 80GB recommended (A100 80GB / L40S 48GB work) with NVIDIA drivers and NVIDIA Container Toolkit

> macOS and Windows hosts are not yet supported; use a Linux workstation or server (Ubuntu 22.04 LTS recommended).

## Quick Start (GPU-required, model-first)

1) Install the CLI

```bash
# From this repo
npm install -g ./cli
# or from npm
npm install -g @tsbx/cli
```

2) Verify host prerequisites

```bash
tsbx doctor
```

- If any checks fail, run `tsbx fix` (with `--pull` or other flags as needed) and re-run `tsbx doctor`.

3) (Optional) Warm up the LLM

```bash
# Start only the LLM service on GPU with the default 20B model
tsbx start --require-gpu --default-model gpt-oss:20b ollama

# Pre-pull to avoid first-request latency
docker exec ollama ollama pull gpt-oss:20b

# (Optional) tune resources for larger models
#   add to the command above: --ollama-memory 64g --ollama-shm-size 64g --ollama-context-length 131072
```

> Need the 120B model? Set `TSBX_DEFAULT_MODEL=gpt-oss:120b` (or pass `--default-model`) before `tsbx start` to override the default.

4) Configure host branding (optional; defaults to `TaskSandbox` + `http://localhost` if unset) and any host overrides

```bash
# macOS/Linux
# Override defaults when you need custom branding or a non-localhost URL
export TSBX_HOST_NAME="Acme Labs"
export TSBX_HOST_URL="https://operator.acme.dev"
```

Optional: run the exposed host port on a value other than `80` (update `TSBX_HOST_URL` so links stay correct).

```bash
export TSBX_HOST_PORT=8080
export TSBX_HOST_URL="http://localhost:8080"
```

If you previously started the gateway, run `tsbx stop gateway` before `tsbx start ...` so the new port mapping is applied.

5) Start TaskSandbox core services

```bash
tsbx start
```

6) Verify

- Operator UI: <http://localhost>
- API:  <http://localhost/api>
