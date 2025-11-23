<script>
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import Card from '/src/components/bootstrap/Card.svelte';
  import { setPageTitle } from '$lib/utils.js';
  import { apiFetch } from '$lib/api/client.js';
  import { isAuthenticated } from '$lib/auth.js';

setPageTitle('Start Sandbox');
export let data;
let idleTimeoutSeconds = 900; // default 15 minutes
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
  let startupTask = '';
let description = '';
let descriptionInput;

const inferenceProviders = Array.isArray(data?.inferenceProviders) ? data.inferenceProviders : [];
let selectedProviderName =
  inferenceProviders.find((p) => p.is_default)?.name ||
  inferenceProviders[0]?.name ||
  '';
$: selectedProvider =
  inferenceProviders.find((p) => p.name === selectedProviderName) || null;
$: selectedProviderLabel = selectedProvider?.display_name || selectedProvider?.name || '';
$: availableModels = selectedProvider
  ? selectedProvider.models.map((model) => ({
      value: model.name,
      label: model.display_name || model.name
    }))
  : [];
let selectedModel = selectedProvider ? selectedProvider.default_model : '';
let inferenceApiKey = ''; // New variable for inference API key
$: if ((!selectedModel || !selectedModel.trim()) && availableModels.length) {
  selectedModel = availableModels[0].value;
}
let lastProviderName = selectedProvider?.name || '';
$: if ((selectedProvider?.name || '') !== lastProviderName) {
  lastProviderName = selectedProvider?.name || '';
  const providerDefault =
    selectedProvider &&
    selectedProvider.default_model &&
    availableModels.find((model) => model.value === selectedProvider.default_model)
      ? selectedProvider.default_model
      : null;
  const providerFirst = availableModels[0]?.value || '';
  const desired = providerDefault || providerFirst;
  if (desired) {
    selectedModel = desired;
  }
}
$: hasProviders = inferenceProviders.length > 0;

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
  function handleCtrlEnter(event) {
    if (event.key === 'Enter' && event.ctrlKey && !loading) {
      event.preventDefault();
      submit(event);
    }
  }

  onMount(() => {
    if (!isAuthenticated()) {
      goto('/login');
    }
    descriptionInput?.focus();
  });

  async function submit(e) {
    e?.preventDefault?.();
    error = null;
    loading = true;
    try {
      if (!hasProviders) {
        throw new Error('No inference providers are configured.');
      }
      if (!selectedModel || !selectedModel.trim()) {
        throw new Error('Please select an inference model.');
      }
      // Parse metadata
      let metadata = {};
      try { metadata = metadataText ? JSON.parse(metadataText) : {}; }
      catch (e) { throw new Error('Invalid JSON for metadata: ' + e.message); }

      const body = {
        description: description?.trim() ? description : null,
        metadata,
        tags: parseTags(),
        idle_timeout_seconds: Number(idleTimeoutSeconds) || 900,
        instructions: instructions?.trim() ? instructions : null,
        setup: setup?.trim() ? setup : null,
        startup_task: startupTask?.trim() ? startupTask : null,
        inference_provider: selectedProvider?.name || null,
        inference_model: selectedModel.trim(),
        inference_api_key: inferenceApiKey?.trim() ? inferenceApiKey : null, // Include inference API key
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
          <div class="small text-body text-opacity-75 d-none d-sm-block">Ctrl+Enter to submit</div>
          <button type="button" class="btn btn-outline-theme btn-sm" on:click|preventDefault={submit} disabled={loading || !selectedModel || !hasProviders} aria-label="Submit">
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
        {#if !hasProviders}
          <div class="alert alert-warning small">No inference providers are configured. Update <code>~/.tsbx/tsbx.json</code> (see <code>config/tsbx.sample.json</code>) and restart the stack before creating a sandbox.</div>
        {:else if hasProviders && !availableModels.length}
          <div class="alert alert-warning small">The selected provider has no models configured. Update <code>~/.tsbx/tsbx.json</code> before continuing.</div>
        {/if}
        <form on:submit|preventDefault={submit} on:keydown={handleCtrlEnter}>
          <div class="row g-3">
            <div class="col-12">
              <label class="form-label" for="description">Description (optional)</label>
              <input
                id="description"
                class="form-control"
                bind:this={descriptionInput}
                bind:value={description}
                placeholder="Short description of this sandbox"
              />
            </div>
            <div class="col-12 col-md-6">
              <label class="form-label" for="inference-provider">Inference Provider</label>
              <select
                id="inference-provider"
                class="form-select"
                bind:value={selectedProviderName}
                disabled={!hasProviders || loading || inferenceProviders.length === 1}
              >
                {#if hasProviders}
                  {#each inferenceProviders as provider}
                    <option value={provider.name}>{provider.display_name || provider.name}</option>
                  {/each}
                {:else}
                  <option value="">No providers configured</option>
                {/if}
              </select>
              <div class="form-text">
                Choose which configured provider should back this sandbox.
              </div>
            </div>
            <div class="col-12 col-md-6">
              <label class="form-label d-flex align-items-center gap-2" for="inference-model">
                <span>Inference Model</span>
              </label>
              <select
                id="inference-model"
                class="form-select"
                bind:value={selectedModel}
                disabled={!availableModels.length || loading || availableModels.length === 1}
              >
                {#if availableModels.length}
                  {#each availableModels as model}
                    <option value={model.value}>{model.label}</option>
                  {/each}
                {:else}
                  <option value="">No models configured</option>
                {/if}
              </select>
              <div class="form-text">
                Pick the model this sandbox should use for inference. This cannot be changed later.
              </div>
            </div>
            
            <div class="col-12">
              <label class="form-label" for="inference-api-key">Inference API Key (Optional)</label>
              <input
                type="text"
                class="form-control"
                id="inference-api-key"
                placeholder="Enter Inference API Key"
                bind:value={inferenceApiKey}
              />
              <div class="form-text">
                If provided, this key will be used for NL tasks in this sandbox only and will not be stored.
              </div>
              <div class="alert alert-info small mt-2 mb-0">
                Natural Language (NL) tasks require an inference key. Leave this blank only if you do not need NL tasks for the sandbox.
              </div>
            </div>

            <div class="col-12">
              <label class="form-label" for="startup-task">Startup Task (optional)</label>
              <textarea
                id="startup-task"
                class="form-control"
                rows="3"
                bind:value={startupTask}
              ></textarea>
              <div class="form-text">Use Ctrl+Enter anywhere on this page to submit.</div>
            </div>

            <div class="col-12 col-md-6">
              <label class="form-label" for="instructions">Startup instructions.md (optional)</label>
              <textarea id="instructions" class="form-control font-monospace" rows="6" bind:value={instructions}></textarea>
              <div class="form-text">You can change these later by directly asking the sandbox to update its instructions.</div>
            </div>
            <div class="col-12 col-md-6">
              <label class="form-label" for="setup">Startup setup.sh (optional)</label>
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
              <div class="form-text">Stop after inactivity (default 900).</div>
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
              <button type="button" class="btn btn-outline-theme" on:click|preventDefault={submit} disabled={loading || !selectedModel}>{#if loading}<span class="spinner-border spinner-border-sm me-2"></span>Submitting…{:else}Submit{/if}</button>
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
