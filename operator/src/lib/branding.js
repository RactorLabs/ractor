export function getHostName() {
  // Prefer runtime-injected global set by layout
  if (typeof window !== 'undefined' && window.__TSBX_HOST_NAME__) {
    return window.__TSBX_HOST_NAME__;
  }
  // Try server-side env during SSR (non-public)
  if (typeof process !== 'undefined' && process.env && process.env.TSBX_HOST_NAME) {
    return process.env.TSBX_HOST_NAME;
  }
  return 'TSBX';
}

export function getHostUrl() {
  // Prefer runtime-injected global set by layout (gateway root)
  if (typeof window !== 'undefined' && window.__TSBX_HOST_URL__) {
    const url = String(window.__TSBX_HOST_URL__ || '').replace(/\/$/, '');
    return url || 'http://localhost';
  }
  // Try server-side env during SSR
  if (typeof process !== 'undefined' && process.env && process.env.TSBX_HOST_URL) {
    return String(process.env.TSBX_HOST_URL).replace(/\/$/, '');
  }
  // Final fallback
  return 'http://localhost';
}
