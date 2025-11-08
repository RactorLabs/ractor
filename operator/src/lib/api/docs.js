// API documentation data source for TaskSandbox UI
// Covers endpoints defined in src/api/rest/routes.rs
import { getHostUrl } from '../branding.js';


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
          desc: 'Get API namespace and server version.',
          params: [],
          example: `curl -s ${BASE}/api/v0/version`,
          resp: { schema: 'Version' },
          responses: [
            { status: 200, body: `{"version":"0.5.3","api":"v0"}` }
          ]
        }
      ]
    },
    {
      id: 'auth',
      title: 'Authentication',
      description: 'Token validation and blocklist management.',
      endpoints: [
        {
          method: 'GET',
          path: '/api/v0/auth',
          auth: 'bearer',
          desc: 'Validate token and return authenticated principal profile.',
          params: [],
          example: `curl -s ${BASE}/api/v0/auth -H "Authorization: Bearer <token>"`,
          resp: { schema: 'AuthProfile' },
          responses: [
            { status: 200, body: `{"user":"admin","type":"Admin"}` }
          ]
        },
        {
          method: 'POST',
          path: '/api/v0/auth/token',
          auth: 'bearer',
          adminOnly: true,
          desc: 'Issue a token for a principal (admin only).',
          params: [
            { in: 'body', name: 'principal', type: 'string', required: true, desc: 'Principal name (user or admin id)' },
            { in: 'body', name: 'type', type: 'string', required: true, desc: "Principal type: 'User' or 'Admin'" },
            { in: 'body', name: 'ttl_hours', type: 'number', required: false, desc: 'Optional token TTL in hours (<=0 or omitted for no expiry).' }
          ],
          example: `curl -s -X POST ${BASE}/api/v0/auth/token -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"principal":"user1","type":"User","ttl_hours":12}'`,
          resp: { schema: 'TokenResponse' },
          responses: [
            { status: 200, body: `{"token":"<jwt>","token_type":"Bearer","expires_at":"2025-01-01T12:34:56Z","user":"user1","role":"user"}` }
          ]
        },
        {
          method: 'GET',
          path: '/api/v0/auth/blocklist',
          auth: 'bearer',
          adminOnly: true,
          desc: 'List blocked principals.',
          params: [],
          example: `curl -s ${BASE}/api/v0/auth/blocklist -H "Authorization: Bearer <token>"`,
          resp: { schema: 'BlockedPrincipal', array: true },
          responses: [
            { status: 200, body: `[{"principal":"user1","principal_type":"User","created_at":"2025-01-01T00:00:00Z"}]` }
          ]
        },
        {
          method: 'POST',
          path: '/api/v0/auth/blocklist/block',
          auth: 'bearer',
          adminOnly: true,
          desc: "Block a principal by name. Defaults to type 'User'.",
          params: [
            { in: 'body', name: 'principal', type: 'string', required: true, desc: 'Principal name' },
            { in: 'body', name: 'type', type: 'string', required: false, desc: "Optional principal type: 'User' or 'Admin' (default 'User')" }
          ],
          example: `curl -s -X POST ${BASE}/api/v0/auth/blocklist/block -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"principal":"user1"}'`,
          resp: { schema: 'Empty' },
          responses: [
            { status: 200, body: `{"blocked":true,"created":true}` }
          ]
        },
        {
          method: 'POST',
          path: '/api/v0/auth/blocklist/unblock',
          auth: 'bearer',
          adminOnly: true,
          desc: "Unblock a principal by name. Defaults to type 'User'.",
          params: [
            { in: 'body', name: 'principal', type: 'string', required: true, desc: 'Principal name' },
            { in: 'body', name: 'type', type: 'string', required: false, desc: "Optional principal type: 'User' or 'Admin' (default 'User')" }
          ],
          example: `curl -s -X POST ${BASE}/api/v0/auth/blocklist/unblock -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"principal":"user1"}'`,
          resp: { schema: 'Empty' },
          responses: [
            { status: 200, body: `{"blocked":false,"deleted":true}` }
          ]
        }
      ]
    },
    {
      id: 'operators',
      title: 'Operators',
      description: 'Operator management endpoints (protected).',
      endpoints: [
        {
          method: 'POST',
          path: '/api/v0/auth/operators/{name}/login',
          auth: 'public',
          desc: 'Login with operator name and password. Returns JWT token and role info.',
          params: [
            { in: 'path', name: 'name', type: 'string', required: true, desc: 'Operator username' },
            { in: 'body', name: 'pass', type: 'string', required: true, desc: 'Operator password' },
            { in: 'body', name: 'ttl_hours', type: 'number', required: false, desc: 'Optional token TTL in hours (<=0 or omitted for default persistent tokens).' }
          ],
          example: `curl -s -X POST ${BASE}/api/v0/auth/operators/<name>/login -H "Content-Type: application/json" -d '{"pass":"<password>","ttl_hours":24}'`,
          resp: { schema: 'TokenResponse' },
          responses: [
            { status: 200, body: `{"token":"<jwt>","token_type":"Bearer","expires_at":"2025-01-01T12:34:56Z","user":"admin","role":"admin"}` }
          ]
        },
        {
          method: 'GET',
          path: '/api/v0/auth/operators',
          auth: 'bearer',
          desc: 'List operators. Admins see all; non-admins receive their own record.',
          params: [],
          example: `curl -s ${BASE}/api/v0/auth/operators -H "Authorization: Bearer <token>"`,
          resp: { schema: 'Operator', array: true },
          responses: [
            { status: 200, body: `[{"user":"admin","description":null,"active":true,"created_at":"2025-01-01T00:00:00Z","updated_at":"2025-01-01T00:00:00Z","last_login_at":"2025-01-01T12:00:00Z"}]` }
          ]
        },
        {
          method: 'POST',
          path: '/api/v0/auth/operators',
          auth: 'bearer',
          adminOnly: true,
          desc: 'Create an operator account.',
          params: [
            { in: 'body', name: 'user', type: 'string', required: true, desc: 'Operator username' },
            { in: 'body', name: 'pass', type: 'string', required: true, desc: 'Password' },
            { in: 'body', name: 'description', type: 'string|null', required: false, desc: 'Optional description' }
          ],
          example: `curl -s -X POST ${BASE}/api/v0/auth/operators -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"user":"alice","pass":"<password>","description":"Team operator"}'`,
          resp: { schema: 'Operator' },
          responses: [
            { status: 200, body: `{"user":"alice","description":"Team operator","active":true,"created_at":"2025-01-01T00:00:00Z","updated_at":"2025-01-01T00:00:00Z","last_login_at":null}` }
          ]
        },
        {
          method: 'GET',
          path: '/api/v0/auth/operators/{name}',
          auth: 'bearer',
          desc: 'Get operator by name. Operators may read themselves; admins require permission to read others.',
          params: [
            { in: 'path', name: 'name', type: 'string', required: true, desc: 'Operator username' }
          ],
          example: `curl -s ${BASE}/api/v0/auth/operators/<name> -H "Authorization: Bearer <token>"`,
          resp: { schema: 'Operator' },
          responses: [
            { status: 200, body: `{"user":"alice","description":"Team operator","active":true,"created_at":"2025-01-01T00:00:00Z","updated_at":"2025-01-02T10:00:00Z","last_login_at":null}` }
          ]
        },
        {
          method: 'PUT',
          path: '/api/v0/auth/operators/{name}',
          auth: 'bearer',
          desc: 'Update operator metadata (description, active). Requires permission or self-update.',
          params: [
            { in: 'path', name: 'name', type: 'string', required: true, desc: 'Operator username' },
            { in: 'body', name: 'description', type: 'string|null', required: false, desc: 'Optional description' },
            { in: 'body', name: 'active', type: 'boolean|null', required: false, desc: 'Set active status; must be boolean or null' }
          ],
          example: `curl -s -X PUT ${BASE}/api/v0/auth/operators/<name> -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"description":"Updated","active":true}'`,
          resp: { schema: 'Operator' },
          responses: [
            { status: 200, body: `{"user":"alice","description":"Updated","active":true,"created_at":"2025-01-01T00:00:00Z","updated_at":"2025-01-02T12:00:00Z","last_login_at":null}` }
          ]
        },
        {
          method: 'DELETE',
          path: '/api/v0/auth/operators/{name}',
          auth: 'bearer',
          adminOnly: true,
          desc: 'Delete an operator account.',
          params: [
            { in: 'path', name: 'name', type: 'string', required: true, desc: 'Operator username' }
          ],
          example: `curl -s -X DELETE ${BASE}/api/v0/auth/operators/<name> -H "Authorization: Bearer <token>"`,
          resp: { schema: 'Empty' },
          responses: [
            { status: 200 }
          ]
        },
        {
          method: 'PUT',
          path: '/api/v0/auth/operators/{name}/password',
          auth: 'bearer',
          desc: 'Update operator password (self-service or admin).',
          params: [
            { in: 'path', name: 'name', type: 'string', required: true, desc: 'Operator username' },
            { in: 'body', name: 'current_password', type: 'string', required: true, desc: 'Current password' },
            { in: 'body', name: 'new_password', type: 'string', required: true, desc: 'New password' }
          ],
          example: `curl -s -X PUT ${BASE}/api/v0/auth/operators/<name>/password -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"current_password":"old","new_password":"new"}'`,
          resp: { schema: 'Empty' },
          responses: [
            { status: 200 }
          ]
        }
      ]
    },
    {
      id: 'sandboxes',
      title: 'Sandboxes',
      description: 'Sandbox lifecycle and management endpoints.',
      endpoints: [
        {
          method: 'GET',
          path: '/api/v0/sandboxes',
          auth: 'bearer',
          desc: 'List sandboxes owned by the caller (admins may filter across all sandboxes).',
          params: [
            { in: 'query', name: 'state', type: 'string', required: false, desc: "Filter by state: 'init'|'idle'|'busy'|'terminated'" },
            { in: 'query', name: 'q', type: 'string', required: false, desc: 'Substring match on description (case-insensitive).' },
            { in: 'query', name: 'tags', type: 'string or string[]', required: false, desc: "Filter by tags. Provide multiple 'tags' params or a comma-separated string." },
            { in: 'query', name: 'limit', type: 'int', required: false, desc: 'Page size (default 30, max 100).' },
            { in: 'query', name: 'page', type: 'int', required: false, desc: '1-based page number (ignored when offset present).' },
            { in: 'query', name: 'offset', type: 'int', required: false, desc: 'Row offset (0-based).' }
          ],
          example: `curl -s ${BASE}/api/v0/sandboxes?state=idle&tags=prod&tags=team/core&limit=20 -H "Authorization: Bearer <token>"`,
          resp: { schema: 'ListSandboxesResult' },
          responses: [
            { status: 200, body: `{"items":[{"id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","created_by":"admin","state":"idle","description":"Demo sandbox","snapshot_id":null,"created_at":"2025-01-01T12:00:00Z","last_activity_at":"2025-01-01T12:10:00Z","metadata":{},"tags":["prod","team/core"],"idle_timeout_seconds":900,"idle_from":"2025-01-01T12:10:00Z","busy_from":null,"context_cutoff_at":null,"last_context_length":2048}],"total":1,"limit":20,"offset":0,"page":1,"pages":1}` }
          ]
        },
        {
          method: 'POST',
          path: '/api/v0/sandboxes',
          auth: 'bearer',
          desc: 'Create a new sandbox owned by the caller.',
          params: [
            { in: 'body', name: 'description', type: 'string|null', required: false, desc: 'Optional description.' },
            { in: 'body', name: 'metadata', type: 'object', required: false, desc: 'Arbitrary metadata JSON (default {}).' },
            { in: 'body', name: 'tags', type: 'string[]', required: false, desc: "Tag list (stored lowercase; allowed characters letters, digits, '/', '-', '_', '.')." },
            { in: 'body', name: 'env', type: 'object<string,string>', required: false, desc: 'Environment variable map to inject on boot.' },
            { in: 'body', name: 'instructions', type: 'string|null', required: false, desc: 'Optional instructions passed to the sandbox runtime.' },
            { in: 'body', name: 'setup', type: 'string|null', required: false, desc: 'Optional setup script executed on boot.' },
            { in: 'body', name: 'prompt', type: 'string|null', required: false, desc: 'Optional initial prompt for the sandbox task loop.' },
            { in: 'body', name: 'idle_timeout_seconds', type: 'int|null', required: false, desc: 'Override idle timeout seconds (defaults to 900).' },
            { in: 'body', name: 'snapshot_id', type: 'string|null', required: false, desc: 'Restore from an existing snapshot (files copied by the controller).' }
          ],
          example: `curl -s -X POST ${BASE}/api/v0/sandboxes -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"description":"Demo","tags":["prod"]}'`,
          resp: { schema: 'Sandbox' },
          responses: [
            { status: 200, body: `{"id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","created_by":"admin","state":"init","description":"Demo","snapshot_id":null,"created_at":"2025-01-01T12:00:00Z","last_activity_at":null,"metadata":{},"tags":["prod"],"idle_timeout_seconds":900,"idle_from":null,"busy_from":null,"context_cutoff_at":null,"last_context_length":0}` }
          ]
        },
        {
          method: 'GET',
          path: '/api/v0/sandboxes/{id}',
          auth: 'bearer',
          desc: 'Get sandbox by ID.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
          ],
          example: `curl -s ${BASE}/api/v0/sandboxes/<id> -H "Authorization: Bearer <token>"`,
          resp: { schema: 'Sandbox' },
          responses: [
            { status: 200, body: `{"id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","created_by":"admin","state":"idle","description":"Demo sandbox","snapshot_id":null,"created_at":"2025-01-01T12:00:00Z","last_activity_at":"2025-01-01T12:10:00Z","metadata":{},"tags":[],"idle_timeout_seconds":900,"idle_from":"2025-01-01T12:10:00Z","busy_from":null,"context_cutoff_at":null,"last_context_length":1024}` }
          ]
        },
        {
          method: 'PUT',
          path: '/api/v0/sandboxes/{id}',
          auth: 'bearer',
          desc: 'Update sandbox metadata (description, tags, metadata, idle timeout).',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
            { in: 'body', name: 'metadata', type: 'object|null', required: false, desc: 'Replace metadata (omit to keep current).' },
            { in: 'body', name: 'description', type: 'string|null', required: false, desc: 'Replace description.' },
            { in: 'body', name: 'tags', type: 'string[]|null', required: false, desc: 'Replace tag list (same validation as create).' },
            { in: 'body', name: 'idle_timeout_seconds', type: 'int|null', required: false, desc: 'Update idle timeout in seconds.' }
          ],
          example: `curl -s -X PUT ${BASE}/api/v0/sandboxes/<id> -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"description":"Updated","tags":["prod","team"]}'`,
          resp: { schema: 'Sandbox' },
          responses: [
            { status: 200, body: `{"id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","created_by":"admin","state":"idle","description":"Updated","snapshot_id":null,"created_at":"2025-01-01T12:00:00Z","last_activity_at":"2025-01-01T12:20:00Z","metadata":{},"tags":["prod","team"],"idle_timeout_seconds":900,"idle_from":"2025-01-01T12:20:00Z","busy_from":null,"context_cutoff_at":null,"last_context_length":1024}` }
          ]
        },
        {
          method: 'PUT',
          path: '/api/v0/sandboxes/{id}/state',
          auth: 'bearer',
          desc: 'Update sandbox state directly (owner or admin).',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
            { in: 'body', name: 'state', type: 'string', required: true, desc: "Desired state ('init','idle','busy','terminated')." }
          ],
          example: `curl -s -X PUT ${BASE}/api/v0/sandboxes/<id>/state -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"state":"idle"}'`,
          resp: { schema: 'StateAck' },
          responses: [
            { status: 200, body: `{"success":true,"state":"idle"}` }
          ]
        },
        {
          method: 'POST',
          path: '/api/v0/sandboxes/{id}/state/busy',
          auth: 'bearer',
          desc: 'Mark sandbox busy (typically called by the sandbox container).',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
          ],
          example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/state/busy -H "Authorization: Bearer <token>"`,
          resp: { schema: 'BusyIdleAck' },
          responses: [
            { status: 200, body: `{"success":true,"state":"busy","timeout_status":"paused"}` }
          ]
        },
        {
          method: 'POST',
          path: '/api/v0/sandboxes/{id}/state/idle',
          auth: 'bearer',
          desc: 'Mark sandbox idle (typically called by the sandbox container).',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
          ],
          example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/state/idle -H "Authorization: Bearer <token>"`,
          resp: { schema: 'BusyIdleAck' },
          responses: [
            { status: 200, body: `{"success":true,"state":"idle","timeout_status":"active"}` }
          ]
        },
        {
          method: 'GET',
          path: '/api/v0/sandboxes/{id}/runtime',
          auth: 'bearer',
          desc: 'Get cumulative runtime across sandbox lifetimes and current session runtime.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
          ],
          example: `curl -s ${BASE}/api/v0/sandboxes/<id>/runtime -H "Authorization: Bearer <token>"`,
          resp: { schema: 'RuntimeTotal' },
          responses: [
            { status: 200, body: `{"sandbox_id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","total_runtime_seconds":1234,"current_sandbox_seconds":321}` }
          ]
        },
        {
          method: 'GET',
          path: '/api/v0/sandboxes/{id}/context',
          auth: 'bearer',
          desc: 'Get the latest context usage estimation for a sandbox.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
          ],
          example: `curl -s ${BASE}/api/v0/sandboxes/<id>/context -H "Authorization: Bearer <token>"`,
          resp: { schema: 'SandboxContextUsage' },
          responses: [
            { status: 200, body: `{"sandbox":"fa36e542-b9b8-11f0-aadd-064ac08387fc","soft_limit_tokens":128000,"used_tokens_estimated":12345,"used_percent":9.6,"basis":"inference_last_context_length","cutoff_at":"2025-01-01T12:34:56Z","measured_at":"2025-01-01T13:00:00Z","total_messages_considered":0}` }
          ]
        },
        {
          method: 'POST',
          path: '/api/v0/sandboxes/{id}/context/clear',
          auth: 'bearer',
          desc: 'Clear sandbox context and set a new cutoff timestamp.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
          ],
          example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/context/clear -H "Authorization: Bearer <token>"`,
          resp: { schema: 'SandboxContextUsage' },
          responses: [
            { status: 200, body: `{"sandbox":"fa36e542-b9b8-11f0-aadd-064ac08387fc","soft_limit_tokens":128000,"used_tokens_estimated":0,"used_percent":0,"basis":"inference_last_context_length","cutoff_at":"2025-01-01T13:00:00Z","measured_at":"2025-01-01T13:00:00Z","total_messages_considered":0}` }
          ]
        },
        {
          method: 'POST',
          path: '/api/v0/sandboxes/{id}/context/compact',
          auth: 'bearer',
          desc: 'Compact sandbox context (summarize recent history and reset usage).',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
          ],
          example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/context/compact -H "Authorization: Bearer <token>"`,
          resp: { schema: 'SandboxContextUsage' },
          responses: [
            { status: 200, body: `{"sandbox":"fa36e542-b9b8-11f0-aadd-064ac08387fc","soft_limit_tokens":128000,"used_tokens_estimated":0,"used_percent":0,"basis":"inference_last_context_length","cutoff_at":"2025-01-01T13:05:00Z","measured_at":"2025-01-01T13:05:00Z","total_messages_considered":0}` }
          ]
        },
        {
          method: 'POST',
          path: '/api/v0/sandboxes/{id}/context/usage',
          auth: 'bearer',
          desc: 'Report the latest context length (tokens) after an inference call.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
            { in: 'body', name: 'tokens', type: 'int', required: true, desc: 'Latest context length tokens (non-negative).' }
          ],
          example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/context/usage -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"tokens":4096}'`,
          resp: { schema: 'ContextUsageAck' },
          responses: [
            { status: 200, body: `{"success":true,"last_context_length":4096}` }
          ]
        },
        {
          method: 'DELETE',
          path: '/api/v0/sandboxes/{id}',
          auth: 'bearer',
          desc: 'Schedule sandbox termination (controller stops container and marks state terminated). Any in-flight tasks are cancelled immediately.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
          ],
          example: `curl -s -X DELETE ${BASE}/api/v0/sandboxes/<id> -H "Authorization: Bearer <token>"`,
          resp: { schema: 'Empty' },
          responses: [
            { status: 200 }
          ]
        },
        {
          method: 'GET',
          path: '/api/v0/sandboxes/{id}/tasks',
          auth: 'bearer',
          desc: 'List tasks for a sandbox in chronological order.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
            { in: 'query', name: 'limit', type: 'int', required: false, desc: 'Max records (default 100, max 1000).' },
            { in: 'query', name: 'offset', type: 'int', required: false, desc: 'Row offset (default 0).' }
          ],
          example: `curl -s ${BASE}/api/v0/sandboxes/<id>/tasks?limit=20 -H "Authorization: Bearer <token>"`,
          resp: { schema: 'TaskObject', array: true },
          responses: [
            { status: 200, body: `[{"id":"task_123","sandbox_id":"<id>","status":"completed","input_content":[{"type":"text","content":"hi"}],"output_content":[{"type":"text","content":"hello"}],"segments":[{"type":"final","channel":"final","text":"hello"}],"timeout_seconds":600,"timeout_at":"2025-01-01T12:10:00Z","created_at":"2025-01-01T12:00:00Z","updated_at":"2025-01-01T12:00:10Z"}]` }
          ]
        },
        {
          method: 'POST',
          path: '/api/v0/sandboxes/{id}/tasks',
          auth: 'bearer',
          desc: 'Create a task (enqueue user input). Optional background=false blocks until completion or 15-minute timeout.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
            { in: 'body', name: 'input', type: 'object', required: true, desc: "User input JSON; preferred shape { content: [{ type: 'text', content: string }] }." },
            { in: 'body', name: 'background', type: 'boolean', required: false, desc: 'Defaults to true (non-blocking). Set false to wait for completion.' },
            { in: 'body', name: 'timeout_seconds', type: 'int|null', required: false, desc: 'Per-task timeout seconds (defaults to 3600, 0 to disable).' }
          ],
          example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/tasks -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"input":{"content":[{"type":"text","content":"hello"}]},"background":false}'`,
          resp: { schema: 'TaskObject' },
          responses: [
            { status: 200, body: `{"id":"task_123","sandbox_id":"<id>","status":"completed","input_content":[{"type":"text","content":"hello"}],"output_content":[{"type":"text","content":"hi there"}],"segments":[{"type":"final","channel":"final","text":"hi there"}],"timeout_seconds":600,"timeout_at":"2025-01-01T12:10:00Z","created_at":"2025-01-01T12:00:00Z","updated_at":"2025-01-01T12:00:10Z"}` },
            { status: 504, body: `{"message":"Timed out waiting for task to complete"}` }
          ]
        },
        {
          method: 'GET',
          path: '/api/v0/sandboxes/{id}/tasks/{task_id}',
          auth: 'bearer',
          desc: 'Fetch a task by ID within a sandbox.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
            { in: 'path', name: 'task_id', type: 'string', required: true, desc: 'Task ID (UUID)' }
          ],
          example: `curl -s ${BASE}/api/v0/sandboxes/<id>/tasks/<task_id> -H "Authorization: Bearer <token>"`,
          resp: { schema: 'TaskObject' },
          responses: [
            { status: 200, body: `{"id":"task_123","sandbox_id":"<id>","status":"processing","input_content":[{"type":"text","content":"hi"}],"output_content":[],"segments":[],"timeout_seconds":600,"timeout_at":"2025-01-01T12:10:00Z","created_at":"2025-01-01T12:00:00Z","updated_at":"2025-01-01T12:05:00Z"}` }
          ]
        },
        {
          method: 'PUT',
          path: '/api/v0/sandboxes/{id}/tasks/{task_id}',
          auth: 'bearer',
          desc: 'Update a task record (status, input, output, timeout). Typically used by controller.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
            { in: 'path', name: 'task_id', type: 'string', required: true, desc: 'Task ID (UUID)' },
            { in: 'body', name: 'status', type: "'pending'|'processing'|'completed'|'failed'|'cancelled'", required: false, desc: 'Status update.' },
            { in: 'body', name: 'input', type: 'object', required: false, desc: 'Optional input update (replaces existing input JSON).' },
            { in: 'body', name: 'output', type: 'object', required: false, desc: 'Output update; merges text/items into existing output.' },
            { in: 'body', name: 'timeout_seconds', type: 'int|null', required: false, desc: 'Reset per-task timeout (<=0 clears).' }
          ],
          example: `curl -s -X PUT ${BASE}/api/v0/sandboxes/<id>/tasks/<task_id> -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"status":"completed","output":{"text":"done"}}'`,
          resp: { schema: 'TaskObject' },
          responses: [
            { status: 200, body: `{"id":"task_123","sandbox_id":"<id>","status":"completed","input_content":[{"type":"text","content":"hello"}],"output_content":[{"type":"text","content":"done"}],"segments":[{"type":"final","channel":"final","text":"done"}],"timeout_seconds":null,"timeout_at":null,"created_at":"2025-01-01T12:00:00Z","updated_at":"2025-01-01T12:05:00Z"}` }
          ]
        },
        {
          method: 'POST',
          path: '/api/v0/sandboxes/{id}/tasks/{task_id}/cancel',
          auth: 'bearer',
          desc: 'Cancel a specific pending or processing task. If the task is already complete, returns HTTP 409.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
            { in: 'path', name: 'task_id', type: 'string', required: true, desc: 'Task ID (UUID)' }
          ],
          example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/tasks/<task_id>/cancel -H "Authorization: Bearer <token>"`,
          responses: [
            { status: 200, body: `{"status":"ok","sandbox":"<id>","task":"<task_id>","cancelled":true}` },
            { status: 409, body: `{"message":"Task is not in a cancellable state"}` }
          ]
        },
        {
          method: 'GET',
          path: '/api/v0/sandboxes/{id}/tasks/count',
          auth: 'bearer',
          desc: 'Count tasks for a sandbox.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
          ],
          example: `curl -s ${BASE}/api/v0/sandboxes/<id>/tasks/count -H "Authorization: Bearer <token>"`,
          resp: { schema: 'Count' },
          responses: [
            { status: 200, body: `{"count":42,"sandbox_id":"<id>"}` }
          ]
        },
      ]
    },
    {
      id: 'snapshots',
      title: 'Snapshots',
      description: 'Snapshot management and file browsing.',
      endpoints: [
        {
          method: 'GET',
          path: '/api/v0/snapshots',
          auth: 'bearer',
          desc: 'List snapshots. Filter by sandbox_id to scope results.',
          params: [
            { in: 'query', name: 'sandbox_id', type: 'string', required: false, desc: 'Optional sandbox ID filter.' }
          ],
          example: `curl -s ${BASE}/api/v0/snapshots?sandbox_id=<sandbox> -H "Authorization: Bearer <token>"`,
          resp: { schema: 'PaginatedSnapshots' },
          responses: [
            { status: 200, body: `{"items":[{"id":"snp_123","sandbox_id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","trigger_type":"user","created_at":"2025-01-01T15:00:00Z","metadata":{}}],"total":1,"limit":100,"offset":0,"page":1,"pages":1}` }
          ]
        },
        {
          method: 'GET',
          path: '/api/v0/snapshots/{id}',
          auth: 'bearer',
          desc: 'Fetch a snapshot by ID.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Snapshot ID (UUID)' }
          ],
          example: `curl -s ${BASE}/api/v0/snapshots/<id> -H "Authorization: Bearer <token>"`,
          resp: { schema: 'Snapshot' },
          responses: [
            { status: 200, body: `{"id":"snp_123","sandbox_id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","trigger_type":"user","created_at":"2025-01-01T15:00:00Z","metadata":{}}` }
          ]
        },
        {
          method: 'DELETE',
          path: '/api/v0/snapshots/{id}',
          auth: 'bearer',
          desc: 'Delete a snapshot by ID.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Snapshot ID (UUID)' }
          ],
          example: `curl -s -X DELETE ${BASE}/api/v0/snapshots/<id> -H "Authorization: Bearer <token>"`,
          resp: { schema: 'Empty' },
          responses: [
            { status: 204 }
          ]
        },
        {
          method: 'POST',
          path: '/api/v0/snapshots/{id}/create',
          auth: 'bearer',
          desc: 'Create a new sandbox from a snapshot ID (same payload as POST /sandboxes).',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Snapshot ID (UUID)' },
            { in: 'body', name: 'payload', type: 'object', required: false, desc: 'Fields mirror sandbox creation (description, tags, env, etc.).' }
          ],
          example: `curl -s -X POST ${BASE}/api/v0/snapshots/<id>/create -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"description":"Restored"}'`,
          resp: { schema: 'Sandbox' },
          responses: [
            { status: 200, body: `{"id":"new_sandbox","created_by":"admin","state":"init","description":"Restored","snapshot_id":"<id>","created_at":"2025-01-01T16:00:00Z","last_activity_at":null,"metadata":{},"tags":[],"idle_timeout_seconds":900,"idle_from":null,"busy_from":null,"context_cutoff_at":null,"last_context_length":0}` }
          ]
        },
        {
          method: 'GET',
          path: '/api/v0/snapshots/{id}/files/list',
          auth: 'bearer',
          desc: 'List files in the root of a snapshot.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Snapshot ID (UUID)' },
            { in: 'query', name: 'offset', type: 'int', required: false, desc: 'Offset (default 0).' },
            { in: 'query', name: 'limit', type: 'int', required: false, desc: 'Page size (default 100).' }
          ],
          example: `curl -s ${BASE}/api/v0/snapshots/<id>/files/list?limit=50 -H "Authorization: Bearer <token>"`,
          resp: { schema: 'SnapshotFileList' },
          responses: [
            { status: 200, body: `{"items":[{"name":"README.md","is_dir":false,"size":2048,"modified":1735737600}],"total":1,"offset":0,"limit":50}` }
          ]
        },
        {
          method: 'GET',
          path: '/api/v0/snapshots/{id}/files/list/{path...}',
          auth: 'bearer',
          desc: 'List files under a directory inside a snapshot.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Snapshot ID (UUID)' },
            { in: 'path', name: 'path...', type: 'string', required: true, desc: 'Directory path relative to snapshot sandbox root.' },
            { in: 'query', name: 'offset', type: 'int', required: false, desc: 'Offset (default 0).' },
            { in: 'query', name: 'limit', type: 'int', required: false, desc: 'Page size (default 100).' }
          ],
          example: `curl -s ${BASE}/api/v0/snapshots/<id>/files/list/src -H "Authorization: Bearer <token>"`,
          resp: { schema: 'SnapshotFileList' },
          responses: [
            { status: 200, body: `{"items":[{"name":"main.rs","is_dir":false,"size":1024,"modified":1735737600}],"total":1,"offset":0,"limit":100}` }
          ]
        },
        {
          method: 'GET',
          path: '/api/v0/snapshots/{id}/files/metadata/{path...}',
          auth: 'bearer',
          desc: 'Get metadata for a file within a snapshot.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Snapshot ID (UUID)' },
            { in: 'path', name: 'path...', type: 'string', required: true, desc: 'Path relative to snapshot sandbox root.' }
          ],
          example: `curl -s ${BASE}/api/v0/snapshots/<id>/files/metadata/src/main.rs -H "Authorization: Bearer <token>"`,
          resp: { schema: 'SnapshotFileMetadata' },
          responses: [
            { status: 200, body: `{"is_dir":false,"is_file":true,"size":1024,"modified":1735737600}` }
          ]
        },
        {
          method: 'GET',
          path: '/api/v0/snapshots/{id}/files/read/{path...}',
          auth: 'bearer',
          desc: 'Read a file from a snapshot and stream its raw bytes.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Snapshot ID (UUID)' },
            { in: 'path', name: 'path...', type: 'string', required: true, desc: 'Path relative to snapshot sandbox root.' }
          ],
          example: `curl -s -OJ ${BASE}/api/v0/snapshots/<id>/files/read/src/main.rs -H "Authorization: Bearer <token>"`,
          resp: { schema: 'Empty' },
          responses: [
            { status: 200 }
          ]
        },
        {
          method: 'GET',
          path: '/api/v0/sandboxes/{id}/snapshots',
          auth: 'bearer',
          desc: 'List snapshots for a specific sandbox.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
          ],
          example: `curl -s ${BASE}/api/v0/sandboxes/<id>/snapshots -H "Authorization: Bearer <token>"`,
          resp: { schema: 'PaginatedSnapshots' },
          responses: [
            { status: 200, body: `{"items":[{"id":"snp_123","sandbox_id":"<id>","trigger_type":"user","created_at":"2025-01-01T15:00:00Z","metadata":{}}],"total":1,"limit":100,"offset":0,"page":1,"pages":1}` }
          ]
        },
        {
          method: 'POST',
          path: '/api/v0/sandboxes/{id}/snapshots',
          auth: 'bearer',
          desc: 'Create a snapshot of a sandbox. Blocks until controller reports completion or timeout.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
            { in: 'body', name: 'metadata', type: 'object', required: false, desc: 'Optional metadata stored with snapshot.' }
          ],
          example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/snapshots -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"metadata":{"note":"before deploy"}}'`,
          resp: { schema: 'Snapshot' },
          responses: [
            { status: 201, body: `{"id":"snp_123","sandbox_id":"<id>","trigger_type":"user","created_at":"2025-01-01T15:00:00Z","metadata":{"note":"before deploy"}}` }
          ]
        }
      ]
    },
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
