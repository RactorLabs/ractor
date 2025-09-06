<script>
  import { onMount } from 'svelte';
  import { apiDocs, methodClass } from '$lib/api/docs.js';
  import { setPageTitle } from '$lib/utils.js';

  let apiVersion = null;
  let apiError = null;

  setPageTitle('API Documentation');

  onMount(async () => {
    try {
      const res = await fetch('/api/v0/version');
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      apiVersion = data?.version ? `${data.version} (${data.api})` : JSON.stringify(data);
    } catch (e) {
      apiError = e.message;
    }
  });
</script>

<div class="row">
  <div class="col-xl-9">
    <div class="card mb-3">
      <div class="card-body">
        <div class="d-flex align-items-center">
          <div class="flex-1">
            <div class="fs-18px fw-bold">Raworc REST API</div>
            <div class="text-body text-opacity-75">Public documentation of REST endpoints. Interactive pages require login.</div>
          </div>
          <div>
            {#if apiVersion}
              <span class="badge bg-secondary">Version: {apiVersion}</span>
            {:else if apiError}
              <span class="badge bg-danger">Version unavailable: {apiError}</span>
            {:else}
              <span class="badge bg-light text-muted">Fetching version…</span>
            {/if}
          </div>
        </div>
        {#if apiError}
        <div class="alert alert-warning mt-3 small">
          <div class="fw-bold mb-1">Why am I seeing “Version unavailable: {apiError}”?</div>
          <div>
            The docs page calls <code>/api/v0/version</code> on the current origin. A 404 usually means the UI is running via the Vite dev server without a proxy to the API, or the API isn’t running at this origin.
          </div>
          <div class="mt-2">
            Fix options:
            <ul class="mb-0">
              <li>Start services with the CLI so the Operator is served alongside the API: <code>raworc start server controller</code> (or <code>raworc start</code>).</li>
              <li>If developing Operator standalone, add a Vite proxy for <code>/api/v0</code> to your API (e.g. <code>http://localhost:9000</code>).</li>
            </ul>
          </div>
        </div>
        {/if}
      </div>
    </div>

    {#each apiDocs as section}
    <div id={section.id} class="card mb-3">
      <div class="card-header d-flex align-items-center">
        <div class="flex-1">
          <div class="fw-bold">{section.title}</div>
          <div class="text-body text-opacity-75 small">{section.description}</div>
        </div>
      </div>
      <div class="card-body p-0">
        <div class="table-responsive">
          <table class="table table-sm table-borderless align-middle mb-0">
            <thead>
              <tr class="text-body text-opacity-75 small">
                <th style="width: 90px;">Method</th>
                <th>Path</th>
                <th>Auth</th>
                <th>Description</th>
              </tr>
            </thead>
            <tbody>
              {#each section.endpoints as ep}
                <tr>
                  <td><span class={methodClass(ep.method)}>{ep.method}</span></td>
                  <td class="font-monospace">{ep.path}</td>
                  <td>
                    {#if ep.auth === 'bearer'}<span class="badge bg-dark">Bearer</span>
                    {:else}<span class="badge bg-success">Public</span>{/if}
                  </td>
                  <td>
                    <div>{ep.desc}</div>
                    {#if ep.body}
                      <div class="mt-1 small"><span class="text-body text-opacity-75">Body:</span> <code>{ep.body}</code></div>
                    {/if}
                    {#if ep.example}
                      <pre class="small mt-2 bg-dark text-white p-2 rounded"><code>{ep.example}</code></pre>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>
    </div>
    {/each}
  </div>

  <div class="col-xl-3">
    <div class="card">
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
    </div>
  </div>
</div>
