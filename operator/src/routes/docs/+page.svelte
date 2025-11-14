<script>
  import { getApiDocs, methodClass } from '$lib/api/docs.js';
  import { setPageTitle } from '$lib/utils.js';
  import Card from '/src/components/bootstrap/Card.svelte';
  import { page } from '$app/stores';
  import { getHostName } from '$lib/branding.js';

  // Hard-coded docs version; update during version bumps
  const API_VERSION = '0.11.0 (v0)';
  // Compute docs once reactively and order Published section last
  $: docs = (() => {
    const list = (getApiDocs($page?.data?.hostUrl) || [])
      .filter((sec) => sec.id !== 'auth' && sec.id !== 'content') // hide auth + content sections from public docs
      .map((sec) => ({ ...sec, endpoints: (sec.endpoints || []).filter((ep) => !ep.adminOnly) }))
      .filter((sec) => (sec.endpoints || []).length > 0);
    return [
      ...list.filter((s) => s.id !== 'published'),
      ...list.filter((s) => s.id === 'published')
    ];
  })();
  

  setPageTitle('API Documentation');

  // Format single-line curl into multi-line with one parameter per line
  function formatCurl(example) {
    if (!example || !example.trim().startsWith('curl')) return example;

    // Tokenize by spaces while respecting simple quotes
    const tokens = [];
    let buf = '';
    let inQuote = false;
    let quoteChar = '';
    for (let i = 0; i < example.length; i++) {
      const ch = example[i];
      if ((ch === '"' || ch === "'") && (!inQuote || ch === quoteChar)) {
        if (!inQuote) { inQuote = true; quoteChar = ch; }
        else { inQuote = false; quoteChar = ''; }
        buf += ch;
        continue;
      }
      if (!inQuote && /\s/.test(ch)) {
        if (buf.length) { tokens.push(buf); buf = ''; }
        continue;
      }
      buf += ch;
    }
    if (buf.length) tokens.push(buf);

    if (tokens[0] !== 'curl') return example;

    const lines = ['curl'];
    let i = 1;
    while (i < tokens.length) {
      const t = tokens[i];
      const next = tokens[i + 1];
      if (t.startsWith('-')) {
        // Option + possible value on same line
        if (next && !next.startsWith('-')) {
          lines.push(`  ${t} ${next}`);
          i += 2;
        } else {
          lines.push(`  ${t}`);
          i += 1;
        }
      } else if (/^https?:\/\//.test(t)) {
        lines.push(`  ${t}`);
        i += 1;
      } else {
        // Fallback: attach to last line
        const last = lines.length - 1;
        lines[last] = `${lines[last]} ${t}`;
        i += 1;
      }
    }

    // Join with line continuations except last line
    return lines.map((l, idx) => idx < lines.length - 1 ? `${l} \\` : l).join('\n');
  }

  // No live fetch — version shown here is managed with releases

  function formatJsonSample(body) {
    if (!body || typeof body !== 'string') return body;
    const trimmed = body.trim();
    if (!trimmed || (!trimmed.startsWith('{') && !trimmed.startsWith('['))) {
      return body;
    }
    try {
      const parsed = JSON.parse(trimmed);
      return JSON.stringify(parsed, null, 2);
    } catch (_) {
      return body;
    }
  }
</script>

<div class="container-xxl">
  <div class="row justify-content-center">
    <div class="col-12 col-xxl-10">
      <Card class="mb-3">
        <div class="card-body p-4">
          <div class="text-center mb-2">
            <div class="fs-20px fw-bold">{$page?.data?.hostName || getHostName()} REST API</div>
            <div class="text-body text-opacity-75">Public documentation of REST endpoints. Interactive pages require login.</div>
          </div>
          <div class="text-center">
            <span class="badge bg-secondary">Version: {API_VERSION}</span>
          </div>
        </div>
      </Card>

      {#if docs && docs.length}
        <div class="mb-4">
          <div class="fw-500 text-body text-opacity-75 small mb-2">Jump to section</div>
          <div class="d-flex flex-wrap gap-2">
            {#each docs as section}
              <a class="btn btn-outline-secondary btn-sm" href={`#${section.id}`}>{section.title}</a>
            {/each}
          </div>
        </div>
      {/if}

      {#each docs as section}
        <div id={section.id} class="mb-3">
          <div class="mb-2">
            <div class="fw-bold fs-20px">{section.title}</div>
            <div class="text-body text-opacity-75 small">{section.description}</div>
          </div>
          <div>
            <div class="row g-3">
              {#each section.endpoints as ep}
                <div class="col-12">
                  <details class="api-details">
                    <summary class="summary-row d-flex align-items-center gap-2 text-break" aria-label="Toggle endpoint details">
                      <span class={methodClass(ep.method)}>{ep.method}</span>
                      <span class="font-monospace flex-grow-1 text-break">{ep.path}</span>
                    </summary>
                    <div class="mt-3">
                      {#if ep.desc}
                        <div class="mb-3">{ep.desc}</div>
                      {/if}

                      {#if ep.params && ep.params.length}
                        <div class="mb-3">
                          {#if ep.params.filter(p => p.in === 'path').length}
                            <div class="fw-500 small text-body text-opacity-75 mb-1">Path parameters</div>
                            <div class="table-responsive">
                              <table class="table table-sm table-bordered small mb-2">
                                <thead><tr><th>Name</th><th>Type</th><th>Req</th><th>Description</th></tr></thead>
                                <tbody>
                                  {#each ep.params.filter(p => p.in === 'path') as p}
                                    <tr><td class="font-monospace">{p.name}</td><td>{p.type}</td><td>{p.required ? 'yes' : 'no'}</td><td>{p.desc}</td></tr>
                                  {/each}
                                </tbody>
                              </table>
                            </div>
                          {/if}
                          {#if ep.params.filter(p => p.in === 'query').length}
                            <div class="fw-500 small text-body text-opacity-75 mb-1">Query parameters</div>
                            <div class="table-responsive">
                              <table class="table table-sm table-bordered small mb-2">
                                <thead><tr><th>Name</th><th>Type</th><th>Req</th><th>Description</th></tr></thead>
                                <tbody>
                                  {#each ep.params.filter(p => p.in === 'query') as p}
                                    <tr><td class="font-monospace">{p.name}</td><td>{p.type}</td><td>{p.required ? 'yes' : 'no'}</td><td>{p.desc}</td></tr>
                                  {/each}
                                </tbody>
                              </table>
                            </div>
                          {/if}
                          {#if ep.params.filter(p => p.in === 'body').length}
                            <div class="fw-500 small text-body text-opacity-75 mb-1">Body fields</div>
                            <div class="table-responsive">
                              <table class="table table-sm table-bordered small mb-0">
                                <thead><tr><th>Name</th><th>Type</th><th>Req</th><th>Description</th></tr></thead>
                                <tbody>
                                  {#each ep.params.filter(p => p.in === 'body') as p}
                                    <tr><td class="font-monospace">{p.name}</td><td>{p.type}</td><td>{p.required ? 'yes' : 'no'}</td><td>{p.desc}</td></tr>
                                  {/each}
                                </tbody>
                              </table>
                            </div>
                          {/if}
                        </div>
                      {/if}

                      {#if ep.example}
                        <div class="mb-3">
                          <div class="fw-500 small text-body text-opacity-75 mb-1">Example</div>
                          <pre class="small bg-dark text-white p-2 rounded mb-0 code-wrap"><code>{formatCurl(ep.example)}</code></pre>
                        </div>
                      {/if}

                      {#if ep.responses && ep.responses.length}
                        <div class="mb-3">
                          <div class="fw-500 small text-body text-opacity-75 mb-1">Response</div>
                          {#each ep.responses as r}
                            <div class="mb-2">
                              <span class="badge bg-primary">HTTP {r.status}</span>
                            </div>
                            {#if r.body}
                              <pre class="small bg-light p-2 rounded mb-0 code-wrap"><code>{formatJsonSample(r.body)}</code></pre>
                            {/if}
                          {/each}
                        </div>
                      {/if}

                    </div>
                  </details>
                </div>
              {/each}
            </div>
          </div>
        </div>
      {/each}
    </div>
  </div>

  <style>
    :global(pre.code-wrap) {
      white-space: pre-wrap;
      word-break: break-word;
      overflow-wrap: anywhere;
    }
    :global(pre.code-wrap code) {
      white-space: inherit;
    }
    /* Ensure anchor targets are not hidden beneath the fixed header */
    :global([id]) {
      scroll-margin-top: 80px;
    }
    :global(details.api-details summary) {
      cursor: pointer;
      list-style: none;
      padding: 0;
      margin: 0;
    }
    :global(details.api-details summary::-webkit-details-marker) {
      display: none;
    }
    :global(.summary-row .badge) {
      display: inline-flex;
      justify-content: center;
      align-items: center;
      min-width: 75px;
    }
  </style>
</div>
