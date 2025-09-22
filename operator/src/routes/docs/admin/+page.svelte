<script>
  import { getApiDocs, methodClass, getCommonSchemas } from '$lib/api/docs.js';
  import { setPageTitle } from '$lib/utils.js';
  import Card from '/src/components/bootstrap/Card.svelte';
  import { page } from '$app/stores';
  import { auth } from '$lib/auth.js';
  import { getHostName } from '$lib/branding.js';

  setPageTitle('Admin APIs');
  const schemas = getCommonSchemas();
  $: hostName = $page?.data?.hostName || getHostName();
  $: isAdmin = $auth && String($auth.type || '').toLowerCase() === 'admin';
  $: base = $page?.data?.hostUrl;
  $: all = getApiDocs(base) || [];
  // Keep only adminOnly endpoints
  $: adminDocs = all
    .map((sec) => ({ ...sec, endpoints: (sec.endpoints || []).filter((ep) => ep.adminOnly) }))
    .filter((sec) => (sec.endpoints || []).length > 0);

  function formatCurl(example) {
    if (!example || !example.trim().startsWith('curl')) return example;
    const tokens = [];
    let buf = '';
    let inQuote = false;
    let quoteChar = '';
    for (let i = 0; i < example.length; i++) {
      const ch = example[i];
      if ((ch === '"' || ch === "'") && (!inQuote || ch === quoteChar)) {
        if (!inQuote) { inQuote = true; quoteChar = ch; }
        else { inQuote = false; quoteChar = ''; }
        buf += ch; continue;
      }
      if (!inQuote && /\s/.test(ch)) { if (buf.length) { tokens.push(buf); buf = ''; } continue; }
      buf += ch;
    }
    if (buf.length) tokens.push(buf);
    if (tokens[0] !== 'curl') return example;
    const lines = ['curl'];
    let i = 1;
    while (i < tokens.length) {
      const t = tokens[i]; const next = tokens[i+1];
      if (t.startsWith('-')) { if (next && !next.startsWith('-')) { lines.push(`  ${t} ${next}`); i+=2; } else { lines.push(`  ${t}`); i+=1; } }
      else if (/^https?:\/\//.test(t)) { lines.push(`  ${t}`); i+=1; }
      else { const last = lines.length - 1; lines[last] = `${lines[last]} ${t}`; i+=1; }
    }
    return lines.map((l, idx) => idx < lines.length - 1 ? `${l} \\` : l).join('\n');
  }
</script>

<div class="container-xxl">
  <div class="row justify-content-center">
    <div class="col-12 col-xxl-10">
      <div class="row">
        <div class="col-xl-9">
          <Card class="mb-3">
            <div class="card-body p-4">
              <div class="text-center mb-2">
                <div class="fs-20px fw-bold">{hostName} Admin APIs</div>
                <div class="text-body text-opacity-75">Endpoints restricted to admin operators.</div>
              </div>
              {#if !isAdmin}
                <div class="alert alert-warning small mb-0">You are not logged in as an admin. Some endpoints may require admin privileges to access.</div>
              {/if}
            </div>
          </Card>

          

          {#each adminDocs as section}
            <div id={section.id} class="mb-3">
              <div class="mb-2">
                <div class="fw-bold fs-20px">{section.title}</div>
                <div class="text-body text-opacity-75 small">{section.description}</div>
              </div>
              <div>
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
                              <span class="badge bg-dark">Admin</span>
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

                          {#if ep.responses && ep.responses.length}
                            <div class="mt-3">
                              <div class="fw-500 small text-body text-opacity-75 mb-1">Response</div>
                              {#each ep.responses as r}
                                <div class="mb-2"><span class="badge bg-primary">HTTP {r.status}</span></div>
                                {#if r.body}
                                  <pre class="small bg-light p-2 rounded mb-0 code-wrap"><code>{r.body}</code></pre>
                                {/if}
                              {/each}
                            </div>
                          {/if}

                          {#if ep.resp}
                            <div class="mt-3">
                              <div class="fw-500 small text-body text-opacity-75 mb-1">Response parameters</div>
                              {#if schemas && ep.resp.schema && schemas[ep.resp.schema] && schemas[ep.resp.schema].length}
                                <div class="small text-body text-opacity-50 mb-1">Schema: {ep.resp.array ? `${ep.resp.schema}[]` : ep.resp.schema}</div>
                                <div class="table-responsive">
                                  <table class="table table-sm table-bordered small mb-0">
                                    <thead><tr><th>Name</th><th>Type</th><th>Description</th></tr></thead>
                                    <tbody>
                                      {#each schemas[ep.resp.schema] as f}
                                        <tr><td class="font-monospace">{f.name}</td><td>{f.type}</td><td>{f.desc}</td></tr>
                                      {/each}
                                    </tbody>
                                  </table>
                                </div>
                              {:else}
                                <div class="small text-body text-opacity-75">No JSON body.</div>
                              {/if}
                            </div>
                          {/if}
                        </div>
                      </Card>
                    </div>
                  {/each}
                </div>
              </div>
            </div>
          {/each}

          <!-- Moved reference sections to end -->
          <Card class="mb-3 mt-3">
            <div class="card-header fw-bold">Error Format</div>
            <div class="card-body p-3 p-sm-4 small">
              <div>On error, endpoints return an HTTP status and a JSON body:</div>
              <pre class="bg-light p-2 rounded mb-0 code-wrap"><code>{`{
  "message": "Error description"
}`}</code></pre>
            </div>
          </Card>

          <Card class="mb-3">
            <div class="card-header fw-bold">Response Object</div>
            <div class="card-body p-3 p-sm-4 small">
              <div class="mb-2">Standard object used by agent response endpoints.</div>
              <div class="table-responsive">
                <table class="table table-sm table-bordered mb-2">
                  <thead><tr><th>Name</th><th>Type</th><th>Description</th></tr></thead>
                  <tbody>
                    <tr><td class="font-monospace">id</td><td>string</td><td>Response ID (UUID)</td></tr>
                    <tr><td class="font-monospace">agent_name</td><td>string</td><td>Agent name</td></tr>
                    <tr><td class="font-monospace">status</td><td>string</td><td>One of: <span class="font-monospace">pending</span>, <span class="font-monospace">processing</span>, <span class="font-monospace">completed</span>, <span class="font-monospace">failed</span>, <span class="font-monospace">cancelled</span></td></tr>
                    <tr><td class="font-monospace">input_content</td><td>array</td><td>User input content items (preferred shape uses <span class="font-monospace">content</span> array; legacy <span class="font-monospace">text</span> accepted)</td></tr>
                    <tr><td class="font-monospace">output_content</td><td>array</td><td>Final content items extracted from <span class="font-monospace">segments</span></td></tr>
                    <tr><td class="font-monospace">segments</td><td>array</td><td>All step-by-step segments/items including commentary, tool calls/results, markers, and final.</td></tr>
                    <tr><td class="font-monospace">created_at</td><td>string (RFC3339)</td><td>Creation timestamp (UTC)</td></tr>
                    <tr><td class="font-monospace">updated_at</td><td>string (RFC3339)</td><td>Last update timestamp (UTC)</td></tr>
                  </tbody>
                </table>
              </div>
            </div>
          </Card>

          <Card class="mb-3">
            <div class="card-header fw-bold">Segments Structure</div>
            <div class="card-body p-3 p-sm-4 small">
              <div class="mb-2">The <span class="font-monospace">segments</span> array captures step-by-step progress, tool usage, and final output.</div>
              <div class="mb-2 small text-body text-opacity-75">For <span class="font-monospace">tool_result</span> items, <span class="font-monospace">output</span> may be a string or JSON. The UI auto-parses JSON-like strings for display.</div>
              <div class="table-responsive mb-2">
                <table class="table table-sm table-bordered">
                  <thead><tr><th>type</th><th>Shape</th><th>Purpose</th></tr></thead>
                  <tbody>
                    <tr><td class="font-monospace">commentary</td><td class="font-monospace">&#123; type, channel: 'analysis', text &#125;</td><td>Internal analysis</td></tr>
                    <tr><td class="font-monospace">tool_call</td><td class="font-monospace">&#123; type, tool, args &#125;</td><td>Declares a tool invocation</td></tr>
                    <tr><td class="font-monospace">tool_result</td><td class="font-monospace">&#123; type, tool, output &#125;</td><td>Result of tool invocation</td></tr>
                    <tr><td class="font-monospace">final</td><td class="font-monospace">&#123; type, channel: 'final', text &#125;</td><td>Final answer (also reflected in <span class="font-monospace">output_content</span>)</td></tr>
                  </tbody>
                </table>
              </div>
            </div>
          </Card>
        </div>

        <style>
          :global(pre.code-wrap) { white-space: pre-wrap; word-break: break-word; overflow-wrap: anywhere; }
          :global(pre.code-wrap code) { white-space: inherit; }
          :global([id]) { scroll-margin-top: 80px; }
        </style>

        <div class="col-xl-3">
          <Card>
            <div class="card-header fw-bold">Sections</div>
            <div class="list-group list-group-flush">
              {#each adminDocs as section}
                <a class="list-group-item list-group-item-action" href={'#' + section.id}>{section.title}</a>
              {/each}
            </div>
          </Card>
        </div>
      </div>
    </div>
  </div>
</div>
