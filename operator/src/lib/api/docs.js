// API documentation data source for Raworc UI
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
    Agent: [
      { name: 'name', type: 'string', desc: 'Agent name (primary key)' },
      { name: 'created_by', type: 'string', desc: 'Owner username' },
      { name: 'state', type: 'string', desc: 'init|idle|busy|slept' },
      { name: 'description', type: 'string|null', desc: 'Optional description' },
      { name: 'parent_agent_name', type: 'string|null', desc: 'Parent agent name if remixed' },
      { name: 'created_at', type: 'string (RFC3339)', desc: 'Creation timestamp' },
      { name: 'last_activity_at', type: 'string|null (RFC3339)', desc: 'Last activity timestamp' },
      { name: 'metadata', type: 'object', desc: 'Arbitrary JSON metadata' },
      { name: 'tags', type: 'string[]', desc: 'Array of alphanumeric tags' },
      { name: 'is_published', type: 'boolean', desc: 'Published state' },
      { name: 'published_at', type: 'string|null (RFC3339)', desc: 'When published' },
      { name: 'published_by', type: 'string|null', desc: 'Who published' },
      { name: 'publish_permissions', type: 'object', desc: '{ code: boolean, secrets: boolean, content: boolean }' },
      { name: 'idle_timeout_seconds', type: 'int', desc: 'Idle timeout' },
      { name: 'busy_timeout_seconds', type: 'int', desc: 'Busy timeout' },
      { name: 'idle_from', type: 'string|null (RFC3339)', desc: 'When idle started' },
      { name: 'busy_from', type: 'string|null (RFC3339)', desc: 'When busy started' },
    ],
    ResponseObject: [
      { name: 'id', type: 'string', desc: 'Response ID (UUID)' },
      { name: 'agent_name', type: 'string', desc: 'Agent name' },
      { name: 'status', type: 'string', desc: "'pending'|'processing'|'completed'|'failed'" },
      { name: 'input', type: 'object', desc: 'User input JSON (e.g., { text: string })' },
      { name: 'output', type: 'object', desc: 'Agent output JSON (see items structure)' },
      { name: 'created_at', type: 'string (RFC3339)', desc: 'Creation timestamp' },
      { name: 'updated_at', type: 'string (RFC3339)', desc: 'Last update timestamp' },
    ],
    Count: [
      { name: 'count', type: 'int', desc: 'Count value' },
      { name: 'agent_name', type: 'string', desc: 'Agent identifier' },
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
    title: 'Published Agents (Public)',
    description: 'Publicly visible agents and details.',
    endpoints: [
      {
        method: 'GET',
        path: '/api/v0/published/agents',
        auth: 'public',
        desc: 'List all published agents.',
        params: [],
        example: `curl -s ${BASE}/api/v0/published/agents`,
        resp: { schema: 'Agent', array: true },
        responses: [
          {
            status: 200,
            body: `[
  {
    "name": "demo",
    "created_by": "admin",
    "state": "idle",
    "description": "Demo agent",
    "parent_agent_name": null,
    "created_at": "2025-01-01T12:00:00Z",
    "last_activity_at": "2025-01-01T12:00:00Z",
    "metadata": {},
    "tags": ["example"],
    "is_published": true,
    "published_at": "2025-01-01T12:30:00Z",
    "published_by": "admin",
    "publish_permissions": {"code": true, "secrets": false, "content": true},
    "idle_timeout_seconds": 300,
    "busy_timeout_seconds": 900,
    "idle_from": "2025-01-01T12:10:00Z",
    "busy_from": null
  }
]`
          }
        ]
      },
      {
        method: 'GET',
        path: '/api/v0/published/agents/{name}',
        auth: 'public',
        desc: 'Get details of a published agent by name.',
        params: [
          { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' }
        ],
        example: `curl -s ${BASE}/api/v0/published/agents/<name>`,
        resp: { schema: 'Agent' },
        responses: [
          {
            status: 200,
            body: `{
  "name": "demo",
  "created_by": "admin",
  "state": "idle",
  "description": "Demo agent",
  "parent_agent_name": null,
  "created_at": "2025-01-01T12:00:00Z",
  "last_activity_at": "2025-01-01T12:00:00Z",
  "metadata": {},
  "tags": ["example"],
  "is_published": true,
  "published_at": "2025-01-01T12:30:00Z",
  "published_by": "admin",
  "publish_permissions": {"code": true, "secrets": false, "content": true},
  "idle_timeout_seconds": 300,
  "busy_timeout_seconds": 900,
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
      { method: 'GET', path: '/api/v0/operators', auth: 'bearer', desc: 'List operators.', params: [], example: `curl -s ${BASE}/api/v0/operators -H "Authorization: Bearer <token>"`, resp: { schema: 'Operator', array: true }, responses: [{ status: 200, body: `[{"user":"admin","description":null,"active":true,"created_at":"2025-01-01T00:00:00Z","updated_at":"2025-01-01T00:00:00Z","last_login_at":"2025-01-01T12:00:00Z"}]` }] },
      { method: 'POST', path: '/api/v0/operators', auth: 'bearer', desc: 'Create operator.', params: [
        { in: 'body', name: 'user', type: 'string', required: true, desc: 'Operator username' },
        { in: 'body', name: 'pass', type: 'string', required: true, desc: 'Password' },
        { in: 'body', name: 'description', type: 'string', required: false, desc: 'Optional description' }
      ], example: `curl -s -X POST ${BASE}/api/v0/operators -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"user":"alice","pass":"<password>","description":"Team operator"}'`, resp: { schema: 'Operator' }, responses: [{ status: 200, body: `{"user":"alice","description":"Team operator","active":true,"created_at":"2025-01-01T00:00:00Z","updated_at":"2025-01-01T00:00:00Z","last_login_at":null}` }] },
      { method: 'GET', path: '/api/v0/operators/{name}', auth: 'bearer', desc: 'Get operator.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Operator username' }
      ], example: `curl -s ${BASE}/api/v0/operators/<name> -H "Authorization: Bearer <token>"`, resp: { schema: 'Operator' }, responses: [{ status: 200, body: `{"user":"alice","description":"Team operator","active":true,"created_at":"2025-01-01T00:00:00Z","updated_at":"2025-01-02T10:00:00Z","last_login_at":null}` }] },
      { method: 'PUT', path: '/api/v0/operators/{name}', auth: 'bearer', desc: 'Update operator.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Operator username' },
        { in: 'body', name: 'description', type: 'string', required: false, desc: 'Optional description' },
        { in: 'body', name: 'active', type: 'boolean|null', required: false, desc: 'Set active status; must be boolean or null' }
      ], example: `curl -s -X PUT ${BASE}/api/v0/operators/<name> -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"description":"Updated desc","active":true}'`, resp: { schema: 'Operator' }, responses: [{ status: 200, body: `{"user":"alice","description":"Updated desc","active":true,"created_at":"2025-01-01T00:00:00Z","updated_at":"2025-01-02T12:00:00Z","last_login_at":null}` }] },
      { method: 'DELETE', path: '/api/v0/operators/{name}', auth: 'bearer', desc: 'Delete operator.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Operator username' }
      ], example: `curl -s -X DELETE ${BASE}/api/v0/operators/<name> -H "Authorization: Bearer <token>"`, resp: { schema: 'Empty' }, responses: [{ status: 200 }] },
      { method: 'PUT', path: '/api/v0/operators/{name}/password', auth: 'bearer', desc: 'Update operator password.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Operator username' },
        { in: 'body', name: 'current_password', type: 'string', required: true, desc: 'Current password' },
        { in: 'body', name: 'new_password', type: 'string', required: true, desc: 'New password' }
      ], example: `curl -s -X PUT ${BASE}/api/v0/operators/<name>/password -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"current_password":"<old>","new_password":"<new>"}'`, resp: { schema: 'Empty' }, responses: [{ status: 200 }] }
    ]
  },
  {
    id: 'agents',
    title: 'Agents',
    description: 'Agent lifecycle and management endpoints (protected).',
    endpoints: [
      { method: 'GET', path: '/api/v0/agents', auth: 'bearer', desc: 'List agents.', params: [
        { in: 'query', name: 'state', type: 'string', required: false, desc: 'Filter by state (e.g., init|idle|busy|slept)' }
      ], example: `curl -s ${BASE}/api/v0/agents -H "Authorization: Bearer <token>"`, resp: { schema: 'Agent', array: true }, responses: [{ status: 200, body: `[{"name":"demo","created_by":"admin","state":"idle","description":null,"parent_agent_name":null,"created_at":"2025-01-01T12:00:00Z","last_activity_at":"2025-01-01T12:10:00Z","metadata":{},"tags":[],"is_published":false,"published_at":null,"published_by":null,"publish_permissions":{"code":true,"secrets":true,"content":true},"idle_timeout_seconds":300,"busy_timeout_seconds":900,"idle_from":"2025-01-01T12:10:00Z","busy_from":null}]` }] },
      { method: 'POST', path: '/api/v0/agents', auth: 'bearer', desc: 'Create agent.', params: [
        { in: 'body', name: 'name', type: 'string', required: true, desc: 'Agent name; must match ^[A-Za-z][A-Za-z0-9-]{0,61}[A-Za-z0-9]$' },
        { in: 'body', name: 'description', type: 'string|null', required: false, desc: 'Optional human-readable description' },
        { in: 'body', name: 'metadata', type: 'object', required: false, desc: 'Arbitrary JSON metadata (default: {})' },
        { in: 'body', name: 'tags', type: 'string[]', required: false, desc: 'Array of tags; each tag must be alphanumeric (A-Za-z0-9), no spaces/symbols (default: [])' },
        { in: 'body', name: 'secrets', type: 'object<string,string>', required: false, desc: 'Key/value secrets map (default: empty)' },
        { in: 'body', name: 'instructions', type: 'string|null', required: false, desc: 'Optional instructions' },
        { in: 'body', name: 'setup', type: 'string|null', required: false, desc: 'Optional setup script or commands' },
        { in: 'body', name: 'prompt', type: 'string|null', required: false, desc: 'Optional initial prompt' },
        { in: 'body', name: 'idle_timeout_seconds', type: 'int|null', required: false, desc: 'Idle timeout seconds (default 300)' },
        { in: 'body', name: 'busy_timeout_seconds', type: 'int|null', required: false, desc: 'Busy timeout seconds (default 900)' }
      ], example: `curl -s -X POST ${BASE}/api/v0/agents -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"name":"demo","description":"Demo agent"}'`, resp: { schema: 'Agent' }, responses: [{ status: 200, body: `{"name":"demo","created_by":"admin","state":"init","description":"Demo agent","parent_agent_name":null,"created_at":"2025-01-01T12:00:00Z","last_activity_at":null,"metadata":{},"tags":[],"is_published":false,"published_at":null,"published_by":null,"publish_permissions":{"code":true,"secrets":true,"content":true},"idle_timeout_seconds":300,"busy_timeout_seconds":900,"idle_from":null,"busy_from":null}` }] },
      { method: 'GET', path: '/api/v0/agents/{name}', auth: 'bearer', desc: 'Get agent by name.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' }
      ], example: `curl -s ${BASE}/api/v0/agents/<name> -H "Authorization: Bearer <token>"`, resp: { schema: 'Agent' }, responses: [{ status: 200, body: `{"name":"demo","created_by":"admin","state":"idle","description":"Demo agent","parent_agent_name":null,"created_at":"2025-01-01T12:00:00Z","last_activity_at":"2025-01-01T12:10:00Z","metadata":{},"tags":[],"is_published":false,"published_at":null,"published_by":null,"publish_permissions":{"code":true,"secrets":true,"content":true},"idle_timeout_seconds":300,"busy_timeout_seconds":900,"idle_from":"2025-01-01T12:10:00Z","busy_from":null}` }] },
      { method: 'PUT', path: '/api/v0/agents/{name}', auth: 'bearer', desc: 'Update agent by name.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' },
        { in: 'body', name: 'metadata', type: 'object|null', required: false, desc: 'Replace metadata (omit to keep)' },
        { in: 'body', name: 'description', type: 'string|null', required: false, desc: 'Update description' },
        { in: 'body', name: 'tags', type: 'string[]|null', required: false, desc: 'Replace tags array; each tag must be alphanumeric (A-Za-z0-9), no spaces/symbols' },
        { in: 'body', name: 'idle_timeout_seconds', type: 'int|null', required: false, desc: 'Update idle timeout seconds' },
        { in: 'body', name: 'busy_timeout_seconds', type: 'int|null', required: false, desc: 'Update busy timeout seconds' }
      ], example: `curl -s -X PUT ${BASE}/api/v0/agents/<name> -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"description":"Updated"}'`, resp: { schema: 'Agent' }, responses: [{ status: 200, body: `{"name":"demo","created_by":"admin","state":"idle","description":"Updated","parent_agent_name":null,"created_at":"2025-01-01T12:00:00Z","last_activity_at":"2025-01-01T12:20:00Z","metadata":{},"tags":[],"is_published":false,"published_at":null,"published_by":null,"publish_permissions":{"code":true,"secrets":true,"content":true},"idle_timeout_seconds":300,"busy_timeout_seconds":900,"idle_from":"2025-01-01T12:20:00Z","busy_from":null}` }] },
      { method: 'PUT', path: '/api/v0/agents/{name}/state', auth: 'bearer', desc: 'Update agent state (generic).', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' },
        { in: 'body', name: 'state', type: 'string', required: true, desc: 'New state (e.g., init|idle|busy|slept)' }
      ], example: `curl -s -X PUT ${BASE}/api/v0/agents/<name>/state -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"state":"idle"}'`, resp: { schema: 'StateAck' }, responses: [{ status: 200, body: `{"success":true,"state":"idle"}` }] },
      { method: 'POST', path: '/api/v0/agents/{name}/busy', auth: 'bearer', desc: 'Set agent busy.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' }
      ], example: `curl -s -X POST ${BASE}/api/v0/agents/<name>/busy -H "Authorization: Bearer <token>"`, resp: { schema: 'BusyIdleAck' }, responses: [{ status: 200, body: `{"success":true,"state":"busy","timeout_status":"paused"}` }] },
      { method: 'POST', path: '/api/v0/agents/{name}/idle', auth: 'bearer', desc: 'Set agent idle.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' }
      ], example: `curl -s -X POST ${BASE}/api/v0/agents/<name>/idle -H "Authorization: Bearer <token>"`, resp: { schema: 'BusyIdleAck' }, responses: [{ status: 200, body: `{"success":true,"state":"idle","timeout_status":"active"}` }] },
      { method: 'POST', path: '/api/v0/agents/{name}/sleep', auth: 'bearer', desc: 'Put agent to sleep.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' }
      ], example: `curl -s -X POST ${BASE}/api/v0/agents/<name>/sleep -H "Authorization: Bearer <token>"`, resp: { schema: 'Agent' }, responses: [{ status: 200, body: `{"name":"demo","created_by":"admin","state":"slept",...}` }] },
      { method: 'POST', path: '/api/v0/agents/{name}/wake', auth: 'bearer', desc: 'Wake agent.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' },
        { in: 'body', name: 'prompt', type: 'string|null', required: false, desc: 'Optional prompt to send on wake' }
      ], example: `curl -s -X POST ${BASE}/api/v0/agents/<name>/wake -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"prompt":"get ready"}'`, resp: { schema: 'Agent' }, responses: [{ status: 200, body: `{"name":"demo","created_by":"admin","state":"init",...}` }] },
      { method: 'POST', path: '/api/v0/agents/{name}/remix', auth: 'bearer', desc: 'Remix agent (create a new agent from parent).', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Parent agent name' },
        { in: 'body', name: 'name', type: 'string', required: true, desc: 'New agent name; must match ^[A-Za-z][A-Za-z0-9-]{0,61}[A-Za-z0-9]$' },
        { in: 'body', name: 'metadata', type: 'object|null', required: false, desc: 'Optional metadata override' },
        { in: 'body', name: 'code', type: 'boolean', required: false, desc: 'Copy code (default true)' },
        { in: 'body', name: 'secrets', type: 'boolean', required: false, desc: 'Copy secrets (default true)' },
        { in: 'body', name: 'content', type: 'boolean', required: false, desc: 'Copy content (always true in v0.4.0+)' },
        { in: 'body', name: 'prompt', type: 'string|null', required: false, desc: 'Optional initial prompt' }
      ], example: `curl -s -X POST ${BASE}/api/v0/agents/<name>/remix -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"name":"demo-copy","code":true,"secrets":false,"prompt":"clone and adjust"}'`, resp: { schema: 'Agent' }, responses: [{ status: 200, body: `{"name":"demo-copy","created_by":"admin","state":"init",...}` }] },
      { method: 'POST', path: '/api/v0/agents/{name}/publish', auth: 'bearer', desc: 'Publish agent.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' },
        { in: 'body', name: 'code', type: 'boolean', required: false, desc: 'Allow code remix (default true)' },
        { in: 'body', name: 'secrets', type: 'boolean', required: false, desc: 'Allow secrets remix (default true)' },
        { in: 'body', name: 'content', type: 'boolean', required: false, desc: 'Publish content (default true)' }
      ], example: `curl -s -X POST ${BASE}/api/v0/agents/<name>/publish -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"code":true,"secrets":false,"content":true}'`, resp: { schema: 'Agent' }, responses: [{ status: 200, body: `{"name":"demo","is_published":true,"published_at":"2025-01-01T12:30:00Z",...}` }] },
      { method: 'POST', path: '/api/v0/agents/{name}/unpublish', auth: 'bearer', desc: 'Unpublish agent.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' }
      ], example: `curl -s -X POST ${BASE}/api/v0/agents/<name>/unpublish -H "Authorization: Bearer <token>"`, resp: { schema: 'Agent' }, responses: [{ status: 200, body: `{"name":"demo","is_published":false,"published_at":null,...}` }] },
      { method: 'DELETE', path: '/api/v0/agents/{name}', auth: 'bearer', desc: 'Delete agent.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' }
      ], example: `curl -s -X DELETE ${BASE}/api/v0/agents/<name> -H "Authorization: Bearer <token>"`, resp: { schema: 'Empty' }, responses: [{ status: 200 }] }
    ]
  },
  {
    id: 'responses',
    title: 'Agent Responses',
    description: 'Composite input→output exchanges with live items (protected).',
    endpoints: [
      { method: 'GET', path: '/api/v0/agents/{name}/responses', auth: 'bearer', desc: 'List responses for agent.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' },
        { in: 'query', name: 'limit', type: 'int', required: false, desc: 'Max responses (0..1000, default 100)' },
        { in: 'query', name: 'offset', type: 'int', required: false, desc: 'Offset for pagination (default 0)' }
      ], example: `curl -s ${BASE}/api/v0/agents/<name>/responses?limit=20 -H "Authorization: Bearer <token>"`, resp: { schema: 'ResponseObject', array: true }, responses: [{ status: 200, body: `[{"id":"uuid","agent_name":"demo","status":"completed","input":{"text":"hi"},"output":{"text":"hello","items":[]},"created_at":"2025-01-01T12:00:00Z","updated_at":"2025-01-01T12:00:10Z"}]` }] },
      { method: 'POST', path: '/api/v0/agents/{name}/responses', auth: 'bearer', desc: 'Create a response (user input).', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' },
        { in: 'body', name: 'input', type: 'object', required: true, desc: 'User input; shape: { text: string }' }
      ], example: `curl -s -X POST ${BASE}/api/v0/agents/<name>/responses -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"input":{"text":"hello"}}'`, resp: { schema: 'ResponseObject' }, responses: [{ status: 200, body: `{"id":"uuid","status":"pending",...}` }] },
      { method: 'PUT', path: '/api/v0/agents/{name}/responses/{id}', auth: 'bearer', desc: 'Update a response (agent-only typical). Used to append output.items and mark status.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' },
        { in: 'path', name: 'id', type: 'string', required: true, desc: 'Response id' },
        { in: 'body', name: 'status', type: "'pending'|'processing'|'completed'|'failed'", required: false, desc: 'Status update' },
        { in: 'body', name: 'input', type: 'object', required: false, desc: 'Optional input update; replaces existing input JSON' },
        { in: 'body', name: 'output', type: 'object', required: false, desc: 'Output update; shape: { text?: string, items?: [] }' }
      ], example: `curl -s -X PUT ${BASE}/api/v0/agents/<name>/responses/<id> -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"status":"completed","output":{"text":"done","items":[{"type":"final","text":"done"}]}}'`, resp: { schema: 'ResponseObject' }, responses: [{ status: 200, body: `{"id":"uuid","status":"completed",...}` }] },
      { method: 'GET', path: '/api/v0/agents/{name}/responses/count', auth: 'bearer', desc: 'Get response count for agent.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' }
      ], example: `curl -s ${BASE}/api/v0/agents/<name>/responses/count -H "Authorization: Bearer <token>"`, resp: { schema: 'Count' }, responses: [{ status: 200, body: `{"count":123,"agent_name":"demo"}` }] }
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
