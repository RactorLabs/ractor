import { getHostName } from '$lib/branding.js';

export function setPageTitle(title) {
  if (typeof document !== 'undefined') {
    const name = getHostName();
    document.title = `${name} | ${title}`;
  }
}
