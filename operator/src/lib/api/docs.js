// API documentation data source for TaskSandbox UI
// Covers endpoints defined in src/server/rest/routes.rs
import { getHostUrl } from '../branding.js';

// Common response schemas used across endpoints
export function getCommonSchemas() {
  return {
    Version: [
      { name: 'version', type: 'string', desc: 'Semantic version of server' },
      { name: 'api', type: 'string', desc: "API namespace (e.g., 'v0')" },
    ],
    TokenResponse: [
      { name: 'token', type: 'string', desc: 'JWT token' },
      { name: 'token_type', type: 'string', desc: "Always 'Bearer'" },
      { name: 'expires_at', type: 'string (RFC3339)', desc: 'Expiry timestamp' },
      { name: 'user', type: 'string', desc: 'Principal name associated with token' },
      { name: 'role', type: 'string', desc: "'admin' or 'user'" },
    ],
    AuthProfile: [
      { name: 'user', type: 'string', desc: 'Principal name' },
      { name: 'type', type: 'string', desc: "'Admin' or 'User'" },
    ],
    Operator: [
      { name: 'user', type: 'string', desc: 'Operator username' },
      { name: 'description', type: 'string|null', desc: 'Optional description' },
      { name: 'active', type: 'boolean', desc: 'Account active flag' },
      { name: 'created_at', type: 'string (RFC3339)', desc: 'Creation timestamp' },
      { name: 'updated_at', type: 'string (RFC3339)', desc: 'Last update timestamp' },
      { name: 'last_login_at', type: 'string|null (RFC3339)', desc: 'Last login timestamp' },
    ],
    Session: [
      { name: 'name', type: 'string', desc: 'Session name (primary key)' },
      { name: 'created_by', type: 'string', desc: 'Owner username' },
      { name: 'state', type: 'string', desc: 'init|idle|busy|slept' },
      { name: 'description', type: 'string|null', desc: 'Optional description' },
      { name: 'parent_session_name', type: 'string|null', desc: 'Parent session name if branched' },
      { name: 'created_at', type: 'string (RFC3339)', desc: 'Creation timestamp' },
      { name: 'last_activity_at', type: 'string|null (RFC3339)', desc: 'Last activity timestamp' },
      { name: 'metadata', type: 'object', desc: 'Arbitrary JSON metadata' },
      { name: 'tags', type: 'string[]', desc: "Array of tags (letters, digits, '/', '-', '_', '.' only; stored lowercase)" },
      { name: 'is_published', type: 'boolean', desc: 'Published state' },
      { name: 'published_at', type: 'string|null (RFC3339)', desc: 'When published' },
      { name: 'published_by', type: 'string|null', desc: 'Who published' },
      { name: 'publish_permissions', type: 'object', desc: '{ code: boolean, env: boolean, content: boolean }' },
      { name: 'idle_timeout_seconds', type: 'int', desc: 'Idle timeout' },
      { name: 'busy_timeout_seconds', type: 'int', desc: 'Busy timeout' },
      { name: 'idle_from', type: 'string|null (RFC3339)', desc: 'When idle started' },
      { name: 'busy_from', type: 'string|null (RFC3339)', desc: 'When busy started' },
      { name: 'context_cutoff_at', type: 'string|null (RFC3339)', desc: 'Current context cutoff timestamp if set' },
    ],
    ListSessionsResult: [
      { name: 'items', type: 'Session[]', desc: 'Array of sessions for current page' },
      { name: 'total', type: 'int', desc: 'Total sessions matching filters' },
      { name: 'limit', type: 'int', desc: 'Page size' },
      { name: 'offset', type: 'int', desc: 'Row offset (0-based)' },
      { name: 'page', type: 'int', desc: 'Current page number (1-based)' },
      { name: 'pages', type: 'int', desc: 'Total page count' },
    ],
    ResponseObject: [
      { name: 'id', type: 'string', desc: 'Response ID (UUID)' },
      { name: 'session_name', type: 'string', desc: 'Session name' },
      { name: 'status', type: 'string', desc: "'pending'|'processing'|'completed'|'failed'|'cancelled'" },
      { name: 'input_content', type: 'array', desc: "User input content items (e.g., [{ type: 'text', content: 'hello' }]). Preferred input shape uses 'content' array; legacy { text: string } is accepted but not echoed in input_content." },
      { name: 'output_content', type: 'array', desc: "Final content items extracted from segments (typically the 'output' tool_result payload)" },
      { name: 'segments', type: 'array', desc: 'All step-by-step segments/items: commentary, tool calls/results, system markers, final' },
      { name: 'created_at', type: 'string (RFC3339)', desc: 'Creation timestamp' },
      { name: 'updated_at', type: 'string (RFC3339)', desc: 'Last update timestamp' },
    ],
    BlockedPrincipal: [
      { name: 'principal', type: 'string', desc: 'Principal name' },
      { name: 'principal_type', type: 'string', desc: "'User' or 'Admin'" },
      { name: 'created_at', type: 'string (RFC3339)', desc: 'When the principal was blocked' },
    ],
    Count: [
      { name: 'count', type: 'int', desc: 'Count value' },
      { name: 'session_name', type: 'string', desc: 'Session identifier' },
    ],
    RuntimeTotal: [
      { name: 'session_name', type: 'string', desc: 'Session name' },
      { name: 'total_runtime_seconds', type: 'int', desc: 'Total runtime across sessions (seconds)' },
      { name: 'current_session_seconds', type: 'int', desc: 'Current session runtime (seconds), 0 if sleeping' },
    ],
    BusyIdleAck: [
      { name: 'success', type: 'boolean', desc: 'true on success' },
      { name: 'state', type: 'string', desc: "'busy' or 'idle'" },
      { name: 'timeout_status', type: 'string', desc: "'paused' (busy) or 'active' (idle)" },
    ],
    StateAck: [
      { name: 'success', type: 'boolean', desc: 'true on success' },
      { name: 'state', type: 'string', desc: 'New state value' },
    ],
    SessionContextUsage: [
      { name: 'session', type: 'string', desc: 'Session name' },
      { name: 'soft_limit_tokens', type: 'int', desc: 'Soft limit (tokens)' },
      { name: 'used_tokens_estimated', type: 'int', desc: 'Estimated tokens since cutoff' },
      { name: 'used_percent', type: 'float', desc: 'Usage percent of soft limit' },
      { name: 'basis', type: 'string', desc: 'Estimation method' },
      { name: 'cutoff_at', type: 'string|null (RFC3339)', desc: 'Current context cutoff (null when absent)' },
      { name: 'measured_at', type: 'string (RFC3339)', desc: 'Measurement timestamp' },
      { name: 'total_messages_considered', type: 'int', desc: 'Messages scanned to compute estimate' },
    ],
    FileEntry: [
      { name: 'name', type: 'string', desc: 'Entry name (no path)' },
      { name: 'kind', type: 'string', desc: "'file' | 'dir' | 'symlink'" },
      { name: 'size', type: 'int', desc: 'Size in bytes' },
      { name: 'mode', type: 'string', desc: 'Permissions in chmod-style octal (e.g., 0755)' },
      { name: 'mtime', type: 'string (RFC3339)', desc: 'Last modified time' },
    ],
    FileListResult: [
      { name: 'entries', type: 'FileEntry[]', desc: 'Entries for the requested folder' },
      { name: 'offset', type: 'int', desc: 'Offset of this page' },
      { name: 'limit', type: 'int', desc: 'Page size' },
      { name: 'next_offset', type: 'int|null', desc: 'Offset for the next page, or null if end' },
      { name: 'total', type: 'int', desc: 'Total number of entries' },
    ],
    FileMetadata: [
      { name: 'kind', type: 'string', desc: "'file' | 'dir' | 'symlink'" },
      { name: 'size', type: 'int', desc: 'Size in bytes' },
      { name: 'mode', type: 'string', desc: 'Permissions in chmod-style octal (e.g., 0644)' },
      { name: 'mtime', type: 'string (RFC3339)', desc: 'Last modified time' },
      { name: 'link_target', type: 'string (symlink only)', desc: 'Target path for symlink', optional: true },
    ],
    CancelAck: [
      { name: 'status', type: 'string', desc: "Always 'ok' on success" },
      { name: 'session', type: 'string', desc: 'Session name' },
      { name: 'cancelled', type: 'boolean', desc: 'true if a pending/processing response or queued update was cancelled' },
    ],
    Empty: [],
  };
}

export function getApiDocs(base) {
  const BASE = base || getHostUrl();
  return [
  {
    id: 'version',
    title: 'Version',
    description: 'Public API version information.',
    endpoints: [
      {
        method: 'GET',
        path: '/api/v0/version',
        auth: 'public',
        desc: 'Get API version and current version string.',
        params: [],
        example: `curl -s ${BASE}/api/v0/version`,
        resp: { schema: 'Version' },
        responses: [
          {
            status: 200,
            body: `{
  "version": "0.x.y",
  "api": "v0"
}`
          }
        ]
      }
    ]
  },
  
  
  
  {
    id: 'admin',
    title: 'Admin / Security',
    description: 'Administrative endpoints (blocklist management).',
    endpoints: [
      {
        method: 'GET',
        path: '/api/v0/blocklist',
        auth: 'bearer',
        adminOnly: true,
        desc: 'List blocked principals.',
        params: [],
        example: `curl -s ${BASE}/api/v0/blocklist -H "Authorization: Bearer <token>"`,
        resp: { schema: 'BlockedPrincipal', array: true },
        responses: [
          { status: 200, body: `[{"principal":"user1","principal_type":"User","created_at":"2025-01-01T00:00:00Z"}]` }
        ]
      },
      {
        method: 'POST',
        path: '/api/v0/blocklist/block',
        auth: 'bearer',
        adminOnly: true,
        desc: "Block a principal by name. Defaults to type 'User'.",
        params: [
          { in: 'body', name: 'principal', type: 'string', required: true, desc: 'Principal name' },
          { in: 'body', name: 'type', type: 'string', required: false, desc: "Optional. Principal type: 'User' or 'Admin' (default: 'User')" }
        ],
        example: `curl -s -X POST ${BASE}/api/v0/blocklist/block -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"principal":"user1"}'`,
        resp: { schema: 'Empty' },
        responses: [ { status: 200, body: `{"blocked":true,"created":true}` } ]
      },
      {
        method: 'POST',
        path: '/api/v0/blocklist/unblock',
        auth: 'bearer',
        adminOnly: true,
        desc: "Unblock a principal by name. Defaults to type 'User'.",
        params: [
          { in: 'body', name: 'principal', type: 'string', required: true, desc: 'Principal name' },
          { in: 'body', name: 'type', type: 'string', required: false, desc: "Optional. Principal type: 'User' or 'Admin' (default: 'User')" }
        ],
        example: `curl -s -X POST ${BASE}/api/v0/blocklist/unblock -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"principal":"user1"}'`,
        resp: { schema: 'Empty' },
        responses: [ { status: 200, body: `{"blocked":false,"deleted":true}` } ]
      }
    ]
  },
  {
    id: 'auth',
    title: 'Authentication',
    description: 'Login and token management for Admin and users.',
    endpoints: [
      {
        method: 'POST',
        path: '/api/v0/operators/{name}/login',
        auth: 'public',
        desc: 'Login with operator name and password. Returns JWT token and user info.',
        params: [
          { in: 'path', name: 'name', type: 'string', required: true, desc: 'Operator username' },
          { in: 'body', name: 'pass', type: 'string', required: true, desc: 'Operator password' },
          { in: 'body', name: 'ttl_hours', type: 'number', required: false, desc: 'Optional token TTL in hours; omit or <= 0 for no expiry (default)' }
        ],
        example: `curl -s -X POST ${BASE}/api/v0/operators/<name>/login -H "Content-Type: application/json" -d '{"pass":"<password>","ttl_hours":24}'\n\n# ttl_hours is optional. Omit or set <=0 for no expiry (default).`,
        resp: { schema: 'TokenResponse' },
        responses: [
          {
            status: 200,
            body: `{
  "token": "<jwt>",
  "token_type": "Bearer",
  "expires_at": "2025-01-01T12:34:56Z",
  "user": "admin",
  "role": "admin"
}`
          },
        ]
      },
      {
        method: 'GET',
        path: '/api/v0/auth',
        auth: 'bearer',
        desc: 'Get authenticated profile (validate token).',
        params: [],
        example: `curl -s ${BASE}/api/v0/auth -H "Authorization: Bearer <token>"`,
        resp: { schema: 'AuthProfile' },
        responses: [
          {
            status: 200,
            body: `{
  "user": "admin",
  "type": "Admin"
}`
          }
        ]
      },
      {
        method: 'POST',
        path: '/api/v0/auth/token',
        auth: 'bearer',
        desc: 'Create a new token for a principal (admin-only).',
        adminOnly: true,
        params: [
          { in: 'body', name: 'principal', type: 'string', required: true, desc: 'Principal name (user or admin id)' },
          { in: 'body', name: 'type', type: 'string', required: true, desc: "Principal type: 'User' or 'Admin'" },
          { in: 'body', name: 'ttl_hours', type: 'number', required: false, desc: 'Optional token TTL in hours; omit or <= 0 for no expiry (default)' }
        ],
        example: `curl -s -X POST ${BASE}/api/v0/auth/token -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"principal":"some-user","type":"User","ttl_hours":12}'\n\n# ttl_hours is optional. Omit or set <=0 for no expiry (default).`,
        resp: { schema: 'TokenResponse' },
        responses: [
          {
            status: 200,
            body: `{
  "token": "<jwt>",
  "token_type": "Bearer",
  "expires_at": "2025-01-01T12:34:56Z",
  "user": "some-user",
  "role": "user"
}`
          }
        ]
      }
    ]
  },
  {
    id: 'published',
    title: 'Published Sessions (Public)',
    description: 'Publicly visible sessions and details.',
    endpoints: [
      {
        method: 'GET',
        path: '/api/v0/published/sessions',
        auth: 'public',
        desc: 'List all published sessions.',
        params: [],
        example: `curl -s ${BASE}/api/v0/published/sessions`,
        resp: { schema: 'Session', array: true },
        responses: [
          {
            status: 200,
            body: `[
  {
    "name": "demo",
    "created_by": "admin",
    "state": "idle",
    "description": "Demo session",
    "parent_session_name": null,
    "created_at": "2025-01-01T12:00:00Z",
    "last_activity_at": "2025-01-01T12:00:00Z",
    "metadata": {},
    "tags": ["example"],
    "is_published": true,
    "published_at": "2025-01-01T12:30:00Z",
    "published_by": "admin",
    "publish_permissions": {"code": true, "env": false, "content": true},
    "idle_timeout_seconds": 300,
    "busy_timeout_seconds": 3600,
    "idle_from": "2025-01-01T12:10:00Z",
    "busy_from": null
  }
]`
          }
        ]
      },
      {
        method: 'GET',
        path: '/api/v0/published/sessions/{name}',
        auth: 'public',
        desc: 'Get details of a published session by name.',
        params: [
          { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' }
        ],
        example: `curl -s ${BASE}/api/v0/published/sessions/<name>`,
        resp: { schema: 'Session' },
        responses: [
          {
            status: 200,
            body: `{
  "name": "demo",
  "created_by": "admin",
  "state": "idle",
  "description": "Demo session",
  "parent_session_name": null,
  "created_at": "2025-01-01T12:00:00Z",
  "last_activity_at": "2025-01-01T12:00:00Z",
  "metadata": {},
  "tags": ["example"],
  "is_published": true,
  "published_at": "2025-01-01T12:30:00Z",
  "published_by": "admin",
  "publish_permissions": {"code": true, "env": false, "content": true},
  "idle_timeout_seconds": 300,
  "busy_timeout_seconds": 3600,
  "idle_from": "2025-01-01T12:10:00Z",
  "busy_from": null
}`
          }
        ]
      }
    ]
  },
  {
    id: 'operators',
    title: 'Operators',
    description: 'Operator management endpoints (protected).',
    endpoints: [
      { method: 'POST', path: '/api/v0/operators/{name}/login', auth: 'public', adminOnly: true, desc: 'Operator login with username and password. Returns JWT token and user info.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Operator username' },
        { in: 'body', name: 'pass', type: 'string', required: true, desc: 'Operator password' },
        { in: 'body', name: 'ttl_hours', type: 'number', required: false, desc: 'Optional token TTL in hours; omit or <= 0 for no expiry (default)' }
      ], example: `curl -s -X POST ${BASE}/api/v0/operators/<name>/login -H "Content-Type: application/json" -d '{"pass":"<password>","ttl_hours":24}'\n\n# ttl_hours is optional. Omit or set <=0 for no expiry (default).`, resp: { schema: 'TokenResponse' }, responses: [ { status: 200, body: `{"token":"<jwt>","token_type":"Bearer","expires_at":"2025-01-01T12:34:56Z","user":"admin","role":"admin"}` } ] },
      { method: 'GET', path: '/api/v0/operators', auth: 'bearer', desc: 'List operators.', adminOnly: true, params: [], example: `curl -s ${BASE}/api/v0/operators -H "Authorization: Bearer <token>"`, resp: { schema: 'Operator', array: true }, responses: [{ status: 200, body: `[{"user":"admin","description":null,"active":true,"created_at":"2025-01-01T00:00:00Z","updated_at":"2025-01-01T00:00:00Z","last_login_at":"2025-01-01T12:00:00Z"}]` }] },
      { method: 'POST', path: '/api/v0/operators', auth: 'bearer', desc: 'Create operator.', adminOnly: true, params: [
        { in: 'body', name: 'user', type: 'string', required: true, desc: 'Operator username' },
        { in: 'body', name: 'pass', type: 'string', required: true, desc: 'Password' },
        { in: 'body', name: 'description', type: 'string', required: false, desc: 'Optional description' }
      ], example: `curl -s -X POST ${BASE}/api/v0/operators -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"user":"alice","pass":"<password>","description":"Team operator"}'`, resp: { schema: 'Operator' }, responses: [{ status: 200, body: `{"user":"alice","description":"Team operator","active":true,"created_at":"2025-01-01T00:00:00Z","updated_at":"2025-01-01T00:00:00Z","last_login_at":null}` }] },
      { method: 'GET', path: '/api/v0/operators/{name}', auth: 'bearer', desc: 'Get operator.', adminOnly: true, params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Operator username' }
      ], example: `curl -s ${BASE}/api/v0/operators/<name> -H "Authorization: Bearer <token>"`, resp: { schema: 'Operator' }, responses: [{ status: 200, body: `{"user":"alice","description":"Team operator","active":true,"created_at":"2025-01-01T00:00:00Z","updated_at":"2025-01-02T10:00:00Z","last_login_at":null}` }] },
      { method: 'PUT', path: '/api/v0/operators/{name}', auth: 'bearer', desc: 'Update operator.', adminOnly: true, params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Operator username' },
        { in: 'body', name: 'description', type: 'string', required: false, desc: 'Optional description' },
        { in: 'body', name: 'active', type: 'boolean|null', required: false, desc: 'Set active status; must be boolean or null' }
      ], example: `curl -s -X PUT ${BASE}/api/v0/operators/<name> -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"description":"Updated desc","active":true}'`, resp: { schema: 'Operator' }, responses: [{ status: 200, body: `{"user":"alice","description":"Updated desc","active":true,"created_at":"2025-01-01T00:00:00Z","updated_at":"2025-01-02T12:00:00Z","last_login_at":null}` }] },
      { method: 'DELETE', path: '/api/v0/operators/{name}', auth: 'bearer', desc: 'Delete operator.', adminOnly: true, params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Operator username' }
      ], example: `curl -s -X DELETE ${BASE}/api/v0/operators/<name> -H "Authorization: Bearer <token>"`, resp: { schema: 'Empty' }, responses: [{ status: 200 }] },
      { method: 'PUT', path: '/api/v0/operators/{name}/password', auth: 'bearer', desc: 'Update operator password.', adminOnly: true, params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Operator username' },
        { in: 'body', name: 'current_password', type: 'string', required: true, desc: 'Current password' },
        { in: 'body', name: 'new_password', type: 'string', required: true, desc: 'New password' }
      ], example: `curl -s -X PUT ${BASE}/api/v0/operators/<name>/password -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"current_password":"<old>","new_password":"<new>"}'`, resp: { schema: 'Empty' }, responses: [{ status: 200 }] }
    ]
  },
  {
    id: 'sessions',
    title: 'Sessions',
    description: 'Session lifecycle and management endpoints (protected).',
    endpoints: [
      { method: 'GET', path: '/api/v0/sessions', auth: 'bearer', desc: 'List/search sessions with pagination.', params: [
        { in: 'query', name: 'q', type: 'string', required: false, desc: 'Search substring over name and description (case-insensitive)' },
        { in: 'query', name: 'tags', type: 'string (comma-separated)', required: false, desc: 'Filter by tags (INTERSECTION/AND). Provide multiple tags as a comma-separated list (e.g., tags=prod,team). Tags are matched case-insensitively and stored lowercase.' },
        { in: 'query', name: 'state', type: 'string', required: false, desc: 'Filter by state: init|idle|busy|slept' },
        { in: 'query', name: 'limit', type: 'int', required: false, desc: 'Page size (default 30, max 100)' },
        { in: 'query', name: 'page', type: 'int', required: false, desc: 'Page number (1-based). Ignored when offset is set.' },
        { in: 'query', name: 'offset', type: 'int', required: false, desc: 'Row offset (0-based). Takes precedence over page.' }
      ], example: `curl -s ${BASE}/api/v0/sessions?q=demo&tags=prod,team/core&state=idle&limit=30&page=1 -H "Authorization: Bearer <token>"`, resp: { schema: 'ListSessionsResult' }, responses: [{ status: 200, body: `{"items":[{"name":"demo","created_by":"admin","state":"idle","description":"Demo session","parent_session_name":null,"created_at":"2025-01-01T12:00:00Z","last_activity_at":"2025-01-01T12:10:00Z","metadata":{},"tags":["prod","team/core"],"is_published":false,"published_at":null,"published_by":null,"publish_permissions":{"code":true,"env":true,"content":true},"idle_timeout_seconds":300,"busy_timeout_seconds":3600,"idle_from":"2025-01-01T12:10:00Z","busy_from":null}],"total":1,"limit":30,"offset":0,"page":1,"pages":1}` }] },
      { method: 'POST', path: '/api/v0/sessions', auth: 'bearer', desc: 'Create session.', params: [
        { in: 'body', name: 'name', type: 'string', required: true, desc: 'Session name; must match ^[A-Za-z][A-Za-z0-9-]{0,61}[A-Za-z0-9]$' },
        { in: 'body', name: 'description', type: 'string|null', required: false, desc: 'Optional human-readable description' },
        { in: 'body', name: 'metadata', type: 'object', required: false, desc: 'Arbitrary JSON metadata (default: {})' },
        { in: 'body', name: 'tags', type: 'string[]', required: false, desc: "Array of tags; allowed characters are letters, digits, '/', '-', '_', '.'; no spaces (default: [])" },
        { in: 'body', name: 'env', type: 'object<string,string>', required: false, desc: 'Key/value env map (default: empty)' },
        { in: 'body', name: 'instructions', type: 'string|null', required: false, desc: 'Optional instructions' },
        { in: 'body', name: 'setup', type: 'string|null', required: false, desc: 'Optional setup script or commands' },
        { in: 'body', name: 'prompt', type: 'string|null', required: false, desc: 'Optional initial prompt' },
        { in: 'body', name: 'idle_timeout_seconds', type: 'int|null', required: false, desc: 'Idle timeout seconds (default 300)' },
        { in: 'body', name: 'busy_timeout_seconds', type: 'int|null', required: false, desc: 'Busy timeout seconds (default 3600)' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sessions -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"name":"demo","description":"Demo session"}'`, resp: { schema: 'Session' }, responses: [{ status: 200, body: `{"name":"demo","created_by":"admin","state":"init","description":"Demo session","parent_session_name":null,"created_at":"2025-01-01T12:00:00Z","last_activity_at":null,"metadata":{},"tags":[],"is_published":false,"published_at":null,"published_by":null,"publish_permissions":{"code":true,"env":true,"content":true},"idle_timeout_seconds":300,"busy_timeout_seconds":3600,"idle_from":null,"busy_from":null}` }] },
      { method: 'GET', path: '/api/v0/sessions/{name}', auth: 'bearer', desc: 'Get session by name.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' }
      ], example: `curl -s ${BASE}/api/v0/sessions/<name> -H "Authorization: Bearer <token>"`, resp: { schema: 'Session' }, responses: [{ status: 200, body: `{"name":"demo","created_by":"admin","state":"idle","description":"Demo session","parent_session_name":null,"created_at":"2025-01-01T12:00:00Z","last_activity_at":"2025-01-01T12:10:00Z","metadata":{},"tags":[],"is_published":false,"published_at":null,"published_by":null,"publish_permissions":{"code":true,"env":true,"content":true},"idle_timeout_seconds":300,"busy_timeout_seconds":3600,"idle_from":"2025-01-01T12:10:00Z","busy_from":null}` }] },
      { method: 'PUT', path: '/api/v0/sessions/{name}', auth: 'bearer', desc: 'Update session by name.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' },
        { in: 'body', name: 'metadata', type: 'object|null', required: false, desc: 'Replace metadata (omit to keep)' },
        { in: 'body', name: 'description', type: 'string|null', required: false, desc: 'Update description' },
        { in: 'body', name: 'tags', type: 'string[]|null', required: false, desc: "Replace tags array; allowed characters are letters, digits, '/', '-', '_', '.'; no spaces" },
        { in: 'body', name: 'idle_timeout_seconds', type: 'int|null', required: false, desc: 'Update idle timeout seconds' },
        { in: 'body', name: 'busy_timeout_seconds', type: 'int|null', required: false, desc: 'Update busy timeout seconds' }
      ], example: `curl -s -X PUT ${BASE}/api/v0/sessions/<name> -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"description":"Updated"}'`, resp: { schema: 'Session' }, responses: [{ status: 200, body: `{"name":"demo","created_by":"admin","state":"idle","description":"Updated","parent_session_name":null,"created_at":"2025-01-01T12:00:00Z","last_activity_at":"2025-01-01T12:20:00Z","metadata":{},"tags":[],"is_published":false,"published_at":null,"published_by":null,"publish_permissions":{"code":true,"env":true,"content":true},"idle_timeout_seconds":300,"busy_timeout_seconds":3600,"idle_from":"2025-01-01T12:20:00Z","busy_from":null}` }] },
      { method: 'PUT', path: '/api/v0/sessions/{name}/state', auth: 'bearer', desc: 'Update session state (generic).', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' },
        { in: 'body', name: 'state', type: 'string', required: true, desc: 'New state (e.g., init|idle|busy|slept)' }
      ], example: `curl -s -X PUT ${BASE}/api/v0/sessions/<name>/state -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"state":"idle"}'`, resp: { schema: 'StateAck' }, responses: [{ status: 200, body: `{"success":true,"state":"idle"}` }] },
      { method: 'POST', path: '/api/v0/sessions/{name}/busy', auth: 'bearer', desc: 'Set session busy.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sessions/<name>/busy -H "Authorization: Bearer <token>"`, resp: { schema: 'BusyIdleAck' }, responses: [{ status: 200, body: `{"success":true,"state":"busy","timeout_status":"paused"}` }] },
      { method: 'POST', path: '/api/v0/sessions/{name}/idle', auth: 'bearer', desc: 'Set session idle.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sessions/<name>/idle -H "Authorization: Bearer <token>"`, resp: { schema: 'BusyIdleAck' }, responses: [{ status: 200, body: `{"success":true,"state":"idle","timeout_status":"active"}` }] },
      { method: 'POST', path: '/api/v0/sessions/{name}/sleep', auth: 'bearer', desc: 'Schedule session to sleep after an optional delay (min/default 5s).', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' },
        { in: 'body', name: 'delay_seconds', type: 'int|null', required: false, desc: 'Delay before sleeping (min/default 5 seconds)' },
        { in: 'body', name: 'note', type: 'string|null', required: false, desc: 'Optional note to display in chat when sleep occurs' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sessions/<name>/sleep -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"delay_seconds":10,"note":"User requested sleep"}'\n\n# The session will sleep after the delay. State may not change immediately in the response.`, resp: { schema: 'Session' }, responses: [{ status: 200, body: `{"name":"demo","created_by":"admin","state":"idle",...}` }] },
      { method: 'POST', path: '/api/v0/sessions/{name}/cancel', auth: 'bearer', desc: 'Cancel the most recent pending/processing response (or queued update) and set session to idle.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sessions/<name>/cancel -H "Authorization: Bearer <token>"`, resp: { schema: 'CancelAck' }, responses: [{ status: 200, body: `{"status":"ok","session":"demo","cancelled":true}` }] },
      { method: 'POST', path: '/api/v0/sessions/{name}/wake', auth: 'bearer', desc: 'Wake session (optionally send a prompt).', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' },
        { in: 'body', name: 'prompt', type: 'string|null', required: false, desc: 'Optional prompt to send on wake' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sessions/<name>/wake -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"prompt":"get ready"}'`, resp: { schema: 'Session' }, responses: [{ status: 200, body: `{"name":"demo","created_by":"admin","state":"init",...}` }] },
      { method: 'GET', path: '/api/v0/sessions/{name}/runtime', auth: 'bearer', desc: 'Get total runtime across sessions (seconds). Includes current session (since last wake or creation).', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' }
      ], example: `curl -s ${BASE}/api/v0/sessions/<name>/runtime -H "Authorization: Bearer <token>"`, resp: { schema: 'RuntimeTotal' }, responses: [{ status: 200, body: `{"session_name":"demo","total_runtime_seconds":1234,"current_session_seconds":321}` }] },
      { method: 'POST', path: '/api/v0/sessions/{name}/branch', auth: 'bearer', desc: 'Branch session (create a new session from parent).', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Parent session name' },
        { in: 'body', name: 'name', type: 'string', required: true, desc: 'New session name; must match ^[A-Za-z][A-Za-z0-9-]{0,61}[A-Za-z0-9]$' },
        { in: 'body', name: 'metadata', type: 'object|null', required: false, desc: 'Optional metadata override' },
        { in: 'body', name: 'code', type: 'boolean', required: false, desc: 'Copy code (default true)' },
        { in: 'body', name: 'env', type: 'boolean', required: false, desc: 'Copy env (default true)' },
        { in: 'body', name: 'content', type: 'boolean', required: false, desc: 'Copy content (always true in v0.4.0+)' },
        { in: 'body', name: 'prompt', type: 'string|null', required: false, desc: 'Optional initial prompt' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sessions/<name>/branch -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"name":"demo-copy","code":true,"env":false,"prompt":"clone and adjust"}'`, resp: { schema: 'Session' }, responses: [{ status: 200, body: `{"name":"demo-copy","created_by":"admin","state":"init",...}` }] },
      { method: 'POST', path: '/api/v0/sessions/{name}/publish', auth: 'bearer', desc: 'Publish session.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' },
        { in: 'body', name: 'code', type: 'boolean', required: false, desc: 'Allow code branch (default true)' },
        { in: 'body', name: 'env', type: 'boolean', required: false, desc: 'Allow env branch (default true)' },
        { in: 'body', name: 'content', type: 'boolean', required: false, desc: 'Publish content (default true)' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sessions/<name>/publish -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"code":true,"env":false,"content":true}'`, resp: { schema: 'Session' }, responses: [{ status: 200, body: `{"name":"demo","is_published":true,"published_at":"2025-01-01T12:30:00Z",...}` }] },
      { method: 'POST', path: '/api/v0/sessions/{name}/unpublish', auth: 'bearer', desc: 'Unpublish session.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sessions/<name>/unpublish -H "Authorization: Bearer <token>"`, resp: { schema: 'Session' }, responses: [{ status: 200, body: `{"name":"demo","is_published":false,"published_at":null,...}` }] },
      { method: 'DELETE', path: '/api/v0/sessions/{name}', auth: 'bearer', desc: 'Delete session.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' }
      ], example: `curl -s -X DELETE ${BASE}/api/v0/sessions/<name> -H "Authorization: Bearer <token>"`, resp: { schema: 'Empty' }, responses: [{ status: 200 }] }
    ]
  },
  {
    id: 'responses',
    title: 'Session Responses',
    description: 'Composite input→output exchanges with live items (protected).',
    endpoints: [
      { method: 'GET', path: '/api/v0/sessions/{name}/responses', auth: 'bearer', desc: 'List responses for session.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' },
        { in: 'query', name: 'limit', type: 'int', required: false, desc: 'Max responses (0..1000, default 100)' },
        { in: 'query', name: 'offset', type: 'int', required: false, desc: 'Offset for pagination (default 0)' }
      ], example: `curl -s ${BASE}/api/v0/sessions/<name>/responses?limit=20 -H "Authorization: Bearer <token>"`, resp: { schema: 'ResponseObject', array: true }, responses: [{ status: 200, body: `[{"id":"uuid","session_name":"demo","status":"completed","input_content":[{"type":"text","content":"hi"}],"output_content":[{"type":"text","content":"hello"}],"segments":[{"type":"final","channel":"final","text":"hello"}],"created_at":"2025-01-01T12:00:00Z","updated_at":"2025-01-01T12:00:10Z"}]` }] },
      { method: 'POST', path: '/api/v0/sessions/{name}/responses', auth: 'bearer', desc: 'Create a response (user input). Supports blocking when background=false.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' },
        { in: 'body', name: 'input', type: 'object', required: true, desc: "User input; preferred shape: { content: [{ type: 'text', content: string }] }. Legacy: { text: string } also accepted." },
        { in: 'body', name: 'background', type: 'boolean', required: false, desc: "Default true. If false, request blocks up to 15 minutes until the response reaches a terminal status (completed|failed|cancelled). Returns 504 on timeout. If true or omitted, returns immediately (typically status=pending)." }
      ], example: `curl -s -X POST ${BASE}/api/v0/sessions/<name>/responses -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"input":{"content":[{"type":"text","content":"hello"}]},"background":false}'`, resp: { schema: 'ResponseObject' }, responses: [
        { status: 200, body: `{"id":"uuid","session_name":"demo","status":"completed","input_content":[{"type":"text","content":"hello"}],"output_content":[{"type":"text","content":"..."}],"segments":[{"type":"final","channel":"final","text":"..."}],"created_at":"...","updated_at":"..."}` },
        { status: 504, body: `{"message":"Timed out waiting for response to complete"}` }
      ] },
      { method: 'GET', path: '/api/v0/sessions/{name}/responses/{id}', auth: 'bearer', desc: 'Get a single response by id.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' },
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Response id' }
      ], example: `curl -s ${BASE}/api/v0/sessions/<name>/responses/<id> -H "Authorization: Bearer <token>"`, resp: { schema: 'ResponseObject' }, responses: [
        { status: 200, body: `{"id":"uuid","session_name":"demo","status":"processing","input_content":[{"type":"text","content":"hi"}],"output_content":[],"segments":[{"type":"tool_call","tool":"search","args":{}}],"created_at":"...","updated_at":"..."}` }
      ] },
      { method: 'PUT', path: '/api/v0/sessions/{name}/responses/{id}', auth: 'bearer', desc: 'Update a response (session-only typical). Used to append output.items and mark status.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' },
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Response id' },
        { in: 'body', name: 'status', type: "'pending'|'processing'|'completed'|'failed'", required: false, desc: 'Status update' },
        { in: 'body', name: 'input', type: 'object', required: false, desc: 'Optional input update; replaces existing input JSON' },
        { in: 'body', name: 'output', type: 'object', required: false, desc: 'Output update; shape: { text?: string, items?: [] }' }
      ], example: `curl -s -X PUT ${BASE}/api/v0/sessions/<name>/responses/<id> -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"status":"completed","output":{"text":"done","items":[{"type":"final","channel":"final","text":"done"}]}}'`, resp: { schema: 'ResponseObject' }, responses: [{ status: 200, body: `{"id":"uuid","session_name":"demo","status":"completed","input_content":[],"output_content":[{"type":"text","content":"done"}],"segments":[{"type":"final","channel":"final","text":"done"}],"created_at":"...","updated_at":"..."}` }] },
      { method: 'GET', path: '/api/v0/sessions/{name}/responses/count', auth: 'bearer', desc: 'Get response count for session.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' }
      ], example: `curl -s ${BASE}/api/v0/sessions/<name>/responses/count -H "Authorization: Bearer <token>"`, resp: { schema: 'Count' }, responses: [{ status: 200, body: `{"count":123,"session_name":"demo"}` }] }
    ]
  },
  {
    id: 'files',
    title: 'Session Files',
    description: 'Read-only browsing of an session\'s /session workspace (protected). Paths are relative to /session.',
    endpoints: [
      {
        method: 'GET',
        path: '/api/v0/sessions/{name}/files/list',
        auth: 'bearer',
        desc: 'List immediate children at /session (root). Sorted by name (case-insensitive). Supports pagination with offset+limit and returns total and next_offset.',
        params: [
          { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' },
          { in: 'query', name: 'offset', type: 'int', required: false, desc: 'Offset (default 0)' },
          { in: 'query', name: 'limit', type: 'int', required: false, desc: 'Page size (default 100, max 500)' },
        ],
        example: `curl -s ${BASE}/api/v0/sessions/<name>/files/list -H "Authorization: Bearer <token>"`,
        resp: { schema: 'FileListResult' },
        responses: [
          { status: 200, body: `{"entries":[{"name":"code","kind":"dir","size":0,"mode":"0755","mtime":"2025-01-01T12:00:00Z"}],"offset":0,"limit":100,"next_offset":null,"total":1}` }
        ]
      },
      {
        method: 'GET',
        path: '/api/v0/sessions/{name}/files/list/{path...}',
        auth: 'bearer',
        desc: 'List immediate children under a relative path (e.g., code/src). Sorted by name (case-insensitive). Supports pagination with offset+limit and returns total and next_offset. Path must be safe (no leading \'/\', no ..).',
        params: [
          { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' },
          { in: 'path', name: 'path...', type: 'string', required: true, desc: 'Path relative to /session (no leading slash)' },
          { in: 'query', name: 'offset', type: 'int', required: false, desc: 'Offset (default 0)' },
          { in: 'query', name: 'limit', type: 'int', required: false, desc: 'Page size (default 100, max 500)' },
        ],
        example: `curl -s ${BASE}/api/v0/sessions/<name>/files/list/code -H "Authorization: Bearer <token>"`,
        resp: { schema: 'FileListResult' },
        responses: [
          { status: 200, body: `{"entries":[{"name":"main.rs","kind":"file","size":1024,"mode":"0644","mtime":"2025-01-01T12:00:00Z"}],"offset":0,"limit":100,"next_offset":null,"total":1}` }
        ]
      },
      {
        method: 'GET',
        path: '/api/v0/sessions/{name}/files/metadata/{path...}',
        auth: 'bearer',
        desc: 'Get metadata for a file or directory. For symlinks, includes link_target. Returns 409 if the session is sleeping; 400 for invalid paths; 404 if not found.',
        params: [
          { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' },
          { in: 'path', name: 'path...', type: 'string', required: true, desc: 'Path relative to /session (no leading slash)' }
        ],
        example: `curl -s ${BASE}/api/v0/sessions/<name>/files/metadata/code/src/main.rs -H "Authorization: Bearer <token>"`,
        resp: { schema: 'FileMetadata' },
        responses: [
          { status: 200, body: `{"kind":"file","size":1024,"mode":"0644","mtime":"2025-01-01T12:00:00Z"}` }
        ]
      },
      {
        method: 'GET',
        path: '/api/v0/sessions/{name}/files/read/{path...}',
        auth: 'bearer',
        desc: 'Read a file and return its raw bytes. Sets Content-Type (guessed by filename) and X-TaskSandbox-File-Size headers. Max size 25MB; larger files return 413. Returns 409 if session is sleeping; 404 if not found; 400 for invalid paths.',
        params: [
          { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' },
          { in: 'path', name: 'path...', type: 'string', required: true, desc: 'Path relative to /session (no leading slash)' }
        ],
        example: `curl -s -OJ ${BASE}/api/v0/sessions/<name>/files/read/content/index.html -H "Authorization: Bearer <token>" -D -`,
        resp: { schema: 'Empty' },
        responses: [
          { status: 200 }
        ]
      },
      {
        method: 'DELETE',
        path: '/api/v0/sessions/{name}/files/delete/{path...}',
        auth: 'bearer',
        desc: 'Delete a file or empty directory. Returns { deleted: true } on success. May be disabled in some environments.',
        params: [
          { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' },
          { in: 'path', name: 'path...', type: 'string', required: true, desc: 'Path relative to /session (no leading slash)' }
        ],
        example: `curl -s -X DELETE ${BASE}/api/v0/sessions/<name>/files/delete/code/tmp.txt -H "Authorization: Bearer <token>"`,
        resp: { schema: 'Empty' },
        responses: [
          { status: 200, body: `{"deleted":true}` }
        ]
      }
    ]
  },
  {
    id: 'context',
    title: 'Session Context',
    description: 'Context usage and management (protected).',
    endpoints: [
      { method: 'GET', path: '/api/v0/sessions/{name}/context', auth: 'bearer', desc: 'Get the latest reported context usage from the session.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' }
      ], example: `curl -s ${BASE}/api/v0/sessions/<name>/context -H "Authorization: Bearer <token>"`, resp: { schema: 'SessionContextUsage' }, responses: [{ status: 200, body: `{"session":"demo","soft_limit_tokens":128000,"used_tokens_estimated":12345,"used_percent":9.6,"basis":"ollama_last_context_length","cutoff_at":"2025-01-01T12:34:56Z","measured_at":"2025-01-01T13:00:00Z","total_messages_considered":0}` }] },
      { method: 'POST', path: '/api/v0/sessions/{name}/context/clear', auth: 'bearer', desc: 'Clear context by setting a new cutoff at now. Adds a "Context Cleared" marker response.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sessions/<name>/context/clear -H "Authorization: Bearer <token>"`, resp: { schema: 'SessionContextUsage' }, responses: [{ status: 200, body: `{"session":"demo","soft_limit_tokens":128000,"used_tokens_estimated":0,"used_percent":0.0,"basis":"ollama_last_context_length","cutoff_at":"2025-01-01T13:00:00Z","measured_at":"2025-01-01T13:00:00Z","total_messages_considered":0}` }] },
      { method: 'POST', path: '/api/v0/sessions/{name}/context/compact', auth: 'bearer', desc: 'Compact context by summarizing recent conversation via LLM and setting a new cutoff. Adds a "Context Compacted" marker response with the summary in output.text.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sessions/<name>/context/compact -H "Authorization: Bearer <token>"`, resp: { schema: 'SessionContextUsage' }, responses: [{ status: 200, body: `{"session":"demo","soft_limit_tokens":128000,"used_tokens_estimated":0,"used_percent":0.0,"basis":"ollama_last_context_length","cutoff_at":"2025-01-01T13:05:00Z","measured_at":"2025-01-01T13:05:00Z","total_messages_considered":0}` }] },
      { method: 'POST', path: '/api/v0/sessions/{name}/context/usage', auth: 'bearer', desc: 'Report the latest context length (tokens) after an LLM call.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Session name' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sessions/<name>/context/usage -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"tokens": 4096}'`, resp: { schema: 'Empty' }, responses: [{ status: 200, body: `{"success":true,"last_context_length":4096}` }] }
    ]
  },
  {
    id: 'content',
    title: 'Content (Public)',
    description: 'Static content served by tsbx-content service, mounted under /content.',
    endpoints: [
      { method: 'GET', path: '/content/health', auth: 'public', desc: 'Health endpoint for the content server.', params: [], example: `curl -s ${BASE}/content/health`, resp: { schema: 'Empty' }, responses: [ { status: 200, body: `{"status":"healthy","service":"tsbx-content"}` } ] },
      { method: 'GET', path: '/content/', auth: 'public', desc: 'Root of published content. Returns 200 with no body.', params: [], example: `curl -i ${BASE}/content/`, resp: { schema: 'Empty' }, responses: [ { status: 200 } ] },
      { method: 'GET', path: '/content/{path...}', auth: 'public', desc: 'Serve static files from published content. 404 returns a small HTML page indicating no content.', params: [ { in: 'path', name: 'path...', type: 'string', required: true, desc: 'Path within content volume (e.g., {session}/index.html)' } ], example: `curl -i ${BASE}/content/<session>/index.html`, resp: { schema: 'Empty' }, responses: [ { status: 200 }, { status: 404, body: '<html>...No Content...</html>' } ] }
    ]
  }
  ];
}

export function methodClass(method) {
  switch ((method || '').toUpperCase()) {
    case 'GET': return 'badge bg-success';
    case 'POST': return 'badge bg-theme';
    case 'PUT': return 'badge bg-warning text-dark';
    case 'DELETE': return 'badge bg-danger';
    case 'PATCH': return 'badge bg-info text-dark';
    default: return 'badge bg-secondary';
  }
}
