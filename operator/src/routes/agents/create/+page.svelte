<script>
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import Card from '/src/components/bootstrap/Card.svelte';
  import { setPageTitle } from '$lib/utils.js';
  import { apiFetch } from '$lib/api/client.js';
  import { isAuthenticated } from '$lib/auth.js';

  setPageTitle('Create Agent');

  // Simple readable-name generator obeying ^[a-z][a-z0-9-]{0,61}[a-z0-9]$
  // Name format: super_power + creature (animal/bird/mythical)
  const superPowers = [
    'blaze','thunder','quantum','shadow','solar','lunar','arcane','cyber','turbo','frost','ember','vortex','echo','phantom','nova','aether','plasma','neon','storm','iron',
    'cosmic','stellar','galactic','astral','radiant','prismatic','crystal','obsidian','onyx','azure','crimson','verdant','golden','silver','bronze','alpha','omega','gamma','delta','ultra','hyper','nano','micro','macro','zenith','apex'
  ];
  const creatures = [
    'wolf','falcon','dragon','phoenix','griffin','hydra','leviathan','unicorn','pegasus','tiger','hawk','raven','lynx','otter','manticore','basilisk','sphinx','wyvern','kitsune','naga',
    'eagle','sparrow','owl','condor','heron','ibis','crane','stork','albatross','swallow','panther','lion','leopard','cheetah','jaguar','fox','bear','stag','boar','ram',
    'viper','cobra','python','anaconda','orca','dolphin','kraken','yeti','sasquatch','cerberus','minotaur','chimera','fenrir','banshee','kelpie','selkie'
  ];
  function pick(arr) { return arr[Math.floor(Math.random() * arr.length)]; }
  function slugify(s) { return s.toLowerCase().replace(/[^a-z0-9-]/g,'').replace(/^-+|\-+$/g,''); }
  function genName() {
    const s = `${pick(superPowers)}-${pick(creatures)}`;
    let name = slugify(s);
    if (!/^[a-z]/.test(name)) name = 'a' + name;
    if (/[^a-z0-9]$/.test(name)) name += '0';
    if (name.length > 63) name = name.slice(0,63);
    return name;
  }

  let name = genName();
  let idleTimeoutSeconds = 300; // default 5 minutes
  let busyTimeoutSeconds = 900; // default 15 minutes
  let metadataText = '{}';
  // Tags input (comma-separated, alphanumeric only per tag)
  let tagsInput = '';
  function parseTags() {
    const parts = tagsInput.split(',').map(s => s.trim()).filter(Boolean);
    const re = /^[A-Za-z0-9]+$/;
    for (const t of parts) {
      if (!re.test(t)) throw new Error(`Invalid tag '${t}'. Tags must be alphanumeric.`);
    }
    return parts;
  }
  // Leave empty by default; show samples in the sidebar instead
  let instructions = '';
  let setup = '';
  const sampleInstructions = `# Agent Instructions\n\n- Greet the user and explain your purpose.\n- You can run shell commands and edit files via tools when asked.\n- Ask clarifying questions before taking destructive actions.\n- Keep responses concise unless the user requests more detail.`;
  const sampleSetup = `#!/usr/bin/env bash\n# Optional setup script\n# Install packages or prepare files needed by your agent\nset -euo pipefail\n\n# examples:\n# apt-get update && apt-get install -y jq ripgrep\n# echo \"Hello from setup\" > /agent/content/hello.txt`;
  let prompt = '';
  let description = '';

  // Secrets as dynamic rows
  let secrets = [{ key: '', val: '' }];
  function addSecretRow() { secrets = [...secrets, { key: '', val: '' }]; }
  function asSecretsMap() {
    const map = {};
    for (const r of secrets) {
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
      // Validate name
      if (!/^[A-Za-z][A-Za-z0-9-]{0,61}[A-Za-z0-9]$/.test(name)) {
        throw new Error('Name must match ^[A-Za-z][A-Za-z0-9-]{0,61}[A-Za-z0-9]$');
      }
      // Parse metadata
      let metadata = {};
      try { metadata = metadataText ? JSON.parse(metadataText) : {}; }
      catch (e) { throw new Error('Invalid JSON for metadata: ' + e.message); }

      const body = {
        name,
        description: description?.trim() ? description : null,
        metadata,
        tags: parseTags(),
        idle_timeout_seconds: Number(idleTimeoutSeconds) || 300,
        busy_timeout_seconds: Number(busyTimeoutSeconds) || 900,
        instructions: instructions?.trim() ? instructions : null,
        setup: setup?.trim() ? setup : null,
        prompt: prompt?.trim() ? prompt : null,
        secrets: asSecretsMap()
      };

      const res = await apiFetch('/agents', { method: 'POST', body: JSON.stringify(body) });
      if (!res.ok) {
        const msg = res?.data?.message || res?.data?.error || `Create failed (HTTP ${res.status})`;
        throw new Error(msg);
      }
      // Navigate to the created agent page
      goto(`/agents/${encodeURIComponent(name)}`);
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
  <div class="col-xl-8">
    <Card class="mb-3">
      <div class="card-header d-flex align-items-center">
        <div class="fw-bold">Create Agent</div>
        <div class="ms-auto small text-body text-opacity-75">Defaults prefilled — adjust as needed</div>
      </div>
      <div class="card-body">
        {#if error}<div class="alert alert-danger small">{error}</div>{/if}
        <form on:submit|preventDefault={submit}>
          <div class="row g-3">
            <div class="col-12">
              <label class="form-label" for="agent-name">Name</label>
              <div class="input-group">
                <input id="agent-name" class="form-control" bind:value={name} />
                <button class="btn btn-outline-secondary" on:click|preventDefault={() => name = genName()}>Shuffle</button>
              </div>
              <div class="form-text">Letters, digits, dashes; max 63.</div>
            </div>
            <div class="col-12">
              <label class="form-label" for="description">Description (optional)</label>
              <input id="description" class="form-control" bind:value={description} placeholder="Short description of this agent" />
            </div>
            <!-- Timeouts moved to a new line below name -->
            <div class="col-12 col-md-3">
              <label class="form-label" for="idle-timeout">Idle Timeout (seconds)</label>
              <input id="idle-timeout" type="number" min="1" class="form-control" bind:value={idleTimeoutSeconds} />
              <div class="form-text">Sleep after idle (default 300).</div>
            </div>
            <div class="col-12 col-md-3">
              <label class="form-label" for="busy-timeout">Busy Timeout (seconds)</label>
              <input id="busy-timeout" type="number" min="1" class="form-control" bind:value={busyTimeoutSeconds} />
              <div class="form-text">Sleep after busy too long (default 900).</div>
            </div>

            <div class="col-12">
              <label class="form-label" for="tags">Tags (comma-separated)</label>
              <input id="tags" class="form-control" bind:value={tagsInput} placeholder="e.g. Alpha,Internal,Beta" />
              <div class="form-text">Tags must be alphanumeric only; no spaces or symbols.</div>
            </div>

            <div class="col-12">
              <label class="form-label" for="prompt">Starting Prompt (optional)</label>
              <textarea id="prompt" class="form-control" rows="3" bind:value={prompt}></textarea>
            </div>

            <div class="col-12 col-md-6">
              <label class="form-label" for="instructions">Starting System Instruction (Markdown)</label>
              <textarea id="instructions" class="form-control font-monospace" rows="6" bind:value={instructions}></textarea>
              <div class="form-text">You can change these later by directly asking the agent to update its instructions.</div>
            </div>
            <div class="col-12 col-md-6">
              <label class="form-label" for="setup">Starting Setup Script (bash)</label>
              <textarea id="setup" class="form-control font-monospace" rows="6" bind:value={setup}></textarea>
              <div class="form-text">You can modify this later by asking the agent to update its setup.sh.</div>
            </div>

            <div class="col-12">
              <div class="fw-500 small text-body text-opacity-75 mb-1">Secrets (key/value)</div>
              <div class="row g-2 align-items-end">
                {#each secrets as row, idx}
                  <div class="col-6 col-md-4">
                    <label class="form-label" for={'skey_'+idx}>Key</label>
                    <input id={'skey_'+idx} class="form-control" bind:value={row.key} placeholder="API_KEY" />
                  </div>
                  <div class="col-6 col-md-6">
                    <label class="form-label" for={'sval_'+idx}>Value</label>
                    <input id={'sval_'+idx} class="form-control" bind:value={row.val} placeholder="secret" />
                  </div>
                {/each}
                <div class="col-12"><button class="btn btn-outline-secondary btn-sm" on:click|preventDefault={addSecretRow}>+ Add secret</button></div>
              </div>
            </div>

            <!-- Metadata moved to the bottom of the form -->
            <div class="col-12">
              <label class="form-label" for="metadata">Metadata (JSON)</label>
              <textarea id="metadata" class="form-control font-monospace" rows="4" bind:value={metadataText}></textarea>
            </div>

            <div class="col-12 d-flex gap-2">
              <button class="btn btn-theme" disabled={loading}>{#if loading}<span class="spinner-border spinner-border-sm me-2"></span>Creating…{:else}Create Agent{/if}</button>
              <a class="btn btn-outline-secondary" href="/agents">Cancel</a>
            </div>
          </div>
        </form>
      </div>
    </Card>
  </div>
  <div class="col-xl-4">
    <Card>
      <div class="card-header fw-bold">About</div>
      <div class="card-body small text-body text-opacity-75">
        <ul class="ps-3">
          <li>Adjust name and timeout; other fields are optional.</li>
          <li>Instructions and setup are not prefilled; see samples below.</li>
          <li>Secrets are saved to the agent volume and injected at runtime.</li>
        </ul>
        <div class="fw-500 mt-3">Sample Instructions</div>
        <pre class="small bg-dark text-white p-2 rounded code-wrap"><code>{sampleInstructions}</code></pre>
        <div class="fw-500 mt-3">Sample Setup Script</div>
        <pre class="small bg-dark text-white p-2 rounded code-wrap"><code>{sampleSetup}</code></pre>
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
