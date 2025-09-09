export function getHostName() {
  // Prefer runtime-injected global set by layout
  if (typeof window !== 'undefined' && window.__RAWORC_HOST_NAME__) {
    return window.__RAWORC_HOST_NAME__;
  }
  // Try server-side env during SSR (non-public)
  if (typeof process !== 'undefined' && process.env && process.env.RAWORC_HOST_NAME) {
    return process.env.RAWORC_HOST_NAME;
  }
  return 'Raworc';
}

