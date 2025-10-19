<p align="center">
  <img src="assets/logo.png" alt="Ractor logo" width="140" />
</p>
<h1 align="center">Ractor</h1>


<p align="center">
  <a href="https://ractorlabs.com/"><img src="https://img.shields.io/badge/website-ractorlabs.com-0A66C2?logo=google-chrome&logoColor=white" alt="Website" /></a>
  <a href="https://x.com/ractorlabs"><img src="https://img.shields.io/badge/Follow-@ractorlabs-000000?logo=x&logoColor=white" alt="Follow on X" /></a>
  <a href="https://discord.gg/bUNKNtxey7"><img src="https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white" alt="Discord" /></a>
  <a href="https://github.com/Ractorlabs/ractor/releases"><img src="https://img.shields.io/github/v/release/Ractorlabs/ractor?display_name=tag&sort=semver" alt="Release" /></a>
  <a href="https://github.com/Ractorlabs/ractor/actions/workflows/build.yml"><img src="https://github.com/Ractorlabs/ractor/actions/workflows/build.yml/badge.svg" alt="Build" /></a>

</p>

## What is Ractor
Ractor is an infrastructure runtime for long-lived, stateful agent sessions. It turns computer-use workflows into durable, observable services with guardrails and an operator UI.

## Why Ractor
- Built-in sandbox — Per-session container and volume with file/network guardrails and a private `.env` to run code safely.
- Persistent memory — `/session` state, files, and logs survive restarts; no cold starts.
- Agent + tools in every session — An agent runtime with a tool registry (bash, file edits, package/env helpers, etc.).
- Observability — Structured service/request logs and per-session logs you can tail or ship.
- Operator-first — UI for status, logs, timeouts, and lifecycle actions (pause/kill/resume/sleep/wake).
- API-first — Clean REST endpoints for sessions, responses, files, operators, and auth.
- Portable dev→prod — Same Docker images locally and in CI/CD; one CLI to build/run.
- Rust core — Fast, memory-safe services with structured logging.

## Requirements
- Docker with Buildx (20.10+)
- Node.js 20+ and npm
- Rust 1.82+ (for local builds/tools)
- OS: Linux, macOS, or Windows (WSL2 for Windows dev). GPU host recommended on Linux (Ubuntu 22.04)
- GPU: NVIDIA H100 80GB recommended (A100 80GB / L40S 48GB work) with NVIDIA drivers and NVIDIA Container Toolkit
- LLM runtime: Ollama (the CLI runs it as container name `ollama`)

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

4) Configure host branding (required for the Operator UI)
```bash
# macOS/Linux
export RACTOR_HOST_NAME="Ractor"
export RACTOR_HOST_URL="http://localhost"

# Windows PowerShell
$env:RACTOR_HOST_NAME = "Ractor"
$env:RACTOR_HOST_URL  = "http://localhost"
```

5) Start Ractor core services
```bash
ractor start mysql api operator content controller gateway
```

6) Verify
- Operator UI: http://localhost
- API:  http://localhost/api
