# Repository Guidelines

## Project Structure & Module Organization
- `src/`: Rust services — `api/` (REST API), `controller/` (orchestration), `agent/` (runtime), `content/` (public content server), `shared/` (common code). Binaries: `raworc-api`, `raworc-controller`, `raworc-agent`, `raworc-content`.
- `cli/`: Node.js CLI (`raworc`).
- `scripts/`: Dev automation (`build.sh`, `link.sh`).
- `db/migrations/`: SQLx migrations (MySQL). Seeds an `admin` operator.
- `assets/`: Static assets.

## Build, Test, and Development Commands
- Build Rust: `cargo build --release` (creates binaries in `target/release/`).
- Run CI-like checks: `cargo test --verbose`.
- Start services (Docker via CLI): `raworc start [components...]`
  - Defaults to MySQL (`3307`), Ollama, API (`9000`), Operator, Content (`8000`), Controller, Gateway (`80`).
  - In dev, use `./scripts/build.sh` to build images when needed.
- Stop: `raworc stop [components...]` (supports `agents` to stop all agent containers).
- Dev CLI link: `./scripts/link.sh` then use `raworc --help` or `raworc start`.

## Contributor Workflow Rules
- Use the CLI for service management: `raworc start|stop|doctor`.
- Use repo scripts only where needed: `./scripts/build.sh`, `./scripts/link.sh`.
- Always run `./scripts/link.sh` before invoking the `raworc` CLI during development.
- Keep changes minimal and consistent with existing patterns; prefer editing within current modules.

## Coding Style & Naming Conventions
- Rust 2021, 4-space indent, `snake_case` for modules, `CamelCase` for types, `SCREAMING_SNAKE_CASE` for consts.
- Format: `cargo fmt` (check with `cargo fmt --check`).
- Lint: `cargo clippy -- -D warnings` (fix or justify warnings).
- Files and bins use `raworc-*` naming (e.g., `raworc-api`).

## Testing Guidelines
- Framework: Rust `#[test]` unit tests; optional integration tests under `tests/`.
- Run all tests: `cargo test`.
- Prefer small unit tests near code; name tests after behavior (e.g., `handles_invalid_token`).
- Database-involving tests should be feature-gated or isolated; avoid mutating real data.
- Integration smoke test: `./scripts/build.sh && raworc start ollama mysql api controller && ./scripts/link.sh && raworc --version`.

## Commit & Pull Request Guidelines
- Conventional commits: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`, `perf:`, `style:`.
- PRs must include: summary, test plan, breaking changes (if any), and linked issues.
- Before pushing: `cargo fmt --check`, `cargo clippy`, `cargo test`, and ensure services still start: `raworc start api`.
- Etiquette: no emojis, no AI-assistant references; imperative subject (<50 chars) with details in body when needed.
- Branch naming: `type/short-description` (e.g., `feat/session-timeout`).

Note on commit message formatting:
- Do not include literal escape sequences like `\n` in commit subjects or bodies.
- Use actual newlines for paragraphs/bullets. If amending via scripts, verify the resulting message with `git log -1`.

## Security & Configuration Tips
- .env files are no longer required for starting services via CLI. Pass configuration via `raworc start` flags. Avoid committing secrets.
- Required vars: `DATABASE_URL`, `JWT_SECRET`, `RUST_LOG`.
 
- Example local DB: `mysql://raworc:raworc@localhost:3307/raworc`.
- Use least-privileged credentials and rotate `JWT_SECRET` in production.
- Migrations auto-run on startup; set `SKIP_MIGRATIONS=1` to skip if DB is pre-provisioned.
 - The CLI injects `RAWORC_HOST_NAME`/`RAWORC_HOST_URL` into Operator, Controller and agents for consistent links/branding.

## Data Model Highlights (Agents)
- Name-based primary key: agents are addressed by `name` (no numeric ID).
- Core fields: `state` (`init|idle|busy|slept`), `created_by`, timestamps, `metadata` (JSON), `content_port` (host-mapped port for agent content).
- Publishing fields: `is_published`, `published_at`, `published_by`, `publish_permissions` (JSON flags for `code`,`secrets`,`content`).
- Timeouts: `idle_timeout_seconds`, `busy_timeout_seconds` with tracking via `idle_from` and `busy_from`.
- Tags: `tags JSON NOT NULL DEFAULT []` — an array of alphanumeric strings used for categorization. No spaces or symbols; remix copies parent tags.

## Agent Lifecycle & API
- Controller creates the agent container and sets initial DB state to `init` (only if still `init`, to avoid racing agent updates).
- The agent, on boot, calls the API to report state:
  - `POST /api/v0/agents/{name}/idle` when ready (sets state to `idle` and starts idle timer).
  - `POST /api/v0/agents/{name}/busy` when processing (sets `busy` and starts busy timer).
- Sleep/Wake actions:
  - `POST /agents/{name}/sleep` schedules container stop and sets state to `slept`.
  - `POST /agents/{name}/wake` restarts container and transitions via `init`.
- Messages: `GET/POST /agents/{name}/messages` for user<->agent chat, stored in `agent_messages`.

## Operator UI
- Primary routes live under `/agents` (list, create, details/chat). Legacy `/app/*` routes have been removed.
- Agent page shows tags and supports “Remix”, “Edit Tags”, “Delete” via modals. Sleep/Wake buttons appear only when actionable.
- Published content is served by the `raworc-content` service under `/content/{agent}` and proxied publicly via the Gateway at port 80.

## Agent-Specific Instructions
- Use the CLI for service control and avoid ad‑hoc `docker build/run` sequences.
- Link the CLI before usage: `./scripts/link.sh`, then prefer `raworc ...` commands for checks (e.g., `raworc --version`).
- Coordinate actions: wait for explicit maintainer instruction before running long/destructive ops, publishing, or committing.
- Commit policy: never reference AI/assistants; no emojis; write professional, imperative, conventional commits.
- Pre‑commit checklist: `cargo fmt --check`, `cargo clippy`, `cargo build --release`, `cargo test`, and verify services start.
- Licensing: repository is intentionally unlicensed; do not add or suggest license files.

### HTML Page Creation Guidelines
When creating HTML pages, use Bootstrap 5.3 by default unless explicitly told not to. Follow this standard structure:

#### Standard Bootstrap HTML Template
```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Page Title</title>
    <link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.8/dist/css/bootstrap.min.css" rel="stylesheet">
  </head>
  <body>
    <!-- Page content here -->
    <script src="https://cdn.jsdelivr.net/npm/bootstrap@5.3.8/dist/js/bootstrap.bundle.min.js"></script>
  </body>
</html>
```

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

## Command Playbooks (.claude/commands)
### Commit
- Review: `git status`, `git diff`, `git log --oneline -5`.
- Stage: `git add .`.
- Message: conventional type + concise subject; detailed body if helpful; no AI/emojis.
- Verify: `git status`. Do not push without approval.

### Bump (version management)
- Preferred: run the helper (repairs docs badge safely and builds Operator):
  - `bash scripts/bump.sh 0.X.Y` or just `bash scripts/bump.sh` to bump patch
- What it updates:
  - `Cargo.toml` (top-level `version = "x.y.z"`)
  - `cli/package.json` (`version` field)
  - Operator docs badge in `operator/src/routes/docs/+page.svelte` (`const API_VERSION = 'x.y.z (v0)';`)
- The script avoids node_modules/lockfiles and repairs the docs badge line if it was ever corrupted by prior bumps.
- If doing manually, audit occurrences (avoid lockfiles and node_modules):
  - `prev=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)`
  - `rg -n --hidden -S "$prev" -g '!target/**' -g '!**/node_modules/**'`
  - Update the docs badge with a targeted replace to avoid syntax errors:
    - `perl -0777 -pe "s/(const\s+API_VERSION\s*=\s*')\d+\.\d+\.\d+(\s*\(v0\)';)/\1$new\2/" -i operator/src/routes/docs/+page.svelte`
  
- After bump: the script runs `cargo build --release` and builds the Operator (`npm ci|install && npm run build`).
- It stages, commits, and pushes changes automatically.

### Release
- Update docs: top-level README, Operator docs page (version badge), CLI README, CLAUDE.md.
- Stage/commit docs: `git add .` then a clear docs commit.
- Stage/commit remaining changes as needed.
- Get version from `Cargo.toml`; tag without prefix: `git tag 0.X.Y`.
- Push: `git push origin main && git push origin 0.X.Y` (triggers CI).
- After release: run Bump to prepare the next version and commit those updates.
