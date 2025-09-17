<script>
  import { getApiDocs, methodClass } from '$lib/api/docs.js';
  import { setPageTitle } from '$lib/utils.js';
  import Card from '/src/components/bootstrap/Card.svelte';
  import { page } from '$app/stores';
  import { getHostName } from '$lib/branding.js';

  // Hard-coded docs version; update during version bumps
  const API_VERSION = '0.7.9 (v0)';
  

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
          <div class="fs-20px fw-bold">{$page?.data?.hostName || getHostName()} REST API</div>
          <div class="text-body text-opacity-75">Public documentation of REST endpoints. Interactive pages require login.</div>
        </div>
        <div class="text-center">
          <span class="badge bg-secondary">Version: {API_VERSION}</span>
        </div>
      </div>
    </Card>

    <Card class="mb-3">
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
        <div class="mb-2">Standard object returned by <span class="font-monospace">/api/v0/agents/{name}/responses</span> endpoints.</div>
        <div class="table-responsive">
          <table class="table table-sm table-bordered mb-2">
            <thead>
              <tr><th>Name</th><th>Type</th><th>Description</th></tr>
            </thead>
            <tbody>
              <tr><td class="font-monospace">id</td><td>string</td><td>Response ID (UUID)</td></tr>
              <tr><td class="font-monospace">agent_name</td><td>string</td><td>Agent name</td></tr>
              <tr><td class="font-monospace">status</td><td>string</td><td>One of: <span class="font-monospace">pending</span>, <span class="font-monospace">processing</span>, <span class="font-monospace">completed</span>, <span class="font-monospace">failed</span></td></tr>
              <tr><td class="font-monospace">input</td><td>object</td><td>User input JSON (typically <span class="font-monospace">{ text: string }</span>)</td></tr>
              <tr><td class="font-monospace">output</td><td>object</td><td>Agent output JSON with fields below</td></tr>
              <tr><td class="font-monospace">created_at</td><td>string (RFC3339)</td><td>Creation timestamp (UTC)</td></tr>
              <tr><td class="font-monospace">updated_at</td><td>string (RFC3339)</td><td>Last update timestamp (UTC)</td></tr>
            </tbody>
          </table>
        </div>
        <div class="fw-500 small text-body text-opacity-75 mb-1">output fields</div>
        <div class="table-responsive">
          <table class="table table-sm table-bordered mb-2">
            <thead>
              <tr><th>Name</th><th>Type</th><th>Description</th></tr>
            </thead>
            <tbody>
              <tr><td class="font-monospace">text</td><td>string</td><td>Final assistant message (may be empty while processing)</td></tr>
              <tr><td class="font-monospace">items</td><td>array</td><td>Ordered list of structured items (see “Items Structure”)</td></tr>
            </tbody>
          </table>
        </div>
        <ul class="mb-0">
          <li>GET list is ordered by <span class="font-monospace">created_at</span> ascending.</li>
          <li>Update semantics: <span class="font-monospace">output.text</span> replaces; <span class="font-monospace">output.items</span> appends; other <span class="font-monospace">output</span> keys overwrite.</li>
          <li>Typical <span class="font-monospace">input</span> is <span class="font-monospace">{ text: string }</span>, but arbitrary JSON is allowed.</li>
        </ul>
      </div>
    </Card>

    <Card class="mb-3">
      <div class="card-header fw-bold">Items Structure (output.items)</div>
      <div class="card-body p-3 p-sm-4 small">
        <div class="mb-2">The <span class="font-monospace">output.items</span> array captures step-by-step progress, tool usage, and final output. Items are appended in order.</div>
        <div class="fw-500 small text-body text-opacity-75 mb-1">Item types</div>
        <div class="table-responsive mb-2">
          <table class="table table-sm table-bordered">
            <thead><tr><th>type</th><th>Shape</th><th>Purpose</th></tr></thead>
            <tbody>
              <tr>
                <td class="font-monospace">commentary</td>
                <td class="font-monospace">{ type, channel: 'analysis', text }</td>
                <td>Internal thinking/analysis. Hidden in UI unless details are shown.</td>
              </tr>
              <tr>
                <td class="font-monospace">tool_call</td>
                <td class="font-monospace">{ type, tool, args }</td>
                <td>Declares a tool invocation with arguments.</td>
              </tr>
              <tr>
                <td class="font-monospace">tool_result</td>
                <td class="font-monospace">{ type, tool, output }</td>
                <td>Result of the preceding matching <span class="font-monospace">tool_call</span>.</td>
              </tr>
              <tr>
                <td class="font-monospace">final</td>
                <td class="font-monospace">{ type, channel: 'final', text }</td>
                <td>Final assistant answer (mirrors <span class="font-monospace">output.text</span>).</td>
              </tr>
            </tbody>
          </table>
        </div>
        <div class="fw-500 small text-body text-opacity-75 mb-1">Examples</div>
        <pre class="small bg-light p-2 rounded mb-2 code-wrap"><code>{JSON.stringify({ type: 'commentary', channel: 'analysis', text: 'Thinking about the approach…' }, null, 2)}</code></pre>
        <pre class="small bg-light p-2 rounded mb-2 code-wrap"><code>{JSON.stringify({ type: 'tool_call', tool: 'bash', args: { command: 'ls -la', cwd: '/agent/code' } }, null, 2)}</code></pre>
        <pre class="small bg-light p-2 rounded mb-2 code-wrap"><code>{JSON.stringify({ type: 'tool_result', tool: 'bash', output: '[exit_code:0]\nREADME.md\nsrc/' }, null, 2)}</code></pre>
        <pre class="small bg-light p-2 rounded mb-0 code-wrap"><code>{JSON.stringify({ type: 'final', channel: 'final', text: 'All set! Here are the results…' }, null, 2)}</code></pre>
        <div class="mt-2 small text-body text-opacity-75">Notes: Tool outputs may be truncated for size; the UI pairs each <span class="font-monospace">tool_call</span> with the next <span class="font-monospace">tool_result</span> having the same <span class="font-monospace">tool</span>.</div>
      </div>
    </Card>

    {#each getApiDocs($page?.data?.hostUrl) as section}
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

                    {#if ep.responses && ep.responses.length}
                      <div class="mt-3">
                        <div class="fw-500 small text-body text-opacity-75 mb-1">Response</div>
                        {#each ep.responses as r}
                          <div class="mb-2">
                            <span class="badge bg-primary">HTTP {r.status}</span>
                          </div>
                          {#if r.body}
                            <pre class="small bg-light p-2 rounded mb-0 code-wrap"><code>{r.body}</code></pre>
                          {/if}
                        {/each}
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
        {#each getApiDocs($page?.data?.hostUrl) as section}
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
