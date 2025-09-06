<script>
  import { onMount, onDestroy, tick } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import Card from '/src/components/bootstrap/Card.svelte';
  import { setPageTitle } from '$lib/utils.js';
  import { isAuthenticated } from '$lib/auth.js';
  import { apiFetch } from '$lib/api/client.js';
  import { appOptions } from '/src/stores/appOptions.js';

  let id = '';
  $: id = $page.params.id;
  setPageTitle(`Agent: ${id}`);

  let agent = null;
  let messages = [];
  let loading = true;
  let error = null;
  let input = '';
  let sending = false;
  let pollHandle = null;

  function stateClass(state) {
    const s = String(state || '').toLowerCase();
    if (s === 'init') return 'badge rounded-pill bg-light text-dark';
    if (s === 'idle') return 'badge rounded-pill bg-success';
    if (s === 'busy') return 'badge rounded-pill bg-warning text-dark';
    return 'badge rounded-pill bg-secondary';
  }

  async function fetchAgent() {
    const res = await apiFetch(`/agents/${encodeURIComponent(id)}`);
    if (res.ok) agent = res.data || res;
  }

  async function fetchMessages() {
    const res = await apiFetch(`/agents/${encodeURIComponent(id)}/messages?limit=200`);
    if (res.ok) {
      const list = Array.isArray(res.data) ? res.data : (res.data?.messages || []);
      messages = list;
      await tick();
      scrollToBottom();
    }
  }

  function startPolling() {
    stopPolling();
    pollHandle = setInterval(async () => {
      await fetchMessages();
      await fetchAgent();
    }, 2000);
  }
  function stopPolling() { if (pollHandle) { clearInterval(pollHandle); pollHandle = null; } }

  function scrollToBottom() {
    const el = document.getElementById('chat-body');
    if (el) el.scrollTop = el.scrollHeight;
  }

  async function sendMessage(e) {
    e?.preventDefault?.();
    const content = (input || '').trim();
    if (!content || sending) return;
    sending = true;
    try {
      const res = await apiFetch(`/agents/${encodeURIComponent(id)}/messages`, {
        method: 'POST',
        body: JSON.stringify({ role: 'user', content })
      });
      if (!res.ok) throw new Error(res?.data?.error || `Send failed (HTTP ${res.status})`);
      input = '';
      // Optimistic add
      messages = [...messages, { role: 'user', content }];
      await tick();
      scrollToBottom();
      // Let polling pick up the agent's response
    } catch (e) {
      error = e.message || String(e);
    } finally {
      sending = false;
    }
  }

  async function clearMessages() {
    if (!confirm('Clear this conversation?')) return;
    const res = await apiFetch(`/agents/${encodeURIComponent(id)}/messages`, { method: 'DELETE' });
    if (res.ok) messages = [];
  }

  onMount(async () => {
    if (!isAuthenticated()) { goto('/login'); return; }
    $appOptions.appContentClass = 'p-3';
    $appOptions.appContentFullHeight = true;
    try {
      await fetchAgent();
      await fetchMessages();
      loading = false;
      startPolling();
    } catch (e) {
      error = e.message || String(e);
      loading = false;
    }
  });
  onDestroy(() => { stopPolling(); $appOptions.appContentClass = ''; $appOptions.appContentFullHeight = false; });
</script>

<div class="row g-3">
  <div class="col-xl-8">
    <Card class="h-100" style="z-index: 1020;">
      <div class="card-header d-flex align-items-center gap-2">
        <div class="fw-bold">{id}</div>
        <div>{#if agent}<span class={stateClass(agent.state)}>{agent.state}</span>{/if}</div>
        <div class="ms-auto d-flex gap-2">
          <button class="btn btn-outline-secondary btn-sm" on:click={fetchMessages} aria-label="Refresh">Refresh</button>
          <button class="btn btn-outline-danger btn-sm" on:click={clearMessages} aria-label="Clear conversation">Clear</button>
        </div>
      </div>
      <div class="card-body d-flex flex-column px-3 px-lg-4 py-2" style="min-height: 60vh; background: transparent;">
        {#if loading}
          <div class="flex-fill d-flex align-items-center justify-content-center">
            <div class="text-body text-opacity-75 text-center">
              <div class="spinner-border text-theme mb-3"></div>
              <div>Loading…</div>
            </div>
          </div>
        {:else}
          <div id="chat-body" class="flex-fill overflow-auto px-1 px-sm-2 py-2" style="background: transparent;">
            {#if messages && messages.length}
              {#each messages as m, i}
                {#if m.role === 'user'}
                  <div class="d-flex mb-3 justify-content-end">
                    <div class="p-2 rounded-3 bg-theme text-white" style="max-width: 80%; white-space: pre-wrap; word-break: break-word;">
                      {m.content}
                    </div>
                  </div>
                {:else}
                  <div class="d-flex mb-3 justify-content-start">
                    <div class="text-body" style="max-width: 80%; white-space: pre-wrap; word-break: break-word;">
                      {m.content}
                    </div>
                  </div>
                {/if}
              {/each}
            {:else}
              <div class="text-body text-opacity-75">No messages yet. Say hello!</div>
            {/if}
          </div>
          <form class="border-top pt-2" on:submit|preventDefault={sendMessage}>
            <div class="input-group">
              <input aria-label="Message input" class="form-control" placeholder="Type a message…" bind:value={input} on:keydown={(e)=>{ if(e.key==='Enter' && !e.shiftKey){ e.preventDefault(); sendMessage(); }}} />
              <button class="btn btn-theme" aria-label="Send message" disabled={sending || !input.trim()}>Send</button>
            </div>
          </form>
        {/if}
      </div>
    </Card>
  </div>
  <div class="col-xl-4">
    <Card>
      <div class="card-header fw-bold">Details</div>
      <div class="card-body small">
        {#if agent}
          <div class="mb-2"><span class="text-body text-opacity-75">Name:</span> <span class="font-monospace">{agent.name}</span></div>
          <div class="mb-2"><span class="text-body text-opacity-75">State:</span> <span class={stateClass(agent.state)}>{agent.state}</span></div>
          {#if agent.timeout_seconds !== undefined}
            <div class="mb-2"><span class="text-body text-opacity-75">Timeout:</span> {agent.timeout_seconds}s</div>
          {/if}
          {#if agent.content_port}
            <div class="mb-2"><span class="text-body text-opacity-75">Content:</span> <a href={'/content/'+agent.name} target="_blank" class="text-decoration-none">/content/{agent.name}</a></div>
          {/if}
          {#if agent.metadata}
            <div class="fw-500 mt-3">Metadata</div>
            <pre class="bg-dark text-white p-2 rounded code-wrap"><code>{JSON.stringify(agent.metadata, null, 2)}</code></pre>
          {/if}
        {:else}
          <div class="text-body text-opacity-75">No agent details.</div>
        {/if}
      </div>
    </Card>
  </div>

  <style>
    :global(pre.code-wrap) { white-space: pre-wrap; word-break: break-word; overflow-wrap: anywhere; }
  </style>
</div>
