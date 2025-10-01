<script>
  import { onMount, onDestroy, tick } from 'svelte';
  import { goto } from '$app/navigation';
  import { isAuthenticated } from '$lib/auth.js';
  import { apiFetch } from '$lib/api/client.js';
  import Card from '/src/components/bootstrap/Card.svelte';
  import { setPageTitle } from '$lib/utils.js';

  setPageTitle('Agents');

  let loading = true;
  let error = null;
  let agents = [];
  // Filters + pagination
  let q = '';
  let stateFilter = '';
  let tagsText = '';
  let limit = 30;
  let pageNum = 1; // 1-based
  let total = 0;
  let pages = 1;
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
    if (s === 'slept') return 'bi bi-moon';
    if (s === 'idle') return 'bi bi-sun';
    if (s === 'busy') return 'spinner-border spinner-border-sm';
    if (s === 'init') return 'spinner-border spinner-border-sm';
    return 'bi bi-circle';
  }

import { getHostUrl } from '$lib/branding.js';

  function buildQuery() {
    const params = new URLSearchParams();
    if (q && q.trim().length) params.set('q', q.trim());
    if (stateFilter && stateFilter.trim().length) params.set('state', stateFilter.trim());
    if (tagsText && tagsText.trim().length) {
      const tags = tagsText.split(',').map(t => t.trim().toLowerCase()).filter(Boolean);
      if (tags.length) params.set('tags', tags.join(','));
    }
    if (limit) params.set('limit', String(limit));
    if (pageNum) params.set('page', String(pageNum));
    return params.toString();
  }

  async function fetchAgents() {
    const qs = buildQuery();
    const res = await apiFetch(`/agents?${qs}`);
    if (!res.ok) {
      error = res?.data?.message || `Failed to load agents (HTTP ${res.status})`;
      loading = false;
      return;
    }
    const data = res.data || {};
    agents = Array.isArray(data.items) ? data.items : [];
    total = Number(data.total || 0);
    limit = Number(data.limit || limit);
    const offset = Number(data.offset || 0);
    pageNum = Number(data.page || (limit ? (Math.floor(offset / limit) + 1) : 1));
    pages = Number(data.pages || (limit ? Math.max(1, Math.ceil(total / limit)) : 1));
  }

  async function sleepAgent(name) {
    let delaySeconds = 5;
    try {
      const input = prompt('Sleep in how many seconds? (min 5)', '5');
      if (input !== null) {
        const n = Math.floor(Number(input));
        if (Number.isFinite(n)) delaySeconds = Math.max(5, n);
      }
    } catch (_) {}
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
    const res = await apiFetch(`/agents/${encodeURIComponent(name)}/publish`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ code: true, env: true, content: true }) });
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
    const body = { name: newName.trim(), code: true, env: true, content: true };
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
    const filtersActive = (q && q.trim()) || (tagsText && tagsText.trim()) || (stateFilter && stateFilter.trim());
    if (!filtersActive && pageNum === 1) {
      pollHandle = setInterval(async () => { try { await fetchAgents(); } catch (_) {} }, 3000);
    }
  }
  function stopPolling() {
    if (pollHandle) { clearInterval(pollHandle); pollHandle = null; }
  }

  function syncUrl() {
    try {
      const qs = buildQuery();
      const url = qs ? `/agents?${qs}` : '/agents';
      goto(url, { replaceState: true, keepfocus: true, noScroll: true });
    } catch (_) {}
  }

  async function applyFilters() {
    pageNum = 1;
    syncUrl();
    loading = true;
    await fetchAgents();
    loading = false;
    startPolling();
  }

  onMount(async () => {
    if (!isAuthenticated()) {
      goto('/login');
      return;
    }
    try { operatorName = getOperatorName() || ''; } catch (_) { operatorName = ''; }
    // Seed from URL
    try {
      const sp = new URLSearchParams(location.search || '');
      q = sp.get('q') || '';
      stateFilter = sp.get('state') || '';
      const t = [...sp.getAll('tags[]'), ...sp.getAll('tags')];
      tagsText = t && t.length ? t.join(',') : '';
      limit = Number(sp.get('limit') || 30);
      pageNum = Number(sp.get('page') || 1);
    } catch (_) {}
    await fetchAgents();
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
<div class="d-flex align-items-center flex-wrap gap-2 mb-2">
  <div class="fw-bold fs-20px">Agents</div>
  <div class="ms-auto d-flex align-items-center gap-2">
    <a href="/agents/create" class="btn btn-outline-theme btn-sm"><i class="bi bi-plus me-1"></i>Create Agent</a>
  </div>
  
  <!-- Filters row -->
  <div class="w-100"></div>
  <div class="w-100 mb-2">
    <form class="row g-2" on:submit|preventDefault={applyFilters}>
      <div class="col-12 col-md-6">
        <div class="input-group input-group-sm flex-nowrap">
          <span class="input-group-text bg-body-secondary border-0"><i class="bi bi-search"></i></span>
          <input class="form-control" placeholder="Search by name or description" bind:value={q} name="q" autocapitalize="none" />
        </div>
      </div>
      <div class="col-6 col-md-3">
        <div class="input-group input-group-sm flex-nowrap">
          <span class="input-group-text bg-body-secondary border-0"><i class="bi bi-activity"></i></span>
          <select class="form-select form-select-sm" bind:value={stateFilter} aria-label="State filter" name="state">
            <option value="">All states</option>
            <option value="init">init</option>
            <option value="idle">idle</option>
            <option value="busy">busy</option>
            <option value="slept">slept</option>
          </select>
        </div>
      </div>
      <div class="col-6 col-md-3">
        <div class="input-group input-group-sm flex-nowrap">
          <span class="input-group-text bg-body-secondary border-0"><i class="bi bi-tags"></i></span>
          <input class="form-control" placeholder="tags,comma,separated" bind:value={tagsText} name="tags" autocapitalize="none" />
        </div>
      </div>
      <div class="col-12 col-md-auto">
        <button type="submit" class="btn btn-outline-secondary btn-sm w-100"><i class="bi bi-funnel me-1"></i>Apply Filter</button>
      </div>
      <!-- Desktop total aligned to far right -->
      <div class="col-12 col-md-auto ms-md-auto d-none d-md-flex align-items-center">
        <div class="small text-body text-opacity-75">{total} total</div>
      </div>
      <div class="col-12 d-md-none">
        <div class="small text-body text-opacity-75">{total} total</div>
      </div>
    </form>
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
            <a href="/agents/create" class="btn btn-outline-theme"><i class="bi bi-plus me-1"></i>Create your first agent</a>
          </div>
        {:else}
          <div class="row g-3">
            {#each agents as a}
              <div class="col-12 col-sm-6 col-lg-4">
                <Card class="h-100">
                  <div class="card-body d-flex flex-column">
                    <div class="d-flex align-items-center gap-2 mb-1">
                      <a class="fw-bold text-decoration-none fs-18px" href={'/agents/' + encodeURIComponent(a.name || '')}>{a.name || '-'}</a>
                      {#if a.is_published}
                        <a class="small ms-1 text-decoration-none text-body-secondary" href={`${getHostUrl()}/content/${a?.name || ''}/`} target="_blank" rel="noopener noreferrer">(public link)</a>
                      {/if}
                    </div>
                    <div class="small text-body text-opacity-75 flex-grow-1 text-truncate" title={a.description || a.desc || ''}>{a.description || a.desc || 'No description'}</div>
                    {#if isAdmin}
                      <div class="small text-body-secondary mt-1">Owner: <span class="font-monospace">{a.created_by}</span></div>
                    {/if}
                    <!-- Public URL in main card list -->
                    
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
                      <div class="ms-auto d-flex align-items-center flex-wrap gap-2 list-actions">
                        {#if ['idle','busy'].includes(String(a.state||'').toLowerCase())}
                          <button class="btn btn-outline-primary btn-sm" on:click={() => sleepAgent(a.name)} aria-label="Put agent to sleep">
                            <i class="bi bi-moon me-1"></i><span>Sleep</span>
                          </button>
                        {/if}
                        {#if a.is_published}
                          <div class="dropdown">
                          <button class="btn btn-outline-success btn-sm fw-bold dropdown-toggle published-toggle" type="button" data-bs-toggle="dropdown" data-bs-display="static" aria-expanded="false" aria-label="Published options">
                              <i class="bi bi-globe me-1"></i><span>Published</span>
                          </button>
                            <ul class="dropdown-menu dropdown-menu-end">
                              <li>
                                <a class="dropdown-item" href={`${getHostUrl()}/content/${a?.name || ''}/`} target="_blank" rel="noopener noreferrer"><i class="bi bi-box-arrow-up-right me-2"></i>Open Public URL</a>
                              </li>
                              <li>
                                <button class="dropdown-item" on:click={() => publishAgent(a.name)}><i class="bi bi-cloud-arrow-up me-2"></i>Publish New Version</button>
                              </li>
                              <li>
                                <button class="dropdown-item text-danger" on:click={() => unpublishAgent(a.name)}><i class="bi bi-eye-slash me-2"></i>Unpublish</button>
                              </li>
                            </ul>
                          </div>
                        {:else}
                          <button type="button" class="btn btn-outline-secondary btn-sm" on:click={() => publishAgent(a.name)} aria-label="Publish content">
                            <i class="bi bi-cloud-arrow-up me-1"></i><span>Publish</span>
                          </button>
                        {/if}
                        <div class="dropdown">
                          <button class="btn btn-outline-secondary btn-sm" type="button" data-bs-toggle="dropdown" data-bs-display="static" aria-expanded="false" aria-label="More actions">
                            <i class="bi bi-three-dots"></i>
                          </button>
                          <ul class="dropdown-menu dropdown-menu-end">
                            <li><button class="dropdown-item" on:click={() => remixAgent(a.name)}><i class="bi bi-shuffle me-2"></i>Remix</button></li>
                            <li><button class="dropdown-item" on:click={() => goto('/agents/' + encodeURIComponent(a.name))}><i class="bi bi-tags me-2"></i>Edit Tags</button></li>
                            <li><hr class="dropdown-divider" /></li>
                            <li><button class="dropdown-item text-danger" on:click={() => deleteAgent(a.name)}><i class="bi bi-trash me-2"></i>Delete</button></li>
                          </ul>
                        </div>
                      </div>
</div>
</div>

<style>
  /* Ensure dropdown menus overlay adjacent cards */
  :global(.card) { overflow: visible; }
  :global(.list-actions) { position: relative; z-index: 3001; isolation: isolate; }
  :global(.list-actions .dropdown-menu) {
    position: absolute;
    z-index: 9999 !important;
    /* Ensure menu obscures underlying card text on dark/light themes */
    background-color: var(--bs-dropdown-bg, var(--bs-card-bg, var(--bs-body-bg, #fff)));
    box-shadow: 0 .5rem 1rem rgba(0,0,0,.25);
    backdrop-filter: none;
  }
  :global(.list-actions .dropdown-item) {
    background-color: inherit;
  }
  /* Prevent decorative card arrow from overlapping dropdowns */
  :global(.card .card-arrow) { z-index: 0; pointer-events: none; }
  .text-truncate { display: block; }
</style>
                </Card>
              </div>
            {/each}
          </div>
          {#if pages > 1}
          <div class="d-flex align-items-center justify-content-center mt-3 gap-1">
            <button class="btn btn-sm btn-outline-secondary" disabled={pageNum <= 1} on:click={async () => { pageNum = Math.max(1, pageNum-1); syncUrl(); loading = true; await fetchAgents(); loading = false; startPolling(); }}>Prev</button>
            {#each Array(pages) as _, idx}
              {#if Math.abs((idx+1) - pageNum) <= 2 || idx === 0 || idx+1 === pages}
                <button class={`btn btn-sm ${idx+1===pageNum ? 'btn-theme' : 'btn-outline-secondary'}`} on:click={async () => { pageNum = idx+1; syncUrl(); loading = true; await fetchAgents(); loading = false; startPolling(); }}>{idx+1}</button>
              {:else if Math.abs((idx+1) - pageNum) === 3}
                <span class="px-1">…</span>
              {/if}
            {/each}
            <button class="btn btn-sm btn-outline-secondary" disabled={pageNum >= pages} on:click={async () => { pageNum = Math.min(pages, pageNum+1); syncUrl(); loading = true; await fetchAgents(); loading = false; startPolling(); }}>Next</button>
          </div>
          {/if}
        {/if}
    </div>
  </div>
</div>
</div>
