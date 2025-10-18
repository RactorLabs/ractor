# RACTOR Apps: AskRepo

`ractor-apps-askrepo` is a small Rust service that polls Twitter mentions for a target account and provisions RACTOR sessions to answer repository questions referenced in those tweets.

## How It Works
- Every 60 seconds (configurable), the service calls `GET /2/users/:id/mentions` on the Twitter API using the provided user identifier.
- Each mention is mapped to an session named `Tweet_<tweet_id>`. Existing sessions are reused; new tweets trigger a fresh session creation through the RACTOR API.
- The newly created session receives:
  - Metadata describing the source tweet.
  - A guardrail-focused instruction set directing the session to clone `github.com/ractor/twitter_api_client`, gather the conversation thread, vet the request, inspect the referenced repository, and reply using the Twitter client tooling.
  - The `tweet_id` as the initial prompt.

## Required Environment Variables
| Variable | Description |
| --- | --- |
| `RACTOR_HOST_URL` | Base URL for RACTOR (e.g., `http://localhost:9000`). |
| `RACTOR_APPS_ASKREPO_ADMIN_TOKEN` | Operator token with permission to create sessions. |
| `RACTOR_APPS_ASKREPO_TWITTER_BEARER_TOKEN` | Twitter API v2 bearer token. |
| `RACTOR_APPS_ASKREPO_TWITTER_USER_ID` | Twitter numeric user id whose mentions should be polled. |

### Optional Environment Variables
| Variable | Default | Description |
| --- | --- | --- |
| `RACTOR_APPS_ASKREPO_TWITTER_API_BASE` | `https://api.x.com` | Override for the Twitter API base URL. |
| `RACTOR_APPS_ASKREPO_POLL_INTERVAL_SECS` | `90` | Poll cadence in seconds (minimum 10s enforced). |
| `RACTOR_APPS_ASKREPO_TWITTER_SINCE_ID` | unset | Seed `since_id` to skip older mentions on startup. |
| `RACTOR_APPS_ASKREPO_TWITTER_API_KEY` / `TWITTER_API_KEY` | unset | OAuth consumer key forwarded to sessions when set. |
| `RACTOR_APPS_ASKREPO_TWITTER_API_SECRET` / `TWITTER_API_SECRET` | unset | OAuth consumer secret forwarded to sessions when set. |
| `RACTOR_APPS_ASKREPO_TWITTER_ACCESS_TOKEN` / `TWITTER_ACCESS_TOKEN` | unset | OAuth access token forwarded to sessions when set. |
| `RACTOR_APPS_ASKREPO_TWITTER_ACCESS_TOKEN_SECRET` / `TWITTER_ACCESS_TOKEN_SECRET` | unset | OAuth access token secret forwarded to sessions when set. |

## Running Locally
> Requires Rust 1.82 or newer (`rustup update stable`).

```bash
cargo run --manifest-path apps/ractor-apps-askrepo/Cargo.toml
```

The service listens for `Ctrl+C` and will exit gracefully.

## Container Usage
- Build image: `./scripts/build.sh app_askrepo` (or `ractor build app_askrepo` in dev).
- Start container: `ractor start app_askrepo` (requires the env vars above in your shell).
- Logs: `docker logs ractor_app_askrepo -f`.

## Notes
- Tags applied to provisioned sessions: `askrepo`, `twitter`, and `tweet<tweet_id>`.
- Sessions are created with a 15-minute busy timeout and receive the `tweet_id` as their initial prompt.
- Twitter API rate limits are surfaced via logs; the service will retry on the next polling interval.
- When present, the Twitter credentials listed above are copied into the session `.env` file as `TWITTER_*` keys so the `twitter_api_client` tooling can authenticate.
