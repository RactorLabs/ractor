import { getToken } from '$lib/auth.js';

function ensureApiPath(path) {
  if (path.startsWith('/api/v0')) return path;
  if (path.startsWith('/')) return `/api/v0${path}`;
  return `/api/v0/${path}`;
}

export async function apiFetch(path, options = {}) {
  const url = ensureApiPath(path);
  const token = getToken();

  const headers = new Headers(options.headers || {});
  headers.set('Content-Type', 'application/json');
  if (token) headers.set('Authorization', `Bearer ${token}`);

  const res = await fetch(url, { ...options, headers });
  let data = null;
  try { data = await res.json(); } catch (_) { /* ignore */ }
  return { ok: res.ok, status: res.status, data };
}

