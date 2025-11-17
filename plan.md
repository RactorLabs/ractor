## Task-Type Support Plan

### Objectives
- Introduce a `task_type` attribute for sandbox tasks with enum values: `NL` (Natural Language, default), `SH` (Shell), `PY` (Python), `JS` (JavaScript).
- Keep the current inference-driven experience for `NL` tasks while adding new executors for the other modes that operate locally inside the sandbox container.
- Ensure inference/token/tool metrics remain exclusive to NL tasks.

### Current Flow (NL-only)
1. **Creation** – `POST /sandboxes/{id}/tasks` accepts text input, stores a row in `sandbox_tasks`, and enqueues a `create_task` request.
2. **Controller** – `SandboxManager` pulls `create_task` requests, forwards them to the sandbox container via Docker exec/websocket.
3. **Sandbox runtime** – `task_handler.rs` drives the inference pipeline (`inference.rs`) which streams steps/output back to the API and updates metrics.
4. **UI** – Operator front-end posts tasks and renders responses without awareness of task types.

### Proposed Changes

#### 1. Data Model & API
- Extend `sandbox_tasks` initial schema:
  - `task_type CHAR(2) NOT NULL DEFAULT 'NL'`.
  - Index on `(sandbox_id, task_type)` for future filtering.
- Update Rust models (`CreateTaskRequest`, `TaskSummary`, `TaskView`) to include `task_type`.
- REST handler validation:
  - Accept optional `task_type`; default to `NL`.
  - Reject unknown values with `400`.
- OpenAPI & docs:
  - Document the enum and semantics.
  - Note that non-NL tasks do not hit inference metrics.

#### 2. Controller & Queue Payloads
- Include `task_type` in `sandbox_requests` payloads for:
  - `create_task`.
  - Startup tasks if we ever expose non-NL there (initially keep NL).
- Ensure controller passes the type into sandbox exec commands (environment variable or JSON message).

#### 3. Sandbox Runtime Abstraction
- Define a `TaskExecutor` trait (e.g., `execute(&TaskContext) -> TaskResult`).
- Implementations:
  - `NlExecutor`: wraps existing inference logic and keeps updating tokens/tool counts.
  - `ShellExecutor`: run `sh -lc <input>`; capture stdout/stderr; emit steps.
  - `PythonExecutor`: pipe code to `python3 - <<'PY'`; same capture/limits.
  - `JavaScriptExecutor`: use `node - <<'JS'` (or `node` temp file).
- Safety considerations:
  - Enforce timeouts per executor (reuse existing task timeout).
  - Limit output size and redact secrets (respect `.env`?).
  - Ensure commands run as sandbox user (current container default).
- Routing logic:
  - On task start, match `task_type` and dispatch to the corresponding executor.
  - Fallback to NL if unknown (to be defensive).

#### 4. Metrics & Telemetry
- Inference/token counters and `tool_count` increments only within `NlExecutor`.
- Other executors still log steps/status but skip inference stats.
- API stats (`/sandboxes/{id}/stats`) remain unchanged; they naturally reflect NL-only inference usage.

#### 5. Operator UI
- Task submission form:
  - Add a type selector (radio buttons or dropdown) defaulting to Natural Language.
  - Show contextual help (e.g., “Shell: run commands via /bin/sh”).
- Task list/detail:
  - Display task type badges.
  - For non-NL tasks, present output plainly (no inference streaming).
- Optional future enhancement: filter by task type.

#### 6. Testing & Rollout
1. **Backend**
   - Unit tests for `CreateTaskRequest` validation + new enum.
   - Integration tests to ensure storing/fetching `task_type`.
2. **Sandbox Runtime**
   - Tests per executor (mock command success/failure, timeout).
   - E2E run to ensure inference metrics unaffected for non-NL.
3. **UI**
   - Cypress/Playwright scenario: create shell task, observe output.
4. **Deployment Order**
   1. Update schema (single initial migration edit since DB reprovision planned).
   2. Ship API/controller changes tolerant of missing type (default NL).
   3. Deploy sandbox runtime with executor abstraction.
   4. Enable UI selector once backend/runtime are live.

This plan keeps existing NL workflows untouched while layering in flexible executors and the plumbing needed to track task types safely.
