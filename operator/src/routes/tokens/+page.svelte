<script>
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { isAuthenticated, getOperatorName, logoutClientSide } from '$lib/auth.js';
  import { apiFetch } from '$lib/api/client.js';
  import Card from '/src/components/bootstrap/Card.svelte';
  import { setPageTitle } from '$lib/utils.js';

  setPageTitle('Tokens');

  let operatorName = '';
  let username = '';
  let token = '';
  let loading = false;
  let error = null;

  onMount(() => {
    if (!isAuthenticated()) { goto('/login'); return; }
    try { operatorName = getOperatorName() || ''; } catch (_) { operatorName = ''; }
  });

  async function generateToken() {
    if (loading) return;
    error = null; token = '';
    if (!username || username.trim().length === 0) { error = 'User is required'; return; }
    loading = true;
    const body = { principal: username.trim(), type: 'User' };
    const res = await apiFetch('/auth/token', { method: 'POST', body: JSON.stringify(body) }, { noAutoLogout: true });
    loading = false;
    if (!res.ok) { error = res?.data?.error || `Failed to create token (HTTP ${res.status})`; return; }
    token = res?.data?.token || '';
  }

  async function copyToken() {
    try { await navigator.clipboard.writeText(token || ''); } catch (_) {}
  }

  function doLogout() {
    try { logoutClientSide(); } catch (_) {}
    goto('/login');
  }
</script>

<div class="container-xxl">
  <div class="row justify-content-center">
    <div class="col-12 col-xxl-8">
      <Card>
        <div class="card-header fw-bold d-flex align-items-center">
          <div>Generate User Token</div>
          <div class="ms-auto"><button class="btn btn-outline-danger btn-sm" on:click={doLogout}>Logout</button></div>
        </div>
        <div class="card-body">
          <div class="alert alert-info small">You are logged in as <strong>{operatorName || 'operator'}</strong>. Enter any user name to mint a token for that user.</div>
          {#if error}
            <div class="alert alert-danger py-2 small">{error}</div>
          {/if}
          <div class="mb-3">
            <label class="form-label" for="user">User:</label>
            <input id="user" class="form-control" bind:value={username} placeholder="e.g., alice" />
          </div>
          <div class="d-flex gap-2">
            <button class="btn btn-theme" on:click|preventDefault={generateToken} disabled={loading}>
              {#if loading}<span class="spinner-border spinner-border-sm me-2"></span>Generating…{:else}Create Token{/if}
            </button>
            <a class="btn btn-outline-secondary" href="/agents">Back to Agents</a>
          </div>

          {#if token}
            <div class="mt-3">
              <label class="form-label">Token</label>
              <div class="input-group">
                <input class="form-control" readonly value={token} />
                <button class="btn btn-outline-secondary" on:click={copyToken}>Copy</button>
              </div>
            </div>
          {/if}
        </div>
      </Card>
    </div>
  </div>
</div>

