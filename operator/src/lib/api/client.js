import { getToken } from '$lib/auth.js';

function ensureApiPath(path) {
  if (path.startsWith('/api/v0')) return path;
  if (path.startsWith('/')) return `/api/v0${path}`;
  return `/api/v0/${path}`;
}

// opts: { noAutoLogout?: boolean }
export async function apiFetch(path, options = {}, opts = {}) {
  let url = ensureApiPath(path);
  const token = getToken();

  const headers = new Headers(options.headers || {});
  headers.set('Content-Type', 'application/json');
  if (token) headers.set('Authorization', `Bearer ${token}`);
  // Reduce chances of stale GETs by disabling cache on client requests
  const method = String(options.method || 'GET').toUpperCase();
  if (!opts?.allowCache && method === 'GET') {
    try {
      headers.set('Cache-Control', 'no-cache, no-store, max-age=0, must-revalidate');
      headers.set('Pragma', 'no-cache');
    } catch (_) {}
    const sep = url.includes('?') ? '&' : '?';
    url = `${url}${sep}_=${Date.now()}`;
  }

  const res = await fetch(url, { ...options, headers });
  let data = null;
  try { data = await res.json(); } catch (_) { /* ignore */ }
  if (!res.ok) {
    try {
      // Surface useful debug info in browser console for troubleshooting
      // eslint-disable-next-line no-console
      console.error('[apiFetch]', method, url, 'HTTP', res.status, data);
    } catch (_) {}
  }
  // Centralized auth failure handling
  // 401 = invalid/expired token: log out and redirect to login
  // 403 = insufficient permissions: keep token so UI can show an error state
  if (!opts?.noAutoLogout && res.status === 401) {
    try {
      const mod = await import('$lib/auth.js');
      mod.logoutClientSide?.();
    } catch (_) {}
    if (typeof window !== 'undefined') {
      try { window.location.replace('/login'); } catch (_) { window.location.href = '/login'; }
    }
  }
  return { ok: res.ok, status: res.status, data };
}
