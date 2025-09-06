<script>
  import { onMount } from 'svelte';
  import Card from '/src/components/bootstrap/Card.svelte';
  import { setPageTitle } from '$lib/utils.js';
  import { apiDocs } from '$lib/api/docs.js';
  import { playground } from '/src/stores/playground.js';

  setPageTitle('API Playground');

  // Flatten endpoints
  const endpoints = [];
  for (const section of apiDocs) {
    for (const ep of section.endpoints) {
      endpoints.push({ ...ep, section: section.title });
    }
  }

  let selected = endpoints[0];
  let pathParams = {};
  let queryParams = {};
  let bodyText = '';
  let extraQuery = [{ key: '', val: '' }];
  let loading = false;
  let result = null; // { status, ms, data, text }

  $: token = $playground.token || '';
  $: remember = $playground.remember;

  function onSelect(idx) {
    selected = endpoints[idx];
    // seed params
    pathParams = {};
    queryParams = {};
    if (selected.params) {
      for (const p of selected.params) {
        if (p.in === 'path') pathParams[p.name] = '';
        if (p.in === 'query') queryParams[p.name] = '';
      }
    }
    // seed body from params if any
    const bodyFields = (selected.params || []).filter(p => p.in === 'body');
    if (bodyFields.length) {
      const obj = {};
      for (const f of bodyFields) obj[f.name] = null;
      bodyText = JSON.stringify(obj, null, 2);
    } else {
      bodyText = '';
    }
    extraQuery = [{ key: '', val: '' }];
    result = null;
  }

  function buildUrl() {
    // Replace path params
    let path = selected.path;
    for (const [k, v] of Object.entries(pathParams)) {
      path = path.replace(`{${k}}`, encodeURIComponent(v || ''));
    }
    const qp = new URLSearchParams();
    for (const [k, v] of Object.entries(queryParams)) {
      if (v !== undefined && v !== null && String(v).length > 0) qp.set(k, v);
    }
    for (const row of extraQuery) {
      if (row.key && row.val !== undefined && row.val !== null && String(row.val).length > 0) qp.set(row.key, row.val);
    }
    const qs = qp.toString();
    return qs ? `${path}?${qs}` : path;
  }

  function fullUrl() {
    const u = buildUrl();
    const base = typeof window !== 'undefined' ? window.location.origin : '';
    return base + u;
  }

  function formatCurl() {
    const url = fullUrl();
    const parts = ['curl'];
    parts.push('-s');
    parts.push('-X ' + selected.method);
    parts.push(url);
    if (token) parts.push('-H "Authorization: Bearer ' + token + '"');
    if (bodyText && selected.method !== 'GET' && selected.method !== 'DELETE') {
      parts.push('-H "Content-Type: application/json"');
      parts.push("-d '" + bodyText.replace(/'/g, "'\\''") + "'");
    }
    // format one per line
    const lines = [];
    for (let i = 0; i < parts.length; i++) {
      if (i === 0) lines.push(parts[i]);
      else lines.push('  ' + parts[i]);
    }
    return lines.map((l, i) => (i < lines.length - 1 ? l + ' \\' : l)).join('\n');
  }

  async function execute() {
    loading = true; result = null;
    const url = buildUrl();
    const headers = new Headers();
    if (token) headers.set('Authorization', `Bearer ${token}`);
    let body;
    if (bodyText && selected.method !== 'GET' && selected.method !== 'DELETE') {
      headers.set('Content-Type', 'application/json');
      try { body = bodyText ? JSON.stringify(JSON.parse(bodyText)) : undefined; }
      catch (e) { result = { status: 0, ms: 0, text: 'Invalid JSON body: ' + e.message }; loading = false; return; }
    }
    const t0 = performance.now();
    try {
      const res = await fetch(url, { method: selected.method, headers, body });
      const ms = Math.round(performance.now() - t0);
      let data = null, text = '';
      try { data = await res.json(); }
      catch (_) { text = await res.text().catch(()=> ''); }
      result = { status: res.status, ms, data, text };
    } catch (e) {
      const ms = Math.round(performance.now() - t0);
      result = { status: 0, ms, text: e.message };
    } finally {
      loading = false;
    }
  }

  function addExtraQueryRow() {
    extraQuery = [...extraQuery, { key: '', val: '' }];
  }
</script>

<div class="row g-3">
  <div class="col-xl-8">
    <Card class="mb-3">
      <div class="card-header d-flex align-items-center">
        <div class="fw-bold">API Playground</div>
      </div>
      <div class="card-body">
        <div class="row g-3 align-items-end">
          <div class="col-12 col-md-4">
            <label class="form-label">Endpoint</label>
            <select class="form-select" on:change={(e)=>onSelect(e.target.selectedIndex)}>
              {#each endpoints as ep, i}
                <option value={i} selected={selected===ep}>{ep.method} {ep.path} — {ep.section}</option>
              {/each}
            </select>
          </div>
          <div class="col-12 col-md-4">
            <label class="form-label">Auth Token</label>
            <input class="form-control" placeholder="Bearer token" bind:value={$playground.token} />
            <div class="form-check mt-1">
              <input class="form-check-input" type="checkbox" id="rememberToken" bind:checked={$playground.remember}>
              <label class="form-check-label" for="rememberToken">Remember in this browser</label>
            </div>
          </div>
          <div class="col-12 col-md-4 text-md-end">
            <button class="btn btn-theme" on:click|preventDefault={execute} disabled={loading}>
              {#if loading}<span class="spinner-border spinner-border-sm me-2"></span>Calling…{:else}Call API{/if}
            </button>
          </div>
        </div>

        <!-- Params -->
        {#if selected.params && selected.params.some(p=>p.in==='path')}
          <div class="mt-3">
            <div class="fw-500 small text-body text-opacity-75 mb-1">Path parameters</div>
            <div class="row g-2">
              {#each selected.params.filter(p=>p.in==='path') as p}
                <div class="col-12 col-md-4">
                  <label class="form-label">{p.name}</label>
                  <input class="form-control" bind:value={pathParams[p.name]} placeholder={p.desc || ''} />
                </div>
              {/each}
            </div>
          </div>
        {/if}

        {#if selected.params && selected.params.some(p=>p.in==='query')}
          <div class="mt-3">
            <div class="fw-500 small text-body text-opacity-75 mb-1">Query parameters</div>
            <div class="row g-2">
              {#each selected.params.filter(p=>p.in==='query') as p}
                <div class="col-12 col-md-4">
                  <label class="form-label">{p.name}</label>
                  <input class="form-control" bind:value={queryParams[p.name]} placeholder={p.desc || ''} />
                </div>
              {/each}
              {#each extraQuery as row, idx}
                <div class="col-6 col-md-3"><input class="form-control" placeholder="key" bind:value={row.key} /></div>
                <div class="col-6 col-md-3"><input class="form-control" placeholder="value" bind:value={row.val} /></div>
              {/each}
              <div class="col-12"><button class="btn btn-outline-secondary btn-sm" on:click|preventDefault={addExtraQueryRow}>+ Add query</button></div>
            </div>
          </div>
        {/if}

        {#if selected.method !== 'GET' && selected.method !== 'DELETE'}
          <div class="mt-3">
            <div class="fw-500 small text-body text-opacity-75 mb-1">Request Body (JSON)</div>
            <textarea class="form-control font-monospace" rows="8" bind:value={bodyText} placeholder='&#123;"field":"value"&#125;'></textarea>
          </div>
        {/if}

        <div class="mt-3">
          <div class="fw-500 small text-body text-opacity-75 mb-1">Request</div>
          <div class="small"><span class="badge bg-secondary me-2">{selected.method}</span><span class="font-monospace">{buildUrl()}</span></div>
          <pre class="small bg-dark text-white p-2 rounded mb-0 code-wrap"><code>{formatCurl()}</code></pre>
        </div>

        <div class="mt-3">
          <div class="fw-500 small text-body text-opacity-75 mb-1">Response</div>
          {#if result}
            <div class="small mb-2">Status: <span class="fw-bold">{result.status}</span> • Time: {result.ms} ms</div>
            {#if result.data}
              <pre class="small bg-dark text-white p-2 rounded mb-0 code-wrap"><code>{JSON.stringify(result.data, null, 2)}</code></pre>
            {:else}
              <pre class="small bg-dark text-white p-2 rounded mb-0 code-wrap"><code>{result.text || ''}</code></pre>
            {/if}
          {:else}
            <div class="text-body text-opacity-75">No response yet</div>
          {/if}
        </div>
      </div>
    </Card>
  </div>
  <div class="col-xl-4">
    <Card>
      <div class="card-header fw-bold">Tips</div>
      <div class="card-body small text-body text-opacity-75">
        <ul class="mb-0 ps-3">
          <li>Token is stored in this browser only (session/local storage), not cookies.</li>
          <li>For protected endpoints, paste a Bearer token after logging in.</li>
          <li>Use additional query rows to include parameters not listed.</li>
        </ul>
      </div>
    </Card>
  </div>

  <style>
    :global(pre.code-wrap) { white-space: pre-wrap; word-break: break-word; overflow-wrap: anywhere; }
    .font-monospace { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace; }
  </style>
</div>
