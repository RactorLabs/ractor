<script>
  import PerfectScrollbar from 'perfect-scrollbar';
  import { onMount, onDestroy } from 'svelte';

  let container;
  let ps;

  onMount(() => {
    ps = new PerfectScrollbar(container);
  });
  onDestroy(() => {
    if (ps) ps.destroy();
  });
</script>
<div bind:this={container} {...$$restProps}>
  <slot />
</div>
