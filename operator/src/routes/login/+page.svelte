<script>
  import { goto } from '$app/navigation';
  import { setPageTitle } from '$lib/utils.js';
  import { onMount, onDestroy } from 'svelte';
  import { appOptions } from '/src/stores/appOptions.js';
  import { setToken, setOperatorName, setPrincipalType, logoutClientSide } from '$lib/auth.js';

  let token = '';
  let loading = false;
  let error = null;

  async function submitForm() {
    if (loading) return;
    error = null;
    loading = true;
    try {
      if (!token || token.trim().length === 0) throw new Error('Token is required');
      // Validate token by calling /auth
      const res = await fetch('/api/v0/auth', { headers: { 'Authorization': `Bearer ${token}` } });
      const data = await res.json().catch(() => ({}));
      if (!res.ok) throw new Error(data?.error || `Invalid token (HTTP ${res.status})`);

      // Save token and principal info
      setToken(token);
      if (data?.type) setPrincipalType(data.type);
      if (data?.user) setOperatorName(data.user);
      goto('/agents');
    } catch (e) {
      error = e.message;
    } finally {
      loading = false;
    }
  }

  onMount(async () => {
    setPageTitle('Login with Token');
    $appOptions.appContentClass = 'p-0';
    $appOptions.appSidebarHide = true;
    $appOptions.appHeaderHide = true;
  });

  onDestroy(() => {
    $appOptions.appContentClass = '';
    $appOptions.appSidebarHide = false;
    $appOptions.appHeaderHide = false;
  });
</script>

<div class="login">
  <div class="login-content">
    <form on:submit|preventDefault={submitForm} method="POST" name="login_token_form">
      <h1 class="text-center">Sign In with Token</h1>
      <div class="text-inverse text-opacity-50 text-center mb-4">
        Paste a valid JWT to continue.
      </div>

      {#if error}
        <div class="alert alert-danger py-2 small">{error}</div>
      {/if}

      <div class="mb-3">
        <label class="form-label" for="token">Auth Token <span class="text-danger">*</span></label>
        <textarea class="form-control form-control-lg bg-white bg-opacity-5" id="token" rows="6" bind:value={token} placeholder="eyJhbGciOiJI..." required></textarea>
      </div>

      <button type="submit" aria-label="button" class="btn btn-outline-theme btn-lg d-block w-100 fw-500 mb-3" disabled={loading}>
        {#if loading}
          <span class="spinner-border spinner-border-sm me-2"></span>Verifying…
        {:else}
          Sign In
        {/if}
      </button>
      <div class="text-center text-inverse text-opacity-50">
        Prefer password? <a href="/login/operator" aria-label="link">Go to operator login</a>.
      </div>
      <div class="text-center text-inverse text-opacity-50 mt-2">
        Need API reference? <a href="/docs" aria-label="link">View docs</a>.
      </div>
    </form>
  </div>
  <!-- END login-content -->
</div>
