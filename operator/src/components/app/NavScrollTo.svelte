<script>
	function scrollTo(event) {
		event.preventDefault();

		var targetAttr = $$props.target;
		var targetElm = document.querySelectorAll(targetAttr)[0];
		var targetHeader = document.querySelectorAll('.app-header')[0];
		var targetHeight = targetHeader.offsetHeight;
		if (targetElm) {
			var targetTop = targetElm.offsetTop - targetHeight - 24;
			window.scrollTo({top: targetTop, behavior: 'smooth'});
		}
	}
</script>
<a class="nav-link" href={$$props.target} on:click={scrollTo}>
	<slot />
</a>