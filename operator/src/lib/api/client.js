import { getToken } from '$lib/auth.js';

function ensureApiPath(path) {
  if (path.startsWith('/api/v0')) return path;
  if (path.startsWith('/')) return `/api/v0${path}`;
  return `/api/v0/${path}`;
}

// opts: { noAutoLogout?: boolean }
export async function apiFetch(path, options = {}, opts = {}) {
  const url = ensureApiPath(path);
  const token = getToken();

  const headers = new Headers(options.headers || {});
  headers.set('Content-Type', 'application/json');
  if (token) headers.set('Authorization', `Bearer ${token}`);

  const res = await fetch(url, { ...options, headers });
  let data = null;
  try { data = await res.json(); } catch (_) { /* ignore */ }
  // Centralized auth failure handling to prevent redirect loops
  if (!opts?.noAutoLogout && (res.status === 401 || res.status === 403)) {
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
