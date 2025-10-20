# Repository Guidelines

## Project Structure & Module Organization

- `src/`: Rust services — `api/` (REST API), `controller/` (orchestration), `session/` (runtime), `content/` (public content server), `shared/` (common code). Binaries: `ractor-api`, `ractor-controller`, `ractor-session`, `ractor-content`.
- `cli/`: Node.js CLI (`ractor`).
- `scripts/`: Dev automation (`build.sh`, `link.sh`, `install.sh`, `rebuild.sh`, `publish.sh`, `release.sh`, `bump.sh`, `push.sh`).
- `db/migrations/`: SQLx migrations (MySQL). Seeds an `admin` operator.
- `assets/`: Static assets.

## Build, Test, and Development Commands

- Build Rust: `cargo build --release` (creates binaries in `target/release/`).
- Run CI-like checks: `cargo test --verbose`.
- Start services (Docker via CLI): `ractor start [components...]`
  - Defaults to MySQL (`3307`), Ollama, API (`9000`), Operator, Content (`8000`), Controller, Gateway (`80`).
  - In dev, use `./scripts/build.sh` to build images when needed.
- Stop: `ractor stop [components...]` (supports `sessions` to stop all session containers).
- Dev CLI link: `./scripts/link.sh` then use `ractor --help` or `ractor start`.

## Contributor Workflow Rules

- Use the CLI for service management: `ractor start|stop|doctor|reset|clean|pull|fix` (plus `dev_build`/`dev_rebuild` shortcuts for local Docker image work).
- Use repo scripts only where needed: `./scripts/build.sh`, `./scripts/link.sh`.
- Always run `./scripts/link.sh` before invoking the `ractor` CLI during development.
- Keep changes minimal and consistent with existing patterns; prefer editing within current modules.

## Coding Style & Naming Conventions

- Rust 2021, 4-space indent, `snake_case` for modules, `CamelCase` for types, `SCREAMING_SNAKE_CASE` for consts.
- Format: `cargo fmt` (check with `cargo fmt --check`).
- Lint: `cargo clippy -- -D warnings` (fix or justify warnings).
- Files and bins use `ractor-*` naming (e.g., `ractor-api`).

## Testing Guidelines

- Framework: Rust `#[test]` unit tests; optional integration tests under `tests/`.
- Run all tests: `cargo test`.
- Prefer small unit tests near code; name tests after behavior (e.g., `handles_invalid_token`).
- Database-involving tests should be feature-gated or isolated; avoid mutating real data.
- Integration smoke test: `./scripts/build.sh && ractor start ollama mysql api controller && ./scripts/link.sh && ractor --version`.

## Commit & Pull Request Guidelines

- Conventional commits: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`, `perf:`, `style:`.
- PRs must include: summary, test plan, breaking changes (if any), and linked issues.
- Before pushing: `cargo fmt --check`, `cargo clippy`, `cargo test`, and ensure services still start: `ractor start api`.
- Etiquette: no emojis, no AI-assistant references; imperative subject (<50 chars) with details in body when needed.
- Branch naming: `type/short-description` (e.g., `feat/session-timeout`).

Note on commit message formatting:

- Do not include literal escape sequences like `\n` in commit subjects or bodies.
- Use actual newlines for paragraphs/bullets. If amending via scripts, verify the resulting message with `git log -1`.

## Security & Configuration Tips

- .env files are no longer required for starting services via CLI. Pass configuration via `ractor start` flags. Avoid committing environment values.
- Required vars: `DATABASE_URL`, `JWT_SECRET`, `RUST_LOG`.

- Example local DB: `mysql://ractor:ractor@localhost:3307/ractor`.
- Use least-privileged credentials and rotate `JWT_SECRET` in production.
- Migrations auto-run on startup; set `SKIP_MIGRATIONS=1` to skip if DB is pre-provisioned.
- The CLI injects `RACTOR_HOST_NAME`/`RACTOR_HOST_URL` into Operator, Controller and sessions for consistent links/branding.

## Data Model Highlights (Sessions)

- Name-based primary key: sessions are addressed by `name` (no numeric ID).
- Core fields: `state` (`init|idle|busy|slept`), `created_by`, timestamps, `metadata` (JSON).
- Publishing fields: `is_published`, `published_at`, `published_by`, `publish_permissions` (JSON flags for `code`,`env`,`content`).
- Timeouts: `idle_timeout_seconds`, `busy_timeout_seconds` with tracking via `idle_from` and `busy_from`.
- Tags: `tags JSON NOT NULL DEFAULT []` — an array of alphanumeric strings used for categorization. No spaces or symbols; remix copies parent tags.

## Session Lifecycle & API

- Controller creates the session container and sets initial DB state to `init` (only if still `init`, to avoid racing session updates).
- The session runtime, on boot, calls the API to report state:
  - `POST /api/v0/sessions/{name}/idle` when ready (sets state to `idle` and starts idle timer).
  - `POST /api/v0/sessions/{name}/busy` when processing (sets `busy` and starts busy timer).
- Sleep/Wake actions:
  - `POST /sessions/{name}/sleep` schedules container stop and sets state to `slept`.
  - `POST /sessions/{name}/wake` restarts container and transitions via `init`.
- Responses: `GET/POST /sessions/{name}/responses` for user↔session exchanges, stored in `session_responses`.
  - `POST` body accepts `{ input: { text: string }, background?: boolean }`.
  - `background` defaults to `true`. When set to `false`, the API call blocks up to 15 minutes until the response reaches a terminal status (`completed` or `failed`). If it times out, the server returns HTTP `504`.

## Operator UI

- Primary routes live under `/sessions` (list, create, details/chat). Legacy `/app/*` routes have been removed.
- Session pages show tags and support “Remix”, “Edit Tags”, “Delete” via modals. Sleep/Wake buttons appear only when actionable.
- Published content is served by the `ractor-content` service under `/content/{session}` and proxied publicly via the Gateway at port 80.

## Session-Specific Instructions

- Use the CLI for service control and avoid ad‑hoc `docker build/run` sequences.
- Link the CLI before usage: `./scripts/link.sh`, then prefer `ractor ...` commands for checks (e.g., `ractor --version`).
- Coordinate actions: wait for explicit maintainer instruction before running long/destructive ops, publishing, or committing.
- Commit policy: never reference AI/assistants; no emojis; write professional, imperative, conventional commits.
- Pre‑commit checklist: `cargo fmt --check`, `cargo clippy`, `cargo build --release`, `cargo test`, and verify services start.
- Licensing: the project ships under the Server Side Public License (SSPL); see `LICENSE.md` for terms and do not introduce conflicting licenses.

#### Required Elements

- HTML5 doctype (`<!doctype html>`)
- Viewport meta tag for responsive design
- Bootstrap CSS CDN link in `<head>`
- Bootstrap JS bundle before closing `</body>`

#### Grid System Usage

- Use `.container` or `.container-fluid` for page layout
- Structure content with `.row` and `.col-*` classes
- Leverage responsive breakpoints (sm, md, lg, xl, xxl)
- Example: `<div class="container"><div class="row"><div class="col-md-8">Main</div><div class="col-md-4">Sidebar</div></div></div>`

#### When NOT to use Bootstrap

- Only skip Bootstrap when explicitly requested
- When building custom CSS frameworks  
- When working with existing non-Bootstrap projects
