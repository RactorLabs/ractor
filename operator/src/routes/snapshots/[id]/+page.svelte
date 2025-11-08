<script>
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import { isAuthenticated, getToken } from '$lib/auth.js';
  import { apiFetch } from '$lib/api/client.js';
  import Card from '/src/components/bootstrap/Card.svelte';
  import { setPageTitle } from '$lib/utils.js';

  $: snapshotId = $page.params.id || '';
  setPageTitle('Snapshot');

  let snapshot = null;
  let loading = true;
  let error = null;

  // File browser state
  let currentPath = '';
  let files = [];
  let filesLoading = false;
  let filesError = null;
  let breadcrumbs = [];
  let previewFile = null;
  let previewContent = '';
  let previewLoading = false;
  let previewError = null;

  function updateBreadcrumbs() {
    breadcrumbs = [{ name: 'Root', path: '' }];
    if (currentPath) {
      const parts = currentPath.split('/').filter(Boolean);
      let accumulated = '';
      for (const part of parts) {
        accumulated = accumulated ? `${accumulated}/${part}` : part;
        breadcrumbs.push({ name: part, path: accumulated });
      }
    }
  }

  async function fetchSnapshot() {
    try {
      const res = await apiFetch(`/snapshots/${encodeURIComponent(snapshotId)}`);
      if (!res.ok) {
        error = res?.data?.message || `Failed to load snapshot (HTTP ${res.status})`;
        loading = false;
        return;
      }
      snapshot = res.data;
      loading = false;
    } catch (e) {
      error = e.message || String(e);
      loading = false;
    }
  }

  async function fetchFiles(path = '') {
    filesLoading = true;
    filesError = null;
    resetPreview();
    try {
      const encodedPath = path ? `/list/${encodeURIComponent(path)}` : '/list';
      const res = await apiFetch(`/snapshots/${encodeURIComponent(snapshotId)}/files${encodedPath}`);
      if (!res.ok) {
        filesError = res?.data?.message || `Failed to load files (HTTP ${res.status})`;
        filesLoading = false;
        return;
      }
      files = res.data?.items || [];
      filesLoading = false;
    } catch (e) {
      filesError = e.message || String(e);
      filesLoading = false;
    }
  }

  function navigateToPath(path) {
    currentPath = path;
    updateBreadcrumbs();
    fetchFiles(path);
  }

  function resetPreview() {
    previewFile = null;
    previewContent = '';
    previewLoading = false;
    previewError = null;
  }

  function openFile(file) {
    if (file.is_dir) {
      const newPath = currentPath ? `${currentPath}/${file.name}` : file.name;
      navigateToPath(newPath);
    } else {
      const filePath = currentPath ? `${currentPath}/${file.name}` : file.name;
      loadPreview(filePath, file.name);
    }
  }

  async function loadPreview(path, name) {
    previewFile = { name, path };
    previewContent = '';
    previewError = null;
    previewLoading = true;
    try {
      const url = `/api/v0/snapshots/${encodeURIComponent(snapshotId)}/files/read/${encodeURIComponent(path)}`;
      const headers = new Headers();
      const token = getToken();
      if (token) headers.set('Authorization', `Bearer ${token}`);
      const resp = await fetch(url, { headers });
      if (!resp.ok) {
        previewError = `Failed to open file (HTTP ${resp.status})`;
        previewLoading = false;
        return;
      }
      const text = await resp.text();
      previewContent = text;
      previewLoading = false;
    } catch (err) {
      previewError = err?.message || String(err);
      previewLoading = false;
    }
  }


  function formatSize(bytes) {
    if (!bytes || bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
  }

  function formatDate(timestamp) {
    if (!timestamp) return '-';
    try {
      const d = new Date(timestamp * 1000);
      return d.toLocaleString();
    } catch (_) {
      return '-';
    }
  }

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

  async function deleteSnapshot() {
    const ok = confirm(`Delete snapshot '${snapshotId}'? This cannot be undone.`);
    if (!ok) return;
    const res = await apiFetch(`/snapshots/${encodeURIComponent(snapshotId)}`, { method: 'DELETE' });
    if (!res.ok) {
      error = res?.data?.message || 'Delete failed';
      return;
    }
    goto('/snapshots');
  }

  async function createFromSnapshot() {
    const ok = confirm(`Create a new sandbox from snapshot '${snapshotId}'?`);
    if (!ok) return;
    const res = await apiFetch(`/snapshots/${encodeURIComponent(snapshotId)}/create`, { method: 'POST', body: JSON.stringify({}) });
    if (!res.ok) {
      error = res?.data?.message || 'Create from snapshot failed';
      return;
    }
    const newSandbox = res.data;
    if (newSandbox && newSandbox.id) {
      goto(`/sandboxes/${encodeURIComponent(newSandbox.id)}`);
    }
  }

  onMount(async () => {
    if (!isAuthenticated()) {
      goto('/login');
      return;
    }
    updateBreadcrumbs();
    await fetchSnapshot();
    await fetchFiles();
  });
</script>

<div class="container-xxl">
  <div class="row justify-content-center">
    <div class="col-12 col-xxl-10">
      {#if loading}
        <div class="d-flex align-items-center justify-content-center" style="min-height: 30vh;">
          <div class="text-center text-body text-opacity-75">
            <div class="spinner-border text-theme mb-3"></div>
            <div>Loading snapshot…</div>
          </div>
        </div>
      {:else if error}
        <div class="alert alert-danger small">{error}</div>
      {:else if snapshot}
        <div class="d-flex align-items-center justify-content-between mb-3 flex-wrap gap-2">
          <div>
            <a href="/snapshots" class="text-decoration-none text-body text-opacity-75 small"><i class="bi bi-arrow-left me-1"></i>Back to Snapshots</a>
          </div>
          <div class="d-flex align-items-center gap-2">
            <button class="btn btn-outline-theme btn-sm" on:click={createFromSnapshot}>
              <i class="bi bi-plus-circle me-1"></i>Create Sandbox
            </button>
            <button class="btn btn-outline-danger btn-sm" on:click={deleteSnapshot}>
              <i class="bi bi-trash me-1"></i>Delete
            </button>
          </div>
        </div>

        <Card class="mb-3">
          <div class="card-body">
            <div class="d-flex align-items-center gap-2 mb-2">
              <h5 class="card-title mb-0 font-monospace">{snapshot.id}</h5>
              <span class={triggerTypeBadgeClass(snapshot.trigger_type)}>{triggerTypeLabel(snapshot.trigger_type)}</span>
            </div>
            <div class="small text-body text-opacity-75 mb-1">
              Source Sandbox: <a href="/sandboxes/{encodeURIComponent(snapshot.sandbox_id)}" class="font-monospace">{snapshot.sandbox_id}</a>
            </div>
            <div class="small text-body text-opacity-50">
              Created: {new Date(snapshot.created_at).toLocaleString()}
            </div>
          </div>
        </Card>

        <Card>
          <div class="card-body">
            <h6 class="card-title mb-3">Files</h6>

            <!-- Breadcrumb navigation -->
            <nav aria-label="breadcrumb" class="mb-3">
              <ol class="breadcrumb mb-0">
                {#each breadcrumbs as crumb, idx}
                  {#if idx === breadcrumbs.length - 1}
                    <li class="breadcrumb-item active" aria-current="page">{crumb.name}</li>
                  {:else}
                    <li class="breadcrumb-item">
                      <button class="btn btn-link p-0 text-decoration-none" on:click={() => navigateToPath(crumb.path)}>
                        {crumb.name}
                      </button>
                    </li>
                  {/if}
                {/each}
              </ol>
            </nav>

            {#if filesLoading}
              <div class="text-center text-body text-opacity-75 py-4">
                <div class="spinner-border spinner-border-sm text-theme mb-2"></div>
                <div class="small">Loading files…</div>
              </div>
            {:else if filesError}
              <div class="alert alert-danger small mb-0">{filesError}</div>
            {:else if files.length === 0}
              <div class="text-body text-opacity-75 py-4 text-center">
                <i class="bi bi-folder-x fs-3 mb-2 d-block"></i>
                <div class="small">This directory is empty</div>
              </div>
            {:else}
              <div class="table-responsive">
                <table class="table table-hover table-sm mb-0">
                  <thead>
                    <tr>
                      <th>Name</th>
                      <th class="text-end">Size</th>
                      <th class="text-end">Modified</th>
                    </tr>
                  </thead>
                  <tbody>
                    {#each files as file}
                      <tr class="cursor-pointer" on:click={() => openFile(file)}>
                        <td>
                          <i class="bi {file.is_dir ? 'bi-folder-fill text-warning' : 'bi-file-earmark text-secondary'} me-2"></i>
                          {file.name}
                        </td>
                        <td class="text-end text-body text-opacity-75 small">
                          {file.is_dir ? '-' : formatSize(file.size)}
                        </td>
                        <td class="text-end text-body text-opacity-75 small">
                          {formatDate(file.modified)}
                        </td>
                      </tr>
                    {/each}
                  </tbody>
                </table>
              </div>
              {#if previewFile}
                <div class="mt-3 border rounded">
                  <div class="d-flex align-items-center justify-content-between px-3 py-2 border-bottom bg-light">
                    <div class="d-flex flex-column">
                      <span class="fw-semibold">Preview: {previewFile.name}</span>
                      <span class="small text-body text-opacity-75">{previewFile.path}</span>
                    </div>
                    <div class="d-flex align-items-center gap-2">
                      <a
                        class="btn btn-sm btn-outline-secondary"
                        href={`/api/v0/snapshots/${encodeURIComponent(snapshotId)}/files/read/${encodeURIComponent(previewFile.path)}`}
                        target="_blank"
                        rel="noreferrer"
                      >
                        Open in new tab
                      </a>
                    </div>
                  </div>
                  <div class="px-3 py-3">
                    {#if previewLoading}
                      <div class="text-center text-body text-opacity-75 py-4">
                        <div class="spinner-border spinner-border-sm text-theme mb-2"></div>
                        <div class="small">Loading file…</div>
                      </div>
                    {:else if previewError}
                      <div class="alert alert-danger small mb-0">{previewError}</div>
                    {:else}
                      <pre class="snapshot-file-preview"><code>{previewContent}</code></pre>
                    {/if}
                  </div>
                </div>
              {/if}
            {/if}
          </div>
        </Card>
      {/if}
    </div>
  </div>
</div>

<style>
  .cursor-pointer {
    cursor: pointer;
  }
  .breadcrumb-item button {
    font-size: inherit;
    line-height: inherit;
  }
  .snapshot-file-preview {
    background: #111827;
    color: #f8fafc;
    padding: 1rem;
    border-radius: 0.5rem;
    max-height: 420px;
    overflow: auto;
    font-family: var(--bs-font-monospace, ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace);
    font-size: 0.85rem;
    margin: 0;
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
