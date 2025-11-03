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
  import { page } from '$app/stores';
  import { getHostName, getHostUrl } from '$lib/branding.js';
  export let data;

  onMount(async () => {
    import('bootstrap');
    document.querySelector('body').classList.add('app-init');
    // Expose host name and host URL for client-side usage
    if (typeof window !== 'undefined') {
      window.__TSBX_HOST_NAME__ = (data && data.hostName) ? data.hostName : getHostName();
      window.__TSBX_HOST_URL__ = (data && data.hostUrl) ? data.hostUrl : getHostUrl();
    }

    // Disable auto-capitalization in all text inputs/textareas/contenteditable fields
    const applyNoAutoCaps = (root) => {
      const scope = root && root.querySelectorAll ? root : document;
      const nodes = scope.querySelectorAll('input, textarea, [contenteditable], [contenteditable="true"], [contenteditable=""]');
      nodes.forEach((el) => {
        try { el.setAttribute('autocapitalize', 'none'); } catch (_) {}
      });
    };
    applyNoAutoCaps(document);
    // Watch for dynamic DOM updates and enforce attribute
    const observer = new MutationObserver((mutations) => {
      for (const m of mutations) {
        m.addedNodes && m.addedNodes.forEach((n) => {
          if (n && n.nodeType === 1) { // ELEMENT_NODE
            applyNoAutoCaps(n);
          }
        });
      }
    });
    try { observer.observe(document.body, { childList: true, subtree: true }); } catch (_) {}
    
    // Set dynamic viewport unit for iOS Safari and older browsers
    const setViewportUnit = () => {
      const vh = window.innerHeight * 0.01;
      document.documentElement.style.setProperty('--app-vh', `${vh}px`);
    };
    setViewportUnit();
    window.addEventListener('resize', setViewportUnit);
    window.addEventListener('orientationchange', setViewportUnit);

    // Sync theme color with header background for iOS Safari top bar
    const setThemeColor = () => {
      const root = document.documentElement;
      const cs = getComputedStyle(root);
      // Try app header bg var, then body bg
      const headerBg = cs.getPropertyValue('--bs-app-header-bg') || cs.getPropertyValue('--bs-body-bg');
      const color = (headerBg && headerBg.trim().length) ? headerBg.trim() : '#111111';
      let meta = document.querySelector('meta[name="theme-color"]');
      if (!meta) {
        meta = document.createElement('meta');
        meta.setAttribute('name', 'theme-color');
        document.head.appendChild(meta);
      }
      meta.setAttribute('content', color);
    };
    setThemeColor();

    $appVariables = generateVariables();
    // Always keep sidebar minimized
    $appOptions.appSidebarCollapsed = true;
    $appOptions.appSidebarToggled = false;
  });

  // Keep sidebar minimized on navigation
  $: (async () => {
    const _ = $page.url.pathname; // react to route changes
    $appOptions.appSidebarCollapsed = true;
    $appOptions.appSidebarToggled = false;
    // Re-evaluate theme color on navigation (theme may change per route/theme panel)
    if (typeof window !== 'undefined') {
      try { const evt = new Event('resize'); window.dispatchEvent(evt); } catch {}
      try {
        const cs = getComputedStyle(document.documentElement);
        const headerBg = cs.getPropertyValue('--bs-app-header-bg') || cs.getPropertyValue('--bs-body-bg');
        const color = (headerBg && headerBg.trim().length) ? headerBg.trim() : '#111111';
        let meta = document.querySelector('meta[name="theme-color"]');
        if (!meta) { meta = document.createElement('meta'); meta.setAttribute('name', 'theme-color'); document.head.appendChild(meta); }
        meta.setAttribute('content', color);
      } catch {}
    }
  })();
</script>

<svelte:head>
  <!-- Use SSR-provided hostName for hydration stability -->
  <title>{(data && data.hostName) ? data.hostName : getHostName()}</title>
  <meta name="application-name" content={(data && data.hostName) ? data.hostName : getHostName()}>
</svelte:head>

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
  {#if !$appOptions.appHeaderHide}<AppHeader hostName={(data && data.hostName) ? data.hostName : getHostName()} />{/if}
	{#if !$appOptions.appSidebarHide}<AppSidebar />{/if}
	{#if $appOptions.appTopNav}<AppTopNav />{/if}
	<AppThemePanel />
	
  <div id="content" class="app-content{($appOptions.appContentClass) ? ' '+ $appOptions.appContentClass : ''}">
		<slot />
	</div>
	
  {#if $appOptions.appFooter}<AppFooter hostName={(data && data.hostName) ? data.hostName : getHostName()} />{/if}
</div>
