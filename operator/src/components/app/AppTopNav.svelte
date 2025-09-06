<script>
  import { appOptions } from '../../stores/appOptions.js';
  import { appTopNavMenus } from '../../stores/appTopNavMenus.js';
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
  
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
  
	function handleUnlimitedTopNavRender() {
		"use strict";
		// function handle menu button action - next / prev
		function handleMenuButtonAction(element, direction) {
			var obj = element.closest('.menu');
			var objStyle = window.getComputedStyle(obj);
			var bodyStyle = window.getComputedStyle(document.querySelector('body'));
			var targetCss = (bodyStyle.getPropertyValue('direction') == 'rtl') ? 'margin-right' : 'margin-left';
			var marginLeft = parseInt(objStyle.getPropertyValue(targetCss));  
			var containerWidth = document.querySelector('.app-top-nav').clientWidth - document.querySelector('.app-top-nav').clientHeight * 2;
			var totalWidth = 0;
			var finalScrollWidth = 0;
			var controlPrevObj = obj.querySelector('.menu-control-start');
			var controlPrevWidth = (controlPrevObj) ? controlPrevObj.clientWidth : 0;
			var controlNextObj = obj.querySelector('.menu-control-end');
			var controlNextWidth = (controlPrevObj) ? controlNextObj.clientWidth : 0;
			var controlWidth = controlPrevWidth + controlNextWidth;
			var widthLeft = 0;
		
			var elms = [].slice.call(obj.querySelectorAll('.menu-item'));
			if (elms) {
				elms.map(function(elm) {
					if (!elm.classList.contains('.menu-control')) {
						totalWidth += elm.clientWidth;
					}
				});
			}

			switch (direction) {
				case 'next':
					widthLeft = totalWidth + marginLeft - containerWidth;
					if (widthLeft <= containerWidth) {
						finalScrollWidth = widthLeft - marginLeft - controlWidth;
						setTimeout(function() {
							obj.querySelector('.menu-control.menu-control-end').classList.remove('show');
						}, 300);
					} else {
						finalScrollWidth = containerWidth - marginLeft - controlWidth;
					}

					if (finalScrollWidth !== 0) {
						obj.style.transitionProperty = 'height, margin, padding';
						obj.style.transitionDuration = '300ms';
						if (bodyStyle.getPropertyValue('direction') != 'rtl') {
							obj.style.marginLeft = '-' + finalScrollWidth + 'px';
						} else {
							obj.style.marginRight = '-' + finalScrollWidth + 'px';
						}
						setTimeout(function() {
							obj.style.transitionProperty = '';
							obj.style.transitionDuration = '';
							obj.querySelector('.menu-control.menu-control-start').classList.add('show');
						}, 300);
					}
					break;
				case 'prev':
					widthLeft = -marginLeft;

					if (widthLeft <= containerWidth) {
						obj.querySelector('.menu-control.menu-control-start').classList.remove('show');
						finalScrollWidth = 0;
					} else {
						finalScrollWidth = widthLeft - containerWidth + controlWidth;
					}
				
					obj.style.transitionProperty = 'height, margin, padding';
					obj.style.transitionDuration = '300ms';
				
					if (bodyStyle.getPropertyValue('direction') != 'rtl') {
						obj.style.marginLeft = '-' + finalScrollWidth + 'px';
					} else {
						obj.style.marginRight = '-' + finalScrollWidth + 'px';
					}
				
					setTimeout(function() {
						obj.style.transitionProperty = '';
						obj.style.transitionDuration = '';
						obj.querySelector('.menu-control.menu-control-end').classList.add('show');
					}, 300);
					break;
			}
		}

		// handle page load active menu focus
		function handlePageLoadMenuFocus() {
			var targetMenu = document.querySelector('.app-top-nav .menu');
			if (!targetMenu) {
				return;
			}
			var targetMenuStyle = window.getComputedStyle(targetMenu);
			var bodyStyle = window.getComputedStyle(document.body);
			var targetCss = (bodyStyle.getPropertyValue('direction') == 'rtl') ? 'margin-right' : 'margin-left';
			var marginLeft = parseInt(targetMenuStyle.getPropertyValue(targetCss));
			var viewWidth = document.querySelector('.app-top-nav').clientWidth;
			var prevWidth = 0;
			var speed = 0;
			var fullWidth = 0;
			var controlPrevObj = targetMenu.querySelector('.menu-control-start');
			var controlPrevWidth = (controlPrevObj) ? controlPrevObj.clientWidth : 0;
			var controlNextObj = targetMenu.querySelector('.menu-control-end');
			var controlNextWidth = (controlPrevObj) ? controlNextObj.clientWidth : 0;
			var controlWidth = 0;

			var elms = [].slice.call(document.querySelectorAll('.app-top-nav .menu > .menu-item'));
			if (elms) {
				var found = false;
				elms.map(function(elm) {
					if (!elm.classList.contains('menu-control')) {
						fullWidth += elm.clientWidth;
						if (!found) {
							prevWidth += elm.clientWidth;
						}
						if (elm.classList.contains('active')) {
							found = true;
						}
					}
				});
			}
		
			var elm = targetMenu.querySelector('.menu-control.menu-control-end');
			if (prevWidth != fullWidth && fullWidth >= viewWidth) {
				elm.classList.add('show');
				controlWidth += controlNextWidth;
			} else {
				elm.classList.remove('show');
			}
		
			elm = targetMenu.querySelector('.menu-control.menu-control-start');
			if (prevWidth >= viewWidth && fullWidth >= viewWidth) {
				elm.classList.add('show');
			} else {
				elm.classList.remove('show');
			}
		
			if (prevWidth >= viewWidth) {
				var finalScrollWidth = prevWidth - viewWidth + controlWidth;
				if (bodyStyle.getPropertyValue('direction') != 'rtl') {
					targetMenu.style.marginLeft = '-' + finalScrollWidth + 'px';
				} else {
					targetMenu.style.marginRight = '-' + finalScrollWidth + 'px';
				}
			}
		}

		// handle menu next button click action
		var elm = document.querySelector('[data-toggle="app-top-nav-next"]');
		if (elm) {
			elm.onclick = function(e) {
				e.preventDefault();
				handleMenuButtonAction(this,'next');
			};
		}
	
		// handle menu prev button click action
		elm = document.querySelector('[data-toggle="app-top-nav-prev"]');
		if (elm) {
			elm.onclick = function(e) {
				e.preventDefault();
				handleMenuButtonAction(this,'prev');
			};
		}
	
	
		function enableFluidContainerDrag(containerClassName) {
			const container = document.querySelector(containerClassName);
			if (!container) {
				return;
			}
			const menu = container.querySelector('.menu');
			const menuItem = menu.querySelectorAll('.menu-item:not(.menu-control)');

			let startX, scrollLeft, mouseDown;
			let menuWidth = 0;
			let maxScroll = 0;

			menuItem.forEach((element) => {
				menuWidth += element.offsetWidth;
			});

			container.addEventListener('mousedown', (e) => {
				mouseDown = true;
				startX = e.pageX;
				scrollLeft = (menu.style.marginLeft) ? parseInt(menu.style.marginLeft) : 0;
				maxScroll = container.offsetWidth - menuWidth;
			});

			container.addEventListener('touchstart', (e) => {
				mouseDown = true;
				const touch = e.targetTouches[0];
				startX = touch.pageX;
				scrollLeft = (menu.style.marginLeft) ? parseInt(menu.style.marginLeft) : 0;
				maxScroll = container.offsetWidth - menuWidth;
			});

			container.addEventListener('mouseup', () => {
				mouseDown = false;
			});

			container.addEventListener('touchend', () => {
				mouseDown = false;
			});

			container.addEventListener('mousemove', (e) => {
				if (!startX || !mouseDown) return;
				if (window.innerWidth < 768) return;
				e.preventDefault();
				const x = e.pageX;
				const walkX = (x - startX) * 1;
				var totalMarginLeft = scrollLeft + walkX;
				if (totalMarginLeft <= maxScroll) {
					totalMarginLeft = maxScroll;
					menu.querySelector('.menu-control.menu-control-end').classList.remove('show');
				} else {
					menu.querySelector('.menu-control.menu-control-end').classList.add('show');
				}
				if (menuWidth < container.offsetWidth) {
					menu.querySelector('.menu-control.menu-control-start').classList.remove('show');
				}
				if (maxScroll > 0) {
					menu.querySelector('.menu-control.menu-control-end').classList.remove('show');
				}
				if (totalMarginLeft > 0) {
					totalMarginLeft = 0;
					menu.querySelector('.menu-control.menu-control-start').classList.remove('show');
				} else {
					menu.querySelector('.menu-control.menu-control-start').classList.add('show');
				}
				menu.style.marginLeft = totalMarginLeft + 'px';
			});

			container.addEventListener('touchmove', (e) => {
				if (!startX || !mouseDown) return;
				if (window.innerWidth < 768) return;
				e.preventDefault();
				const touch = e.targetTouches[0];
				const x = touch.pageX;
				const walkX = (x - startX) * 1;
				var totalMarginLeft = scrollLeft + walkX;
				if (totalMarginLeft <= maxScroll) {
					totalMarginLeft = maxScroll;
					menu.querySelector('.menu-control.menu-control-end').classList.remove('show');
				} else {
					menu.querySelector('.menu-control.menu-control-end').classList.add('show');
				}
				if (menuWidth < container.offsetWidth) {
					menu.querySelector('.menu-control.menu-control-start').classList.remove('show');
				}
				if (maxScroll > 0) {
					menu.querySelector('.menu-control.menu-control-end').classList.remove('show');
				}
				if (totalMarginLeft > 0) {
					totalMarginLeft = 0;
					menu.querySelector('.menu-control.menu-control-start').classList.remove('show');
				} else {
					menu.querySelector('.menu-control.menu-control-start').classList.add('show');
				}
				menu.style.marginLeft = totalMarginLeft + 'px';
			});
		}
	
		window.addEventListener('resize', function() {
			if (window.innerWidth >= 768) {
				var targetElm = document.querySelector('.app-top-nav');
				if (targetElm) {
					targetElm.removeAttribute('style');
				}
				var targetElm2 = document.querySelector('.app-top-nav .menu');
				if (targetElm2) {
					targetElm2.removeAttribute('style');
				}
				var targetElm3 = document.querySelectorAll('.app-top-nav .menu-submenu');
				if (targetElm3) {
					targetElm3.forEach((elm3) => {
						elm3.removeAttribute('style');
					});
				}
				handlePageLoadMenuFocus();
			}
			enableFluidContainerDrag('.app-top-nav');
		});
	
		if (window.innerWidth >= 768) {
			handlePageLoadMenuFocus();
			enableFluidContainerDrag('.app-top-nav');
		}
	};

	function handleTopNavToggle(menus, forMobile = false) {
		menus.map(function(menu) {
			menu.onclick = function(e) {
				e.preventDefault();
			
				if (!forMobile || (forMobile && document.body.clientWidth < 768)) {
					var target = this.nextElementSibling;
					menus.map(function(m) {
						var otherTarget = m.nextElementSibling;
						if (otherTarget !== target) {
							slideUp(otherTarget);
							otherTarget.closest('.menu-item').classList.remove('expand');
							otherTarget.closest('.menu-item').classList.add('closed');
						}
					});
			
					slideToggle(target);
				}
			}
		});
	};

	function handleTopNavSubMenu() {
		"use strict";
	
		var menuBaseSelector = '.app-top-nav .menu > .menu-item.has-sub';
		var submenuBaseSelector = ' > .menu-submenu > .menu-item.has-sub';
	
		// Menu - Toggle / Collapse
		var menuLinkSelector =  menuBaseSelector + ' > .menu-link';
		var menus = [].slice.call(document.querySelectorAll(menuLinkSelector));
		handleTopNavToggle(menus, true);
	
		// Menu - Submenu lvl 1
		var submenuLvl1Selector = menuBaseSelector + submenuBaseSelector;
		var submenusLvl1 = [].slice.call(document.querySelectorAll(submenuLvl1Selector + ' > .menu-link'));
		handleTopNavToggle(submenusLvl1);
	
		// Menu - Submenu lvl 2
		var submenuLvl2Selector = menuBaseSelector + submenuBaseSelector + submenuBaseSelector;
		var submenusLvl2 = [].slice.call(document.querySelectorAll(submenuLvl2Selector + ' > .menu-link'));
		handleTopNavToggle(submenusLvl2);
	};
  
	onMount(async () => {
		handleUnlimitedTopNavRender();
		handleTopNavSubMenu();
	});
</script>
<div id="top-nav" class="app-top-nav">
	<div class="menu">
		{#each $appTopNavMenus as menu}
			<div class="menu-item" class:has-sub={menu.children} class:active={$page.url.pathname === menu.url || checkChildMenu(menu.children)}>
				<a href={menu.url} class="menu-link" aria-label="link">
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
								<a href={childMenu.url} class="menu-link" aria-label="link">
									<span class="menu-text">{childMenu.text}</span>
								</a>
							</div>
						{/each}
					</div>
				{/if}
			</div>
		{/each}
		<div class="menu-item menu-control menu-control-start">
			<a href="#/" class="menu-link" aria-label="Prev Toggler" data-toggle="app-top-nav-prev"><i class="bi bi-caret-left"></i></a>
		</div>
		<div class="menu-item menu-control menu-control-end">
			<a href="#/" class="menu-link" aria-label="Next Toggler" data-toggle="app-top-nav-next"><i class="bi bi-caret-right"></i></a>
		</div>
	</div>
</div>