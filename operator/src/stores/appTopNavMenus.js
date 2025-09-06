import { writable } from 'svelte/store';

const menus = [
  { 'url': '/', 'icon': 'bi bi-house-door', 'text': 'Home' },
  { 'url': '/docs', 'icon': 'bi bi-journal-text', 'text': 'Documentation' },
  { 'url': '/agents', 'icon': 'bi bi-robot', 'text': 'Agents' }
];

// Create a writable store with the initial options
export const appTopNavMenus = writable(menus);
