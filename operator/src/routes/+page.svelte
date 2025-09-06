<script>
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { setPageTitle } from '$lib/utils.js';

  onMount(() => {
    setPageTitle('Login');
    goto('/login');
  });
</script>

<div class="d-flex align-items-center justify-content-center" style="min-height: 50vh;">
  <div class="text-center text-body text-opacity-75">
    <div class="spinner-border text-theme mb-3"></div>
    <div>Redirecting to login… If not redirected, <a href="/login">click here</a>.</div>
  </div>
  
</div>
