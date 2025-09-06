import { writable } from 'svelte/store';

const menus = [{
	'text': 'Navigation',
	'is_header': true
},{
	'url': '/',
	'icon': 'bi bi-house-door',
	'text': 'Home'
}, {
	'is_divider': true
}, {
	'text': 'Documentation',
	'is_header': true
}, {
	'url': '/docs',
	'icon': 'bi bi-journal-text',
	'text': 'API Reference'
}];

// Create a writable store with the initial options
export const appSidebarMenus = writable(menus);
