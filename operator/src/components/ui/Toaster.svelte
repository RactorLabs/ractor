<script>
  import { toasts, removeToast } from '/src/stores/toast.js';
  import { fly, fade } from 'svelte/transition';

  let list = [];
  const unsub = toasts.subscribe((v) => list = v);
</script>

<div class="toasts-container">
  {#each list as t (t.id)}
    <div class="toast show mb-2 border-0 shadow-sm bg-white" in:fly={{ x: 20, duration: 120 }} out:fade>
      <div class="toast-header">
        <span class="badge me-2 bg-{t.variant || 'info'}">{(t.variant || 'info').toUpperCase()}</span>
        <strong class="me-auto">{t.title || ''}</strong>
        <button type="button" class="btn-close" aria-label="Close" on:click={() => removeToast(t.id)}></button>
      </div>
      {#if t.message}
        <div class="toast-body small">{t.message}</div>
      {/if}
    </div>
  {/each}
</div>

