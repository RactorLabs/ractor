<script>
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { isAuthenticated, getToken } from '$lib/auth.js';
  import { apiFetch } from '$lib/api/client.js';
  import Card from '/src/components/bootstrap/Card.svelte';
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

<div class="row">
  <div class="col-12">
    <Card>
      <div class="card-header d-flex align-items-center">
        <div class="fw-bold">Agents</div>
        <div class="ms-auto small text-body text-opacity-75">{agents?.length || 0} total</div>
      </div>
      <div class="card-body">
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
          <div class="table-responsive">
            <table class="table table-sm align-middle">
              <thead>
                <tr>
                  <th class="text-nowrap">Name</th>
                  <th class="text-nowrap">State</th>
                  <th class="text-nowrap">Description</th>
                </tr>
              </thead>
              <tbody>
                {#each agents as a}
                  <tr>
                    <td class="font-monospace"><a class="text-decoration-none" href={'/agents/' + encodeURIComponent(a.name || a.id)}>{a.name || a.id || '-'}</a></td>
                    <td><span class={stateClass(a.state || a.status)}>{a.state || a.status || 'unknown'}</span></td>
                    <td class="small text-body text-opacity-75">{a.description || a.desc || ''}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      </div>
    </Card>
  </div>
</div>
