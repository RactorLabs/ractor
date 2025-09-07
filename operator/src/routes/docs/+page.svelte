<script>
  import { apiDocs, methodClass } from '$lib/api/docs.js';
  import { setPageTitle } from '$lib/utils.js';
  import Card from '/src/components/bootstrap/Card.svelte';

  // Hard-coded docs version; update during version bumps
  .6.1 (v0)';

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
</script>

<div class="container-xxl">
  <div class="row justify-content-center">
    <div class="col-12 col-xxl-10">
      <div class="row">
  <div class="col-xl-9">
    <Card class="mb-3">
      <div class="card-body p-4">
        <div class="text-center mb-2">
          <div class="fs-20px fw-bold">Raworc REST API</div>
          <div class="text-body text-opacity-75">Public documentation of REST endpoints. Interactive pages require login.</div>
        </div>
        <div class="text-center">
          <span class="badge bg-secondary">Version: {API_VERSION}</span>
        </div>
      </div>
    </Card>

    {#each apiDocs as section}
      <Card id={section.id} class="mb-3">
        <div class="card-header">
          <div class="fw-bold">{section.title}</div>
          <div class="text-body text-opacity-75 small">{section.description}</div>
        </div>
        <div class="card-body p-3 p-sm-4">
          <div class="row g-3">
            {#each section.endpoints as ep}
              <div class="col-12">
                <Card>
                  <div class="card-body p-3 p-sm-4">
                    <div class="d-flex align-items-start align-items-sm-center flex-column flex-sm-row gap-2">
                      <div class="d-flex align-items-center gap-2">
                        <span class={methodClass(ep.method)}>{ep.method}</span>
                        <span class="font-monospace">{ep.path}</span>
                      </div>
                      <div class="ms-sm-auto d-flex align-items-center">
                        {#if ep.auth === 'bearer'}
                          <span class="badge bg-dark">Bearer</span>
                        {:else}
                          <span class="badge bg-success">Public</span>
                        {/if}
                      </div>
                    </div>
                    <div class="mt-2">{ep.desc}</div>

                    {#if ep.params && ep.params.length}
                      <div class="mt-3">
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
                      <div class="mt-3">
                        <div class="fw-500 small text-body text-opacity-75 mb-1">Example</div>
                        <pre class="small bg-dark text-white p-2 rounded mb-0 code-wrap"><code>{formatCurl(ep.example)}</code></pre>
                      </div>
                    {/if}
                  </div>
                </Card>
              </div>
            {/each}
          </div>
        </div>
      </Card>
    {/each}
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
  </style>

  <div class="col-xl-3">
    <Card>
      <div class="card-header fw-bold">Sections</div>
      <div class="list-group list-group-flush">
        {#each apiDocs as section}
          <a class="list-group-item list-group-item-action" href={'#' + section.id}>{section.title}</a>
        {/each}
      </div>
      <div class="card-body small text-body text-opacity-75">
        <div>Interactive views are available after login.</div>
        <div class="mt-2"><a href="/login" class="text-decoration-none">Go to Login</a></div>
      </div>
    </Card>
  </div>
      </div>
    </div>
  </div>
</div>
