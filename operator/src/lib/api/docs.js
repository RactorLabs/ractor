// API documentation data source for Raworc Operator
// Covers endpoints defined in src/server/rest/routes.rs

export const apiDocs = [
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
        example: 'curl -s http://localhost:9000/api/v0/version'
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
        body: '{ "pass": "<password>" }',
        example: 'curl -s -X POST http://localhost:9000/api/v0/operators/admin/login -H "Content-Type: application/json" -d \'{"pass":"admin"}\''
      },
      {
        method: 'GET',
        path: '/api/v0/auth',
        auth: 'bearer',
        desc: 'Get authenticated operator profile (validate token).',
        example: 'curl -s http://localhost:9000/api/v0/auth -H "Authorization: Bearer <token>"'
      },
      {
        method: 'POST',
        path: '/api/v0/auth/token',
        auth: 'bearer',
        desc: 'Create a new token for the current operator (if supported).',
        example: 'curl -s -X POST http://localhost:9000/api/v0/auth/token -H "Authorization: Bearer <token>"'
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
        example: 'curl -s http://localhost:9000/api/v0/published/agents'
      },
      {
        method: 'GET',
        path: '/api/v0/published/agents/{id}',
        auth: 'public',
        desc: 'Get details of a published agent by id.',
        example: 'curl -s http://localhost:9000/api/v0/published/agents/<id>'
      }
    ]
  },
  {
    id: 'operators',
    title: 'Operators',
    description: 'Operator management endpoints (protected).',
    endpoints: [
      { method: 'GET', path: '/api/v0/operators', auth: 'bearer', desc: 'List operators.' },
      { method: 'POST', path: '/api/v0/operators', auth: 'bearer', desc: 'Create operator.' },
      { method: 'GET', path: '/api/v0/operators/{name}', auth: 'bearer', desc: 'Get operator.' },
      { method: 'PUT', path: '/api/v0/operators/{name}', auth: 'bearer', desc: 'Update operator.' },
      { method: 'DELETE', path: '/api/v0/operators/{name}', auth: 'bearer', desc: 'Delete operator.' },
      { method: 'PUT', path: '/api/v0/operators/{name}/password', auth: 'bearer', desc: 'Update operator password.' }
    ]
  },
  {
    id: 'agents',
    title: 'Agents',
    description: 'Agent lifecycle and management endpoints (protected).',
    endpoints: [
      { method: 'GET', path: '/api/v0/agents', auth: 'bearer', desc: 'List agents.' },
      { method: 'POST', path: '/api/v0/agents', auth: 'bearer', desc: 'Create agent.' },
      { method: 'GET', path: '/api/v0/agents/{id}', auth: 'bearer', desc: 'Get agent by id.' },
      { method: 'PUT', path: '/api/v0/agents/{id}', auth: 'bearer', desc: 'Update agent by id.' },
      { method: 'PUT', path: '/api/v0/agents/{id}/state', auth: 'bearer', desc: 'Update agent state (generic).' },
      { method: 'POST', path: '/api/v0/agents/{id}/busy', auth: 'bearer', desc: 'Set agent busy.' },
      { method: 'POST', path: '/api/v0/agents/{id}/idle', auth: 'bearer', desc: 'Set agent idle.' },
      { method: 'POST', path: '/api/v0/agents/{id}/sleep', auth: 'bearer', desc: 'Put agent to sleep.' },
      { method: 'POST', path: '/api/v0/agents/{id}/wake', auth: 'bearer', desc: 'Wake agent.' },
      { method: 'POST', path: '/api/v0/agents/{id}/remix', auth: 'bearer', desc: 'Remix agent.' },
      { method: 'POST', path: '/api/v0/agents/{id}/publish', auth: 'bearer', desc: 'Publish agent.' },
      { method: 'POST', path: '/api/v0/agents/{id}/unpublish', auth: 'bearer', desc: 'Unpublish agent.' },
      { method: 'DELETE', path: '/api/v0/agents/{id}', auth: 'bearer', desc: 'Delete agent.' }
    ]
  },
  {
    id: 'messages',
    title: 'Agent Messages',
    description: 'Send and retrieve messages for a given agent (protected).',
    endpoints: [
      { method: 'GET', path: '/api/v0/agents/{id}/messages', auth: 'bearer', desc: 'List messages for agent.' },
      { method: 'POST', path: '/api/v0/agents/{id}/messages', auth: 'bearer', desc: 'Create a message for agent.' },
      { method: 'GET', path: '/api/v0/agents/{id}/messages/count', auth: 'bearer', desc: 'Get message count for agent.' },
      { method: 'DELETE', path: '/api/v0/agents/{id}/messages', auth: 'bearer', desc: 'Clear messages for agent.' }
    ]
  }
];

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

