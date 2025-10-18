# ractor-apps-askrepo Plan

## Goal
Create a Rust-based RACTOR app (`apps/ractor-apps-askrepo`) that polls Twitter mentions, provisions dedicated RACTOR sessions per tweet, and seeds them with instructions to fetch and answer repository-related questions via the twitter_api_client tooling.

## Key Assumptions
- Twitter API credentials (bearer token + target user id) are provided via `RACTOR_APPS_ASKREPO_TWITTER_*` environment variables.
- RACTOR host URL and admin token are available via `RACTOR_HOST_URL` and `RACTOR_APPS_ASKREPO_ADMIN_TOKEN` respectively.
- Optional Twitter OAuth credentials (`RACTOR_APPS_ASKREPO_TWITTER_*` / `TWITTER_*`) may be set so the service can forward them into the session `.env` file for the `twitter_api_client` tools.
- Sessions are uniquely keyed by name `Tweet_<tweet_id>`, so existence checks can reuse that naming convention to avoid duplicates.
- Build tooling assumes Rust 1.82+ (update local toolchain via `rustup update stable`).

## Execution Steps
- [x] Scaffold the Rust project structure under `apps/ractor-apps-askrepo` with Cargo configuration, dependencies (tokio, reqwest, serde, tracing, etc.), and bin entrypoint.
- [x] Implement configuration loading (env-driven), Twitter polling client, and RACTOR API client helpers with models for tweets and sessions.
- [x] Build the polling loop (60s cadence) that fetches mentions (using `since_id` tracking), validates per-tweet requirements, checks/creates sessions, and posts initial prompts with instruction template referencing `twitter_api_client`.
- [x] Add supporting documentation (README-style usage notes) and update `askrepo.md` progress to reflect completed steps.
- [x] Provide Docker packaging plus CLI/build integration (images, volumes, start/stop commands) for running the AskRepo service alongside existing apps.
- [x] Run `cargo fmt`/`cargo check` for the new crate and perform a final self-review ahead of handoff.

## Deliverables
- `apps/ractor-apps-askrepo/` Rust crate with functioning polling app.
- Documentation on required environment variables and run instructions.
- Updated `askrepo.md` showing plan completion status.
