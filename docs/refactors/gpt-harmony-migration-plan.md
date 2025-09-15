# Raworc: Replace Ollama with Transformers (raworc_gpt) and Add Harmony Tool Calling (Rust)

## Summary
- Clean replacement: remove Ollama entirely from code, CLI, Docker, env vars, and docs. No deprecation window, no TODOs — it’s gone.
- Introduce a new GPU-enabled model service `raworc_gpt` using Hugging Face Transformers behind a FastAPI/Uvicorn server. It exposes a simple `/generate` endpoint that returns raw text from `model.generate()`.
- Implement tool calling using the Harmony Rust package in the Agent. The Python server stays dumb (no parsing); the Rust Agent parses Harmony responses, invokes tools, and iterates until completion.

## Goals
- raworc_gpt container builds and runs with CUDA base image; data/logs persisted via volumes.
- CLI `raworc start` uses `gpt` (not `ollama`) and provisions required volumes/networks. No Ollama flags or components remain.
- Controller injects only `RAWORC_GPT_URL` into agent containers (and branding env already used). No `OLLAMA_*` env left.
- Agent uses a new `GptClient` to call `/generate`, and the Harmony Rust crate to parse model output and drive tool execution via the existing Tool trait.
- End-to-end chat with tool execution works; messages persist; publish/content paths unchanged.

## Non-Goals
- No Harmony parsing in Python; no streaming in v1.
- No hybrid Ollama mode.

## Design

### Service: raworc_gpt (Transformers server)
- Base image: CUDA runtime (e.g., `nvidia/cuda:12.x-cudnn*-runtime-ubuntu22.04`) or an appropriate PyTorch CUDA image.
- Server: FastAPI + Uvicorn.
- Model: load by HF model id (default `gpt-oss:120b`, configurable via env `RAWORC_GPT_MODEL`).
- Endpoint: Internal Generate API v1 — Python server mirrors the internal request/response exactly and ignores unknown fields.
  - POST `/generate`
    - Request (JSON):
      - `prompt: string` (required) — single composed prompt from the Agent (includes system, history, tool specs as text)
      - `model?: string` — override model id for this call
      - Generation params (all optional, passed through to `generate()` as applicable):
        - `max_new_tokens, temperature, top_p, top_k, repetition_penalty, do_sample`
        - `eos_token_id, pad_token_id`
        - `stop?: string[]` (server will implement best‑effort stopping by scanning output; model itself is not tool‑aware)
        - `seed?: number`
      - `request_id?: string` — echoed in logs only
      - `metadata?: object` — ignored by server, reserved for client use
    - Response (JSON):
      - `text: string` — raw model output text
      - `usage?: { prompt_tokens?: number, completion_tokens?: number, total_tokens?: number }` (optional best‑effort)
  - GET `/health` → `{ status: 'ok' }`.
- Volumes: `raworc_gpt_data:/app/data`, `raworc_gpt_logs:/app/logs`.

### Agent: Harmony-driven tool calling
- Add dependency on Harmony Rust crate (as per https://github.com/openai/harmony — pinned git revision if not published on crates.io).
- Conversation building stays in Rust: system + history + tool specs.
- Tools registry provides JSON Schema for each tool (already supported by Tool::parameters()).
- Prompting protocol:
  - Provide Harmony tool spec in the prompt (per Harmony guidance) and instruct the model to reply in Harmony format.
  - Send a single `prompt` to raworc_gpt `/generate` and get raw text (Python server strictly mirrors Internal Generate API v1; no parsing).
  - Use Harmony Rust parser to decode tool calls (if any). Execute tools in process and append tool results back into the conversation context. Iterate until the model returns a final assistant message without tool calls or we hit max iterations.
- Busy/idle state changes: set busy before inference/tool loop and restore idle after completion.

### Controller
- Remove all references to `OLLAMA_HOST`, `OLLAMA_MODEL`, `OLLAMA_TIMEOUT_SECS`.
- Inject `RAWORC_GPT_URL` into agent containers (e.g., `http://raworc_gpt:6000`).
- Keep other env (branding, JWT) intact.

### CLI
- Remove `ollama` component and all related flags.
- Add `gpt` component:
  - GPU flags: `--gpt-enable-gpu/--no-gpt-enable-gpu`, `--gpt-cpus`, `--gpt-memory`, `--gpt-shm-size`.
  - Model flag: `--gpt-model <hf_model_id>` with default from env or sensible fallback.
  - Expose `RAWORC_GPT_URL` to API/Controller/Agent containers for internal routing.
- Volumes: create `raworc_gpt_data`, `raworc_gpt_logs`.

### API/Operator
- No endpoint changes.
- Update Operator docs to mention Transformers (raworc_gpt) and Harmony-based tool calling handled in the Agent.

## Security
- Keep raworc_gpt internal to the Docker network by default.
- Avoid logging prompts in production logs; log timings and sizes only.

## Performance
- `gpt-oss:120b` is large; provide doc guidance and CLI flag to switch to a smaller dev model.
- Timeouts unchanged at API level; Agent and Controller should use reasonable request timeouts (e.g., 600s default).

## Testing
- Unit: Harmony parsing integration with mock responses; tool mapping validation; `GptClient` request/response.
- Integration: Start `gpt` + API + Controller + Agent; create agent; send a prompt that triggers a simple tool call and verify results.
- E2E: Full stack start and chat flow.

## File/Module Changes
- New files:
  - `Dockerfile.gpt` — build the Transformers server image.
  - `src/python/gpt_server/app.py` — FastAPI app.
  - Agent: `src/agent/gpt.rs` — HTTP client for `/generate`.
- Modified files:
  - Remove `src/agent/ollama.rs` and all imports.
  - Agent: `src/agent/mod.rs`, `src/agent/message_handler.rs`, `src/agent/tool_registry.rs`, `src/agent/builtin_tools.rs` — integrate Harmony, update the orchestration loop to use Harmony tool calls and results.
  - Controller: `src/controller/docker_manager.rs` — remove OLLAMA env; add `RAWORC_GPT_URL` env to agent containers.
  - CLI: `cli/commands/start.js` and `cli/lib/docker.js` — remove `ollama`, add `gpt` runner and flags, create volumes, set env for API/Controller/Agent.
  - Scripts: `scripts/build.sh` — add `gpt` image to build/rebuild steps as needed.
  - Docs: Operator docs page and README references.

## Repository‑Wide Removal Checklist (Ollama)
Perform a clean, one‑shot removal — no compatibility shims, no deprecation flags.

- Delete files
  - `src/agent/ollama.rs`
- Remove code references (imports, types, env, logs)
  - Agent
    - `src/agent/mod.rs`: remove OLLAMA env handling, client creation, and type usage
    - `src/agent/message_handler.rs`: remove `OllamaClient`, `ChatMessage/ModelResponse` imports and usage
    - `src/agent/tool_registry.rs`: remove `generate_ollama_tools()` and `ToolDef/ToolFunction/ToolType` from ollama module
    - Any `ollama_*` log filenames or tracing labels
  - Controller `src/controller/docker_manager.rs`
    - Remove all `OLLAMA_*` env (HOST, MODEL, TIMEOUT, THINKING tokens, etc.)
    - Remove code paths that read/propagate `OLLAMA_*` secrets
    - Add `RAWORC_GPT_URL` only
  - CLI
    - `cli/commands/start.js`: remove ollama component, flags, start/wait logic and model pull
    - `cli/lib/docker.js`: remove `ollama` image entries and any helper for Ollama
  - Scripts/docs
    - `scripts/build.sh`: remove any ollama mentions; add `gpt` where appropriate
    - `AGENTS.md`: update smoke test and any guidance referencing `ollama`
    - Operator/README: replace Ollama mentions with Transformers raworc_gpt
- Remove environment variables
  - All `OLLAMA_*` across the codebase and docs
  - Replace with `RAWORC_GPT_URL` (and `RAWORC_GPT_MODEL` where needed)
- Search‑and‑destroy patterns (must be zero after change)
  - `\bOLLAMA_\w+\b`, `Ollama`, `ollama`, `raworc_ollama`, `ollama:` (image), `/api/tags` checks
  - Log files: `ollama_request_*.json`, `ollama_response_*.json`

Note: This is a destructive removal — there is no backward compatibility path or fallback to Ollama.

## Env Vars (post-change)
- `RAWORC_GPT_URL` — internal URL of the GPT server (e.g., `http://raworc_gpt:6000`).
- `RAWORC_GPT_MODEL` — HF model id (default `gpt-oss:120b` with documented alternatives for dev).

## Implementation Steps
0) Remove Ollama completely (see repository‑wide removal checklist)
1) Scaffold GPT server
   - Add `Dockerfile.gpt` (CUDA base, installs torch+transformers+fastapi+uvicorn, pins versions; uses `--gpus` at runtime).
   - Implement `src/python/gpt_server/app.py` with `/health` and `/generate` using `model.generate()` (no parsing).
2) CLI support
   - Add `gpt` component in `start.js` and `lib/docker.js`; manage volumes; pass `RAWORC_GPT_MODEL`; publish `RAWORC_GPT_URL` to API/Controller/Agent containers.
   - Ensure no `ollama` references or flags remain anywhere in CLI.
3) Controller wiring
   - Add `RAWORC_GPT_URL` injection for agents; verify zero `OLLAMA_*` usage remains.
4) Agent Harmony integration
   - Add Harmony Rust dependency in `Cargo.toml` (git URL + rev per docs).
   - Implement `GptClient` (timeout, error handling, `POST /generate`).
   - In `message_handler`, build the Harmony prompt with tool schemas, call `GptClient`, parse response with Harmony, execute tools via Tool registry, iteratively resolve calls until final answer, and persist outputs.
   - Ensure state transitions (busy/idle) wrap the whole interaction.
5) Clean repo
   - Confirm all removal checklist items are satisfied and searches return zero matches.
6) Docs & examples
   - Update docs to describe Transformers and Harmony tool calling, and new CLI flags.

## Approval Checklist
- [ ] Confirm Internal Generate API v1 schema (above) for `/generate`
- [ ] Confirm default HF model id and dev fallback
- [ ] Confirm env names: `RAWORC_GPT_URL`, `RAWORC_GPT_MODEL`
- [ ] Confirm removal of all Ollama code/flags/env in one pass
- [ ] Approve Harmony crate usage (git dep) and iteration loop behavior
