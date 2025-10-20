<p align="center">
  <img src="assets/logo.png" alt="Ractor logo" width="140" />
</p>
<h1 align="center">Ractor</h1>

<p align="center">
  <a href="https://ractorlabs.com/"><img src="https://img.shields.io/badge/website-ractorlabs.com-0A66C2?logo=google-chrome&logoColor=white" alt="Website" /></a>
  <a href="https://x.com/ractorlabs"><img src="https://img.shields.io/badge/Follow-@ractorlabs-000000?logo=x&logoColor=white" alt="Follow on X" /></a>
  <a href="https://discord.gg/jTpP6PgZtt"><img src="https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white" alt="Discord" /></a>
  <a href="https://github.com/Ractorlabs/ractor/releases"><img src="https://img.shields.io/github/v/release/Ractorlabs/ractor?display_name=tag&sort=semver" alt="Release" /></a>
  <a href="https://github.com/Ractorlabs/ractor/actions/workflows/build.yml"><img src="https://github.com/Ractorlabs/ractor/actions/workflows/build.yml/badge.svg" alt="Build" /></a>

</p>

## What is Ractor

Ractor is a Rust-first platform for orchestrating long-lived, stateful agent sessions. It provisions Docker-isolated workspaces with persistent storage, wires them to Ollama-powered tooling, and exposes a CLI, REST API, and Operator UI so teams can automate and supervise computer-use workflows.

## Why Ractor

- Session isolation & persistence — Each session runs inside a managed Docker container with a dedicated `/session` volume and private `.env`, created by the controller and reused across sleep/wake cycles.
- Built-in agent tooling — The session runtime ships a tool registry (bash execution, file editing, plan management, publish/sleep helpers, etc.) so agents can automate real workflows safely.
- Observability & lifecycle control — Controller and session services emit structured tracing logs, while the Operator UI surfaces status, timers, and lifecycle actions (sleep, wake, remix, publish) for operators.
- API coverage — The Rust API service exposes REST endpoints for sessions, responses, operators, files, and auth, enabling external orchestration or integration.
- LLM integration — Sessions talk to Ollama via `OLLAMA_HOST`/`OLLAMA_MODEL`, with GPU/CPU toggles and model pre-pull support driven by the CLI.
- Unified CLI workflow — The Node.js `ractor` CLI manages MySQL, Ollama, API, Controller, Operator, Content, and Gateway containers with consistent branding and environment defaults.
- Portable dev→prod — Docker images built via `./scripts/build.sh` are the same ones the CLI pulls or runs in CI/CD, keeping local and production stacks aligned.
- Rust-first core — API, controller, session, and content services are Rust 2021 binaries with structured logging and consistent error handling.

## Requirements

- Docker (20.10+)
- Node.js 16+ and npm (Node 20 recommended)
- Rust 1.82+ (for local builds/tools)
- OS: Linux, macOS, or Windows (WSL2 for Windows dev). GPU host recommended on Linux (Ubuntu 22.04)
- GPU: NVIDIA H100 80GB recommended (A100 80GB / L40S 48GB work) with NVIDIA drivers and NVIDIA Container Toolkit

## Quick Start (GPU-required, model-first)

1) Prepare and verify GPU

```bash
# NVIDIA driver + NVIDIA Container Toolkit installed
# Verify GPU access from Docker:
docker run --rm --gpus all nvidia/cuda:12.3.2-base-ubuntu22.04 nvidia-smi
```

2) Install the CLI

```bash
# From this repo
npm install -g ./cli
# or from npm
npm install -g @ractor/cli
```

3) Start the LLM and pre-pull the model

```bash
# Start only the LLM service on GPU with a model
ractor start --require-gpu --ollama-model gpt-oss:120b ollama

# Pre-pull to avoid first-request latency
docker exec ollama ollama pull gpt-oss:120b

# (Optional) tune resources for larger models
#   add to the command above: --ollama-memory 64g --ollama-shm-size 64g --ollama-context-length 131072
```

4) Configure host branding (optional; defaults to `Ractor` + `http://localhost` if unset) and any host overrides

```bash
# macOS/Linux
# Override defaults when you need custom branding or a non-localhost URL
export RACTOR_HOST_NAME="Acme Labs"
export RACTOR_HOST_URL="https://operator.acme.dev"
```

Optional: run the gateway on a different host port (update `RACTOR_HOST_URL` so links stay correct).

```bash
export RACTOR_GATEWAY_PORT=8080
export RACTOR_HOST_URL="http://localhost:8080"
```

If you previously started the gateway, run `ractor stop gateway` before `ractor start ...` so the new port mapping is applied.

5) Start Ractor core services

```bash
ractor start mysql api operator content controller gateway
```

6) Verify

- Operator UI: <http://localhost>
- API:  <http://localhost/api>
