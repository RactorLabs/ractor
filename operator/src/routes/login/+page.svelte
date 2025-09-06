<script>
  import { onMount } from 'svelte';
  import { setPageTitle } from '$lib/utils.js';
  import { setToken, setOperatorName, isAuthenticated } from '$lib/auth.js';
  import { goto } from '$app/navigation';
  import Card from '/src/components/bootstrap/Card.svelte';

  setPageTitle('Login');

  let operator = 'admin';
  let pass = '';
  let loading = false;
  let error = null;

  onMount(() => {
    if (isAuthenticated()) {
      goto('/app/agents');
    }
  });

  async function submit(e) {
    e.preventDefault();
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

      // Expect token and user in response
      const token = data?.token || data?.jwt || data?.access_token;
      if (!token) throw new Error('Missing token in response');
      setToken(token);
      setOperatorName(operator);
      goto('/app/agents');
    } catch (e) {
      error = e.message;
    } finally {
      loading = false;
    }
  }
</script>

<div class="container">
  <div class="row justify-content-center align-items-center" style="min-height: 70vh;">
    <div class="col-md-6 col-lg-5 col-xl-4">
      <Card class="shadow-sm">
        <div class="card-body p-4">
          <div class="d-flex align-items-center mb-3">
            <div class="brand-logo me-2">
              <span class="brand-img"><span class="brand-img-text text-theme">O</span></span>
            </div>
            <div class="fs-18px fw-bold">Raworc Operator</div>
          </div>
          <div class="mb-3 text-body text-opacity-75">Sign in to continue</div>

          {#if error}
            <div class="alert alert-danger py-2 small">{error}</div>
          {/if}

          <form on:submit|preventDefault={submit}>
            <div class="mb-3">
              <label class="form-label">Operator</label>
              <input class="form-control" bind:value={operator} placeholder="admin" required autocomplete="username" />
            </div>
            <div class="mb-3">
              <label class="form-label">Password</label>
              <input type="password" class="form-control" bind:value={pass} placeholder="••••••••" required autocomplete="current-password" />
            </div>
            <div class="d-grid">
              <button class="btn btn-theme" disabled={loading}>
                {#if loading}
                  <span class="spinner-border spinner-border-sm me-2"></span>Signing in...
                {:else}
                  Sign In
                {/if}
              </button>
            </div>
          </form>
        </div>
      </Card>
      <div class="text-center small text-body text-opacity-75 mt-3">
        Use default admin/admin in dev, then change password.
      </div>
    </div>
  </div>
  <div class="mb-5"></div>
  <div class="mb-5"></div>
</div>
