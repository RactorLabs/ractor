<script>
  import { onMount, onDestroy } from 'svelte';
  import { goto } from '$app/navigation';
  import { isAuthenticated } from '$lib/auth.js';
  import { apiFetch } from '$lib/api/client.js';
  import Card from '/src/components/bootstrap/Card.svelte';
  import { setPageTitle } from '$lib/utils.js';

  setPageTitle('Agents');

  let loading = true;
  let error = null;
  let agents = [];
  import { auth, getOperatorName } from '$lib/auth.js';
  let operatorName = '';
  $: isAdmin = $auth && String($auth.type || '').toLowerCase() === 'admin';

  function stateClass(state) {
    const s = String(state || '').toLowerCase();
    if (s === 'init') return 'badge rounded-pill bg-transparent border border-secondary text-secondary';
    if (s === 'idle') return 'badge rounded-pill bg-transparent border border-success text-success';
    if (s === 'busy') return 'badge rounded-pill bg-transparent border border-warning text-warning';
    return 'badge rounded-pill bg-transparent border border-secondary text-secondary';
  }
  function stateColorClass(state) {
    const s = String(state || '').toLowerCase();
    if (s === 'idle') return 'bg-success border-success';
    if (s === 'busy') return 'bg-warning border-warning';
    if (s === 'init') return 'bg-secondary border-secondary';
    return 'bg-secondary border-secondary';
  }

  function stateIconClass(state) {
    const s = String(state || '').toLowerCase();
    if (s === 'slept') return 'fas fa-moon';
    if (s === 'idle') return 'fas fa-sun';
    if (s === 'busy') return 'fas fa-circle-notch fa-spin';
    if (s === 'init') return 'fas fa-spinner fa-spin';
    return 'fas fa-circle-dot';
  }

import { getHostUrl } from '$lib/branding.js';

  async function refresh() {
    const res = await apiFetch('/agents');
    if (res.ok) {
      agents = Array.isArray(res.data) ? res.data : (res.data?.agents || []);
    }
  }

  async function sleepAgent(name) {
    const delaySeconds = 5;
    const res = await apiFetch(`/agents/${encodeURIComponent(name)}/sleep`, { method: 'POST', body: JSON.stringify({ delay_seconds: delaySeconds }) });
    if (!res.ok) { error = res?.data?.message || 'Sleep failed'; return; }
    // Give the controller time to perform delayed sleep before refreshing
    await new Promise((r) => setTimeout(r, (delaySeconds * 1000) + 500));
    await refresh();
  }
  async function wakeAgent(name) {
    const res = await apiFetch(`/agents/${encodeURIComponent(name)}/wake`, { method: 'POST', body: JSON.stringify({}) });
    if (!res.ok) { error = res?.data?.message || 'Wake failed'; return; }
    await refresh();
  }
  async function publishAgent(name) {
    const res = await apiFetch(`/agents/${encodeURIComponent(name)}/publish`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ code: true, secrets: true, content: true }) });
    if (!res.ok) { error = res?.data?.message || 'Publish failed'; return; }
    await refresh();
  }
  async function unpublishAgent(name) {
    const res = await apiFetch(`/agents/${encodeURIComponent(name)}/unpublish`, { method: 'POST' });
    if (!res.ok) { error = res?.data?.message || 'Unpublish failed'; return; }
    await refresh();
  }
  async function remixAgent(name) {
    const newName = prompt('New Agent Name for Remix');
    if (!newName) return;
    const body = { name: newName.trim(), code: true, secrets: true, content: true };
    const res = await apiFetch(`/agents/${encodeURIComponent(name)}/remix`, { method: 'POST', body: JSON.stringify(body) });
    if (!res.ok) { error = res?.data?.message || 'Remix failed'; return; }
    await refresh();
  }
  async function deleteAgent(name) {
    const ok = confirm(`Delete agent '${name}'? This cannot be undone.`);
    if (!ok) return;
    const res = await apiFetch(`/agents/${encodeURIComponent(name)}`, { method: 'DELETE' });
    if (!res.ok) { error = res?.data?.message || 'Delete failed'; return; }
    await refresh();
  }

  let pollHandle = null;
  function startPolling() {
    stopPolling();
    pollHandle = setInterval(async () => { try { await refresh(); } catch (_) {} }, 2000);
  }
  function stopPolling() {
    if (pollHandle) { clearInterval(pollHandle); pollHandle = null; }
  }

  onMount(async () => {
    if (!isAuthenticated()) {
      goto('/login');
      return;
    }
    try { operatorName = getOperatorName() || ''; } catch (_) { operatorName = ''; }
    const res = await apiFetch('/agents');
    if (!res.ok) {
      error = `Failed to load agents (HTTP ${res.status})`;
    } else {
      agents = Array.isArray(res.data) ? res.data : (res.data?.agents || []);
    }
    loading = false;
    startPolling();
  });

  onDestroy(() => { stopPolling(); });
</script>

<div class="container-xxl">
  <div class="row justify-content-center">
    <div class="col-12 col-xxl-10">
{#if isAdmin}
  <div class="alert alert-info d-flex align-items-center" role="alert">
    <div>
      You are logged in as <strong>{operatorName || 'admin'}</strong>. Please create a token here and use the system as a user.
      <a href="/tokens" class="ms-1">Open Tokens</a>
    </div>
  </div>
{/if}
<div class="d-flex align-items-center mb-3">
  <div class="fw-bold">Agents</div>
  <div class="ms-auto d-flex align-items-center gap-2">
    <div class="small text-body text-opacity-75">{agents?.length || 0} total</div>
    <a href="/agents/create" class="btn btn-theme btn-sm"><i class="bi bi-plus me-1"></i>Create Agent</a>
  </div>
</div>

<div>
        {#if loading}
          <div class="d-flex align-items-center justify-content-center" style="min-height: 30vh;">
            <div class="text-center text-body text-opacity-75">
              <div class="spinner-border text-theme mb-3"></div>
              <div>Loading agents…</div>
            </div>
          </div>
        {:else if error}
          <div class="alert alert-danger small">{error}</div>
        {:else if !agents || agents.length === 0}
          <div class="text-body text-opacity-75">No agents found.</div>
          <div class="mt-3">
            <a href="/agents/create" class="btn btn-theme"><i class="bi bi-plus me-1"></i>Create your first agent</a>
          </div>
        {:else}
          <div class="row g-3">
            {#each agents as a}
              <div class="col-12 col-sm-6 col-lg-4">
                <Card class="h-100">
                  <div class="card-body d-flex flex-column">
                    <div class="d-flex align-items-center gap-2 mb-1">
                      <a class="fw-bold text-decoration-none" href={'/agents/' + encodeURIComponent(a.name || '')}>{a.name || '-'}</a>
                    </div>
                    <div class="small text-body text-opacity-75 flex-grow-1 text-truncate" title={a.description || a.desc || ''}>{a.description || a.desc || 'No description'}</div>
                    {#if isAdmin}
                      <div class="small text-body-secondary mt-1">Owner: <span class="font-monospace">{a.created_by}</span></div>
                    {/if}
                    <div class="mt-2 d-flex flex-wrap gap-1">
                      {#if Array.isArray(a.tags) && a.tags.length}
                        {#each a.tags as t}
                          <span class="badge bg-secondary-subtle text-secondary-emphasis border">{t}</span>
                        {/each}
                      {:else}
                        <span class="text-body-secondary small">No tags</span>
                      {/if}
                    </div>
                    <!-- In-card actions: status on left, buttons on right; no Open button -->
                    <div class="mt-2 d-flex align-items-center flex-wrap">
                      <div class="d-flex align-items-center gap-2">
                        <i class={`${stateIconClass(a.state || a.status)} me-1`}></i>
                        <span class="text-uppercase small fw-bold text-body">{a.state || a.status || 'unknown'}</span>
                      </div>
                      <div class="ms-auto d-flex align-items-center flex-wrap gap-2">
                        {#if (a.state || '').toLowerCase() === 'slept'}
                          <button class="btn btn-outline-success btn-sm" on:click={() => wakeAgent(a.name)} aria-label="Wake agent">
                            <i class="fas fa-sun me-1"></i><span>Wake</span>
                          </button>
                        {:else if ['idle','busy'].includes(String(a.state||'').toLowerCase())}
                          <button class="btn btn-outline-warning btn-sm" on:click={() => sleepAgent(a.name)} aria-label="Put agent to sleep">
                            <i class="fas fa-moon me-1"></i><span>Sleep</span>
                          </button>
                        {/if}
                        {#if a.is_published}
                          <div class="dropdown">
                          <button class="btn btn-success btn-sm fw-bold dropdown-toggle published-toggle" type="button" data-bs-toggle="dropdown" aria-expanded="false" aria-label="Published options">
                              <i class="fas fa-globe me-1"></i><span>Published</span>
                          </button>
                            <ul class="dropdown-menu dropdown-menu-end">
                              <li>
                                <a class="dropdown-item" href={`${getHostUrl()}/content/${a?.name || ''}/`} target="_blank" rel="noopener noreferrer"><i class="fas fa-up-right-from-square me-2"></i>Open Public URL</a>
                              </li>
                              <li>
                                <button class="dropdown-item" on:click={() => publishAgent(a.name)}><i class="fas fa-cloud-arrow-up me-2"></i>Publish New Version</button>
                              </li>
                              <li>
                                <button class="dropdown-item text-danger" on:click={() => unpublishAgent(a.name)}><i class="fas fa-eye-slash me-2"></i>Unpublish</button>
                              </li>
                            </ul>
                          </div>
                        {:else}
                          <button class="btn btn-outline-primary btn-sm" on:click={() => publishAgent(a.name)} aria-label="Publish content">
                            <i class="fas fa-cloud-arrow-up me-1"></i><span>Publish</span>
                          </button>
                        {/if}
                        <div class="dropdown">
                          <button class="btn btn-outline-secondary btn-sm" type="button" data-bs-toggle="dropdown" aria-expanded="false" aria-label="More actions">
                            <i class="bi bi-three-dots"></i>
                          </button>
                          <ul class="dropdown-menu dropdown-menu-end">
                            <li><button class="dropdown-item" on:click={() => remixAgent(a.name)}><i class="fas fa-code-branch me-2"></i>Remix</button></li>
                            <li><button class="dropdown-item" on:click={() => goto('/agents/' + encodeURIComponent(a.name))}><i class="fas fa-tags me-2"></i>Edit Tags</button></li>
                            <li><hr class="dropdown-divider" /></li>
                            <li><button class="dropdown-item text-danger" on:click={() => deleteAgent(a.name)}><i class="fas fa-trash me-2"></i>Delete</button></li>
                          </ul>
                        </div>
                      </div>
</div>
</div>

<style>
  /* Ensure dropdown menus overlay adjacent buttons on the list cards */
  :global(.dropdown-menu) { z-index: 5000; }
  :global(.card) { overflow: visible; }
  .text-truncate { display: block; }
</style>
                </Card>
              </div>
            {/each}
          </div>
        {/if}
    </div>
  </div>
</div>
</div>
