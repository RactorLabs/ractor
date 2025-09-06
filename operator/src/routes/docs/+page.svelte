<script>
  import { apiDocs, methodClass } from '$lib/api/docs.js';
  import { setPageTitle } from '$lib/utils.js';
  import Card from '/src/components/bootstrap/Card.svelte';

  // Hard-coded docs version; update during version bumps
  const API_VERSION = '0.6.0 (v0)';

  setPageTitle('API Documentation');

  // No live fetch — version shown here is managed with releases
</script>

<div class="row">
  <div class="col-xl-9">
    <Card class="mb-3">
      <div class="card-body">
        <div class="d-flex align-items-center">
          <div class="flex-1">
            <div class="fs-18px fw-bold">Raworc REST API</div>
            <div class="text-body text-opacity-75">Public documentation of REST endpoints. Interactive pages require login.</div>
          </div>
          <div>
            <span class="badge bg-secondary">Version: {API_VERSION}</span>
          </div>
        </div>
        <!-- Version is hard-coded; no live fetch on docs page. -->
      </div>
    </Card>

    {#each apiDocs as section}
    <Card id={section.id} class="mb-3">
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
                    {#if ep.params && ep.params.length}
                      <div class="mt-2">
                        {#if ep.params.filter(p => p.in === 'path').length}
                          <div class="fw-500 small mb-1">Path parameters</div>
                          <div class="table-responsive">
                            <table class="table table-sm table-bordered small">
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
                          <div class="fw-500 small mb-1">Query parameters</div>
                          <div class="table-responsive">
                            <table class="table table-sm table-bordered small">
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
                          <div class="fw-500 small mb-1">Body fields</div>
                          <div class="table-responsive">
                            <table class="table table-sm table-bordered small">
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
                      <pre class="small mt-2 bg-dark text-white p-2 rounded"><code>{ep.example}</code></pre>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>
    </Card>
    {/each}
  </div>

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
