<script>
	import '/src/scss/styles.scss';
	import 'bootstrap-icons/font/bootstrap-icons.min.css';
	import '@fortawesome/fontawesome-free/css/all.min.css';
	import 'perfect-scrollbar/css/perfect-scrollbar.css';
	
	import AppHeader from '/src/components/app/AppHeader.svelte';
	import AppSidebar from '/src/components/app/AppSidebar.svelte';
	import AppTopNav from '/src/components/app/AppTopNav.svelte';
	import AppFooter from '/src/components/app/AppFooter.svelte';
	import AppThemePanel from '/src/components/app/AppThemePanel.svelte';
	import { onMount } from 'svelte';
  import { appOptions } from '/src/stores/appOptions.js';
  import { appVariables, generateVariables } from '/src/stores/appVariables.js';
  import { setPageTitle } from '$lib/utils';
	
	onMount(async () => {
		import('bootstrap');
		document.querySelector('body').classList.add('app-init');
		
		$appVariables = generateVariables();
	});
</script>

<div id="app" class="app" 
	class:app-header-menu-search-toggled={$appOptions.appHeaderSearchToggled}
	class:app-sidebar-toggled={$appOptions.appSidebarToggled && !$appOptions.appSidebarHide}
	class:app-sidebar-collapsed={$appOptions.appSidebarCollapsed && !$appOptions.appSidebarHide}
	class:app-sidebar-mobile-toggled={$appOptions.appSidebarMobileToggled}
	class:app-sidebar-mobile-closed={$appOptions.appSidebarMobileClosed}
	class:app-content-full-height={$appOptions.appContentFullHeight}
	class:app-content-full-width={$appOptions.appContentFullWidth}
	class:app-without-sidebar={$appOptions.appSidebarHide}
	class:app-without-header={$appOptions.appHeaderHide}
	class:app-boxed-layout={$appOptions.appBoxedLayout}
	class:app-with-top-nav={$appOptions.appTopNav}
	class:app-footer-fixed={$appOptions.appFooterFixed}
>
	{#if !$appOptions.appHeaderHide}<AppHeader />{/if}
	{#if !$appOptions.appSidebarHide}<AppSidebar />{/if}
	{#if $appOptions.appTopNav}<AppTopNav />{/if}
	<AppThemePanel />
	
	<div id="content" class="app-content{($appOptions.appContentClass) ? ' '+ $appOptions.appContentClass : ''}">
		<slot />
	</div>
	
	{#if $appOptions.appFooter}<AppFooter />{/if}
</div>