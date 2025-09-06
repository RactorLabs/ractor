<script>
  import { appOptions } from '../../stores/appOptions.js';
  import { onMount } from 'svelte';
  import { auth, initAuthFromCookies } from '$lib/auth.js';
  
  let operatorName = '';
  $: operatorName = $auth?.name && $auth.name.trim().length > 0 ? $auth.name : '';
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
			<span class="brand-text">Raworc</span>
		</a>
	</div>
	<!-- END brand -->
	
	<!-- BEGIN menu -->
	<div class="menu">
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
