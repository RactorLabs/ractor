<script>
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import Card from '/src/components/bootstrap/Card.svelte';
  import { setPageTitle } from '$lib/utils.js';
  import { apiFetch } from '$lib/api/client.js';
  import { isAuthenticated } from '$lib/auth.js';

  setPageTitle('Start Sandbox');
let idleTimeoutSeconds = 300; // default 5 minutes
  let metadataText = '{}';
  // Tags input (comma-separated, letters/digits and '/', '-', '_' , '.' per tag)
  let tagsInput = '';
  function parseTags() {
    const parts = tagsInput.split(',').map(s => s.trim().toLowerCase()).filter(Boolean);
    const re = /^[A-Za-z0-9_\/\.\-]+$/;
    for (const t of parts) {
      if (!re.test(t)) throw new Error(`Invalid tag '${t}'. Allowed: letters, digits, '/', '-', '_', '.'.`);
    }
    return parts;
  }
  // Leave empty by default; show samples in the sidebar instead
  let instructions = '';
  let setup = '';
  // (Samples removed with About section)
  let prompt = '';
  let description = '';

  // Environment entries as dynamic rows
  let envEntries = [{ key: '', val: '' }];
  function addEnvRow() { envEntries = [...envEntries, { key: '', val: '' }]; }
  function asEnvMap() {
    const map = {};
    for (const r of envEntries) {
      if (r.key && String(r.val).length > 0) map[r.key] = r.val;
    }
    return map;
  }

  let loading = false;
  let error = null;

  onMount(() => {
    if (!isAuthenticated()) {
      goto('/login');
    }
  });

  async function submit(e) {
    e?.preventDefault?.();
    error = null;
    loading = true;
    try {
      // Parse metadata
      let metadata = {};
      try { metadata = metadataText ? JSON.parse(metadataText) : {}; }
      catch (e) { throw new Error('Invalid JSON for metadata: ' + e.message); }

      const body = {
        description: description?.trim() ? description : null,
        metadata,
        tags: parseTags(),
        idle_timeout_seconds: Number(idleTimeoutSeconds) || 300,
        instructions: instructions?.trim() ? instructions : null,
        setup: setup?.trim() ? setup : null,
        prompt: prompt?.trim() ? prompt : null,
        env: asEnvMap()
      };

      const res = await apiFetch('/sandboxes', { method: 'POST', body: JSON.stringify(body) });
      if (!res.ok) {
        const msg = res?.data?.message || res?.data?.error || `Create failed (HTTP ${res.status})`;
        throw new Error(msg);
      }
      // Navigate to the started sandbox page using the sandbox ID from the response
      const newSandbox = res.data;
      goto(`/sandboxes/${encodeURIComponent(newSandbox.id)}`);
    } catch (e) {
      error = e.message || String(e);
    } finally {
      loading = false;
    }
  }
</script>

<div class="container-xxl">
  <div class="row justify-content-center">
    <div class="col-12 col-xxl-10">
<div class="row g-3">
  <div class="col-xl-12">
    <Card class="mb-3">
      <div class="card-header d-flex align-items-center">
        <div class="fw-bold fs-20px">Start Sandbox</div>
        <div class="ms-auto d-flex align-items-center gap-2">
          <div class="small text-body text-opacity-75 d-none d-sm-block">Defaults prefilled — adjust as needed</div>
          <button type="button" class="btn btn-outline-theme btn-sm" on:click|preventDefault={submit} disabled={loading} aria-label="Submit">
            {#if loading}
              <span class="spinner-border spinner-border-sm me-2"></span>Submitting…
            {:else}
              Submit
            {/if}
          </button>
        </div>
      </div>
      <div class="card-body">
        {#if error}<div class="alert alert-danger small">{error}</div>{/if}
        <form on:submit|preventDefault={submit}>
          <div class="row g-3">
            <div class="col-12">
              <label class="form-label" for="description">Description (optional)</label>
              <input id="description" class="form-control" bind:value={description} placeholder="Short description of this sandbox" />
            </div>
            

            <div class="col-12">
              <label class="form-label" for="prompt">Starting Prompt (optional)</label>
              <textarea
                id="prompt"
                class="form-control"
                rows="3"
                bind:value={prompt}
                on:keydown={(event) => {
                  if (event.key === 'Enter' && event.ctrlKey && !loading) {
                    submit(event);
                  }
                }}
              ></textarea>
              <div class="form-text">Press Ctrl+Enter to start the sandbox.</div>
            </div>

            <div class="col-12 col-md-6">
              <label class="form-label" for="instructions">Starting System Instruction (Markdown)</label>
              <textarea id="instructions" class="form-control font-monospace" rows="6" bind:value={instructions}></textarea>
              <div class="form-text">You can change these later by directly asking the sandbox to update its instructions.</div>
            </div>
            <div class="col-12 col-md-6">
              <label class="form-label" for="setup">Starting Setup Script (bash)</label>
              <textarea id="setup" class="form-control font-monospace" rows="6" bind:value={setup}></textarea>
              <div class="form-text">You can modify this later by asking the sandbox to update its setup.sh.</div>
            </div>

            <div class="col-12">
              <div class="fw-500 small text-body text-opacity-75 mb-1">Environment Variables (key/value)</div>
              <div class="row g-2 align-items-end">
                {#each envEntries as row, idx}
                  <div class="col-6 col-md-4">
                    <label class="form-label" for={'env_key_'+idx}>Key</label>
                    <input id={'env_key_'+idx} class="form-control" bind:value={row.key} placeholder="API_KEY" />
                  </div>
                  <div class="col-6 col-md-6">
                    <label class="form-label" for={'env_val_'+idx}>Value</label>
                    <input id={'env_val_'+idx} class="form-control" bind:value={row.val} placeholder="value" />
                  </div>
                {/each}
                <div class="col-12"><button class="btn btn-outline-secondary btn-sm" on:click|preventDefault={addEnvRow}>+ Add variable</button></div>
              </div>
            </div>

            <!-- Timeout and Tags -->
            <div class="col-12 col-md-4">
              <label class="form-label" for="idle-timeout">Idle Timeout (seconds)</label>
              <input id="idle-timeout" type="number" min="1" class="form-control" bind:value={idleTimeoutSeconds} />
              <div class="form-text">Stop after inactivity (default 300).</div>
            </div>

            <div class="col-12">
              <label class="form-label" for="tags">Tags (comma-separated)</label>
              <input id="tags" class="form-control" bind:value={tagsInput} placeholder="e.g. Alpha,Internal,Beta" />
              <div class="form-text">Allowed characters in tags: letters, digits, '/', '-', '_', '.'. No spaces.</div>
            </div>

            <!-- Metadata moved to the bottom of the form -->
            <div class="col-12">
              <label class="form-label" for="metadata">Metadata (JSON)</label>
              <textarea id="metadata" class="form-control font-monospace" rows="4" bind:value={metadataText}></textarea>
            </div>

            <div class="col-12 d-flex gap-2">
              <button type="button" class="btn btn-outline-theme" on:click|preventDefault={submit} disabled={loading}>{#if loading}<span class="spinner-border spinner-border-sm me-2"></span>Submitting…{:else}Submit{/if}</button>
              <a class="btn btn-outline-secondary" href="/sandboxes">Cancel</a>
            </div>
          </div>
        </form>
      </div>
    </Card>
  </div>
  

  <style>
    .font-monospace { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace; }
    :global(pre.code-wrap) { white-space: pre-wrap; word-break: break-word; overflow-wrap: anywhere; }
  </style>
</div>
    </div>
  </div>
</div>
