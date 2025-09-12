<script>
  import { onMount } from 'svelte';
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

  import { getHostUrl } from '$lib/branding.js';
  import { toast } from '/src/stores/toast.js';

  async function refresh() {
    const res = await apiFetch('/agents');
    if (res.ok) {
      agents = Array.isArray(res.data) ? res.data : (res.data?.agents || []);
    }
  }

  async function sleepAgent(name) {
    const res = await apiFetch(`/agents/${encodeURIComponent(name)}/sleep`, { method: 'POST' });
    if (!res.ok) return toast.error(res?.data?.message || 'Sleep failed');
    toast.success('Agent put to sleep');
    await refresh();
  }
  async function wakeAgent(name) {
    const res = await apiFetch(`/agents/${encodeURIComponent(name)}/wake`, { method: 'POST', body: JSON.stringify({}) });
    if (!res.ok) return toast.error(res?.data?.message || 'Wake failed');
    toast.success('Agent waking');
    await refresh();
  }
  async function publishAgent(name) {
    const res = await apiFetch(`/agents/${encodeURIComponent(name)}/publish`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ code: true, secrets: true, content: true }) });
    if (!res.ok) return toast.error(res?.data?.message || 'Publish failed');
    toast.success('Agent published');
    await refresh();
  }
  async function unpublishAgent(name) {
    const res = await apiFetch(`/agents/${encodeURIComponent(name)}/unpublish`, { method: 'POST' });
    if (!res.ok) return toast.error(res?.data?.message || 'Unpublish failed');
    toast.success('Agent unpublished');
    await refresh();
  }
  async function remixAgent(name) {
    const newName = prompt('New Agent Name for Remix');
    if (!newName) return;
    const body = { name: newName.trim(), code: true, secrets: true, content: true };
    const res = await apiFetch(`/agents/${encodeURIComponent(name)}/remix`, { method: 'POST', body: JSON.stringify(body) });
    if (!res.ok) return toast.error(res?.data?.message || 'Remix failed');
    toast.success('Agent remixed');
    await refresh();
  }
  async function deleteAgent(name) {
    const ok = confirm(`Delete agent '${name}'? This cannot be undone.`);
    if (!ok) return;
    const res = await apiFetch(`/agents/${encodeURIComponent(name)}`, { method: 'DELETE' });
    if (!res.ok) return toast.error(res?.data?.message || 'Delete failed');
    toast.success('Agent deleted');
    await refresh();
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
  });
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
                    {#if Array.isArray(a.tags) && a.tags.length}
                      <div class="mt-2 d-flex flex-wrap gap-1">
                        {#each a.tags as t}
                          <span class="badge bg-secondary-subtle text-secondary-emphasis border">{t}</span>
                        {/each}
                      </div>
                    {/if}
                    <!-- In-card actions: status on left, buttons on right; no Open button -->
                    <div class="mt-2 d-flex align-items-center flex-wrap">
                      <div class="d-flex align-items-center gap-2">
                        <span class={`d-inline-block rounded-circle ${stateColorClass(a.state || a.status)} border`} style="width: 10px; height: 10px;"></span>
                        <span class="text-uppercase small fw-bold text-body">{a.state || a.status || 'unknown'}</span>
                      </div>
                      <div class="ms-auto d-flex align-items-center flex-wrap gap-2">
                        {#if (a.state || '').toLowerCase() === 'slept'}
                          <button class="btn btn-outline-success btn-sm" on:click={() => wakeAgent(a.name)} aria-label="Wake agent">Wake</button>
                        {:else if ['idle','busy'].includes(String(a.state||'').toLowerCase())}
                          <button class="btn btn-outline-warning btn-sm" on:click={() => sleepAgent(a.name)} aria-label="Put agent to sleep">Sleep</button>
                        {/if}
                        {#if a.is_published}
                          <div class="dropdown">
                            <button class="btn btn-success btn-sm fw-bold dropdown-toggle published-toggle" type="button" data-bs-toggle="dropdown" aria-expanded="false" aria-label="Published options">
                              Published
                            </button>
                            <ul class="dropdown-menu dropdown-menu-end">
                              <li>
                                <a class="dropdown-item" href={`${getHostUrl()}/content/${a?.name || ''}/`} target="_blank" rel="noopener noreferrer">Open Public URL ↗</a>
                              </li>
                              <li>
                                <button class="dropdown-item" on:click={() => publishAgent(a.name)}>Publish New Version</button>
                              </li>
                              <li>
                                <button class="dropdown-item text-danger" on:click={() => unpublishAgent(a.name)}>Unpublish</button>
                              </li>
                            </ul>
                          </div>
                        {:else}
                          <button class="btn btn-outline-primary btn-sm" on:click={() => publishAgent(a.name)} aria-label="Publish content">Publish</button>
                        {/if}
                        <div class="dropdown">
                          <button class="btn btn-outline-secondary btn-sm" type="button" data-bs-toggle="dropdown" aria-expanded="false" aria-label="More actions">
                            <i class="bi bi-three-dots"></i>
                          </button>
                          <ul class="dropdown-menu dropdown-menu-end">
                            <li><button class="dropdown-item" on:click={() => remixAgent(a.name)}>Remix</button></li>
                            <li><button class="dropdown-item" on:click={() => goto('/agents/' + encodeURIComponent(a.name))}>Edit Tags</button></li>
                            <li><hr class="dropdown-divider" /></li>
                            <li><button class="dropdown-item text-danger" on:click={() => deleteAgent(a.name)}>Delete</button></li>
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
