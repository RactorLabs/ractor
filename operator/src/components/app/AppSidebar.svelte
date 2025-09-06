<script>
	import PerfectScrollbar from '/src/components/plugins/PerfectScrollbar.svelte';
  import { appOptions } from '../../stores/appOptions.js';
  import { appSidebarMenus } from '../../stores/appSidebarMenus.js';
	import { onMount } from 'svelte';
	import { page, navigating } from '$app/stores';
  
  function mobileToggler() {
  	$appOptions.appSidebarMobileToggled = !$appOptions.appSidebarMobileToggled;
  }
  
  function hideMobileSidebar() {
  	$appOptions.appSidebarMobileToggled = false;
  }
  
  $: if($navigating) hideMobileSidebar();
  
  function checkChildMenu(childMenu) {
  	let check = false;
  	if (childMenu) {
  		for (var i = 0; i < childMenu.length; i++) {
  			if ($page.url.pathname == childMenu[i]['url']) {
  				check = true;
  			}
  		}
  	}
  	return check;
  }
  
  function handleSidebarMenuToggle(menus) {
		menus.map(function(menu) {
			menu.onclick = function(e) {
				e.preventDefault();
				var target = this.nextElementSibling;

				menus.map(function(m) {
					var otherTarget = m.nextElementSibling;
					if (otherTarget !== target) {
						otherTarget.style.display = 'none';
						otherTarget.closest('.menu-item').classList.remove('expand');
					}
				});

				var targetItemElm = target.closest('.menu-item');

				if (targetItemElm.classList.contains('expand') || (targetItemElm.classList.contains('active') && !target.style.display)) {
					targetItemElm.classList.remove('expand');
					target.style.display = 'none';
				} else {
					targetItemElm.classList.add('expand');
					target.style.display = 'block';
				}
			}
		});
	};
	
	function handleSidebarMenu() {
		var menuBaseSelector = '.app-sidebar .menu > .menu-item.has-sub';
		var submenuBaseSelector = ' > .menu-submenu > .menu-item.has-sub';

		// menu
		var menuLinkSelector =  menuBaseSelector + ' > .menu-link';
		var menus = [].slice.call(document.querySelectorAll(menuLinkSelector));
		handleSidebarMenuToggle(menus);

		// submenu lvl 1
		var submenuLvl1Selector = menuBaseSelector + submenuBaseSelector;
		var submenusLvl1 = [].slice.call(document.querySelectorAll(submenuLvl1Selector + ' > .menu-link'));
		handleSidebarMenuToggle(submenusLvl1);

		// submenu lvl 2
		var submenuLvl2Selector = menuBaseSelector + submenuBaseSelector + submenuBaseSelector;
		var submenusLvl2 = [].slice.call(document.querySelectorAll(submenuLvl2Selector + ' > .menu-link'));
		handleSidebarMenuToggle(submenusLvl2);
	};
  
	onMount(async () => {
		handleSidebarMenu();
	});
</script>
<!-- BEGIN #sidebar -->
<div id="sidebar" class="app-sidebar">
	<!-- BEGIN scrollbar -->
	<PerfectScrollbar class="h-100">
		<div class="app-sidebar-content">
			<!-- BEGIN menu -->
			<div class="menu">
				{#each $appSidebarMenus as menu}
					{#if menu.is_header}
						<div class="menu-header">{ menu.text }</div>
					{:else if menu.is_divider}
						<div class="menu-divider"></div>
					{:else}
						<div class="menu-item" class:has-sub={menu.children} class:active={$page.url.pathname === menu.url || checkChildMenu(menu.children)}>
							<a href={menu.url} class="menu-link">
								{#if menu.icon}
									<span class="menu-icon">
										<i class={menu.icon}></i>
										{#if menu.highlight}<span class="w-5px h-5px rounded-3 bg-theme position-absolute top-0 end-0 mt-3px me-3px"></span>{/if}
									</span>
								{/if}
								<span class="menu-text">{menu.text}</span>
								{#if menu.children}
									<span class="menu-caret"><b class="caret"></b></span>
								{/if}
							</a>
						
							{#if menu.children}
								<div class="menu-submenu">
									{#each menu.children as childMenu}
										<div class="menu-item" class:has-sub={childMenu.children}  class:active={$page.url.pathname === childMenu.url}>
											<a href={childMenu.url} class="menu-link">
												<span class="menu-text">{childMenu.text}</span>
											</a>
										</div>
									{/each}
								</div>
							{/if}
						</div>
					{/if}
				{/each}
			</div>
			<!-- END menu -->
			<!-- Removed external vendor documentation link -->
		</div>
	</PerfectScrollbar>
	<!-- END scrollbar -->
</div>
<!-- END #sidebar -->
	
<!-- BEGIN mobile-sidebar-backdrop -->
<button class="app-sidebar-mobile-backdrop" aria-label="button" onclick={mobileToggler}></button>
<!-- END mobile-sidebar-backdrop -->
