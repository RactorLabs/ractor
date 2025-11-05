<script>
  import { onMount, onDestroy } from 'svelte';
  import { goto } from '$app/navigation';
  import { isAuthenticated } from '$lib/auth.js';
  import { apiFetch } from '$lib/api/client.js';
  import Card from '/src/components/bootstrap/Card.svelte';
  import { setPageTitle } from '$lib/utils.js';

  setPageTitle('Snapshots');

  let loading = true;
  let error = null;
  let snapshots = [];
  // Filters + pagination
  let q = '';
  let limit = 30;
  let pageNum = 1; // 1-based
  let total = 0;
  let pages = 1;
  import { auth, getOperatorName } from '$lib/auth.js';
  let operatorName = '';
  $: isAdmin = $auth && String($auth.type || '').toLowerCase() === 'admin';

  function triggerTypeLabel(trigger) {
    const t = String(trigger || '').toLowerCase();
    if (t === 'manual') return 'Manual';
    if (t === 'auto') return 'Auto';
    return trigger || 'Unknown';
  }

  function triggerTypeBadgeClass(trigger) {
    const t = String(trigger || '').toLowerCase();
    if (t === 'manual') return 'badge bg-primary-subtle text-primary-emphasis border';
    if (t === 'auto') return 'badge bg-info-subtle text-info-emphasis border';
    return 'badge bg-secondary-subtle text-secondary-emphasis border';
  }

  function buildQuery() {
    const params = new URLSearchParams();
    if (q && q.trim().length) params.set('q', q.trim());
    if (limit) params.set('limit', String(limit));
    if (pageNum) params.set('page', String(pageNum));
    return params.toString();
  }

  async function fetchSnapshots() {
    const qs = buildQuery();
    const res = await apiFetch(`/snapshots?${qs}`);
    if (!res.ok) {
      error = res?.data?.message || `Failed to load snapshots (HTTP ${res.status})`;
      loading = false;
      return;
    }
    const data = res.data || {};
    snapshots = Array.isArray(data.items) ? data.items : [];
    total = Number(data.total || 0);
    limit = Number(data.limit || limit);
    const offset = Number(data.offset || 0);
    pageNum = Number(data.page || (limit ? (Math.floor(offset / limit) + 1) : 1));
    pages = Number(data.pages || (limit ? Math.max(1, Math.ceil(total / limit)) : 1));
  }

  async function deleteSnapshot(snapshot) {
    const ok = confirm(`Delete snapshot '${snapshot.id || ''}'? This cannot be undone.`);
    if (!ok) return;
    const res = await apiFetch(`/snapshots/${encodeURIComponent(snapshot.id)}`, { method: 'DELETE' });
    if (!res.ok) { error = res?.data?.message || 'Delete failed'; return; }
    await fetchSnapshots();
  }

  async function createFromSnapshot(snapshot) {
    const ok = confirm(`Create a new sandbox from snapshot '${snapshot.id || ''}'?`);
    if (!ok) return;
    const res = await apiFetch(`/snapshots/${encodeURIComponent(snapshot.id)}/create`, { method: 'POST', body: JSON.stringify({}) });
    if (!res.ok) { error = res?.data?.message || 'Create from snapshot failed'; return; }
    const newSandbox = res.data;
    if (newSandbox && newSandbox.id) {
      goto(`/sandboxes/${encodeURIComponent(newSandbox.id)}`);
    } else {
      await fetchSnapshots();
    }
  }

  let pollHandle = null;
  function startPolling() {
    stopPolling();
    const filtersActive = (q && q.trim());
    if (!filtersActive && pageNum === 1) {
      pollHandle = setInterval(async () => { try { await fetchSnapshots(); } catch (_) {} }, 5000);
    }
  }
  function stopPolling() {
    if (pollHandle) { clearInterval(pollHandle); pollHandle = null; }
  }

  function syncUrl() {
    try {
      const qs = buildQuery();
      const url = qs ? `/snapshots?${qs}` : '/snapshots';
      goto(url, { replaceState: true, keepfocus: true, noScroll: true });
    } catch (_) {}
  }

  async function applyFilters() {
    pageNum = 1;
    syncUrl();
    loading = true;
    await fetchSnapshots();
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
      limit = Number(sp.get('limit') || 30);
      pageNum = Number(sp.get('page') || 1);
    } catch (_) {}
    await fetchSnapshots();
    loading = false;
    startPolling();
  });

  onDestroy(() => { stopPolling(); });

  function formatDate(isoString) {
    try {
      const d = new Date(isoString);
      return d.toLocaleString();
    } catch (_) {
      return isoString;
    }
  }
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
  <div class="fw-bold fs-20px">Snapshots</div>

  <!-- Filters row -->
  <div class="w-100"></div>
  <div class="w-100 mb-2">
    <form class="row g-2" on:submit|preventDefault={applyFilters}>
      <div class="col-12 col-md-6">
        <div class="input-group input-group-sm flex-nowrap">
          <span class="input-group-text bg-body-secondary border-0"><i class="bi bi-search"></i></span>
          <input class="form-control" placeholder="Search by ID or sandbox ID" bind:value={q} name="q" autocapitalize="none" />
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
              <div>Loading snapshots…</div>
            </div>
          </div>
        {:else if error}
          <div class="alert alert-danger small">{error}</div>
        {:else if !snapshots || snapshots.length === 0}
          <div class="text-body text-opacity-75">No snapshots found.</div>
          <div class="mt-3">
            <a href="/sandboxes" class="btn btn-outline-theme"><i class="bi bi-box me-1"></i>View Sandboxes</a>
          </div>
        {:else}
          <div class="row g-3">
            {#each snapshots as s}
              <div class="col-12">
                <Card class="h-100">
                  <div class="card-body">
                    <div class="row">
                      <div class="col-md-8">
                        <div class="d-flex align-items-center gap-2 mb-1">
                          <span class="fw-bold text-decoration-none fs-18px font-monospace">{s.id || '-'}</span>
                          <span class={triggerTypeBadgeClass(s.trigger_type)}>{triggerTypeLabel(s.trigger_type)}</span>
                        </div>
                        <div class="small text-body text-opacity-75 mb-1">
                          Source Sandbox: <a href="/sandboxes/{encodeURIComponent(s.sandbox_id || '')}" class="font-monospace">{s.sandbox_id || '-'}</a>
                        </div>
                        <div class="small text-body text-opacity-50">
                          Created: {formatDate(s.created_at)}
                        </div>
                        {#if isAdmin && s.created_by}
                          <div class="small text-body-secondary mt-1">Owner: <span class="font-monospace">{s.created_by}</span></div>
                        {/if}
                      </div>
                      <div class="col-md-4 d-flex align-items-center justify-content-md-end mt-2 mt-md-0">
                        <div class="d-flex align-items-center flex-wrap gap-2">
                          <button class="btn btn-outline-theme btn-sm" on:click={() => createFromSnapshot(s)} aria-label="Create from snapshot">
                            <i class="bi bi-plus-circle me-1"></i><span>Create Sandbox</span>
                          </button>
                          <button class="btn btn-outline-danger btn-sm" on:click={() => deleteSnapshot(s)} aria-label="Delete snapshot">
                            <i class="bi bi-trash me-1"></i><span>Delete</span>
                          </button>
                        </div>
                      </div>
                    </div>
                  </div>
                </Card>
              </div>
            {/each}
          </div>
          {#if pages > 1}
          <div class="d-flex align-items-center justify-content-center mt-3 gap-1">
            <button class="btn btn-sm btn-outline-secondary" disabled={pageNum <= 1} on:click={async () => { pageNum = Math.max(1, pageNum-1); syncUrl(); loading = true; await fetchSnapshots(); loading = false; startPolling(); }}>Prev</button>
            {#each Array(pages) as _, idx}
              {#if Math.abs((idx+1) - pageNum) <= 2 || idx === 0 || idx+1 === pages}
                <button class={`btn btn-sm ${idx+1===pageNum ? 'btn-theme' : 'btn-outline-secondary'}`} on:click={async () => { pageNum = idx+1; syncUrl(); loading = true; await fetchSnapshots(); loading = false; startPolling(); }}>{idx+1}</button>
              {:else if Math.abs((idx+1) - pageNum) === 3}
                <span class="px-1">…</span>
              {/if}
            {/each}
            <button class="btn btn-sm btn-outline-secondary" disabled={pageNum >= pages} on:click={async () => { pageNum = Math.min(pages, pageNum+1); syncUrl(); loading = true; await fetchSnapshots(); loading = false; startPolling(); }}>Next</button>
          </div>
          {/if}
        {/if}
    </div>
  </div>
</div>
</div>
