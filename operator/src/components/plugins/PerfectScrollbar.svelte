<script>
  import PerfectScrollbar from 'perfect-scrollbar';
  import { onMount, onDestroy, afterUpdate } from 'svelte';

  let container;
  let ps;
  let ro;

  onMount(() => {
    ps = new PerfectScrollbar(container, {
      wheelPropagation: true,
      swipePropagation: true,
      suppressScrollX: true
    });
    if (window && 'ResizeObserver' in window) {
      ro = new ResizeObserver(() => { try { ps && ps.update(); } catch (_) {} });
      ro.observe(container);
    }
  });
  afterUpdate(() => { try { ps && ps.update(); } catch (_) {} });
  onDestroy(() => {
    try { ro && ro.disconnect(); } catch (_) {}
    if (ps) ps.destroy();
  });
</script>
<div bind:this={container} {...$$restProps}>
  <slot />
</div>
