<script>
  import { appOptions } from '../../stores/appOptions.js';
  
  let userData = {
  	email: 'username@account.com',
  	img: ''
  };
  
  let notificationData = [
  /*
  {
		icon: 'bi bi-bag text-theme',
		title: 'NEW ORDER RECEIVED ($1,299)',
		time: 'JUST NOW'
	}
	*/
	];
  
  function desktopToggler() {
		$appOptions.appSidebarToggled = ($appOptions.appSidebarCollapsed == false) ? false : true;
		$appOptions.appSidebarCollapsed = ($appOptions.appSidebarCollapsed == false) ? true : false;
  }
  
  function mobileToggler() {
  	$appOptions.appSidebarMobileToggled = !$appOptions.appSidebarMobileToggled;
  }
  
  function searchHeaderSearchToggler() {
  	$appOptions.appHeaderSearchToggled = !$appOptions.appHeaderSearchToggled;
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
				<span class="brand-img-text text-theme">O</span>
			</span>
			<span class="brand-text">Raworc Operator</span>
		</a>
	</div>
	<!-- END brand -->
	
	<!-- BEGIN menu -->
	<div class="menu">
		<div class="menu-item dropdown">
			<a href="#/" aria-label="link" class="menu-link" onclick={searchHeaderSearchToggler}>
				<div class="menu-icon"><i class="bi bi-search nav-icon"></i></div>
			</a>
		</div>
		<div class="menu-item dropdown dropdown-mobile-full">
			<a href="#/" aria-label="link" data-bs-toggle="dropdown" data-bs-display="static" class="menu-link">
				<div class="menu-icon"><i class="bi bi-grid-3x3-gap nav-icon"></i></div>
			</a>
			<div class="dropdown-menu fade dropdown-menu-end w-300px text-center p-0 mt-1">
				<div class="row row-grid gx-0">
					<div class="col-4">
						<a href="/email/inbox" aria-label="link" class="dropdown-item text-decoration-none p-3 bg-none">
							<div class="position-relative">
								<i class="bi bi-circle-fill position-absolute text-theme top-0 mt-n2 me-n2 fs-6px d-block text-center w-100"></i>
								<i class="bi bi-envelope h2 opacity-5 d-block my-1"></i>
							</div>
							<div class="fw-500 fs-10px text-inverse">INBOX</div>
						</a>
					</div>
					<div class="col-4">
						<a href="/pos/customer-order" aria-label="link" class="dropdown-item text-decoration-none p-3 bg-none">
							<div><i class="bi bi-hdd-network h2 opacity-5 d-block my-1"></i></div>
							<div class="fw-500 fs-10px text-inverse">POS SYSTEM</div>
						</a>
					</div>
					<div class="col-4">
						<a href="/calendar" aria-label="link" class="dropdown-item text-decoration-none p-3 bg-none">
							<div><i class="bi bi-calendar4 h2 opacity-5 d-block my-1"></i></div>
							<div class="fw-500 fs-10px text-inverse">CALENDAR</div>
						</a>
					</div>
				</div>
				<div class="row row-grid gx-0">
					<div class="col-4">
						<a href="/helper" aria-label="link" class="dropdown-item text-decoration-none p-3 bg-none">
							<div><i class="bi bi-terminal h2 opacity-5 d-block my-1"></i></div>
							<div class="fw-500 fs-10px text-inverse">HELPER</div>
						</a>
					</div>
					<div class="col-4">
						<a href="/settings" aria-label="link" class="dropdown-item text-decoration-none p-3 bg-none">
							<div class="position-relative">
								<i class="bi bi-circle-fill position-absolute text-theme top-0 mt-n2 me-n2 fs-6px d-block text-center w-100"></i>
								<i class="bi bi-sliders h2 opacity-5 d-block my-1"></i>
							</div>
							<div class="fw-500 fs-10px text-inverse">SETTINGS</div>
						</a>
					</div>
					<div class="col-4">
						<a href="/widgets" aria-label="link" class="dropdown-item text-decoration-none p-3 bg-none">
							<div><i class="bi bi-collection-play h2 opacity-5 d-block my-1"></i></div>
							<div class="fw-500 fs-10px text-inverse">WIDGETS</div>
						</a>
					</div>
				</div>
			</div>
		</div>
		<div class="menu-item dropdown dropdown-mobile-full">
			<a href="#/" aria-label="link" data-bs-toggle="dropdown" data-bs-display="static" class="menu-link">
				<div class="menu-icon"><i class="bi bi-bell nav-icon"></i></div>
				<div class="menu-badge bg-theme"></div>
			</a>
			<div class="dropdown-menu dropdown-menu-end mt-1 w-300px fs-11px pt-1">
				<h6 class="dropdown-header fs-10px mb-1">NOTIFICATIONS</h6>
				<div class="dropdown-divider mt-1"></div>
				
				{#if notificationData && notificationData.length > 0}
					{#each notificationData as notification}
						<a href="#/" aria-label="link" class="d-flex align-items-center py-10px dropdown-item text-wrap fw-semibold">
							<div class="fs-20px">
								<i class={notification.icon}></i>
							</div>
							<div class="flex-1 flex-wrap ps-3">
								<div class="mb-1 text-inverse">{notification.title}</div>
								<div class="small text-inverse text-opacity-50">{notification.time}</div>
							</div>
							<div class="ps-2 fs-16px">
								<i class="bi bi-chevron-right"></i>
							</div>
						</a>
					{/each}
				{:else}
					<div class="px-3 pb-3 pt-2">
						No record found
					</div>
				{/if}
				<hr class="my-0">
				<div class="py-10px mb-n2 text-center">
					<a href="#/" aria-label="link" class="text-decoration-none fw-bold">SEE ALL</a>
				</div>
			</div>
		</div>
		<div class="menu-item dropdown dropdown-mobile-full">
			<a href="#/" aria-label="link" data-bs-toggle="dropdown" data-bs-display="static" class="menu-link">
				<div class="menu-img online">
					{#if userData && userData.img}
						<img src={userData.img} alt="Profile" height="60" />
					{:else}
						<div class="d-flex align-items-center justify-content-center w-100 h-100 bg-inverse bg-opacity-25 text-inverse text-opacity-50 rounded-circle overflow-hidden">
							<i class="bi bi-person-fill fs-32px mb-n3"></i>
						</div>
					{/if}
				</div>
				<div class="menu-text d-sm-block d-none w-170px">{userData.email}</div>
			</a>
			<div class="dropdown-menu dropdown-menu-end me-lg-3 fs-11px mt-1">
				<a aria-label="link" class="dropdown-item d-flex align-items-center" href="/profile">PROFILE <i class="bi bi-person-circle ms-auto text-theme fs-16px my-n1"></i></a>
				<a aria-label="link" class="dropdown-item d-flex align-items-center" href="/email/inbox">INBOX <i class="bi bi-envelope ms-auto text-theme fs-16px my-n1"></i></a>
				<a aria-label="link" class="dropdown-item d-flex align-items-center" href="/calendar">CALENDAR <i class="bi bi-calendar ms-auto text-theme fs-16px my-n1"></i></a>
				<a aria-label="link" class="dropdown-item d-flex align-items-center" href="/settings">SETTINGS <i class="bi bi-gear ms-auto text-theme fs-16px my-n1"></i></a>
				<div class="dropdown-divider"></div>
				<a aria-label="link" class="dropdown-item d-flex align-items-center" href="/login">LOGOUT <i class="bi bi-toggle-off ms-auto text-theme fs-16px my-n1"></i></a>
			</div>
		</div>
	</div>
	<!-- END menu -->
	
	<!-- BEGIN menu-search -->
	<form class="menu-search" method="POST" name="header_search_form">
		<div class="menu-search-container">
			<div class="menu-search-icon"><i class="bi bi-search"></i></div>
			<div class="menu-search-input">
				<input type="text" class="form-control form-control-lg" placeholder="Search menu...">
			</div>
			<div class="menu-search-icon">
				<a href="#/" aria-label="link" onclick={searchHeaderSearchToggler}><i class="bi bi-x-lg"></i></a>
			</div>
		</div>
	</form>
	<!-- END menu-search -->
</div>
<!-- END #header -->
