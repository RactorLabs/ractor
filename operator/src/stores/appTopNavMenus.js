import { writable } from 'svelte/store';

const menus = [
  { 'url': '/', 'icon': 'bi bi-house-door', 'text': 'Home' },
  { 'url': '/sandboxes', 'icon': 'bi bi-robot', 'text': 'Sandboxes' },
  { 'url': '/docs', 'icon': 'bi bi-journal-text', 'text': 'Documentation' }
];

// Create a writable store with the initial options
export const appTopNavMenus = writable(menus);
