import { writable } from 'svelte/store';

const menus = [{
  'url': '/login',
  'icon': 'bi bi-box-arrow-in-right',
  'text': 'Login'
}, {
  'url': '/docs',
  'icon': 'bi bi-journal-text',
  'text': 'API Reference'
}];

// Create a writable store with the initial options
export const appTopNavMenus = writable(menus);
