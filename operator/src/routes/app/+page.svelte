<script>
  import { setPageTitle } from '$lib/utils.js';
  import { getOperatorName } from '$lib/auth.js';
  setPageTitle('Dashboard');
  let name = '';
  $: name = getOperatorName() || 'operator';
</script>

<div class="row">
  <div class="col-xl-12">
    <div class="card">
      <div class="card-body">
        <div class="fs-18px fw-bold mb-1">Welcome</div>
        <div class="text-body text-opacity-75">Hello, {name}. Use the navigation to manage agents and view messages. Interactive pages require your active session.</div>
      </div>
    </div>
  </div>
</div>

