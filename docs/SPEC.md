# Ractor: System Specification (Core)

## Overview
Ractor is an infrastructure runtime for long‑lived, stateful agent sessions. Each session runs inside its own container with a persistent Docker volume, an agent runtime, a tool registry (bash + file editors + publish/sleep), and access to an inference endpoint (Ollama). An API service provides durable state (MySQL/SQLx), RBAC, and an operator‑friendly surface for observability and lifecycle control. A Gateway (nginx) routes `/` to the Operator UI, `/api/` to the API, and `/content/` to the content server.

## Goals
- Durable sessions with fast resume; no cold start once the model is hot.
- Strong containment and auditability (per‑session volume, no host escape).
- Operator‑first tooling (UI, logs, sleep/wake, publish, timeouts).
- Clear API contract for UI and session runtime.

Non‑goals
- Multi‑tenant scheduler/fair‑share. (Single host; simple limits.)
- Model hosting. (Ollama is orchestrated but not implemented here.)

## High‑Level Architecture
- API (Axum/SQLx): JWT auth + RBAC, session/response/files endpoints.
- Controller (Bollard): watches `session_tasks`, manages containers/volumes, publish/unpublish, sleep/wake, selective branch copy.
- Session (Rust binary): polls responses, executes tools, calls Ollama, appends output segments, updates context usage.
- Content server: serves published session content.
- Operator UI (SvelteKit SSR): calls API via relative `/api/v0/**`.
- Gateway (nginx): stable host entry point.

Ports (container‑internal)
- API `:9000`  • Operator `:7000`  • Content `:8000`  • Gateway `:80`

## Data Model (selected tables)
- `sessions(name PK, created_by, state, metadata JSON, tags JSON, is_published, publish_permissions JSON, idle_timeout_seconds, busy_timeout_seconds, idle_from, busy_from, context_cutoff_at, last_context_length, timestamps…)`
- `session_responses(id UUID PK, session_name FK, created_by, status, input JSON, output JSON, created_at, updated_at)`
- `session_tasks(id UUID PK, session_name, task_type, created_by, payload JSON, status, started_at, completed_at, error, timestamps…)`
- `operators(name PK, password_hash, description, active, last_login_at, timestamps…)`
- `roles(name PK, rules JSON)`, `role_bindings(role_name, principal, principal_type)`
- `blocked_principals(principal, principal_type, created_at)`

## Request Flow (Create + Respond)
1. Client calls `POST /api/v0/sessions` with name/env/instructions → API inserts `sessions` row and enqueues a `create_session` task.
2. Controller consumes the task, creates a dedicated Docker volume and container, seeds `/session` with `.env`, `code/` and `template/`, then starts the `ractor-session` binary with `RACTOR_TOKEN` and `OLLAMA_*` env.
3. Client calls `POST /api/v0/sessions/{name}/responses` with user input → API enqueues `create_response` task and returns a stub `ResponseView`.
4. Session runtime polls responses (windowed), sets state busy, builds a conversation from prior responses, executes tools, calls Ollama, appends `segments`/`output` to the response, then marks status `completed`.
5. Timeouts: idle/busy timers are managed in DB; session may be slept (container removed, volume retained). Wake recreates the container from the volume and continues.

## Files API (read‑only)
- `GET /sessions/{name}/files/read/{path}` (<=25MB): returns bytes; content type inferred.
- `GET /sessions/{name}/files/list{,/path}` with paging: returns directory entries.
- `GET /sessions/{name}/files/metadata/{path}`: returns size/kind/mode/mtime.
- `DELETE /sessions/{name}/files/delete/{path}`: regular file removal.

## Security
- JWT (bearer) with claims `{sub, sub_type, exp}` signed with `JWT_SECRET`.
- RBAC rules loaded from DB; Operator `admin` can mint tokens for `User` principals.
- Blocklist enforced for non‑admin subjects.
- Session containers receive a scoped `RACTOR_TOKEN` for Host API auth; sensitive env keys from users are filtered.

## Operational Model
- CLI `ractor start` creates the user network/volumes, pulls images, and starts MySQL, Ollama, API, Controller, Content, Operator, and Gateway in the correct order.
- Images are resolved locally first, then pulled from the remote registry if missing.
- Logs are structured (tracing) and written to container volumes; key actions are logged with context (task id, session name).

## Failure Modes & Resilience
- Controller task claiming must be atomic; use DB transaction (and `FOR UPDATE SKIP LOCKED`) to avoid duplicate processing.
- API long‑poll paths bound by timeouts (responses blocking mode capped; prefer non‑blocking + polling/SSE).
- Defensive caps: file read 25MB, tool outputs truncated in session runtime, path traversal guarded.

## Interfaces (minimal)
- Authentication: `POST /operators/{name}/login` → JWT; `GET /auth` → principal.
- Sessions: `GET/POST /sessions`, `GET/PUT/DELETE /sessions/{name}`, `POST /sessions/{name}/sleep|wake`, `POST /sessions/{name}/publish|unpublish`.
- Responses: `GET/POST /sessions/{name}/responses`, `GET/PUT /sessions/{name}/responses/{id}`, `GET /responses/{id}`.
- Context: `GET /sessions/{name}/context`, `POST /context/clear|compact|usage`.

## Hardening & Backlog
- Require `JWT_SECRET` in API/Controller; fail fast if missing.
- Replace `docker cp/exec` shell outs in publish/unpublish with Bollard equivalents for portability.
- Add minimal unit tests for path‑safety helpers and strict deserializers.
- Consider SSE/WebSocket stream for response segments to reduce DB polling.
- Add transactional task claim in Controller; ensure idempotent task completion.

## Deployment Notes
- Everything runs in a single Docker network (`ractor_network`) with named volumes (`mysql_data`, `ractor_*`). Host branding via `RACTOR_HOST_NAME/URL` propagates to API/Controller/Operator/Session.
- GPU is optional for UI/API; required for LLM if running Ollama locally.
