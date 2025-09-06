import { writable } from 'svelte/store';

const menus = [
  { 'text': 'Navigation', 'is_header': true },
  { 'url': '/agents', 'icon': 'bi bi-robot', 'text': 'Agents' },
  { 'is_divider': true },
  { 'url': '/docs', 'icon': 'bi bi-journal-text', 'text': 'Documentation' }
];

// Create a writable store with the initial options
export const appSidebarMenus = writable(menus);
