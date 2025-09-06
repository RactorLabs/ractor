# Repository Guidelines

## Project Structure & Module Organization
- `src/`: Rust services — `server/` (API), `controller/` (orchestration), `agent/` (runtime), `shared/` (common code). Binaries: `raworc-server`, `raworc-controller`, `raworc-agent`.
- `cli/`: Node.js CLI (`raworc`).
- `scripts/`: Dev automation (`build.sh`, `link.sh`).
- `db/migrations/`: SQLx migrations (MySQL). Default admin: `admin/admin`.
- `assets/`, `website/`: Static assets and docs site.

## Build, Test, and Development Commands
- Build Rust: `cargo build --release` (creates binaries in `target/release/`).
- Run CI-like checks: `cargo test --verbose`.
- Start full stack (Docker): `raworc start` (MySQL on `3307`, API `9000`, public `8000`). Use `./scripts/build.sh` in dev if you need to build images.
- Stop: `raworc stop`.
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
- Files and bins use `raworc-*` naming (e.g., `raworc-server`).

## Testing Guidelines
- Framework: Rust `#[test]` unit tests; optional integration tests under `tests/`.
- Run all tests: `cargo test`.
- Prefer small unit tests near code; name tests after behavior (e.g., `handles_invalid_token`).
- Database-involving tests should be feature-gated or isolated; avoid mutating real data.
- Integration smoke test: `./scripts/build.sh && raworc start ollama mysql server controller && ./scripts/link.sh && raworc --version`.

## Commit & Pull Request Guidelines
- Conventional commits: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`, `perf:`, `style:`.
- PRs must include: summary, test plan, breaking changes (if any), and linked issues.
- Before pushing: `cargo fmt --check`, `cargo clippy`, `cargo test`, and ensure services still start: `raworc start server`.
- Etiquette: no emojis, no AI-assistant references; imperative subject (<50 chars) with details in body when needed.
- Branch naming: `type/short-description` (e.g., `feat/session-timeout`).

## Security & Configuration Tips
- .env files are no longer required for starting services via CLI. Pass configuration via `raworc start` flags. Avoid committing secrets.
- Required vars: `DATABASE_URL`, `JWT_SECRET`, `RUST_LOG`.
 
- Example local DB: `mysql://raworc:raworc@localhost:3307/raworc`.
- Use least-privileged credentials and rotate `JWT_SECRET` in production.
- Migrations auto-run on startup; set `SKIP_MIGRATIONS=1` to skip if DB is pre-provisioned.

## Agent-Specific Instructions
- Use the CLI for service control and avoid ad‑hoc `docker build/run` sequences.
- Link the CLI before usage: `./scripts/link.sh`, then prefer `raworc ...` commands for checks (e.g., `raworc --version`).
- Coordinate actions: wait for explicit maintainer instruction before running long/destructive ops, publishing, or committing.
- Commit policy: never reference AI/assistants; no emojis; write professional, imperative, conventional commits.
- Pre‑commit checklist: `cargo fmt --check`, `cargo clippy`, `cargo build --release`, `cargo test`, and verify services start.
- Licensing: repository is intentionally unlicensed; do not add or suggest license files.

## Command Playbooks (.claude/commands)
### Commit
- Review: `git status`, `git diff`, `git log --oneline -5`.
- Stage: `git add .`.
- Message: conventional type + concise subject; detailed body if helpful; no AI/emojis.
- Verify: `git status`. Do not push without approval.

### Bump (version management)
- Choose target: patch/minor/major or specific (e.g., `0.5.2`).
- Update version refs (8 files: Cargo.toml, cli/ and website/ package.json, API/version docs, etc.).
- Rebuild to update locks: `cargo build --release`; `cd cli && npm install`; `cd website && npm install`.
- Verify modified files via `git status`. Then commit using the Commit playbook (don’t push yet).
- Tip: search and replace prior version across tracked files (review before edit):
  - `prev=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)`
  - `new=0.X.Y`
  - Audit: `rg -n --hidden -S "$prev" -g '!target/**' -g '!node_modules/**' -g '!website/build/**'` 
  - Replace in known refs only (avoid lock files): `sed -i "s/$prev/$new/g" Cargo.toml cli/package.json website/package.json`

### Release
- Update docs: README, website docs/changelog, API docs, CLAUDE.md.
- Stage/commit docs: `git add .` then a clear docs commit.
- Stage/commit remaining changes as needed.
- Get version from `Cargo.toml`; tag without prefix: `git tag 0.X.Y`.
- Push: `git push origin main && git push origin 0.X.Y` (triggers CI).
- After release: run Bump to prepare the next version and commit those updates.
