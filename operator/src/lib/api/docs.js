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
    Sandbox: [
      { name: 'id', type: 'string', desc: 'Sandbox ID (UUID primary key)' },
      { name: 'created_by', type: 'string', desc: 'Owner username' },
      { name: 'state', type: 'string', desc: 'init|idle|busy|deleted' },
      { name: 'description', type: 'string|null', desc: 'Optional description' },
      { name: 'parent_sandbox_id', type: 'string|null', desc: 'Parent sandbox ID if cloned' },
      { name: 'created_at', type: 'string (RFC3339)', desc: 'Creation timestamp' },
      { name: 'last_activity_at', type: 'string|null (RFC3339)', desc: 'Last activity timestamp' },
      { name: 'metadata', type: 'object', desc: 'Arbitrary JSON metadata' },
      { name: 'tags', type: 'string[]', desc: "Array of tags (letters, digits, '/', '-', '_', '.' only; stored lowercase)" },
      { name: 'stop_timeout_seconds', type: 'int', desc: 'Stop timeout' },
      { name: 'archive_timeout_seconds', type: 'int', desc: 'Archive timeout placeholder' },
      { name: 'idle_from', type: 'string|null (RFC3339)', desc: 'When idle started' },
      { name: 'busy_from', type: 'string|null (RFC3339)', desc: 'When busy started' },
      { name: 'context_cutoff_at', type: 'string|null (RFC3339)', desc: 'Current context cutoff timestamp if set' },
    ],
    ListSandboxesResult: [
      { name: 'items', type: 'Sandbox[]', desc: 'Array of sandboxes for current page' },
      { name: 'total', type: 'int', desc: 'Total sandboxes matching filters' },
      { name: 'limit', type: 'int', desc: 'Page size' },
      { name: 'offset', type: 'int', desc: 'Row offset (0-based)' },
      { name: 'page', type: 'int', desc: 'Current page number (1-based)' },
      { name: 'pages', type: 'int', desc: 'Total page count' },
    ],
    TaskObject: [
      { name: 'id', type: 'string', desc: 'Task ID (UUID)' },
      { name: 'sandbox_id', type: 'string', desc: 'Sandbox ID (UUID)' },
      { name: 'status', type: 'string', desc: "'pending'|'processing'|'completed'|'failed'|'cancelled'" },
      { name: 'input_content', type: 'array', desc: "User input content items (e.g., [{ type: 'text', content: 'hello' }]). Preferred input shape uses 'content' array; legacy { text: string } is accepted but not echoed in input_content." },
      { name: 'output_content', type: 'array', desc: "Final content items extracted from segments (typically the 'output' tool_result payload)" },
      { name: 'segments', type: 'array', desc: 'All step-by-step segments/items: commentary, tool calls/results, system markers, final' },
      { name: 'timeout_seconds', type: 'int|null', desc: 'Per-task timeout in seconds (defaults to 3600/1 hour)' },
      { name: 'timeout_at', type: 'string|null (RFC3339)', desc: 'When the task will time out automatically if still pending/processing' },
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
      { name: 'sandbox_id', type: 'string', desc: 'Sandbox ID (UUID)' },
    ],
    RuntimeTotal: [
      { name: 'sandbox_id', type: 'string', desc: 'Sandbox ID (UUID)' },
      { name: 'total_runtime_seconds', type: 'int', desc: 'Total runtime across sandboxes (seconds)' },
      { name: 'current_sandbox_seconds', type: 'int', desc: 'Current sandbox runtime (seconds), 0 if deleted' },
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
    SandboxContextUsage: [
      { name: 'sandbox_id', type: 'string', desc: 'Sandbox ID (UUID)' },
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
      { name: 'sandbox_id', type: 'string', desc: 'Sandbox ID (UUID)' },
      { name: 'cancelled', type: 'boolean', desc: 'true if a pending/processing task or queued update was cancelled' },
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
    id: 'sandboxes',
    title: 'Sandboxes',
    description: 'Sandbox lifecycle and management endpoints (protected).',
    endpoints: [
      { method: 'GET', path: '/api/v0/sandboxes', auth: 'bearer', desc: 'List/search sandboxes with pagination.', params: [
        { in: 'query', name: 'q', type: 'string', required: false, desc: 'Search substring over name and description (case-insensitive)' },
        { in: 'query', name: 'tags', type: 'string (comma-separated)', required: false, desc: 'Filter by tags (INTERSECTION/AND). Provide multiple tags as a comma-separated list (e.g., tags=prod,team). Tags are matched case-insensitively and stored lowercase.' },
        { in: 'query', name: 'state', type: 'string', required: false, desc: 'Filter by state: init|idle|busy|deleted' },
        { in: 'query', name: 'limit', type: 'int', required: false, desc: 'Page size (default 30, max 100)' },
        { in: 'query', name: 'page', type: 'int', required: false, desc: 'Page number (1-based). Ignored when offset is set.' },
        { in: 'query', name: 'offset', type: 'int', required: false, desc: 'Row offset (0-based). Takes precedence over page.' }
      ], example: `curl -s ${BASE}/api/v0/sandboxes?q=demo&tags=prod,team/core&state=idle&limit=30&page=1 -H "Authorization: Bearer <token>"`, resp: { schema: 'ListSandboxesResult' }, responses: [{ status: 200, body: `{"items":[{"id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","created_by":"admin","state":"idle","description":"Demo sandbox","parent_sandbox_id":null,"created_at":"2025-01-01T12:00:00Z","last_activity_at":"2025-01-01T12:10:00Z","metadata":{},"tags":["prod","team/core"],"is_published":false,"published_at":null,"published_by":null,"publish_permissions":{"code":true,"env":true,"content":true},"stop_timeout_seconds":300,"archive_timeout_seconds":86400,"idle_from":"2025-01-01T12:10:00Z","busy_from":null}],"total":1,"limit":30,"offset":0,"page":1,"pages":1}` }] },
      { method: 'POST', path: '/api/v0/sandboxes', auth: 'bearer', desc: 'Start a new sandbox. A UUID will be automatically generated as the sandbox ID.', params: [
        { in: 'body', name: 'description', type: 'string|null', required: false, desc: 'Optional human-readable description' },
        { in: 'body', name: 'metadata', type: 'object', required: false, desc: 'Arbitrary JSON metadata (default: {})' },
        { in: 'body', name: 'tags', type: 'string[]', required: false, desc: "Array of tags; allowed characters are letters, digits, '/', '-', '_', '.'; no spaces (default: [])" },
        { in: 'body', name: 'env', type: 'object<string,string>', required: false, desc: 'Key/value env map (default: empty)' },
        { in: 'body', name: 'instructions', type: 'string|null', required: false, desc: 'Optional instructions' },
        { in: 'body', name: 'setup', type: 'string|null', required: false, desc: 'Optional setup script or commands' },
        { in: 'body', name: 'prompt', type: 'string|null', required: false, desc: 'Optional initial prompt' },
        { in: 'body', name: 'stop_timeout_seconds', type: 'int|null', required: false, desc: 'Stop timeout seconds (default 300)' },
        { in: 'body', name: 'archive_timeout_seconds', type: 'int|null', required: false, desc: 'Archive timeout seconds (default 86400)' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sandboxes -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"description":"Demo sandbox"}'`, resp: { schema: 'Sandbox' }, responses: [{ status: 200, body: `{"id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","created_by":"admin","state":"init","description":"Demo sandbox","parent_sandbox_id":null,"created_at":"2025-01-01T12:00:00Z","last_activity_at":null,"metadata":{},"tags":[],"is_published":false,"published_at":null,"published_by":null,"publish_permissions":{"code":true,"env":true,"content":true},"stop_timeout_seconds":300,"archive_timeout_seconds":86400,"idle_from":null,"busy_from":null}` }] },
      { method: 'GET', path: '/api/v0/sandboxes/{id}', auth: 'bearer', desc: 'Get sandbox by ID.', params: [
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
      ], example: `curl -s ${BASE}/api/v0/sandboxes/<id> -H "Authorization: Bearer <token>"`, resp: { schema: 'Sandbox' }, responses: [{ status: 200, body: `{"id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","created_by":"admin","state":"idle","description":"Demo sandbox","parent_sandbox_id":null,"created_at":"2025-01-01T12:00:00Z","last_activity_at":"2025-01-01T12:10:00Z","metadata":{},"tags":[],"is_published":false,"published_at":null,"published_by":null,"publish_permissions":{"code":true,"env":true,"content":true},"stop_timeout_seconds":300,"archive_timeout_seconds":86400,"idle_from":"2025-01-01T12:10:00Z","busy_from":null}` }] },
      { method: 'PUT', path: '/api/v0/sandboxes/{id}', auth: 'bearer', desc: 'Update sandbox by ID.', params: [
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
        { in: 'body', name: 'metadata', type: 'object|null', required: false, desc: 'Replace metadata (omit to keep)' },
        { in: 'body', name: 'description', type: 'string|null', required: false, desc: 'Update description' },
        { in: 'body', name: 'tags', type: 'string[]|null', required: false, desc: "Replace tags array; allowed characters are letters, digits, '/', '-', '_', '.'; no spaces" },
        { in: 'body', name: 'stop_timeout_seconds', type: 'int|null', required: false, desc: 'Update stop timeout seconds' },
        { in: 'body', name: 'archive_timeout_seconds', type: 'int|null', required: false, desc: 'Update archive timeout seconds' }
      ], example: `curl -s -X PUT ${BASE}/api/v0/sandboxes/<id> -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"description":"Updated"}'`, resp: { schema: 'Sandbox' }, responses: [{ status: 200, body: `{"id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","created_by":"admin","state":"idle","description":"Updated","parent_sandbox_id":null,"created_at":"2025-01-01T12:00:00Z","last_activity_at":"2025-01-01T12:20:00Z","metadata":{},"tags":[],"is_published":false,"published_at":null,"published_by":null,"publish_permissions":{"code":true,"env":true,"content":true},"stop_timeout_seconds":300,"archive_timeout_seconds":86400,"idle_from":"2025-01-01T12:20:00Z","busy_from":null}` }] },
      { method: 'PUT', path: '/api/v0/sandboxes/{id}/state', auth: 'bearer', desc: 'Update sandbox state (generic).', params: [
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
        { in: 'body', name: 'state', type: 'string', required: true, desc: 'New state (e.g., init|idle|busy|deleted)' }
      ], example: `curl -s -X PUT ${BASE}/api/v0/sandboxes/<id>/state -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"state":"idle"}'`, resp: { schema: 'StateAck' }, responses: [{ status: 200, body: `{"success":true,"state":"idle"}` }] },
      { method: 'POST', path: '/api/v0/sandboxes/{id}/busy', auth: 'bearer', desc: 'Set sandbox busy.', params: [
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/busy -H "Authorization: Bearer <token>"`, resp: { schema: 'BusyIdleAck' }, responses: [{ status: 200, body: `{"success":true,"state":"busy","timeout_status":"paused"}` }] },
      { method: 'POST', path: '/api/v0/sandboxes/{id}/idle', auth: 'bearer', desc: 'Set sandbox idle.', params: [
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/idle -H "Authorization: Bearer <token>"`, resp: { schema: 'BusyIdleAck' }, responses: [{ status: 200, body: `{"success":true,"state":"idle","timeout_status":"active"}` }] },
      { method: 'POST', path: '/api/v0/sandboxes/{id}/stop', auth: 'bearer', desc: 'Schedule sandbox to stop after an optional delay (min/default 5s).', params: [
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
        { in: 'body', name: 'delay_seconds', type: 'int|null', required: false, desc: 'Delay before stopping (min/default 5 seconds)' },
        { in: 'body', name: 'note', type: 'string|null', required: false, desc: 'Optional note to display in chat when stop occurs' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/stop -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"delay_seconds":10,"note":"User requested stop"}'\n\n# The sandbox will stop after the delay. State may not change immediately in the response.`, resp: { schema: 'Sandbox' }, responses: [{ status: 200, body: `{"id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","created_by":"admin","state":"idle",...}` }] },
      { method: 'POST', path: '/api/v0/sandboxes/{id}/cancel', auth: 'bearer', desc: 'Cancel the most recent pending/processing task (or queued update) and set sandbox to idle.', params: [
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/cancel -H "Authorization: Bearer <token>"`, resp: { schema: 'CancelAck' }, responses: [{ status: 200, body: `{"status":"ok","sandbox_id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","cancelled":true}` }] },
      { method: 'POST', path: '/api/v0/sandboxes/{id}/restart', auth: 'bearer', desc: 'Restart sandbox (optionally send a prompt).', params: [
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
        { in: 'body', name: 'prompt', type: 'string|null', required: false, desc: 'Optional prompt to send on restart' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/restart -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"prompt":"get ready"}'`, resp: { schema: 'Sandbox' }, responses: [{ status: 200, body: `{"id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","created_by":"admin","state":"init",...}` }] },
      { method: 'GET', path: '/api/v0/sandboxes/{id}/runtime', auth: 'bearer', desc: 'Get total runtime across sandboxes (seconds). Includes current sandbox (since last restart or creation).', params: [
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
      ], example: `curl -s ${BASE}/api/v0/sandboxes/<id>/runtime -H "Authorization: Bearer <token>"`, resp: { schema: 'RuntimeTotal' }, responses: [{ status: 200, body: `{"sandbox_id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","total_runtime_seconds":1234,"current_sandbox_seconds":321}` }] },
      { method: 'POST', path: '/api/v0/sandboxes/{id}/clone', auth: 'bearer', desc: 'Clone sandbox (start a new sandbox from parent).', params: [
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Parent sandbox ID (UUID)' },
        { in: 'body', name: 'metadata', type: 'object|null', required: false, desc: 'Optional metadata override' },
        { in: 'body', name: 'code', type: 'boolean', required: false, desc: 'Copy code (default true)' },
        { in: 'body', name: 'env', type: 'boolean', required: false, desc: 'Copy env (default true)' },
        { in: 'body', name: 'content', type: 'boolean', required: false, desc: 'Copy content (always true in v0.4.0+)' },
        { in: 'body', name: 'prompt', type: 'string|null', required: false, desc: 'Optional initial prompt' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/clone -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"code":true,"env":false,"prompt":"clone and adjust"}'`, resp: { schema: 'Sandbox' }, responses: [{ status: 200, body: `{"id":"b7d4f3e8-c1a2-11f0-aadd-064ac08387fc","created_by":"admin","state":"init",...}` }] },
      { method: 'DELETE', path: '/api/v0/sandboxes/{id}', auth: 'bearer', desc: 'Delete sandbox.', params: [
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
      ], example: `curl -s -X DELETE ${BASE}/api/v0/sandboxes/<id> -H "Authorization: Bearer <token>"`, resp: { schema: 'Empty' }, responses: [{ status: 200 }] }
    ]
  },
  {
    id: 'responses',
    title: 'Sandbox Tasks',
    description: 'Composite input→output exchanges with live items (protected).',
    endpoints: [
      { method: 'GET', path: '/api/v0/sandboxes/{id}/tasks', auth: 'bearer', desc: 'List tasks for sandbox.', params: [
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
        { in: 'query', name: 'limit', type: 'int', required: false, desc: 'Max responses (0..1000, default 100)' },
        { in: 'query', name: 'offset', type: 'int', required: false, desc: 'Offset for pagination (default 0)' }
      ], example: `curl -s ${BASE}/api/v0/sandboxes/<id>/tasks?limit=20 -H "Authorization: Bearer <token>"`, resp: { schema: 'TaskObject', array: true }, responses: [{ status: 200, body: `[{"id":"d8e9f0a1-c2b3-11f0-aadd-064ac08387fc","sandbox_id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","status":"completed","input_content":[{"type":"text","content":"hi"}],"output_content":[{"type":"text","content":"hello"}],"segments":[{"type":"final","channel":"final","text":"hello"}],"timeout_seconds":600,"timeout_at":"2025-01-01T12:10:00Z","created_at":"2025-01-01T12:00:00Z","updated_at":"2025-01-01T12:00:10Z"}]` }] },
      { method: 'POST', path: '/api/v0/sandboxes/{id}/tasks', auth: 'bearer', desc: 'Create a task (user input). Supports blocking when background=false.', params: [
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
        { in: 'body', name: 'input', type: 'object', required: true, desc: "User input; preferred shape: { content: [{ type: 'text', content: string }] }. Legacy: { text: string } also accepted." },
        { in: 'body', name: 'background', type: 'boolean', required: false, desc: "Default true. If false, request blocks up to 15 minutes until the response reaches a terminal status (completed|failed|cancelled). Returns 504 on timeout. If true or omitted, returns immediately (typically status=pending)." },
        { in: 'body', name: 'timeout_seconds', type: 'int|null', required: false, desc: 'Per-task timeout in seconds (defaults to 3600/1 hour; set to 0 to disable auto-cancel).' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/tasks -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"input":{"content":[{"type":"text","content":"hello"}]},"background":false,"timeout_seconds":600}'`, resp: { schema: 'TaskObject' }, responses: [
        { status: 200, body: `{"id":"e9f0a1b2-c3d4-11f0-aadd-064ac08387fc","sandbox_id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","status":"completed","input_content":[{"type":"text","content":"hello"}],"output_content":[{"type":"text","content":"..."}],"segments":[{"type":"final","channel":"final","text":"..."}],"timeout_seconds":600,"timeout_at":"2025-01-01T12:10:00Z","created_at":"...","updated_at":"..."}` },
        { status: 504, body: `{"message":"Timed out waiting for response to complete"}` }
      ] },
      { method: 'GET', path: '/api/v0/sandboxes/{id}/tasks/{task_id}', auth: 'bearer', desc: 'Get a single task by id.', params: [
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
        { in: 'path', name: 'task_id', type: 'string', required: true, desc: 'Task ID (UUID)' }
      ], example: `curl -s ${BASE}/api/v0/sandboxes/<id>/tasks/<task_id> -H "Authorization: Bearer <token>"`, resp: { schema: 'TaskObject' }, responses: [
        { status: 200, body: `{"id":"f0a1b2c3-d4e5-11f0-aadd-064ac08387fc","sandbox_id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","status":"processing","input_content":[{"type":"text","content":"hi"}],"output_content":[],"segments":[{"type":"tool_call","tool":"search","args":{},"arguments":{}}],"timeout_seconds":600,"timeout_at":"2025-01-01T12:10:00Z","created_at":"...","updated_at":"..."}` }
      ] },
      { method: 'PUT', path: '/api/v0/sandboxes/{id}/tasks/{task_id}', auth: 'bearer', desc: 'Update a task record. Used to append output.items and mark status.', params: [
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
        { in: 'path', name: 'task_id', type: 'string', required: true, desc: 'Task ID (UUID)' },
        { in: 'body', name: 'status', type: "'pending'|'processing'|'completed'|'failed'", required: false, desc: 'Status update' },
        { in: 'body', name: 'input', type: 'object', required: false, desc: 'Optional input update; replaces existing input JSON' },
        { in: 'body', name: 'output', type: 'object', required: false, desc: 'Output update; shape: { text?: string, items?: [] }' },
        { in: 'body', name: 'timeout_seconds', type: 'int|null', required: false, desc: 'Reset per-task timeout (<=0 clears)' }
      ], example: `curl -s -X PUT ${BASE}/api/v0/sandboxes/<id>/tasks/<task_id> -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"status":"completed","output":{"text":"done","items":[{"type":"final","channel":"final","text":"done"}]}}'`, resp: { schema: 'TaskObject' }, responses: [{ status: 200, body: `{"id":"a1b2c3d4-e5f6-11f0-aadd-064ac08387fc","sandbox_id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","status":"completed","input_content":[],"output_content":[{"type":"text","content":"done"}],"segments":[{"type":"final","channel":"final","text":"done"}],"timeout_seconds":null,"timeout_at":null,"created_at":"...","updated_at":"..."}` }] },
      { method: 'GET', path: '/api/v0/sandboxes/{id}/tasks/count', auth: 'bearer', desc: 'Get task count for sandbox.', params: [
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
      ], example: `curl -s ${BASE}/api/v0/sandboxes/<id>/tasks/count -H "Authorization: Bearer <token>"`, resp: { schema: 'Count' }, responses: [{ status: 200, body: `{"count":123,"sandbox_id":"fa36e542-b9b8-11f0-aadd-064ac08387fc"}` }] }
    ]
  },
  {
    id: 'files',
    title: 'Sandbox Files',
    description: 'Read-only browsing of a sandbox\'s /sandbox workspace (protected). Paths are relative to /sandbox.',
    endpoints: [
      {
        method: 'GET',
        path: '/api/v0/sandboxes/{id}/files/list',
        auth: 'bearer',
        desc: 'List immediate children at /sandbox (root). Sorted by name (case-insensitive). Supports pagination with offset+limit and returns total and next_offset.',
        params: [
          { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
          { in: 'query', name: 'offset', type: 'int', required: false, desc: 'Offset (default 0)' },
          { in: 'query', name: 'limit', type: 'int', required: false, desc: 'Page size (default 100, max 500)' },
        ],
        example: `curl -s ${BASE}/api/v0/sandboxes/<id>/files/list -H "Authorization: Bearer <token>"`,
        resp: { schema: 'FileListResult' },
        responses: [
          { status: 200, body: `{"entries":[{"name":"code","kind":"dir","size":0,"mode":"0755","mtime":"2025-01-01T12:00:00Z"}],"offset":0,"limit":100,"next_offset":null,"total":1}` }
        ]
      },
      {
        method: 'GET',
        path: '/api/v0/sandboxes/{id}/files/list/{path...}',
        auth: 'bearer',
        desc: 'List immediate children under a relative path (e.g., code/src). Sorted by name (case-insensitive). Supports pagination with offset+limit and returns total and next_offset. Path must be safe (no leading \'/\', no ..).',
        params: [
          { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
          { in: 'path', name: 'path...', type: 'string', required: true, desc: 'Path relative to /sandbox (no leading slash)' },
          { in: 'query', name: 'offset', type: 'int', required: false, desc: 'Offset (default 0)' },
          { in: 'query', name: 'limit', type: 'int', required: false, desc: 'Page size (default 100, max 500)' },
        ],
        example: `curl -s ${BASE}/api/v0/sandboxes/<id>/files/list/code -H "Authorization: Bearer <token>"`,
        resp: { schema: 'FileListResult' },
        responses: [
          { status: 200, body: `{"entries":[{"name":"main.rs","kind":"file","size":1024,"mode":"0644","mtime":"2025-01-01T12:00:00Z"}],"offset":0,"limit":100,"next_offset":null,"total":1}` }
        ]
      },
      {
        method: 'GET',
        path: '/api/v0/sandboxes/{id}/files/metadata/{path...}',
        auth: 'bearer',
        desc: 'Get metadata for a file or directory. For symlinks, includes link_target. Returns 409 if the sandbox is deleted; 400 for invalid paths; 404 if not found.',
        params: [
          { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
          { in: 'path', name: 'path...', type: 'string', required: true, desc: 'Path relative to /sandbox (no leading slash)' }
        ],
        example: `curl -s ${BASE}/api/v0/sandboxes/<id>/files/metadata/code/src/main.rs -H "Authorization: Bearer <token>"`,
        resp: { schema: 'FileMetadata' },
        responses: [
          { status: 200, body: `{"kind":"file","size":1024,"mode":"0644","mtime":"2025-01-01T12:00:00Z"}` }
        ]
      },
      {
        method: 'GET',
        path: '/api/v0/sandboxes/{id}/files/read/{path...}',
        auth: 'bearer',
        desc: 'Read a file and return its raw bytes. Sets Content-Type (guessed by filename) and X-TaskSandbox-File-Size headers. Max size 25MB; larger files return 413. Returns 409 if sandbox is deleted; 404 if not found; 400 for invalid paths.',
        params: [
          { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
          { in: 'path', name: 'path...', type: 'string', required: true, desc: 'Path relative to /sandbox (no leading slash)' }
        ],
        example: `curl -s -OJ ${BASE}/api/v0/sandboxes/<id>/files/read/code/report.html -H "Authorization: Bearer <token>" -D -`,
        resp: { schema: 'Empty' },
        responses: [
          { status: 200 }
        ]
      },
      {
        method: 'DELETE',
        path: '/api/v0/sandboxes/{id}/files/delete/{path...}',
        auth: 'bearer',
        desc: 'Delete a file or empty directory. Returns { deleted: true } on success. May be disabled in some environments.',
        params: [
          { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
          { in: 'path', name: 'path...', type: 'string', required: true, desc: 'Path relative to /sandbox (no leading slash)' }
        ],
        example: `curl -s -X DELETE ${BASE}/api/v0/sandboxes/<id>/files/delete/code/tmp.txt -H "Authorization: Bearer <token>"`,
        resp: { schema: 'Empty' },
        responses: [
          { status: 200, body: `{"deleted":true}` }
        ]
      }
    ]
  },
  {
    id: 'context',
    title: 'Sandbox Context',
    description: 'Context usage and management (protected).',
    endpoints: [
      { method: 'GET', path: '/api/v0/sandboxes/{id}/context', auth: 'bearer', desc: 'Get the latest reported context usage from the sandbox.', params: [
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
      ], example: `curl -s ${BASE}/api/v0/sandboxes/<id>/context -H "Authorization: Bearer <token>"`, resp: { schema: 'SandboxContextUsage' }, responses: [{ status: 200, body: `{"sandbox_id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","soft_limit_tokens":128000,"used_tokens_estimated":12345,"used_percent":9.6,"basis":"inference_last_context_length","cutoff_at":"2025-01-01T12:34:56Z","measured_at":"2025-01-01T13:00:00Z","total_messages_considered":0}` }] },
      { method: 'POST', path: '/api/v0/sandboxes/{id}/context/clear', auth: 'bearer', desc: 'Clear context by setting a new cutoff at now. Adds a "Context Cleared" marker response.', params: [
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/context/clear -H "Authorization: Bearer <token>"`, resp: { schema: 'SandboxContextUsage' }, responses: [{ status: 200, body: `{"sandbox_id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","soft_limit_tokens":128000,"used_tokens_estimated":0,"used_percent":0.0,"basis":"inference_last_context_length","cutoff_at":"2025-01-01T13:00:00Z","measured_at":"2025-01-01T13:00:00Z","total_messages_considered":0}` }] },
      { method: 'POST', path: '/api/v0/sandboxes/{id}/context/compact', auth: 'bearer', desc: 'Compact context by summarizing recent conversation via LLM and setting a new cutoff. Adds a "Context Compacted" marker response with the summary in output.text.', params: [
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/context/compact -H "Authorization: Bearer <token>"`, resp: { schema: 'SandboxContextUsage' }, responses: [{ status: 200, body: `{"sandbox_id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","soft_limit_tokens":128000,"used_tokens_estimated":0,"used_percent":0.0,"basis":"inference_last_context_length","cutoff_at":"2025-01-01T13:05:00Z","measured_at":"2025-01-01T13:05:00Z","total_messages_considered":0}` }] },
      { method: 'POST', path: '/api/v0/sandboxes/{id}/context/usage', auth: 'bearer', desc: 'Report the latest context length (tokens) after an LLM call.', params: [
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
      ], example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/context/usage -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"tokens": 4096}'`, resp: { schema: 'Empty' }, responses: [{ status: 200, body: `{"success":true,"last_context_length":4096}` }] }
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
