<script>
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { setPageTitle } from '$lib/utils.js';
  import { isAuthenticated, getOperatorName, getPrincipalType } from '$lib/auth.js';
  import { apiFetch } from '$lib/api/client.js';

  let operatorName = '';
  let currentPassword = '';
  let newPassword = '';
  let confirmPassword = '';
  let loading = false;
  let error = null;
  let success = null;

  onMount(async () => {
    setPageTitle('Change Password');
    if (!isAuthenticated()) { goto('/login'); return; }
    const t = (getPrincipalType() || '').toLowerCase();
    if (t !== 'admin') { goto('/agents'); return; }
    operatorName = getOperatorName() || '';
  });

  async function submitForm() {
    if (loading) return;
    error = null; success = null;
    if (!currentPassword || !newPassword) { error = 'All fields are required.'; return; }
    if (newPassword !== confirmPassword) { error = 'New passwords do not match.'; return; }
    loading = true;
    try {
      const body = { current_password: currentPassword, new_password: newPassword };
      const res = await apiFetch(`/operators/${encodeURIComponent(operatorName)}/password`, { method: 'PUT', body: JSON.stringify(body) });
      if (!res.ok) {
        const msg = res?.data?.message || res?.data?.error || `Failed to change password (HTTP ${res.status})`;
        throw new Error(msg);
      }
      success = 'Password updated successfully.';
      currentPassword = '';
      newPassword = '';
      confirmPassword = '';
    } catch (e) {
      error = e.message || 'Failed to change password.';
    } finally {
      loading = false;
    }
  }
</script>

<div class="container py-4">
  <div class="row justify-content-center">
    <div class="col-lg-6 col-md-8">
      <div class="card">
        <div class="card-header fw-bold d-flex align-items-center">
          <i class="bi bi-key me-2"></i> Change Password
          <span class="ms-auto small text-muted">{operatorName}</span>
        </div>
        <div class="card-body">
          {#if error}
            <div class="alert alert-danger py-2 small">{error}</div>
          {/if}
          {#if success}
            <div class="alert alert-success py-2 small">{success}</div>
          {/if}
          <form on:submit|preventDefault={submitForm}>
            <div class="mb-3">
              <label for="current" class="form-label">Current Password</label>
              <input id="current" type="password" class="form-control" bind:value={currentPassword} autocomplete="current-password" required />
            </div>
            <div class="mb-3">
              <label for="newpass" class="form-label">New Password</label>
              <input id="newpass" type="password" class="form-control" bind:value={newPassword} autocomplete="new-password" required />
            </div>
            <div class="mb-3">
              <label for="confirm" class="form-label">Confirm New Password</label>
              <input id="confirm" type="password" class="form-control" bind:value={confirmPassword} autocomplete="new-password" required />
            </div>
            <div class="d-flex gap-2">
              <button class="btn btn-outline-theme" type="submit" disabled={loading}>
                {#if loading}
                  <span class="spinner-border spinner-border-sm me-2"></span>Updating…
                {:else}
                  Update Password
                {/if}
              </button>
              <a href="/agents" class="btn btn-outline-secondary">Cancel</a>
            </div>
          </form>
        </div>
      </div>
    </div>
  </div>
  
</div>

