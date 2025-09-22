import { writable } from 'svelte/store';

const menus = [
  { 'text': 'Navigation', 'is_header': true },
  { 'url': '/', 'icon': 'bi bi-house-door', 'text': 'Home' },
  { 'url': '/docs', 'icon': 'bi bi-journal-text', 'text': 'Documentation' },
  { 'url': '/logout', 'icon': 'bi bi-box-arrow-right', 'text': 'Logout' }
];

// Create a writable store with the initial options
export const appSidebarMenus = writable(menus);
