<script>
	import { onMount } from 'svelte';

	function fullscreenToggler(e) {
		e.preventDefault();
	
		var target = this.closest('.card');
		var targetClass = 'card-expand';
		var targetTop = 40;
		if (document) {
			if (document.body.classList.contains(targetClass) && target.classList.contains(targetClass)) {
				target.removeAttribute('style');
				target.classList.remove(targetClass);
				document.body.classList.remove(targetClass);
			} else {
				document.body.classList.add(targetClass);
				target.classList.add(targetClass);
			}
		}

		window.dispatchEvent(new Event('resize'));
	}


	onMount(async () => {
		let bootstrap = await import('bootstrap');
		let targets =  document.querySelectorAll('[data-bs-toggle="tooltip"]');
	
		targets.forEach(target => {
			new bootstrap.Tooltip(target);
		});
	});
</script>

<a href="#/" aria-label="Expand Toggler" onclick={fullscreenToggler} data-bs-toggle="tooltip" data-bs-title="Expand / Compress" data-bs-placement="bottom" data-bs-container="body" class="text-white text-opacity-50 text-decoration-none">
	<i class="bi bi-fullscreen"></i>
</a>

