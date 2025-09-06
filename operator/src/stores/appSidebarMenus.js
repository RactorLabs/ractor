import { writable } from 'svelte/store';

const menus = [{
	'text': 'Navigation',
	'is_header': true
},{
	'url': '/',
	'icon': 'bi bi-house-door',
	'text': 'Home'
}];

// Create a writable store with the initial options
export const appSidebarMenus = writable(menus);