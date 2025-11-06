<script>
  import { onMount, onDestroy, tick } from 'svelte';
  import { goto } from '$app/navigation';
  import { isAuthenticated } from '$lib/auth.js';
  import { apiFetch } from '$lib/api/client.js';
  import Card from '/src/components/bootstrap/Card.svelte';
  import { setPageTitle } from '$lib/utils.js';

  setPageTitle('Sandboxes');

  let loading = true;
  let error = null;
  let sandboxes = [];
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
    if (s === 'deleted') return 'badge rounded-pill bg-transparent border border-danger text-danger';
    return 'badge rounded-pill bg-transparent border border-secondary text-secondary';
  }
  function stateColorClass(state) {
    const s = String(state || '').toLowerCase();
    if (s === 'idle') return 'bg-success border-success';
    if (s === 'busy') return 'bg-warning border-warning';
    if (s === 'init') return 'bg-secondary border-secondary';
    if (s === 'deleted') return 'bg-danger border-danger';
    return 'bg-secondary border-secondary';
  }

  function stateIconClass(state) {
    const s = String(state || '').toLowerCase();
    if (s === 'deleted') return 'bi bi-trash';
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

  async function fetchSandboxes() {
    const qs = buildQuery();
    const res = await apiFetch(`/sandboxes?${qs}`);
    if (!res.ok) {
      error = res?.data?.message || `Failed to load sandboxes (HTTP ${res.status})`;
      loading = false;
      return;
    }
    const data = res.data || {};
    sandboxes = Array.isArray(data.items) ? data.items : [];
    total = Number(data.total || 0);
    limit = Number(data.limit || limit);
    const offset = Number(data.offset || 0);
    pageNum = Number(data.page || (limit ? (Math.floor(offset / limit) + 1) : 1));
    pages = Number(data.pages || (limit ? Math.max(1, Math.ceil(total / limit)) : 1));
  }

  async function deleteSandbox(sandbox) {
    const ok = confirm(`Delete sandbox '${sandbox.id || ''}'? This cannot be undone.`);
    if (!ok) return;
    const res = await apiFetch(`/sandboxes/${encodeURIComponent(sandbox.id)}`, { method: 'DELETE' });
    if (!res.ok) { error = res?.data?.message || 'Delete failed'; return; }
    await fetchSandboxes();
  }

  // Edit Timeouts modal state and actions
  let showTimeoutsModal = false;
  let idleTimeoutInput = 900;
  let currentSandbox = null;
  function openEditTimeouts(sandbox) {
    currentSandbox = sandbox;
    const idle = Number(sandbox?.idle_timeout_seconds ?? 900);
    idleTimeoutInput = Number.isFinite(idle) && idle >= 0 ? idle : 900;
    showTimeoutsModal = true;
  }
  function closeEditTimeouts() { showTimeoutsModal = false; currentSandbox = null; }
  async function saveTimeouts() {
    if (!currentSandbox) return;
    try {
      const idle = Math.max(0, Math.floor(Number(idleTimeoutInput || 900)));
      const body = { idle_timeout_seconds: idle };
      const res = await apiFetch(`/sandboxes/${encodeURIComponent(currentSandbox.id)}`, { method: 'PUT', body: JSON.stringify(body) });
      if (!res.ok) throw new Error(res?.data?.message || res?.data?.error || `Update failed (HTTP ${res.status})`);
      showTimeoutsModal = false;
      currentSandbox = null;
      await fetchSandboxes();
    } catch (e) {
      alert(e.message || String(e));
    }
  }

  // Snapshot modal state and actions
  let showSnapshotModal = false;
  let snapshotError = null;
  function openSnapshotModal(sandbox) {
    currentSandbox = sandbox;
    snapshotError = null;
    showSnapshotModal = true;
  }
  function closeSnapshotModal() { showSnapshotModal = false; currentSandbox = null; }
  async function confirmCreateSnapshot() {
    if (!currentSandbox) return;
    try {
      snapshotError = null;
      const res = await apiFetch(`/sandboxes/${encodeURIComponent(currentSandbox.id)}/snapshots`, {
        method: 'POST',
        body: JSON.stringify({ trigger_type: 'manual' })
      });
      if (!res.ok) {
        snapshotError = res?.data?.message || res?.data?.error || `Snapshot creation failed (HTTP ${res.status})`;
        return;
      }
      showSnapshotModal = false;
      currentSandbox = null;
      // Redirect to snapshots page
      goto('/snapshots');
    } catch (e) {
      snapshotError = e.message || String(e);
    }
  }

  let pollHandle = null;
  function startPolling() {
    stopPolling();
    const filtersActive = (q && q.trim()) || (tagsText && tagsText.trim()) || (stateFilter && stateFilter.trim());
    if (!filtersActive && pageNum === 1) {
      pollHandle = setInterval(async () => { try { await fetchSandboxes(); } catch (_) {} }, 3000);
    }
  }
  function stopPolling() {
    if (pollHandle) { clearInterval(pollHandle); pollHandle = null; }
  }

  function syncUrl() {
    try {
      const qs = buildQuery();
      const url = qs ? `/sandboxes?${qs}` : '/sandboxes';
      goto(url, { replaceState: true, keepfocus: true, noScroll: true });
    } catch (_) {}
  }

  async function applyFilters() {
    pageNum = 1;
    syncUrl();
    loading = true;
    await fetchSandboxes();
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
    await fetchSandboxes();
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
  <div class="fw-bold fs-20px">Sandboxes</div>
  <div class="ms-auto d-flex align-items-center gap-2">
    <a href="/sandboxes/start" class="btn btn-outline-theme btn-sm"><i class="bi bi-plus me-1"></i>Start Sandbox</a>
  </div>

  <!-- Filters row -->
  <div class="w-100"></div>
  <div class="w-100 mb-2">
    <form class="row g-2" on:submit|preventDefault={applyFilters}>
      <div class="col-12 col-md-6">
        <div class="input-group input-group-sm flex-nowrap">
          <span class="input-group-text bg-body-secondary border-0"><i class="bi bi-search"></i></span>
          <input class="form-control" placeholder="Search by ID or description" bind:value={q} name="q" autocapitalize="none" />
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
            <option value="deleted">deleted</option>
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
              <div>Loading sandboxes…</div>
            </div>
          </div>
        {:else if error}
          <div class="alert alert-danger small">{error}</div>
        {:else if !sandboxes || sandboxes.length === 0}
          <div class="text-body text-opacity-75">No sandboxes found.</div>
          <div class="mt-3">
            <a href="/sandboxes/start" class="btn btn-outline-theme"><i class="bi bi-plus me-1"></i>Start your first sandbox</a>
          </div>
        {:else}
          <div class="row g-3">
            {#each sandboxes as a}
              <div class="col-12 col-md-6">
                <Card class="h-100">
                  <div class="card-body d-flex flex-column">
                    <div class="d-flex align-items-center gap-2 mb-1">
                      <a class="fw-bold text-decoration-none fs-18px font-monospace" href={'/sandboxes/' + encodeURIComponent(a.id || '')}>{a.id || '-'}</a>
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
                    <!-- In-card actions: status on left, buttons on right -->
                    <div class="mt-2 d-flex align-items-center flex-wrap">
                      <div class="d-flex align-items-center gap-2">
                        <i class={`${stateIconClass(a.state || a.status)} me-1`}></i>
                        <span class="text-uppercase small fw-bold text-body">{a.state || a.status || 'unknown'}</span>
                      </div>
                      <div class="ms-auto d-flex align-items-center flex-wrap gap-2 list-actions">
                        {#if ['idle','busy'].includes(String(a.state||'').toLowerCase())}
                          <button class="btn btn-outline-danger btn-sm" on:click={() => deleteSandbox(a)} aria-label="Delete sandbox">
                            <i class="bi bi-trash me-1"></i><span>Delete</span>
                          </button>
                        {/if}
                        <div class="dropdown">
                          <button class="btn btn-outline-secondary btn-sm" type="button" data-bs-toggle="dropdown" data-bs-display="static" aria-expanded="false" aria-label="More actions">
                            <i class="bi bi-three-dots"></i>
                          </button>
                          <ul class="dropdown-menu dropdown-menu-end">
                            <li><button class="dropdown-item" on:click={() => goto('/sandboxes/' + encodeURIComponent(a.id))}><i class="bi bi-tags me-2"></i>Edit Tags</button></li>
                            <li><button class="dropdown-item" on:click={() => openEditTimeouts(a)}><i class="bi bi-hourglass-split me-2"></i>Edit Timeouts</button></li>
                            <li><hr class="dropdown-divider" /></li>
                            <li><a class="dropdown-item" href="/snapshots?sandbox_id={a.id}"><i class="bi bi-images me-2"></i>View Snapshots</a></li>
                            <li><button class="dropdown-item" on:click={() => openSnapshotModal(a)}><i class="bi bi-camera me-2"></i>Create Snapshot</button></li>
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
    z-index: 3002 !important;
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
  /* Ensure modals always sit on top */
  :global(.modal) { z-index: 2000; }
  :global(.modal-backdrop) { z-index: 1990; }
</style>
                </Card>
              </div>
            {/each}
          </div>
          {#if pages > 1}
          <div class="d-flex align-items-center justify-content-center mt-3 gap-1">
            <button class="btn btn-sm btn-outline-secondary" disabled={pageNum <= 1} on:click={async () => { pageNum = Math.max(1, pageNum-1); syncUrl(); loading = true; await fetchSandboxes(); loading = false; startPolling(); }}>Prev</button>
            {#each Array(pages) as _, idx}
              {#if Math.abs((idx+1) - pageNum) <= 2 || idx === 0 || idx+1 === pages}
                <button class={`btn btn-sm ${idx+1===pageNum ? 'btn-theme' : 'btn-outline-secondary'}`} on:click={async () => { pageNum = idx+1; syncUrl(); loading = true; await fetchSandboxes(); loading = false; startPolling(); }}>{idx+1}</button>
              {:else if Math.abs((idx+1) - pageNum) === 3}
                <span class="px-1">…</span>
              {/if}
            {/each}
            <button class="btn btn-sm btn-outline-secondary" disabled={pageNum >= pages} on:click={async () => { pageNum = Math.min(pages, pageNum+1); syncUrl(); loading = true; await fetchSandboxes(); loading = false; startPolling(); }}>Next</button>
          </div>
          {/if}
        {/if}
    </div>
  </div>
</div>
</div>

<!-- Edit Timeouts Modal -->
{#if showTimeoutsModal}
  <div class="modal fade show" style="display: block; background: rgba(0,0,0,.3);" tabindex="-1" role="dialog" aria-modal="true">
    <div class="modal-dialog">
      <div class="modal-content">
        <div class="modal-header">
          <h5 class="modal-title">Edit Timeouts</h5>
          <button type="button" class="btn-close" aria-label="Close" on:click={closeEditTimeouts}></button>
        </div>
        <div class="modal-body">
          <div class="row g-3">
            <div class="col-12">
              <label class="form-label" for="idle-timeout">Idle Timeout (seconds)</label>
              <input id="idle-timeout" type="number" min="0" step="1" class="form-control" bind:value={idleTimeoutInput} />
              <div class="form-text">Time of inactivity before sandbox is automatically deleted. Minimum 60 seconds, recommended 900 (15 minutes). Set to 0 to disable.</div>
            </div>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn btn-outline-secondary" on:click={closeEditTimeouts}>Cancel</button>
          <button class="btn btn-theme" on:click={saveTimeouts}>Save</button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Create Snapshot Modal -->
{#if showSnapshotModal}
  <div class="modal fade show" style="display: block; background: rgba(0,0,0,.3);" tabindex="-1" role="dialog" aria-modal="true">
    <div class="modal-dialog">
      <div class="modal-content">
        <div class="modal-header">
          <h5 class="modal-title">Create Snapshot</h5>
          <button type="button" class="btn-close" aria-label="Close" on:click={closeSnapshotModal}></button>
        </div>
        <div class="modal-body">
          {#if snapshotError}
            <div class="alert alert-danger small">{snapshotError}</div>
          {/if}
          <p class="mb-2">Create a snapshot of this sandbox's current state. You can use snapshots to create new sandboxes later.</p>
        </div>
        <div class="modal-footer">
          <button class="btn btn-outline-secondary" on:click={closeSnapshotModal}>Cancel</button>
          <button class="btn btn-theme" on:click={confirmCreateSnapshot}><i class="bi bi-camera me-1"></i>Create Snapshot</button>
        </div>
      </div>
    </div>
  </div>
{/if}
