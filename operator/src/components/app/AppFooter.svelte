<script>
	let year = (new Date()).getFullYear();
  import { getHostName } from '$lib/branding.js';
  export let hostName = '';
  $: resolvedHostName = hostName || getHostName();
</script>
<div id="footer" class="app-footer">
	&copy; {year } {resolvedHostName}
</div>
