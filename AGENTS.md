# Repository Guidelines

## Project Structure & Module Organization

- `src/`: Rust services — `api/` (REST API), `controller/` (orchestration), `sandbox/` (runtime), `shared/` (common code). Binaries: `tsbx-api`, `tsbx-controller`, `tsbx-sandbox`.
- `cli/`: Node.js CLI (`tsbx`).
- `scripts/`: Dev automation (`build.sh`, `link.sh`, `install.sh`, `rebuild.sh`, `release.sh`, `bump.sh`, `push.sh`).
- `db/migrations/`: SQLx migrations (MySQL). Seeds an `admin` operator.
- `assets/`: Static assets.

## Build, Test, and Development Commands

- Build Rust: `cargo build --release` (creates binaries in `target/release/`).
- Run CI-like checks: `cargo test --verbose`.
- Start services (Docker via CLI): `tsbx start [components...]`
  - Defaults to MySQL (`3307`), API (`9000`), Operator, Controller, Gateway (`80`). Inference traffic is proxied to `TSBX_INFERENCE_URL` (default `https://api.positron.ai/v1`).
  - In dev, use `./scripts/build.sh` to build images when needed.
- Stop: `tsbx stop [components...]` (supports `sandboxes` to stop all sandbox containers).
- Dev CLI link: `./scripts/link.sh` then use `tsbx --help` or `tsbx start`.

## Contributor Workflow Rules

- Use the CLI for service management: `tsbx start|stop|doctor|reset|clean|pull|fix` (plus `dev_build`/`dev_rebuild` shortcuts for local Docker image work).
- Use repo scripts only where needed: `./scripts/build.sh`, `./scripts/link.sh`.
- Always run `./scripts/link.sh` before invoking the `tsbx` CLI during development.
- Keep changes minimal and consistent with existing patterns; prefer editing within current modules.

## Coding Style & Naming Conventions

- Rust 2021, 4-space indent, `snake_case` for modules, `CamelCase` for types, `SCREAMING_SNAKE_CASE` for consts.
- Format: `cargo fmt` (check with `cargo fmt --check`).
- Lint: `cargo clippy -- -D warnings` (fix or justify warnings).
- Files and bins use `tsbx-*` naming (e.g., `tsbx-api`).

## Testing Guidelines

- Framework: Rust `#[test]` unit tests; optional integration tests under `tests/`.
- Run all tests: `cargo test`.
- Prefer small unit tests near code; name tests after behavior (e.g., `handles_invalid_token`).
- Database-involving tests should be feature-gated or isolated; avoid mutating real data.
- Integration smoke test: `./scripts/build.sh && tsbx start mysql api controller && ./scripts/link.sh && tsbx --version`.

## Commit & Pull Request Guidelines

- Conventional commits: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`, `perf:`, `style:`.
- PRs must include: summary, test plan, breaking changes (if any), and linked issues.
- Before pushing: `cargo fmt --check`, `cargo clippy`, `cargo test`, and ensure services still start: `tsbx start api`.
- Etiquette: no emojis, no AI-assistant references; imperative subject (<50 chars) with details in body when needed.
- Branch naming: `type/short-description` (e.g., `feat/sandbox-timeout`).

Note on commit message formatting:

- Do not include literal escape sequences like `\n` in commit subjects or bodies.
- Use actual newlines for paragraphs/bullets. If amending via scripts, verify the resulting message with `git log -1`.

## Security & Configuration Tips

- .env files are no longer required for starting services via CLI. Pass configuration via `tsbx start` flags. Avoid committing environment values.
- Required vars: `DATABASE_URL`, `JWT_SECRET`, `RUST_LOG`.

- Example local DB: `mysql://tsbx:tsbx@localhost:3307/tsbx`.
- Use least-privileged credentials and rotate `JWT_SECRET` in production.
- Migrations auto-run on startup; set `SKIP_MIGRATIONS=1` to skip if DB is pre-provisioned.
- The CLI injects `TSBX_HOST_NAME`/`TSBX_HOST_URL` into Operator, Controller and sandboxes for consistent links/branding.

## Data Model Highlights (Sessions)

- UUID-based primary key: sandboxes are identified exclusively by `id` (CHAR(36) UUID).
- Core fields: `state` (`init|idle|busy|terminated`), `created_by`, timestamps, `metadata` (JSON).
- Parent sandboxes: `parent_sandbox_id` (CHAR(36)) references parent sandbox's UUID.
- Timeouts: `stop_timeout_seconds`, `archive_timeout_seconds` with tracking via `idle_from` and `busy_from` (archive timeout currently reserved, defaults to 24 hours).
- Tags: `tags JSON NOT NULL DEFAULT []` — an array of alphanumeric strings used for categorization. No spaces or symbols; remix copies parent tags.
- Docker resources: Container names are `tsbx_sandbox_{id}`, volume names are `tsbx_sandbox_data_{id}`.

## Sandbox Lifecycle & API

- Controller creates the sandbox container and sets initial DB state to `init` (only if still `init`, to avoid racing sandbox requests).
- The sandbox runtime, on boot, calls the API to report state:
  - `POST /api/v0/sandboxes/{id}/state/idle` when ready (sets state to `idle` and starts idle timer).
  - `POST /api/v0/sandboxes/{id}/state/busy` when processing (sets state to `busy` and starts busy timer).
- Stop/Restart actions:
  - `POST /sandboxes/{id}/stop` schedules container stop and sets state to `terminated`.
  - `POST /sandboxes/{id}/restart` restarts container and transitions via `init`.
- Tasks: `GET/POST /sandboxes/{id}/tasks` for user↔sandbox exchanges, stored in `sandbox_tasks`.
  - `POST` body accepts `{ input: { text: string }, background?: boolean }`.
- `background` defaults to `true`. When set to `false`, the API call blocks up to 15 minutes until the task reaches a terminal status (`completed` or `failed`). If it times out, the server returns HTTP `504`.
- All API routes and Docker operations use sandbox UUID `id` exclusively.

## Operator UI

- Primary routes live under `/sandboxes` (list, create, details/chat). Legacy `/app/*` routes have been removed.
- Sessions are displayed by their ID (shortened to first 8 characters, with full ID on hover).
- Sandbox pages show tags and support "Remix", "Edit Tags", "Delete" via modals. Stop/Restart buttons appear only when actionable.
- No name input required when creating or cloning sandboxes - IDs are auto-generated.

## Session-Specific Instructions

- Use the CLI for service control and avoid ad‑hoc `docker build/run` sequences.
- Link the CLI before usage: `./scripts/link.sh`, then prefer `tsbx ...` commands for checks (e.g., `tsbx --version`).
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
