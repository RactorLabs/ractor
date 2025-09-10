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
  $: isOperator = $auth && $auth.type === 'Operator';
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
			<span class="brand-img">
    	<span class="brand-img-text text-theme">ர</span>
    		</span>
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
            <div class="row row-grid gx-0">
              <div class="col-4">
                <a href="/agents" aria-label="Agents" class="dropdown-item text-decoration-none p-3 bg-none">
                  <div><i class="bi bi-robot h2 opacity-5 d-block my-1"></i></div>
                  <div class="fw-500 fs-10px text-inverse">AGENTS</div>
                </a>
              </div>
              <div class="col-4">
                <a href="/playground" aria-label="Playground" class="dropdown-item text-decoration-none p-3 bg-none">
                  <div><i class="bi bi-joystick h2 opacity-5 d-block my-1"></i></div>
                  <div class="fw-500 fs-10px text-inverse">PLAYGROUND</div>
                </a>
              </div>
              {#if isOperator}
              <div class="col-4">
                <a href="/tokens" aria-label="Tokens" class="dropdown-item text-decoration-none p-3 bg-none">
                  <div><i class="bi bi-key h2 opacity-5 d-block my-1"></i></div>
                  <div class="fw-500 fs-10px text-inverse">TOKENS</div>
                </a>
              </div>
              {/if}
              <div class="col-4">
                <a href="/docs" aria-label="Docs" class="dropdown-item text-decoration-none p-3 bg-none">
                  <div><i class="bi bi-journal-text h2 opacity-5 d-block my-1"></i></div>
                  <div class="fw-500 fs-10px text-inverse">DOCS</div>
                </a>
              </div>
            </div>
          </div>
        </div>

        <div class="menu-item dropdown dropdown-mobile-full">
          {#if $auth && $auth.token}
            <a href="#/" aria-label="link" data-bs-toggle="dropdown" data-bs-display="static" class="menu-link">
              <span class="menu-img"><img src="/img/avatar.svg" alt="avatar" /></span>
              <div class="menu-text d-sm-block d-none fw-500">{operatorName}</div>
            </a>
            <div class="dropdown-menu dropdown-menu-end me-lg-3 fs-11px mt-1">
              <a aria-label="link" class="dropdown-item d-flex align-items-center" href="/logout" data-sveltekit-reload onclick={() => { import('$lib/auth.js').then(m => m.logoutClientSide()).catch(()=>{}); }}>LOGOUT <i class="bi bi-toggle-off ms-auto text-theme fs-16px my-n1"></i></a>
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
