<script>
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { isAuthenticated, getToken } from '$lib/auth.js';
  import { apiFetch } from '$lib/api/client.js';
  import { setPageTitle } from '$lib/utils.js';

  setPageTitle('Agents');

  let loading = true;
  let error = null;
  let agents = [];

  function stateClass(state) {
    const s = String(state || '').toLowerCase();
    if (s === 'init') return 'badge rounded-pill bg-light text-dark';
    if (s === 'idle') return 'badge rounded-pill bg-success';
    if (s === 'busy') return 'badge rounded-pill bg-warning text-dark';
    return 'badge rounded-pill bg-secondary';
  }

  onMount(async () => {
    // Guard: if not authed, redirect to login
    if (!isAuthenticated()) {
      goto('/login');
      return;
    }

    const res = await apiFetch('/agents');
    if (!res.ok) {
      error = `Failed to load agents (HTTP ${res.status})`;
    } else {
      agents = Array.isArray(res.data) ? res.data : (res.data?.agents || []);
    }
    loading = false;
  });
</script>

<div class="d-flex align-items-center mb-3">
  <div class="fw-bold">Agents</div>
  <div class="ms-auto small text-body text-opacity-75">{agents?.length || 0} total</div>
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
        {:else}
          <div class="row g-3">
            {#each agents as a}
              <div class="col-12 col-sm-6 col-lg-4">
                <Card class="h-100">
                  <div class="card-body d-flex flex-column">
                    <div class="d-flex align-items-center gap-2 mb-1">
                      <a class="fw-bold text-decoration-none" href={'/agents/' + encodeURIComponent(a.name || '')}>{a.name || '-'}</a>
                      <span class={stateClass(a.state || a.status)}>{a.state || a.status || 'unknown'}</span>
                    </div>
                    <div class="small text-body text-opacity-75 flex-grow-1">{a.description || a.desc || 'No description'}</div>
                    <div class="mt-2 d-flex gap-2">
                      <a class="btn btn-sm btn-outline-theme" href={'/agents/' + encodeURIComponent(a.name || '')}>
                        Open
                      </a>
                    </div>
                  </div>
                </Card>
              </div>
            {/each}
          </div>
        {/if}
</div>
