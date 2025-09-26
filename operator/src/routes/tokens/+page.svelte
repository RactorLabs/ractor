<script>
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { isAuthenticated, getOperatorName, getPrincipalType, logoutClientSide } from '$lib/auth.js';
  import { apiFetch } from '$lib/api/client.js';
  import Card from '/src/components/bootstrap/Card.svelte';
  import { setPageTitle } from '$lib/utils.js';

  setPageTitle('Create Tokens');

  let operatorName = '';
  let username = '';
  let userTtlHours = '';
  let token = '';
  // Operator token section state
  let opToken = '';
  let opLoading = false;
  let opError = null;
  let opCopyStatus = '';
  let opTtlHours = '';
  let loading = false;
  let error = null;
  let copyStatus = '';

  onMount(() => {
    if (!isAuthenticated()) { goto('/login'); return; }
    let t = '';
    try { t = getPrincipalType() || ''; } catch (_) { t = ''; }
    if (String(t || '').toLowerCase() !== 'admin') { goto('/'); return; }
    try { operatorName = getOperatorName() || ''; } catch (_) { operatorName = ''; }
  });

  async function generateToken() {
    if (loading) return;
    error = null; token = '';
    if (!username || username.trim().length === 0) { error = 'User is required'; return; }
    loading = true;
    const body = { principal: username.trim(), type: 'User' };
    const ttlNum = Number(userTtlHours);
    if (Number.isFinite(ttlNum) && ttlNum > 0) body.ttl_hours = ttlNum;
    const res = await apiFetch('/auth/token', { method: 'POST', body: JSON.stringify(body) }, { noAutoLogout: true });
    loading = false;
    if (!res.ok) { error = res?.data?.message || res?.data?.error || `Failed to create token (HTTP ${res.status})`; return; }
    token = res?.data?.token || '';
    copyStatus = '';
  }

  async function generateOperatorToken() {
    if (opLoading) return;
    opError = null; opToken = '';
    opLoading = true;
    const body = { principal: operatorName, type: 'Admin' };
    const ttlNum = Number(opTtlHours);
    if (Number.isFinite(ttlNum) && ttlNum > 0) body.ttl_hours = ttlNum;
    const res = await apiFetch('/auth/token', { method: 'POST', body: JSON.stringify(body) }, { noAutoLogout: true });
    opLoading = false;
    if (!res.ok) { opError = res?.data?.message || res?.data?.error || `Failed to create operator token (HTTP ${res.status})`; return; }
    opToken = res?.data?.token || '';
    opCopyStatus = '';
  }

  async function copyOperatorToken() {
    opCopyStatus = '';
    const text = opToken || '';
    let ok = false;
    if (text) {
      try { await navigator.clipboard.writeText(text); ok = true; }
      catch (_) {
        try {
          const ta = document.createElement('textarea');
          ta.value = text; ta.style.position = 'fixed'; ta.style.opacity = '0';
          document.body.appendChild(ta); ta.focus(); ta.select();
          ok = document.execCommand('copy'); document.body.removeChild(ta);
        } catch (_) { ok = false; }
      }
    }
    opCopyStatus = ok ? 'Copied!' : 'Copy failed';
    try { if (ok) setTimeout(() => { opCopyStatus = ''; }, 1500); } catch (_) {}
  }

  async function copyToken() {
    copyStatus = '';
    const text = token || '';
    let ok = false;
    if (text) {
      try {
        // Try modern clipboard API
        await navigator.clipboard.writeText(text);
        ok = true;
      } catch (_) {
        try {
          // Fallback using a hidden textarea
          const ta = document.createElement('textarea');
          ta.value = text;
          ta.style.position = 'fixed';
          ta.style.opacity = '0';
          document.body.appendChild(ta);
          ta.focus();
          ta.select();
          ok = document.execCommand('copy');
          document.body.removeChild(ta);
        } catch (_) { ok = false; }
      }
    }
    copyStatus = ok ? 'Copied!' : 'Copy failed';
    try { if (ok) setTimeout(() => { copyStatus = ''; }, 1500); } catch (_) {}
  }

  function doLogout() {
    try { logoutClientSide(); } catch (_) {}
    goto('/login');
  }
</script>

<div class="container-xxl">
  <div class="row justify-content-center">
    <div class="col-12 col-xxl-8">
      <!-- User token info removed as requested -->
      <Card class="mb-3">
        <div class="card-header fw-bold d-flex align-items-center">
          <div class="fs-20px">Generate User Token</div>
        </div>
        <div class="card-body">
          {#if error}
            <div class="alert alert-danger py-2 small">{error}</div>
          {/if}
          <div class="mb-3">
            <label class="form-label" for="user">User:</label>
            <input id="user" class="form-control" bind:value={username} placeholder="e.g., alice" />
          </div>
          <div class="mb-3">
            <label class="form-label" for="user-ttl">TTL (hours, optional)</label>
            <input id="user-ttl" type="number" min="0" step="1" class="form-control" bind:value={userTtlHours} placeholder="e.g., 24" />
            <div class="form-text">Leave blank or 0 for no expiry.</div>
          </div>
          <div class="d-flex gap-2 align-items-center">
            <button class="btn btn-outline-theme" on:click|preventDefault={generateToken} disabled={loading}>
              {#if loading}<span class="spinner-border spinner-border-sm me-2"></span>Generating…{:else}Create User Token{/if}
            </button>
          </div>

          {#if token}
            <div class="mt-3">
              <label class="form-label" for="token-output">Token</label>
              <div class="input-group">
                <input id="token-output" class="form-control" readonly value={token} />
                <button class="btn btn-outline-secondary" on:click={copyToken}>Copy</button>
              </div>
              {#if copyStatus}
                <div class="small mt-1 {copyStatus === 'Copied!' ? 'text-success' : 'text-danger'}">{copyStatus}</div>
              {/if}
            </div>
          {/if}

          <div class="mt-3">
            <button class="btn btn-outline-danger" on:click|preventDefault={doLogout}>Logout</button>
          </div>
        </div>
      </Card>
      <!-- Operator token info outside the card -->
      <div class="alert alert-info small mb-2 mt-3">Use with caution.</div>
      <Card>
        <div class="card-header fw-bold d-flex align-items-center">
          <div class="fs-20px">Generate Operator Token</div>
          <span class="ms-auto small text-muted">{operatorName}</span>
        </div>
        <div class="card-body">
          {#if opError}
            <div class="alert alert-danger py-2 small">{opError}</div>
          {/if}
          <div class="mb-3">
            <label class="form-label" for="op-ttl">TTL (hours, optional)</label>
            <input id="op-ttl" type="number" min="0" step="1" class="form-control" bind:value={opTtlHours} placeholder="e.g., 24" />
            <div class="form-text">Leave blank or 0 for no expiry.</div>
          </div>
          <div class="d-flex gap-2 align-items-center">
            <button class="btn btn-outline-theme" on:click|preventDefault={generateOperatorToken} disabled={opLoading}>
              {#if opLoading}<span class="spinner-border spinner-border-sm me-2"></span>Generating…{:else}Create Operator Token{/if}
            </button>
          </div>

          {#if opToken}
            <div class="mt-3">
              <label class="form-label" for="op-token-output">Operator Token</label>
              <div class="input-group">
                <input id="op-token-output" class="form-control" readonly value={opToken} />
                <button class="btn btn-outline-secondary" on:click={copyOperatorToken}>Copy</button>
              </div>
              {#if opCopyStatus}
                <div class="small mt-1 {opCopyStatus === 'Copied!' ? 'text-success' : 'text-danger'}">{opCopyStatus}</div>
              {/if}
            </div>
          {/if}
        </div>
      </Card>
    </div>
  </div>
</div>
