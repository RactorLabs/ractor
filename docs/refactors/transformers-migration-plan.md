# Raworc: Migrate from Ollama to Hugging Face Transformers (raworc_gpt)

## Summary
- Replace the Ollama dependency and container with a new GPU‑accelerated `raworc_gpt` service that hosts Hugging Face Transformers, exposing a simple HTTP API backed by `model.generate()`.
- Keep “plain model communication” at the Python side — no Harmony/function‑call parsing in Python. All orchestration and any tool logic stays in the Rust Agent.
- Maintain the existing Raworc architecture: API, Controller, Agent, Content, Operator, and Gateway; only the model provider changes.

## Goals & Success Criteria
- A new Docker image `raworc/raworc_gpt` runs a Python FastAPI/Uvicorn server using Transformers to serve a configurable model (default `gpt-oss:120b`, with a practical dev fallback).
- GPU support via NVIDIA containers; data and logs persisted via named Docker volumes.
- Agents communicate with `raworc_gpt` via a simple HTTP endpoint (e.g., `POST /generate`), passing text and generation params; response returns generated text only.
- CLI (`raworc start`) manages the new `gpt` component similarly to Ollama today; deprecate but temporarily keep Ollama path for transition.
- Controller/Agent no longer depend on `OLLAMA_HOST`; new `RAWORC_GPT_URL` (or `GPT_HOST`) is propagated to agent containers.
- End‑to‑end flow works: create agent → send message → Agent queries GPT server with `generate()` → output stored as agent message → publishes content if requested.

## Non‑Goals
- No function/tool‑call JSON parsing on the Python side.
- No streaming protocol in v1 (optional future enhancement).
- No auto model sharding/LoRA management in v1.

## High‑Level Design

### New Service: `raworc_gpt` (Transformers Server)
- Base Image: CUDA‑enabled (e.g., `nvidia/cuda:12.2.0-cudnn8-runtime-ubuntu22.04`) or a PyTorch CUDA runtime base.
- App: FastAPI + Uvicorn server, single endpoint `POST /generate`.
- Model Load: `AutoModelForCausalLM` + `AutoTokenizer` (from model ID), move to GPU (`.to('cuda')`) when available.
- Inference: use `model.generate()` with standard parameters (max_new_tokens, temperature, top_p, top_k, repetition_penalty, do_sample, eos_token_id, pad_token_id, etc.).
- Request schema (example):
  - `prompt: string`
  - `max_new_tokens?: number`
  - `temperature?: number`
  - `top_p?: number`
  - `top_k?: number`
  - `repetition_penalty?: number`
  - `stop?: string[]`
- Response schema: `{ text: string, tokens?: number }` (keep minimal).
- Health: `GET /health` returns `{status:'ok'}`.
- Volumes: `raworc_gpt_data:/app/data`, `raworc_gpt_logs:/app/logs`.
- Observability: access logs; optional timing metrics in logs only.

### Agent Changes
- Replace `OllamaClient` with `GptClient` that calls `RAWORC_GPT_URL`.
- Conversation assembly, system prompt, and tool orchestration remain in Rust (MessageHandler). Python server returns plain text only.
- Remove Harmony formatting/aliases from the LLM path. Keep existing tool execution pipeline; in v1, tools are not triggered by LLM tool_calls.
  - Note: If tool triggering is required, handle it in Rust via prompt conventions or heuristic parsing (future decision).

### Controller Changes
- Stop requiring `OLLAMA_HOST`; require `RAWORC_GPT_URL`.
- When creating agent containers, inject `RAWORC_GPT_URL` instead of `OLLAMA_HOST`/`OLLAMA_MODEL`.
- Remove/remap Ollama‑specific env in `docker_manager` startup code.

### CLI Changes
- Add new component `gpt` to `raworc start`. Default stack uses `gpt` instead of `ollama`.
- Options:
  - `--gpt-model <hf_model_id>` (default: `gpt-oss:120b`, with dev fallback e.g., `gpt-oss:20b` or another feasible model)
  - `--gpt-enable-gpu` (default true), `--no-gpt-enable-gpu`
  - `--gpt-cpus <n>`, `--gpt-memory <bytes|g>`, `--gpt-shm-size <size>`
  - `--gpt-keep-alive <dur>` (optional; server process stays resident regardless)
- Export `RAWORC_GPT_URL=http://raworc_gpt:6000` (or similar) to API/Controller/Agent containers for internal routing.
- Keep `ollama` component for one release with a deprecation warning; add `--use-ollama` override for fallback.

### API & Operator
- No functional API changes required.
- Update Operator docs to reference “Transformers (raworc_gpt)” instead of Ollama, and reflect the new env var name in docs.

## Security & Configuration
- Keep GPT server internal on `raworc_network`; expose to host only if explicitly requested.
- Avoid embedding secrets in prompts; ensure server logs exclude prompt content by default in production.
- JWT/RBAC unchanged.

## Performance & Resource Notes
- `gpt-oss:120b` is resource‑intensive; provide practical dev defaults (e.g., smaller model) and document GPU memory expectations.
- Support BF16/FP16 and optional 4‑bit quantization via bitsandbytes where feasible (config toggle, not default).
- Timeouts: server default inference timeout (e.g., 600s); Agent request timeouts aligned (currently 600s defaults exist).

## Backward Compatibility & Rollout
1. Dual‑provider support (transition): Controller/Agent can accept either `RAWORC_GPT_URL` or `OLLAMA_HOST`. If both set, prefer GPT.
2. CLI starts `gpt` by default; `ollama` available via explicit flag.
3. One release later: remove Ollama code paths and flags.

## Testing & Validation
- Unit tests: new `GptClient`, Agent message flow without tool_calls.
- Integration: start GPT service, create agent, send message, ensure generated reply stored, busy→idle transitions, auto‑wake path unchanged.
- E2E smoke: `raworc start mysql gpt api controller operator content`, login, create agent, interact.
- Load test: simple throughput test with short prompts and small model.

## Risks & Open Questions
- Model feasibility: `120b` requires substantial VRAM; adopt a smaller default for dev, allow override in CLI.
- Quantization tradeoffs: add optional flags; document accuracy/perf implications.
- Stop sequences: which defaults to use; pass from Agent as needed.
- Token accounting: whether to return token counts (requires tokenizer usage); defer in v1 if not critical.
- Heuristics for tool triggering (if needed in future) since Harmony/tool_calls aren’t used on Python side.

## Implementation Plan (Phased)

### Phase 1: Introduce GPT Service (coexist with Ollama)
1. Add `Dockerfile.gpt` for FastAPI/Uvicorn server (CUDA base, installs torch+transformers+uvicorn+fastapi, pins versions).
2. Add `src/python/gpt_server/app.py` (or similar) with `/health` and `/generate` endpoints using `model.generate()`.
3. CLI: add `gpt` component (start/stop) with GPU flags; create volumes `raworc_gpt_data`, `raworc_gpt_logs`.
4. Gateway: not required; service is internal.

### Phase 2: Wire Controller & Agent to GPT
5. Controller: replace `OLLAMA_HOST` requirement with optional `RAWORC_GPT_URL` (prefer GPT if set); inject env into agent containers.
6. Agent:
   - Add `gpt.rs` client (or `transformers_client.rs`).
   - Update `message_handler` to use GPT client (drop Harmony/tool_call expectations); maintain message flow and state updates.
   - Keep structure of system prompt; ensure output is appended as agent message.

### Phase 3: Deprecate Ollama (follow‑up release)
7. CLI: default to `gpt`; emit warning if `ollama` used.
8. Remove `ollama` code paths from Controller/Agent; cleanup env vars and docs.

## File/Module Changes (Targeted)
- New:
  - `Dockerfile.gpt`
  - `src/python/gpt_server/app.py` (or `services/gpt-server/…`)
- CLI (`cli/commands/start.js`, `cli/lib/docker.js`): add `gpt` component; flags; set `RAWORC_GPT_URL`; volumes.
- Controller (`src/controller/docker_manager.rs`): stop using `OLLAMA_HOST`; add `RAWORC_GPT_URL` env to agents.
- Agent:
  - Add `src/agent/gpt.rs` client.
  - Update `src/agent/mod.rs` and `src/agent/message_handler.rs` to call GPT client.
  - Remove or quarantine `src/agent/ollama.rs` in Phase 3.
- Docs/Operator: update references and versioned docs page.

## Configuration
- Env vars:
  - `RAWORC_GPT_URL` (e.g., `http://raworc_gpt:7001`)
  - `RAWORC_GPT_MODEL` (defaults to a practical dev model)
  - Optional: `RAWORC_GPT_DTYPE`, `RAWORC_GPT_QUANT` (future)

## Milestones
- M1: GPT server container builds and responds to `/health` and `/generate`.
- M2: CLI can start full stack with GPT; Controller/Agent use `RAWORC_GPT_URL`.
- M3: E2E functional demo; document migration path; emit Ollama deprecation warning.
- M4: Remove Ollama code paths next release.

## Approval Checklist
- [ ] Endpoint contract for `/generate` confirmed (request/response fields)
- [ ] Default model and dev fallback agreed
- [ ] Env var names finalized (`RAWORC_GPT_URL`) and CLI flags
- [ ] Transition window for Ollama (1 release) acceptable
