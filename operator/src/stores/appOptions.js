import { writable } from 'svelte/store';

// Initial app options
const initialOptions = {
	appBoxedLayout: false,
	appSidebarToggled: false,
	appSidebarCollapsed: false,
	appSidebarMobileToggled: false,
	appSidebarMobileClosed: false,
	appSidebarHide: false,
	appHeaderToggled: false,
	appHeaderSearchToggled: false,
	appHeaderHide: false,
	appContentFullHeight: false,
	appContentClass: '',
	appTopNav: false,
	appFooter: false,
	appFooterFixed: false,
	appThemePanelToggled: false
};

// Create a writable store with the initial options
export const appOptions = writable(initialOptions);