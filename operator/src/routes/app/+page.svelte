<script>
  import { setPageTitle } from '$lib/utils.js';
  import { getOperatorName } from '$lib/auth.js';
  import Card from '/src/components/bootstrap/Card.svelte';
  setPageTitle('Dashboard');
  let name = '';
  $: name = getOperatorName() || 'admin';
</script>

<div class="row">
  <div class="col-xl-12">
    <Card>
      <div class="card-body">
        <div class="fs-20px fw-bold mb-1">Welcome</div>
  <div class="text-body text-opacity-75">Hello, {name}. Use the navigation to manage sandboxes and view tasks. Interactive pages require your active auth token.</div>
      </div>
    </Card>
  </div>
</div>
