import { getHostName } from '$lib/branding.js';

export function setPageTitle(title) {
  if (typeof document !== 'undefined') {
    // Prefer runtime global; else SSR-injected meta; else branding fallback
    let name = '';
    try {
      if (typeof window !== 'undefined' && window.__TSBX_HOST_NAME__) {
        name = window.__TSBX_HOST_NAME__;
      }
    } catch (_) {}
    if (!name) {
      const meta = document.querySelector('meta[name="application-name"]');
      if (meta && meta.getAttribute('content')) name = meta.getAttribute('content');
    }
    if (!name) name = getHostName();
    document.title = `${name} | ${title}`;
  }
}
