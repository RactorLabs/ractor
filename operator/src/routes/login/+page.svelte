<script>
  import { goto } from '$app/navigation';
  import { setPageTitle } from '$lib/utils.js';
  import { onMount, onDestroy } from 'svelte';
  import { appOptions } from '/src/stores/appOptions.js';
  import { setToken, setOperatorName, isAuthenticated } from '$lib/auth.js';

  let operator = 'admin';
  let pass = '';
  let loading = false;
  let error = null;

  async function submitForm() {
    if (loading) return;
    error = null;
    loading = true;
    try {
      const res = await fetch(`/api/v0/operators/${encodeURIComponent(operator)}/login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ pass })
      });
      const data = await res.json().catch(() => ({}));
      if (!res.ok) throw new Error(data?.error || `Login failed (HTTP ${res.status})`);
      const token = data?.token || data?.jwt || data?.access_token;
      if (!token) throw new Error('Missing token in response');
      setToken(token);
      setOperatorName(operator);
      goto('/agents');
    } catch (e) {
      error = e.message;
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    setPageTitle('Login');
    $appOptions.appContentClass = 'p-0';
    $appOptions.appSidebarHide = true;
    $appOptions.appHeaderHide = true;
    if (isAuthenticated()) goto('/agents');
  });

  onDestroy(() => {
    $appOptions.appContentClass = '';
    $appOptions.appSidebarHide = false;
    $appOptions.appHeaderHide = false;
  });
</script>

<div class="login">
  <!-- BEGIN login-content -->
  <div class="login-content">
    <form on:submit|preventDefault={submitForm} method="POST" name="login_form">
      <h1 class="text-center">Sign In</h1>
      <div class="text-inverse text-opacity-50 text-center mb-4">
        For your protection, please verify your identity.
      </div>

      {#if error}
        <div class="alert alert-danger py-2 small">{error}</div>
      {/if}

      <div class="mb-3">
        <label class="form-label" for="operator">Operator <span class="text-danger">*</span></label>
        <input type="text" autocomplete="username" class="form-control form-control-lg bg-white bg-opacity-5" id="operator" bind:value={operator} placeholder="admin" required />
      </div>
      <div class="mb-3">
        <div class="d-flex">
          <label class="form-label" for="password">Password <span class="text-danger">*</span></label>
        </div>
        <input type="password" autocomplete="current-password" class="form-control form-control-lg bg-white bg-opacity-5" id="password" bind:value={pass} placeholder="••••••••" required />
      </div>
      <div class="mb-3">
        <div class="form-check">
          <input class="form-check-input" type="checkbox" id="remember" />
          <label class="form-check-label" for="remember">Remember me</label>
        </div>
      </div>
      <button type="submit" aria-label="button" class="btn btn-outline-theme btn-lg d-block w-100 fw-500 mb-3" disabled={loading}>
        {#if loading}
          <span class="spinner-border spinner-border-sm me-2"></span>Signing in...
        {:else}
          Sign In
        {/if}
      </button>
      <div class="text-center text-inverse text-opacity-50">
        Need API reference? <a href="/docs" aria-label="link">View docs</a>.
      </div>
    </form>
  </div>
  <!-- END login-content -->
</div>
