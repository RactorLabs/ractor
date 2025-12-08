// API documentation data source for TSBX UI
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
      title: 'Auth',
      description: 'Operator authentication and management endpoints.',
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
            { in: 'query', name: 'state', type: 'string', required: false, desc: "Filter by state: 'initializing'|'idle'|'busy'|'terminating'|'terminated'" },
            { in: 'query', name: 'q', type: 'string', required: false, desc: 'Substring match on description (case-insensitive).' },
            { in: 'query', name: 'tags', type: 'string or string[]', required: false, desc: "Filter by tags. Provide multiple 'tags' params or a comma-separated string." },
            { in: 'query', name: 'limit', type: 'int', required: false, desc: 'Page size (default 30, max 100).' },
            { in: 'query', name: 'page', type: 'int', required: false, desc: '1-based page number (ignored when offset present).' },
            { in: 'query', name: 'offset', type: 'int', required: false, desc: 'Row offset (0-based).' }
          ],
          example: `curl -s ${BASE}/api/v0/sandboxes?state=idle&tags=prod&tags=team/core&limit=20 -H "Authorization: Bearer <token>"`,
          resp: { schema: 'ListSandboxesResult' },
          responses: [
            { status: 200, body: `{"items":[{"id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","created_by":"admin","state":"idle","description":"Demo sandbox","snapshot_id":null,"created_at":"2025-01-01T12:00:00Z","last_activity_at":"2025-01-01T12:10:00Z","metadata":{},"tags":["prod","team/core"],"inference_provider":"Positron","inference_model":"llama-3.2-3b-instruct-fast-tp2","nl_task_enabled":true,"idle_timeout_seconds":900,"idle_from":"2025-01-01T12:10:00Z","busy_from":null,"tokens_prompt":0,"tokens_completion":0,"tool_count":{"run_bash":3,"read_file":1},"runtime_seconds":0,"tasks_completed":0}],"total":1,"limit":20,"offset":0,"page":1,"pages":1}` }
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
            { in: 'body', name: 'inference_provider', type: 'string|null', required: false, desc: 'Override the inference provider (defaults to the primary provider defined in tsbx.json).' },
            { in: 'body', name: 'inference_model', type: 'string|null', required: false, desc: 'Override inference model for this sandbox (uses system default if omitted).' },
            { in: 'body', name: 'inference_api_key', type: 'string|null', required: false, desc: 'Sandbox-scoped inference API key; required when you want NL tasks available but do not rely on the host default key.' },
            { in: 'body', name: 'env', type: 'object<string,string>', required: false, desc: 'Environment variable map to inject on boot.' },
            { in: 'body', name: 'instructions', type: 'string|null', required: false, desc: 'Optional instructions passed to the sandbox runtime.' },
            { in: 'body', name: 'setup', type: 'string|null', required: false, desc: 'Optional setup script executed on boot.' },
            { in: 'body', name: 'startup_task', type: 'string|null', required: false, desc: 'Optional startup task queued immediately after sandbox creation.' },
            { in: 'body', name: 'idle_timeout_seconds', type: 'int|null', required: false, desc: 'Override idle timeout seconds (defaults to 900).' },
            { in: 'body', name: 'snapshot_id', type: 'string|null', required: false, desc: 'Restore from an existing snapshot (files copied by the controller).' }
          ],
          example: `curl -s -X POST ${BASE}/api/v0/sandboxes -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"description":"Demo","tags":["prod"],"inference_provider":"Positron","inference_model":"llama-3.2-3b-instruct-fast-tp2","inference_api_key":"sandbox-local-key"}'`,
          resp: { schema: 'Sandbox' },
          responses: [
            { status: 200, body: `{"id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","created_by":"admin","state":"initializing","description":"Demo","snapshot_id":null,"created_at":"2025-01-01T12:00:00Z","last_activity_at":null,"metadata":{},"tags":["prod"],"inference_provider":"Positron","inference_model":"llama-3.2-3b-instruct-fast-tp2","nl_task_enabled":true,"idle_timeout_seconds":900,"idle_from":null,"busy_from":null,"tokens_prompt":0,"tokens_completion":0,"tool_count":{},"runtime_seconds":0,"tasks_completed":0}` }
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
            { status: 200, body: `{"id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","created_by":"admin","state":"idle","description":"Demo sandbox","snapshot_id":null,"created_at":"2025-01-01T12:00:00Z","last_activity_at":"2025-01-01T12:10:00Z","metadata":{},"tags":[],"inference_provider":"Positron","inference_model":"llama-3.2-3b-instruct-fast-tp2","nl_task_enabled":true,"idle_timeout_seconds":900,"idle_from":"2025-01-01T12:10:00Z","busy_from":null,"tokens_prompt":0,"tokens_completion":0,"tool_count":{"run_bash":3,"read_file":1},"runtime_seconds":0,"tasks_completed":0}` }
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
            { status: 200, body: `{"id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","created_by":"admin","state":"idle","description":"Updated","snapshot_id":null,"created_at":"2025-01-01T12:00:00Z","last_activity_at":"2025-01-01T12:20:00Z","metadata":{},"tags":["prod","team"],"idle_timeout_seconds":900,"idle_from":"2025-01-01T12:20:00Z","busy_from":null}` }
          ]
        },
        {
          method: 'PUT',
          path: '/api/v0/sandboxes/{id}/state',
          auth: 'bearer',
          desc: 'Update sandbox state directly (owner or admin).',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
            { in: 'body', name: 'state', type: 'string', required: true, desc: "Desired state ('idle','busy','terminated'). System-managed states include 'initializing' and 'terminating'." }
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
          path: '/api/v0/sandboxes/{id}/stats',
          auth: 'bearer',
          desc: 'Fetch sandbox statistics including runtime, usage, and tool counts.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
          ],
          example: `curl -s ${BASE}/api/v0/sandboxes/<id>/stats -H "Authorization: Bearer <token>"`,
          resp: { schema: 'SandboxStats' },
          responses: [
            { status: 200, body: `{"sandbox_id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","container_state":"running","tasks_completed":12,"total_tasks":18,"cpu_usage_percent":18.4,"cpu_limit_cores":4,"memory_usage_bytes":536870912,"memory_limit_bytes":2147483648,"tokens_prompt":4821,"tokens_completion":1734,"tokens_total":6555,"tool_count":{"run_bash":27,"read_file":9,"search_files":2},"runtime_seconds":840,"captured_at":"2025-01-01T15:04:32Z"}` }
          ]
        },
        {
          method: 'GET',
          path: '/api/v0/stats',
          auth: 'bearer',
          desc: 'Global stats across sandboxes including inference configuration and host metrics.',
          example: `curl -s ${BASE}/api/v0/stats -H "Authorization: Bearer <token>"`,
          resp: { schema: 'GlobalStats' },
          responses: [
            { status: 200, body: `{"sandboxes_total":14,"sandboxes_active":12,"sandboxes_terminated":2,"sandboxes_by_state":{"idle":5,"busy":3,"terminating":1,"terminated":2,"initializing":3},"sandbox_tasks_total":240,"sandbox_tasks_active":7,"inference_name":"Positron","inference_url":"https://api.positron.ai/v1/chat/completions","inference_models":["llama-3.2-3b-instruct-fast-tp2","llama-3.2-405b"],"default_inference_model":"llama-3.2-3b-instruct-fast-tp2","captured_at":"2025-01-01T15:04:32Z","host":{"hostname":"tsbx-dev","uptime_seconds":86400,"cpu_cores":16,"cpu_percent":21.4,"load_avg_1m":0.42,"load_avg_5m":0.38,"load_avg_15m":0.33,"memory_total_bytes":33554432000,"memory_used_bytes":21474836480,"memory_used_percent":64.0}}` }
          ]
        },
        {
          method: 'GET',
          path: '/api/v0/inference/providers',
          auth: 'bearer',
          desc: 'List configured inference providers and their supported models.',
          example: `curl -s ${BASE}/api/v0/inference/providers -H "Authorization: Bearer <token>"`,
          resp: { schema: 'InferenceProvider', array: true },
          responses: [
            { status: 200, body: `[{"name":"Positron","display_name":"Positron","url":"https://api.positron.ai/v1/chat/completions","default_model":"llama-3.2-3b-instruct-fast-tp2","is_default":true,"models":[{"name":"llama-3.2-3b-instruct-fast-tp2","display_name":"Llama 3.2 3B Instruct (Fast)"},{"name":"llama-3.2-1b-instruct-fast-tp2","display_name":"Llama 3.2 1B Instruct"}]}]` }
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
            { status: 200, body: `{"items":[{"id":"snp_123","sandbox_id":"<id>","trigger_type":"manual","created_at":"2025-01-01T15:00:00Z","metadata":{}}],"total":1,"limit":100,"offset":0,"page":1,"pages":1}` }
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
            { status: 201, body: `{"id":"snp_123","sandbox_id":"<id>","trigger_type":"manual","created_at":"2025-01-01T15:00:00Z","metadata":{"note":"before deploy"}}` }
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
          method: 'POST',
          path: '/api/v0/sandboxes/{id}/files/upload',
          auth: 'bearer',
          desc: 'Upload or overwrite a file inside /sandbox using a base64-encoded payload (5MB decoded limit).',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
            { in: 'body', name: 'path', type: 'string', required: true, desc: 'Relative path under /sandbox (for example src/main.rs).' },
            { in: 'body', name: 'content_base64', type: 'string', required: true, desc: 'File contents encoded as base64 (decoded max 5MB).' },
            { in: 'body', name: 'overwrite', type: 'boolean', required: false, desc: 'Defaults to true; set false to fail when the file already exists.' },
            { in: 'body', name: 'executable', type: 'boolean', required: false, desc: 'Set true to chmod 755 after upload (default 644).' }
          ],
          example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/files/upload -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"path":"scripts/setup.sh","content_base64":"$(base64 -w0 setup.sh)","executable":true}'`,
          responses: [
            { status: 200, body: `{"path":"scripts/setup.sh","bytes_written":128,"executable":true,"overwrite":true}` },
            { status: 409, body: `{"message":"File already exists."}` }
          ]
        },
        {
          method: 'POST',
          path: '/api/v0/sandboxes/{id}/secrets',
          auth: 'bearer',
          desc: 'Add or update a secret in /sandbox/.env. Secrets are sourced before every bash/python/js task.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
            { in: 'body', name: 'key', type: 'string', required: true, desc: 'Env var name (A-Z0-9_, not starting with TSBX_)' },
            { in: 'body', name: 'value', type: 'string', required: true, desc: 'Secret value (single line, <=8KB)' },
            { in: 'body', name: 'overwrite', type: 'boolean', required: false, desc: 'Defaults to false; set true to replace existing entries.' }
          ],
          example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/secrets -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"key":"OPENAI_API_KEY","value":"sk-***","overwrite":true}'`,
          responses: [
            { status: 200, body: `{"key":"OPENAI_API_KEY","overwrite":true}` },
            { status: 409, body: `{"message":"Secret OPENAI_API_KEY already exists (set overwrite=true to replace)"}` }
          ]
        },
        {
          method: 'POST',
          path: '/api/v0/sandboxes/{id}/repo',
          auth: 'bearer',
          desc: 'Clone a git repository (public or private) into /sandbox/repo, replacing any existing checkout.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
            { in: 'body', name: 'repo_url', type: 'string', required: true, desc: 'Git remote (https://... or git@...)' },
            { in: 'body', name: 'branch', type: 'string', required: false, desc: 'Optional branch/ref (defaults to remote HEAD).' },
            { in: 'body', name: 'auth', type: 'object', required: false, desc: 'Either `{ "type":"https_token","token":"..." }` or `{ "type":"ssh_key","private_key":"..." }`.' }
          ],
          example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/repo -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"repo_url":"https://github.com/octocat/Hello-World.git","branch":"main"}'`,
          responses: [
            { status: 200, body: `{"path":"repo","repo_url":"https://github.com/octocat/Hello-World.git","branch":"main","commit":"<sha>"}` },
            { status: 403, body: `{"message":"Authentication failed while cloning repository"}` }
          ]
        },
        {
          method: 'GET',
          path: '/api/v0/sandboxes/{id}/repo',
          auth: 'bearer',
          desc: 'List git repositories detected in the sandbox (currently /sandbox/repo).',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' }
          ],
          example: `curl -s ${BASE}/api/v0/sandboxes/<id>/repo -H "Authorization: Bearer <token>"`,
          responses: [
            { status: 200, body: `{"repos":[{"path":"repo","branch":"main","commit":"<sha>","repo_url":"https://github.com/octocat/Hello-World.git"}]}` }
          ]
        },
        {
          method: 'GET',
          path: '/api/v0/sandboxes/{id}/tasks',
          auth: 'bearer',
          desc: 'List tasks for a sandbox in chronological order. Inputs include `file_reference` items when a request mentions files with `@path/to/file`.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
            { in: 'query', name: 'limit', type: 'int', required: false, desc: 'Max records (default 100, max 1000).' },
            { in: 'query', name: 'offset', type: 'int', required: false, desc: 'Row offset (default 0).' }
          ],
          example: `curl -s ${BASE}/api/v0/sandboxes/<id>/tasks?limit=20 -H "Authorization: Bearer <token>"`,
          resp: { schema: 'TaskObject', array: true },
          responses: [
            { status: 200, body: `[{"id":"task_123","sandbox_id":"<id>","status":"completed","task_type":"NL","input":[{"type":"text","content":"Review the attached notes "},{"type":"file_reference","path":"docs/notes.md","display":"@docs/notes.md"}],"steps":[{"type":"final","channel":"final","text":"hello"}],"output":[{"type":"md","content":"hello"}],"context_length":2048,"timeout_seconds":600,"timeout_at":"2025-01-01T12:10:00Z","created_at":"2025-01-01T12:00:00Z","updated_at":"2025-01-01T12:00:10Z"}]` }
          ]
        },
        {
          method: 'POST',
          path: '/api/v0/sandboxes/{id}/tasks',
          auth: 'bearer',
          desc: 'Create a task (enqueue user input). Calls block until completion by default; set background=true to return immediately. Include `@relative/path` inside the text body (or send `file_reference` items directly) to mention files you uploaded via the Files API.',
          params: [
            { in: 'path', name: 'id', type: 'string', required: true, desc: 'Sandbox ID (UUID)' },
            { in: 'body', name: 'input', type: 'object', required: true, desc: "User input JSON; preferred shape { content: [{ type: 'text', content: string }] }. Use @relative/path in the text to automatically attach file_reference items or send { type: 'file_reference', path: 'docs/plan.md' } yourself." },
            { in: 'body', name: 'task_type', type: "'NL'|'SH'|'PY'|'JS'", required: false, desc: "Task type: 'NL' (natural language/inference, default), 'SH' (shell), 'PY' (Python), 'JS' (JavaScript)." },
            { in: 'body', name: 'background', type: 'boolean', required: false, desc: 'Defaults to false (blocking). Set true to enqueue asynchronously.' },
            { in: 'body', name: 'timeout_seconds', type: 'int|null', required: false, desc: 'Per-task timeout seconds (defaults to 300, 0 to disable).' }
          ],
          example: `curl -s -X POST ${BASE}/api/v0/sandboxes/<id>/tasks -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"input":{"content":[{"type":"text","content":"hello"}]},"task_type":"NL"}'`,
          resp: { schema: 'TaskObject' },
          responses: [
            { status: 200, body: `{"id":"task_123","sandbox_id":"<id>","status":"completed","task_type":"NL","input":[{"type":"text","content":"Review the uploaded log "},{"type":"file_reference","path":"logs/build.log","display":"@logs/build.log"}],"steps":[{"type":"final","channel":"final","text":"hi there"}],"output":[{"type":"md","content":"hi there"}],"context_length":2048,"timeout_seconds":600,"timeout_at":"2025-01-01T12:10:00Z","created_at":"2025-01-01T12:00:00Z","updated_at":"2025-01-01T12:00:10Z"}` },
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
            { status: 200, body: `{"id":"task_123","sandbox_id":"<id>","status":"processing","task_type":"NL","input":[{"type":"text","content":"hi"}],"steps":[],"output":[],"context_length":1024,"timeout_seconds":600,"timeout_at":"2025-01-01T12:10:00Z","created_at":"2025-01-01T12:00:00Z","updated_at":"2025-01-01T12:05:00Z"}` }
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
            { in: 'body', name: 'status', type: "'queued'|'processing'|'completed'|'failed'|'cancelled'", required: false, desc: 'Status update.' },
            { in: 'body', name: 'input', type: 'object', required: false, desc: 'Optional input update (replaces existing input JSON).' },
            { in: 'body', name: 'output', type: 'object', required: false, desc: 'Output update; merges text/items into existing output.' },
            { in: 'body', name: 'timeout_seconds', type: 'int|null', required: false, desc: 'Reset per-task timeout (<=0 clears).' },
            { in: 'body', name: 'context_length', type: 'int|null', required: false, desc: 'Update latest context length (tokens).' }
          ],
          example: `curl -s -X PUT ${BASE}/api/v0/sandboxes/<id>/tasks/<task_id> -H "Authorization: Bearer <token>" -H "Content-Type: application/json" -d '{"status":"completed","output":{"text":"done"}}'`,
          resp: { schema: 'TaskObject' },
          responses: [
            { status: 200, body: `{"id":"task_123","sandbox_id":"<id>","status":"completed","task_type":"NL","input":[{"type":"text","content":"hello"}],"steps":[{"type":"final","channel":"final","text":"done"}],"output":[{"type":"md","content":"done"}],"context_length":3072,"timeout_seconds":null,"timeout_at":null,"created_at":"2025-01-01T12:00:00Z","updated_at":"2025-01-01T12:05:00Z"}` }
          ]
        },
        {
          method: 'POST',
          path: '/api/v0/sandboxes/{id}/tasks/{task_id}/cancel',
          auth: 'bearer',
          desc: 'Cancel a specific queued or processing task. If the task is already complete, returns HTTP 409.',
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
            { status: 200, body: `{"items":[{"id":"snp_123","sandbox_id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","trigger_type":"manual","created_at":"2025-01-01T15:00:00Z","metadata":{}}],"total":1,"limit":100,"offset":0,"page":1,"pages":1}` }
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
            { status: 200, body: `{"id":"snp_123","sandbox_id":"fa36e542-b9b8-11f0-aadd-064ac08387fc","trigger_type":"manual","created_at":"2025-01-01T15:00:00Z","metadata":{}}` }
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
            { status: 200, body: `{"id":"new_sandbox","created_by":"admin","state":"initializing","description":"Restored","snapshot_id":"<id>","created_at":"2025-01-01T16:00:00Z","last_activity_at":null,"metadata":{},"tags":[],"idle_timeout_seconds":900,"idle_from":null,"busy_from":null}` }
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
      ]
    },
  ];
}


export function methodClass(method) {
  switch ((method || '').toUpperCase()) {
    case 'GET': return 'badge bg-success';
    case 'POST': return 'badge bg-theme text-white';
    case 'PUT': return 'badge bg-warning text-dark';
    case 'DELETE': return 'badge bg-danger';
    case 'PATCH': return 'badge bg-info text-dark';
    default: return 'badge bg-secondary';
  }
}
