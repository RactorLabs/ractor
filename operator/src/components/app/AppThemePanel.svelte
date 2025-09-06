<script>
	import { onMount } from 'svelte';
	import Card from '/src/components/bootstrap/Card.svelte';
	import CardBody from '/src/components/bootstrap/CardBody.svelte';
  import { appVariables, generateVariables } from '../../stores/appVariables.js';
	
	let active = 'false';
	let activeMode = 'dark';
	let activeDirection = '';
	let activeTheme = 'theme-teal';
	let activeCover = 'bg-cover-1';
	
	let modeList = [
	 { name: 'Dark', img: '/img/mode/dark.jpg', value: 'dark' },
	 { name: 'Light', img: '/img/mode/light.jpg', value: 'light' },
	];
	
	let directionList = [
	 { name: 'LTR', icon: 'bi-text-left', value: 'ltr' },
	 { name: 'RTL', icon: 'bi-text-right', value: 'rtl' },
	];

	let themeList = [
	 { name: 'Pink', bgClass: 'bg-pink', themeClass: 'theme-pink' },
	 { name: 'Red', bgClass: 'bg-red', themeClass: 'theme-red' },
	 { name: 'Orange', bgClass: 'bg-warning', themeClass: 'theme-warning' },
	 { name: 'Yellow', bgClass: 'bg-yellow', themeClass: 'theme-yellow' },
	 { name: 'Lime', bgClass: 'bg-lime', themeClass: 'theme-lime' },
	 { name: 'Green', bgClass: 'bg-green', themeClass: 'theme-green' },
	 { name: 'Default', bgClass: 'bg-teal', themeClass: 'theme-teal' },
	 { name: 'Cyan', bgClass: 'bg-info', themeClass: 'theme-info' },
	 { name: 'Blue', bgClass: 'bg-primary', themeClass: 'theme-primary' },
	 { name: 'Purple', bgClass: 'bg-purple', themeClass: 'theme-purple' },
	 { name: 'Indigo', bgClass: 'bg-indigo', themeClass: 'theme-indigo' },
	 { name: 'Gray', bgClass: 'bg-gray-200', themeClass: 'theme-gray-200' }
	];

	let coverList = [
		{ name: 'Default', coverThumbImage: '/img/cover/cover-thumb-1.jpg', coverClass: 'bg-cover-1'},
		{ name: 'Cover 2', coverThumbImage: '/img/cover/cover-thumb-2.jpg', coverClass: 'bg-cover-2'},
		{ name: 'Cover 3', coverThumbImage: '/img/cover/cover-thumb-3.jpg', coverClass: 'bg-cover-3'},
		{ name: 'Cover 4', coverThumbImage: '/img/cover/cover-thumb-4.jpg', coverClass: 'bg-cover-4'},
		{ name: 'Cover 5', coverThumbImage: '/img/cover/cover-thumb-5.jpg', coverClass: 'bg-cover-5'},
		{ name: 'Cover 6', coverThumbImage: '/img/cover/cover-thumb-6.jpg', coverClass: 'bg-cover-6'},
		{ name: 'Cover 7', coverThumbImage: '/img/cover/cover-thumb-7.jpg', coverClass: 'bg-cover-7'},
		{ name: 'Cover 8', coverThumbImage: '/img/cover/cover-thumb-8.jpg', coverClass: 'bg-cover-8'},
		{ name: 'Cover 9', coverThumbImage: '/img/cover/cover-thumb-9.jpg', coverClass: 'bg-cover-9'}
	]

  function themePanelToggler() {
  	active = (active === 'true') ? 'false' : 'true';
  	localStorage.setItem('theme-panel', active);
  }
  
  function themeModeToggler(mode) {
  	activeMode = mode;
  	localStorage.setItem('theme-mode', mode);
  	document.documentElement.setAttribute('data-bs-theme', mode);
  	$appVariables = generateVariables();
  }
  
  function themeColorToggler(themeClass) {
  	activeTheme = themeClass;
  	localStorage.setItem('theme-color', themeClass);
  	
  	for (var x = 0; x < document.body.classList.length; x++) {
			var targetClass = document.body.classList[x];
			if (targetClass.search('theme-') > -1) {
				document.body.classList.remove(targetClass);
			}
		}
	
		document.body.classList.add(themeClass);
		$appVariables = generateVariables();
  }
  
  function themeCoverToggler(coverClass) {
  	activeCover = coverClass;
  	localStorage.setItem('theme-cover', coverClass);
  	
		var htmlElm = document.querySelector('html');
		for (var x = 0; x < document.documentElement.classList.length; x++) {
			var targetClass = document.documentElement.classList[x];
			if (targetClass.search('bg-cover-') > -1) {
				htmlElm.classList.remove(targetClass);
			}
		}
		htmlElm.classList.add(coverClass);
  }
  
  function themeDirectionToggler(direction) {
  	activeDirection = direction;
  	localStorage.setItem('theme-direction', direction);
  	document.documentElement.setAttribute('dir', direction);
  	$appVariables = generateVariables();
  }
  
	onMount(async () => {
		let bootstrap = await import('bootstrap');
		let targets =  document.querySelectorAll('[data-bs-toggle="tooltip"]');
	
		targets.forEach(target => {
			new bootstrap.Tooltip(target);
		});
		
		if (typeof localStorage !== 'undefined') {
			active = (localStorage.getItem('theme-panel')) ? localStorage.getItem('theme-panel') : active;
			activeMode = (localStorage.getItem('theme-mode')) ? localStorage.getItem('theme-mode') : activeMode;
			activeDirection = (localStorage.getItem('theme-direction')) ? localStorage.getItem('theme-direction') : activeDirection;
			activeTheme = (localStorage.getItem('theme-color')) ? localStorage.getItem('theme-color') : activeTheme;
			activeCover = (localStorage.getItem('theme-cover')) ? localStorage.getItem('theme-cover') : activeCover;
			
			themeModeToggler(activeMode);
			themeDirectionToggler(activeDirection);
			themeColorToggler(activeTheme);
			themeCoverToggler(activeCover);
		}
	});
</script>


<!-- BEGIN theme-panel -->
<div class="app-theme-panel" class:active={active === 'true'}>
	<div class="app-theme-panel-container">
		<a href="#/" aria-label="Toggler" on:click|preventDefault={themePanelToggler} class="app-theme-toggle-btn"><i class="bi bi-sliders"></i></a>
		<div class="app-theme-panel-content">
			<div class="small fw-bold text-inverse mb-1">Display Mode</div>
			<Card class="mb-3">
				<CardBody class="p-2">
					<div class="row gx-2">
						{#each modeList as mode}
							<div class="col-6">
								<a href="#/" aria-label="Theme Mode" on:click|preventDefault={() => themeModeToggler(mode.value)} class="app-theme-mode-link" class:active={mode.value == activeMode}>
									<div class="img"><img src={mode.img} class="object-fit-cover" height="76" width="76" alt="{mode.name} Mode"></div>
									<div class="text">{mode.name}</div>
								</a>
							</div>
						{/each}
					</div>
				</CardBody>
			</Card>
			
			<div class="small fw-bold text-inverse mb-1">Direction Mode</div>
			<Card class="mb-3">
				<CardBody class="p-2">
					<div class="row gx-2">
						{#each directionList as direction}
							<div class="col-6">
								<a href="#/" aria-label="Theme Direction" on:click|preventDefault={() => themeDirectionToggler(direction.value)} class="btn btn-sm btn-outline-light d-flex align-items-center justify-content-center gap-2 w-100 rounded-0 fw-bold fs-12px" class:active={direction.value == activeDirection || (direction.value == 'ltr' && !activeDirection)}>
									<i class="bi { direction.icon } fs-16px my-n1 ms-n2"></i> {direction.name}
								</a>
							</div>
						{/each}
					</div>
				</CardBody>
			</Card>
			
			<div class="small fw-bold text-inverse mb-1">Theme Color</div>
			<Card class="mb-3">
				<CardBody class="p-2">
					<div class="app-theme-list">
						{#each themeList as theme}
						<div class="app-theme-list-item" class:active={theme.themeClass == activeTheme}>
							<a href="#/" aria-label="Theme Color" class="app-theme-list-link {theme.bgClass}" on:click|preventDefault={() => themeColorToggler(theme.themeClass)} data-bs-toggle="tooltip" data-bs-trigger="hover" data-bs-container="body" data-bs-title="{theme.name}">&nbsp;</a></div>
						{/each}
					</div>
				</CardBody>
			</Card>
			
			<div class="small fw-bold text-inverse mb-1">Theme Cover</div>
			<Card class="mb-3">
				<CardBody class="p-2">
					<!-- BEGIN theme-cover -->
					<div class="app-theme-cover">
						{#each coverList as cover}
						<div class="app-theme-cover-item" class:active={cover.coverClass == activeCover}>
							<a href="#/" aria-label="Theme Cover" class="app-theme-cover-link" on:click|preventDefault={() => themeCoverToggler(cover.coverClass)} style="background-image: url({cover.coverThumbImage});" data-bs-toggle="tooltip" data-bs-trigger="hover" data-bs-container="body" data-bs-title="{cover.name}">&nbsp;</a>
						</div>
						{/each}
					</div>
				</CardBody>
			</Card>
		</div>
	</div>
</div>
<!-- END theme-panel -->