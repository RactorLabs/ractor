// API documentation data source for Raworc UI
// Covers endpoints defined in src/server/rest/routes.rs
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
        desc: 'Get API version and current version string.',
        params: [],
        example: `curl -s ${BASE}/api/v0/version`
      }
    ]
  },
  {
    id: 'auth',
    title: 'Authentication',
    description: 'Login and token management for Operators.',
    endpoints: [
      {
        method: 'POST',
        path: '/api/v0/operators/{name}/login',
        auth: 'public',
        desc: 'Login with operator name and password. Returns JWT token and user info.',
        params: [
          { in: 'path', name: 'name', type: 'string', required: true, desc: 'Operator username' },
          { in: 'body', name: 'pass', type: 'string', required: true, desc: 'Operator password' }
        ],
        example: `curl -s -X POST ${BASE}/api/v0/operators/admin/login -H "Content-Type: application/json" -d '{"pass":"admin"}'`
      },
      {
        method: 'GET',
        path: '/api/v0/auth',
        auth: 'bearer',
        desc: 'Get authenticated operator profile (validate token).',
        params: [],
        example: `curl -s ${BASE}/api/v0/auth -H "Authorization: Bearer <token>"`
      },
      {
        method: 'POST',
        path: '/api/v0/auth/token',
        auth: 'bearer',
        desc: 'Create a new token for a principal (admin-only).',
        params: [
          { in: 'body', name: 'principal', type: 'string', required: true, desc: 'Principal name (user or operator id)' },
          { in: 'body', name: 'type', type: 'string', required: true, desc: "Principal type: 'User' or 'Operator'" }
        ],
        example: `curl -s -X POST ${BASE}/api/v0/auth/token -H "Authorization: Bearer <token>"`
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
        example: `curl -s ${BASE}/api/v0/published/agents`
      },
      {
        method: 'GET',
        path: '/api/v0/published/agents/{name}',
        auth: 'public',
        desc: 'Get details of a published agent by name.',
        params: [
          { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' }
        ],
        example: `curl -s ${BASE}/api/v0/published/agents/<name>`
      }
    ]
  },
  {
    id: 'operators',
    title: 'Operators',
    description: 'Operator management endpoints (protected).',
    endpoints: [
      { method: 'GET', path: '/api/v0/operators', auth: 'bearer', desc: 'List operators.', params: [] },
      { method: 'POST', path: '/api/v0/operators', auth: 'bearer', desc: 'Create operator.', params: [
        { in: 'body', name: 'user', type: 'string', required: true, desc: 'Operator username' },
        { in: 'body', name: 'pass', type: 'string', required: true, desc: 'Password' },
        { in: 'body', name: 'description', type: 'string', required: false, desc: 'Optional description' }
      ] },
      { method: 'GET', path: '/api/v0/operators/{name}', auth: 'bearer', desc: 'Get operator.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Operator username' }
      ] },
      { method: 'PUT', path: '/api/v0/operators/{name}', auth: 'bearer', desc: 'Update operator.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Operator username' },
        { in: 'body', name: 'description', type: 'string', required: false, desc: 'Optional description' },
        { in: 'body', name: 'active', type: 'boolean|null', required: false, desc: 'Set active status; must be boolean or null' }
      ] },
      { method: 'DELETE', path: '/api/v0/operators/{name}', auth: 'bearer', desc: 'Delete operator.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Operator username' }
      ] },
      { method: 'PUT', path: '/api/v0/operators/{name}/password', auth: 'bearer', desc: 'Update operator password.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Operator username' },
        { in: 'body', name: 'current_password', type: 'string', required: true, desc: 'Current password' },
        { in: 'body', name: 'new_password', type: 'string', required: true, desc: 'New password' }
      ] }
    ]
  },
  {
    id: 'agents',
    title: 'Agents',
    description: 'Agent lifecycle and management endpoints (protected).',
    endpoints: [
      { method: 'GET', path: '/api/v0/agents', auth: 'bearer', desc: 'List agents.', params: [
        { in: 'query', name: 'state', type: 'string', required: false, desc: 'Filter by state (e.g., init|idle|busy|slept)' }
      ] },
      { method: 'POST', path: '/api/v0/agents', auth: 'bearer', desc: 'Create agent.', params: [
        { in: 'body', name: 'name', type: 'string', required: true, desc: 'Agent name; must match ^[A-Za-z][A-Za-z0-9-]{0,61}[A-Za-z0-9]$' },
        { in: 'body', name: 'description', type: 'string|null', required: false, desc: 'Optional human-readable description' },
        { in: 'body', name: 'metadata', type: 'object', required: false, desc: 'Arbitrary JSON metadata (default: {})' },
        { in: 'body', name: 'secrets', type: 'object<string,string>', required: false, desc: 'Key/value secrets map (default: empty)' },
        { in: 'body', name: 'instructions', type: 'string|null', required: false, desc: 'Optional instructions' },
        { in: 'body', name: 'setup', type: 'string|null', required: false, desc: 'Optional setup script or commands' },
        { in: 'body', name: 'prompt', type: 'string|null', required: false, desc: 'Optional initial prompt' },
        { in: 'body', name: 'idle_timeout_seconds', type: 'int|null', required: false, desc: 'Idle timeout seconds (default 300)' },
        { in: 'body', name: 'busy_timeout_seconds', type: 'int|null', required: false, desc: 'Busy timeout seconds (default 900)' }
      ] },
      { method: 'GET', path: '/api/v0/agents/{name}', auth: 'bearer', desc: 'Get agent by name.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' }
      ] },
      { method: 'PUT', path: '/api/v0/agents/{name}', auth: 'bearer', desc: 'Update agent by name.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' },
        { in: 'body', name: 'metadata', type: 'object|null', required: false, desc: 'Replace metadata (omit to keep)' },
        { in: 'body', name: 'description', type: 'string|null', required: false, desc: 'Update description' },
        { in: 'body', name: 'idle_timeout_seconds', type: 'int|null', required: false, desc: 'Update idle timeout seconds' },
        { in: 'body', name: 'busy_timeout_seconds', type: 'int|null', required: false, desc: 'Update busy timeout seconds' }
      ] },
      { method: 'PUT', path: '/api/v0/agents/{name}/state', auth: 'bearer', desc: 'Update agent state (generic).', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' },
        { in: 'body', name: 'state', type: 'string', required: true, desc: 'New state (e.g., init|idle|busy|slept)' },
        { in: 'body', name: 'content_port', type: 'int|null', required: false, desc: 'Optional port used by agent content server' }
      ] },
      { method: 'POST', path: '/api/v0/agents/{name}/busy', auth: 'bearer', desc: 'Set agent busy.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' }
      ] },
      { method: 'POST', path: '/api/v0/agents/{name}/idle', auth: 'bearer', desc: 'Set agent idle.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' }
      ] },
      { method: 'POST', path: '/api/v0/agents/{name}/sleep', auth: 'bearer', desc: 'Put agent to sleep.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' }
      ] },
      { method: 'POST', path: '/api/v0/agents/{name}/wake', auth: 'bearer', desc: 'Wake agent.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' },
        { in: 'body', name: 'prompt', type: 'string|null', required: false, desc: 'Optional prompt to send on wake' }
      ] },
      { method: 'POST', path: '/api/v0/agents/{name}/remix', auth: 'bearer', desc: 'Remix agent (create a new agent from parent).', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Parent agent name' },
        { in: 'body', name: 'name', type: 'string', required: true, desc: 'New agent name; must match ^[A-Za-z][A-Za-z0-9-]{0,61}[A-Za-z0-9]$' },
        { in: 'body', name: 'metadata', type: 'object|null', required: false, desc: 'Optional metadata override' },
        { in: 'body', name: 'code', type: 'boolean', required: false, desc: 'Copy code (default true)' },
        { in: 'body', name: 'secrets', type: 'boolean', required: false, desc: 'Copy secrets (default true)' },
        { in: 'body', name: 'content', type: 'boolean', required: false, desc: 'Copy content (always true in v0.4.0+)' },
        { in: 'body', name: 'prompt', type: 'string|null', required: false, desc: 'Optional initial prompt' }
      ] },
      { method: 'POST', path: '/api/v0/agents/{name}/publish', auth: 'bearer', desc: 'Publish agent.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' },
        { in: 'body', name: 'code', type: 'boolean', required: false, desc: 'Allow code remix (default true)' },
        { in: 'body', name: 'secrets', type: 'boolean', required: false, desc: 'Allow secrets remix (default true)' },
        { in: 'body', name: 'content', type: 'boolean', required: false, desc: 'Publish content (default true)' }
      ] },
      { method: 'POST', path: '/api/v0/agents/{name}/unpublish', auth: 'bearer', desc: 'Unpublish agent.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' }
      ] },
      { method: 'DELETE', path: '/api/v0/agents/{name}', auth: 'bearer', desc: 'Delete agent.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' }
      ] }
    ]
  },
  {
    id: 'messages',
    title: 'Agent Messages',
    description: 'Send and retrieve messages for a given agent (protected).',
    endpoints: [
      { method: 'GET', path: '/api/v0/agents/{name}/messages', auth: 'bearer', desc: 'List messages for agent.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' },
        { in: 'query', name: 'limit', type: 'int', required: false, desc: 'Max messages (0..1000, default 100)' },
        { in: 'query', name: 'offset', type: 'int', required: false, desc: 'Offset for pagination (default 0)' }
      ] },
      { method: 'POST', path: '/api/v0/agents/{name}/messages', auth: 'bearer', desc: 'Create a message for agent.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' },
        { in: 'body', name: 'role', type: "'user'|'agent'|'system'", required: false, desc: "Message role (default 'user')" },
        { in: 'body', name: 'content', type: 'string', required: true, desc: 'Message text content' },
        { in: 'body', name: 'metadata', type: 'object', required: false, desc: 'Arbitrary JSON metadata (default: {})' }
      ] },
      { method: 'GET', path: '/api/v0/agents/{name}/messages/count', auth: 'bearer', desc: 'Get message count for agent.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' }
      ] },
      { method: 'DELETE', path: '/api/v0/agents/{name}/messages', auth: 'bearer', desc: 'Clear messages for agent.', params: [
        { in: 'path', name: 'name', type: 'string', required: true, desc: 'Agent name' }
      ] }
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
