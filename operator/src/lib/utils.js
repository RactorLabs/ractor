export function setPageTitle(title) {
  if (typeof document !== 'undefined') {
    document.title = `Raworc | ${title}`;
  }
}
