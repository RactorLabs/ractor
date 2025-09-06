import { writable } from 'svelte/store';

const menus = [
  { 'text': 'Navigation', 'is_header': true },
  { 'url': '/agents', 'icon': 'bi bi-robot', 'text': 'Agents' },
  { 'url': '/agents/create', 'icon': 'bi bi-plus-circle', 'text': 'Create Agent' },
  { 'url': '/playground', 'icon': 'bi bi-joystick', 'text': 'API Playground' },
  { 'is_divider': true },
  { 'url': '/docs', 'icon': 'bi bi-journal-text', 'text': 'Documentation' }
];

// Create a writable store with the initial options
export const appSidebarMenus = writable(menus);
