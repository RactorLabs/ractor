<script>
  import { onMount } from 'svelte';
  import { logoutClientSide } from '$lib/auth.js';

  onMount(() => {
    // Best-effort client-side cookie clear (server also clears)
    logoutClientSide();
  });
</script>

<div class="d-flex align-items-center justify-content-center" style="min-height: 40vh;">
  <div class="text-center text-body text-opacity-75">
    <div class="spinner-border text-theme mb-3"></div>
    <div>Signing out…</div>
  </div>
</div>

