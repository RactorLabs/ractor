<script>
  import { setPageTitle } from '$lib/utils.js';
  import Card from '/src/components/bootstrap/Card.svelte';
  setPageTitle('Home');
  import { page } from '$app/stores';
  $: hostName = $page?.data?.hostName || 'Raworc';
</script>

<div class="row">
  <div class="col-xl-12">
    <Card>
      <div class="card-body p-4">
        <div class="fs-18px fw-bold mb-1">Welcome to {hostName}</div>
        <div class="text-body text-opacity-75">Use the navigation to explore the UI and API documentation.</div>
        <div class="mt-3">
          <a href="/docs" class="btn btn-outline-theme">View API Documentation</a>
        </div>
      </div>
    </Card>
  </div>
</div>
