<script>
  import { appOptions } from '../../stores/appOptions.js';
  import { appSidebarMenus } from '../../stores/appSidebarMenus.js';
  import { onMount } from 'svelte';
  import { auth, initAuthFromCookies } from '$lib/auth.js';
  import { getHostName } from '$lib/branding.js';
  
  export let hostName = '';
  $: resolvedHostName = hostName || getHostName();
  let operatorName = '';
  $: operatorName = $auth?.name && $auth.name.trim().length > 0 ? $auth.name : '';
  $: isAdmin = $auth && String($auth.type || '').toLowerCase() === 'admin';
  onMount(() => {
    initAuthFromCookies();
  });
  
  function desktopToggler() {
		$appOptions.appSidebarToggled = ($appOptions.appSidebarCollapsed == false) ? false : true;
		$appOptions.appSidebarCollapsed = ($appOptions.appSidebarCollapsed == false) ? true : false;
  }
  
  function mobileToggler() {
  	$appOptions.appSidebarMobileToggled = !$appOptions.appSidebarMobileToggled;
  }
</script>

<!-- BEGIN #header -->
<div id="header" class="app-header">
	<!-- BEGIN desktop-toggler -->
	<div class="desktop-toggler">
		<button type="button" class="menu-toggler" aria-label="Desktop Toggler" onclick={desktopToggler}>
			<span class="bar"></span>
			<span class="bar"></span>
			<span class="bar"></span>
		</button>
	</div>
	<!-- BEGIN desktop-toggler -->
	
	<!-- BEGIN mobile-toggler -->
	<div class="mobile-toggler">
		<button type="button" class="menu-toggler" aria-label="Mobile Toggler" onclick={mobileToggler}>
			<span class="bar"></span>
			<span class="bar"></span>
			<span class="bar"></span>
		</button>
	</div>
	<!-- END mobile-toggler -->
	
	<!-- BEGIN brand -->
	<div class="brand">
		<a href="/" aria-label="link" class="brand-logo">
            <span class="brand-text">{resolvedHostName}</span>
		</a>
	</div>
	<!-- END brand -->
	
	<!-- BEGIN menu -->
	<div class="menu">
        <div class="menu-item dropdown dropdown-mobile-full">
          <!-- App grid with fixed items -->
          <a href="#/" aria-label="Apps" data-bs-toggle="dropdown" data-bs-display="static" class="menu-link">
            <div class="menu-icon"><i class="bi bi-grid-3x3-gap nav-icon"></i></div>
          </a>
          <div class="dropdown-menu fade dropdown-menu-end w-300px text-center p-0 mt-1">
            <!-- Top row -->
            <div class="row row-grid gx-0">
              <div class="col-4">
                <a href="/sessions/start" aria-label="Start Session" class="dropdown-item text-decoration-none p-3 bg-none">
                  <div><i class="bi bi-plus-circle h2 opacity-5 d-block my-1"></i></div>
                  <div class="fw-500 fs-10px text-inverse">START</div>
                </a>
              </div>
              <div class="col-4">
                <a href="/sessions" aria-label="Sessions" class="dropdown-item text-decoration-none p-3 bg-none">
                  <div><i class="bi bi-robot h2 opacity-5 d-block my-1"></i></div>
                  <div class="fw-500 fs-10px text-inverse">SESSIONS</div>
                </a>
              </div>
              <div class="col-4">
                <a href="/docs" aria-label="Docs" class="dropdown-item text-decoration-none p-3 bg-none">
                  <div><i class="bi bi-journal-text h2 opacity-5 d-block my-1"></i></div>
                  <div class="fw-500 fs-10px text-inverse">DOCS</div>
                </a>
              </div>
            </div>
            <!-- (admin items moved to profile menu) -->
          </div>
        </div>

        <div class="menu-item dropdown dropdown-mobile-full">
          {#if $auth && $auth.token}
            <a href="#/" aria-label="link" data-bs-toggle="dropdown" data-bs-display="static" class="menu-link">
              <span class="menu-img"><img src="/img/avatar.svg" alt="avatar" /></span>
              <div class="menu-text d-sm-block d-none fw-500">{operatorName}</div>
            </a>
            <div class="dropdown-menu dropdown-menu-end me-lg-3 fs-11px mt-1">
              {#if isAdmin}
                <a aria-label="Admin APIs" class="dropdown-item d-flex align-items-center justify-content-between gap-2" href="/docs/admin">
                  <span>ADMIN APIs</span>
                  <i class="bi bi-shield-lock text-theme fs-16px my-n1"></i>
                </a>
                <a aria-label="Create Tokens" class="dropdown-item d-flex align-items-center justify-content-between gap-2" href="/tokens">
                  <span>CREATE TOKENS</span>
                  <i class="bi bi-key text-theme fs-16px my-n1"></i>
                </a>
                <a aria-label="Change Password" class="dropdown-item d-flex align-items-center justify-content-between gap-2" href="/profile/password">
                  <span>CHANGE PASSWORD</span>
                  <i class="bi bi-key text-theme fs-16px my-n1"></i>
                </a>
                <div class="dropdown-divider"></div>
              {/if}
              <a aria-label="link" class="dropdown-item d-flex align-items-center justify-content-between gap-2" href="/logout" data-sveltekit-reload onclick={() => { import('$lib/auth.js').then(m => m.logoutClientSide()).catch(()=>{}); }}>
                <span>LOGOUT</span>
                <i class="bi bi-toggle-off text-theme fs-16px my-n1"></i>
              </a>
            </div>
          {:else}
            <a href="/login" aria-label="link" class="menu-link">
              <span class="menu-img"><img src="/img/avatar.svg" alt="avatar" /></span>
              <div class="menu-text d-sm-block d-none fw-500">Login</div>
            </a>
          {/if}
        </div>
	</div>
	<!-- END menu -->

</div>
<!-- END #header -->
