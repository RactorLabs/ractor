# Ractor Operator UI Plan

Goal: Build a complete Operator web UI that documents and interacts with the Ractor Server REST APIs. The documentation pages are public (no auth). Interactive pages require authentication and use a cookie-stored JWT for API calls.

## Scope overview
- API documentation (public): Cover every endpoint under `GET /api/v0/**` with clear method, path, params, request/response examples, and notes.
- Interactive UI (authenticated):
  - Login page (Operator authentication)
  - Sessions list and details
  - Session chat-style responses (send + poll)
  - Session state/actions (wake, sleep, idle, busy, branch, publish)
  - Basic profile/settings for the logged-in operator

## References
- Server routes: `src/server/rest/routes.rs`
  - Public:
    - `GET  /api/v0/version`
    - `POST /api/v0/operators/{name}/login`
    - `GET  /api/v0/published/sessions`
    - `GET  /api/v0/published/sessions/{name}`
  - Protected (Bearer JWT):
    - Auth: `GET /api/v0/auth`, `POST /api/v0/auth/token`
    - Operators: CRUD + password update
    - Sessions: CRUD + state transitions + publish lifecycle
    - Responses: list/create/count
- CLI 0.4.4 (tag) for flows and payload shapes:
  - `cli/lib/api.js` shows: base URL, Bearer token header, endpoint prefixing (`/api/v0`), and typical request/response handling.

## Architecture decisions
- API base URL: default to same origin (`/api/v0`) with option to override via Vite env (e.g. `VITE_API_BASE`); use relative paths by default for Docker deployment behind a reverse proxy.
- Auth token storage: use a cookie set by the client after login:
  - Name: `ractor_token`
  - Attributes: `path=/; sameSite=Lax`; set `secure` in production. (Note: httpOnly cookies cannot be set from JS; for now we prefer simple client-set cookies. Consider server-set httpOnly cookies as a future hardening.)
  - Also store `ractor_operator` for operator name/identity.
- API client: a small wrapper around `fetch`:
  - Prepends `/api/v0` if missing
  - Attaches `Authorization: Bearer <token>` header when cookie present
  - Centralized error handling and JSON parsing
- Routing structure:
  - Public
    - `/` (home/overview)
    - `/docs/*` (API documentation)
    - `/login` (operator login)
  - Authenticated (guarded layout)
    - `/app/sessions` (list)
    - `/app/sessions/[name]` (details + chat/responses)
    - `/app/profile` (basic account info)
    - `/app/settings` (basic settings)
- UI conventions: follow existing Operator template components, SCSS, and page layout options in `appOptions`.

## Phase 1 — API Documentation (public)
Deliverables:
- Docs index at `/docs` with high-level overview and version.
- Categories and pages mapping to server routes:
  - Auth (public + protected)
  - Operators
  - Sessions
  - Responses
  - Published (public catalog)
- Implementation details:
  - Source-of-truth: a typed JSON/TS structure (`src/lib/api/docs.ts` or `static/api-docs.json`) describing each endpoint: method, path, path params, query params, body schema (concise), success/err examples.
  - Svelte pages under `src/routes/docs/**` render from that structure.
  - Link “Try it” buttons route users to corresponding interactive pages (gated by auth).

## Phase 2 — Authentication
Deliverables:
- `/login` page styled per template (see HUD startup login page styles).
- Submit to `POST /api/v0/operators/{name}/login` with `{ pass }`.
- On success:
  - Save JWT to cookie `ractor_token`
  - Save operator name to cookie `ractor_operator`
  - Redirect to `/app/sessions`
- Auth guard:
  - Create `src/routes/app/+layout.svelte` that checks token cookie on mount and redirects to `/login` when missing/invalid.
  - Optionally ping `GET /api/v0/auth` once to confirm validity; if 401, clear cookie and redirect to `/login`.

## Phase 3 — Sessions (list + details)
Deliverables:
- `/app/sessions` page: table of sessions using `GET /api/v0/sessions`
  - Columns: id, name, state, published, created_at, updated_at
  - Actions: open details, wake/sleep/idle/busy, publish/unpublish, delete
- `/app/sessions/[name]` page: basic info panel + messaging panel
  - Info: session metadata and quick actions (wake/sleep/etc.)
  - Responses (chat):
    - List: `GET /api/v0/sessions/{name}/responses`
    - Send: `POST /api/v0/sessions/{name}/responses`
    - Poll: periodic `GET /api/v0/sessions/{name}/responses/count` to detect changes and refresh list
- UX: use template components for layout, forms, and toasts; loading states; error banners.

## Phase 4 — Operators & Profile
Deliverables:
- `/app/profile` page using `GET /api/v0/auth` to show current operator data.
- Optional operators admin (if role allows): list/create/update/delete operators based on:
  - `GET/POST /api/v0/operators`
  - `GET/PUT/DELETE /api/v0/operators/{name}`
  - `PUT /api/v0/operators/{name}/password`

## Phase 5 — Polish & Hardening
- Add logout (clear cookies; redirect to `/login`).
- Global API error handler and notification system.
- Empty states, pagination for lists, and basic filters for sessions.
- Optional: Move to httpOnly cookies with server-set `Set-Cookie` if API evolves to support it.
- Optional: Replace message polling with SSE/WebSocket when server supports it.

## Implementation breakdown (task list)
1) Docs data source and `/docs` routes (public)
2) Login page and auth cookie handling
3) Auth-guarded `/app` layout
4) API client wrapper with token attachment
5) Sessions list page
6) Session details + responses (send/poll)
7) Profile page and logout
8) Operator admin pages (optional by role)
9) Polish: errors, toasts, pagination, filters

## API coverage checklist (server `routes.rs`)
- [ ] GET  /api/v0/version (docs)
- [ ] POST /api/v0/operators/{name}/login (docs + login)
- [ ] GET  /api/v0/published/sessions (docs)
- [ ] GET  /api/v0/published/sessions/{name} (docs)
- [ ] GET  /api/v0/auth (docs + profile)
- [ ] POST /api/v0/auth/token (docs)
- [ ] GET  /api/v0/operators (docs [+ UI if applicable])
- [ ] POST /api/v0/operators (docs [+ UI if applicable])
- [ ] GET  /api/v0/operators/{name} (docs)
- [ ] PUT  /api/v0/operators/{name} (docs)
- [ ] DELETE /api/v0/operators/{name} (docs)
- [ ] PUT  /api/v0/operators/{name}/password (docs)
- [ ] GET  /api/v0/sessions (docs + list UI)
- [ ] POST /api/v0/sessions (docs)
- [ ] GET  /api/v0/sessions/{name} (docs + details UI)
- [ ] PUT  /api/v0/sessions/{name} (docs)
- [ ] PUT  /api/v0/sessions/{name}/state (docs)
- [ ] POST /api/v0/sessions/{name}/busy (docs)
- [ ] POST /api/v0/sessions/{name}/idle (docs)
- [ ] POST /api/v0/sessions/{name}/sleep (docs)
- [ ] POST /api/v0/sessions/{name}/wake (docs + action UI)
- [ ] POST /api/v0/sessions/{name}/branch (docs)
- [ ] POST /api/v0/sessions/{name}/publish (docs)
- [ ] POST /api/v0/sessions/{name}/unpublish (docs)
- [ ] DELETE /api/v0/sessions/{name} (docs)
- [ ] GET  /api/v0/sessions/{name}/responses (docs + chat UI)
- [ ] POST /api/v0/sessions/{name}/responses (docs + chat UI)
- [ ] GET  /api/v0/sessions/{name}/responses/count (docs + polling)

## Notes
- Initial implementation will avoid adding new backend endpoints; only consume existing APIs.
- We’ll follow existing SCSS and Svelte component patterns in `operator/` to keep styling consistent.
- As we implement each phase, we’ll update this plan with status and any deviations.
