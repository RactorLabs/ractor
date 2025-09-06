<script>
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { isAuthenticated, getToken } from '$lib/auth.js';
  import { setPageTitle } from '$lib/utils.js';

  let ready = false;

  setPageTitle('App');

  onMount(async () => {
    if (!isAuthenticated()) {
      goto('/login');
      return;
    }
    // Optionally validate token by pinging /auth
    try {
      const token = getToken();
      const res = await fetch('/api/v0/auth', { headers: { 'Authorization': `Bearer ${token || ''}` } });
      if (!res.ok) throw new Error('unauthorized');
    } catch (_) {
      goto('/login');
      return;
    }
    ready = true;
  });
</script>

{#if !ready}
  <div class="d-flex align-items-center justify-content-center" style="min-height: 50vh;">
    <div class="text-center text-body text-opacity-75">
      <div class="spinner-border text-theme mb-3"></div>
      <div>Loading…</div>
    </div>
  </div>
{:else}
  <slot />
{/if}
