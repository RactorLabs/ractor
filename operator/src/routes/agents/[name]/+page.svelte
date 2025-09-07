<script>
  import { onMount, onDestroy, tick } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import { setPageTitle } from '$lib/utils.js';
  import { isAuthenticated } from '$lib/auth.js';
  import { apiFetch } from '$lib/api/client.js';
  import { appOptions } from '/src/stores/appOptions.js';
  import MarkdownIt from 'markdown-it';
  import taskLists from 'markdown-it-task-lists';
  import { browser } from '$app/environment';

  let md;
  try {
    md = new MarkdownIt({ html: false, linkify: true, breaks: true }).use(taskLists, { label: true, labelAfter: true });
  } catch (e) {
    console.error('Markdown init failed', e);
    md = null;
  }

  function renderMarkdown(s) {
    try {
      if (!s || !s.trim()) return '';
      if (md) return md.render(s);
    } catch (e) {
      console.error('Markdown render failed', e);
    }
    // Fallback: minimal formatting
    const esc = (t) => t.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    return `<pre class="mb-0">${esc(s)}</pre>`;
  }

  let name = '';
  $: name = $page.params.name;
  setPageTitle(`Agent: ${name}`);

  let agent = null;
  let stateStr = '';
  let messages = [];
  let loading = true;
  let error = null;
  let input = '';
  let sending = false;
  let pollHandle = null;
  let frameBaseUrl = '';
  let frameUrl = '';
  let frameOpacity = 1;
  let contentAvailable = false;
  let contentProbing = false;

  function stateClass(state) {
    const s = String(state || '').toLowerCase();
    if (s === 'init') return 'badge rounded-pill bg-light text-dark';
    if (s === 'idle') return 'badge rounded-pill bg-success';
    if (s === 'busy') return 'badge rounded-pill bg-warning text-dark';
    return 'badge rounded-pill bg-secondary';
  }

  function normState(v) { return String(v || '').trim().toLowerCase(); }
  $: stateStr = normState(agent?.state);

  function isSlept() { return stateStr === 'slept'; }
  function isAwake() { return stateStr === 'idle' || stateStr === 'busy'; }
  function isInitOrDeleted() { return stateStr === 'init' || stateStr === 'deleted'; }

  async function fetchAgent() {
    const res = await apiFetch(`/agents/${encodeURIComponent(name)}`);
    if (res.ok && res.data) {
      agent = res.data;
    }
    const baseChanged = computeFrameUrl();
    if (baseChanged) {
      // Only reload the iframe when the base URL actually changed
      refreshFrameUrl();
      // Probe availability only when base changes (no auto refresh)
      probeContent();
    }
  }

  async function fetchMessages() {
    const res = await apiFetch(`/agents/${encodeURIComponent(name)}/messages?limit=200`);
    if (res.ok) {
      const list = Array.isArray(res.data) ? res.data : (res.data?.messages || []);
      // Only auto-stick if near bottom before refresh
      let shouldStick = true;
      try {
        const el = typeof document !== 'undefined' ? document.getElementById('chat-body') : null;
        if (el) {
          const delta = el.scrollHeight - el.scrollTop - el.clientHeight;
          shouldStick = delta < 80;
        }
      } catch (_) {}
      messages = list;
      await tick();
      if (shouldStick) scrollToBottom();
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

  // Auto-refresh logic removed entirely

  function computeFrameUrl() {
    let next = '';
    try {
      if (agent && (agent.content_port || agent.contentPort)) {
        const port = agent.content_port || agent.contentPort;
        const host = typeof window !== 'undefined' ? window.location.hostname : 'localhost';
        const proto = typeof window !== 'undefined' ? window.location.protocol : 'http:';
        next = `${proto}//${host}:${port}/`;
      } else if (agent && agent.name) {
        // Fallback to published content via gateway
        next = `/content/${agent.name}/`;
      } else {
        next = '';
      }
    } catch (_) {
      next = '';
    }
    const changed = next !== frameBaseUrl;
    frameBaseUrl = next;
    return changed;
  }

  function refreshFrameUrl() {
    if (!frameBaseUrl) { frameUrl = ''; return; }
    const sep = frameBaseUrl.includes('?') ? '&' : '?';
    frameOpacity = 0;
    frameUrl = `${frameBaseUrl}${sep}t=${Date.now()}`;
  }

  // No periodic signature checking or automatic refresh

  async function probeContent() {
    contentAvailable = false;
    if (!frameBaseUrl) return;
    contentProbing = true;
    try {
      // If using direct port (cross-origin), skip HEAD to avoid CORS and assume available
      if (/^https?:\/\//.test(frameBaseUrl)) {
        contentAvailable = true;
      } else {
        const url = frameBaseUrl.endsWith('/') ? `${frameBaseUrl}index.html` : frameBaseUrl;
        const res = await fetch(url, { method: 'HEAD' });
        contentAvailable = res.ok;
      }
    } catch (_) {
      contentAvailable = false;
    } finally {
      contentProbing = false;
    }
  }

  function scrollToBottom() {
    try {
      if (typeof document === 'undefined') return;
      const el = document.getElementById('chat-body');
      if (el) el.scrollTop = el.scrollHeight;
    } catch (_) {}
  }

  // Helper: map tool key to display label
  function toolLabel(t) {
    const k = String(t || '').toLowerCase();
    if (k === 'bash') return 'Bash';
    if (k === 'text_editor') return 'Text Editor';
    return k ? (k[0].toUpperCase() + k.slice(1)) : 'Tool';
  }

  // Helper: detect a tool execution message
  function isToolExec(m) {
    try { return m && m.metadata && m.metadata.type === 'tool_execution' && m.metadata.tool_type; } catch (_) { return false; }
  }

  // Helper: detect a tool result message
  function isToolResult(m) {
    try { return m && m.metadata && m.metadata.type === 'tool_result' && m.metadata.tool_type; } catch (_) { return false; }
  }

  // Helper: for Text Editor description like "write /path/file" => { action, path }
  function parseTextEditorDesc(desc) {
    const s = String(desc || '').trim();
    if (!s) return { action: '', path: '' };
    const firstSpace = s.indexOf(' ');
    if (firstSpace === -1) return { action: s, path: '' };
    return { action: s.slice(0, firstSpace), path: s.slice(firstSpace + 1).trim() };
  }

  // Expand/Collapse all tool details helpers
  function expandAllTools() {
    try {
      if (typeof document === 'undefined') return;
      const list = document.querySelectorAll('#chat-body details');
      list.forEach((el) => { try { el.open = true; } catch (_) {} });
    } catch (_) {}
  }
  function collapseAllTools() {
    try {
      if (typeof document === 'undefined') return;
      const list = document.querySelectorAll('#chat-body details');
      list.forEach((el) => { try { el.open = false; } catch (_) {} });
    } catch (_) {}
  }

  // Helper: compact args preview for tool summaries
  function argsPreview(m) {
    try {
      const t = String(m?.metadata?.tool_type || '').toLowerCase();
      const a = m?.metadata?.args;
      if (!a || typeof a !== 'object') return '';
      if (t === 'bash') {
        const cmd = a.command || a.cmd || '';
        if (!cmd) return '';
        return `(${String(cmd).trim().slice(0, 80)})`;
      }
      if (t === 'text_editor') {
        const action = a.action || 'edit';
        const path = a.path || '';
        const extra = path ? ` ${path}` : '';
        return `(${action}${extra})`;
      }
      const json = JSON.stringify(a);
      if (!json) return '';
      const short = json.length > 80 ? json.slice(0, 77) + '…' : json;
      return `(${short})`;
    } catch (_) { return ''; }
  }

  async function sendMessage(e) {
    e?.preventDefault?.();
    const content = (input || '').trim();
    if (!content || sending) return;
    sending = true;
    try {
      const res = await apiFetch(`/agents/${encodeURIComponent(name)}/messages`, {
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

  async function sleepAgent() {
    try {
      const res = await apiFetch(`/agents/${encodeURIComponent(name)}/sleep`, { method: 'POST' });
      if (!res.ok) throw new Error(res?.data?.error || `Sleep failed (HTTP ${res.status})`);
      // Optimistic UI update to reflect new state immediately
      if (agent) agent = { ...(agent || {}), state: 'slept' };
      // Give the controller a moment to persist the state before fetching
      await new Promise((r) => setTimeout(r, 600));
      await fetchAgent();
      error = null;
    } catch (e) {
      error = e.message || String(e);
    }
  }

  async function wakeAgent() {
    try {
      const res = await apiFetch(`/agents/${encodeURIComponent(name)}/wake`, { method: 'POST', body: JSON.stringify({}) });
      if (!res.ok) throw new Error(res?.data?.error || `Wake failed (HTTP ${res.status})`);
      // Optimistic UI update: reflect server semantics (state becomes 'init' first)
      if (agent) agent = { ...(agent || {}), state: 'init' };
      // Give the controller a moment to recreate the container before fetching
      await new Promise((r) => setTimeout(r, 600));
      await fetchAgent();
      error = null;
    } catch (e) {
      error = e.message || String(e);
    }
  }

  async function publishAgent() {
    try {
      const res = await apiFetch(`/agents/${encodeURIComponent(name)}/publish`, {
        method: 'POST',
        body: JSON.stringify({ content: true })
      });
      if (!res.ok) throw new Error(res?.data?.error || `Publish failed (HTTP ${res.status})`);
      if (agent) {
        agent = { ...(agent || {}), is_published: true, isPublished: true };
      }
      // Recompute frame URL to use published path if no direct port
      computeFrameUrl();
      error = null;
    } catch (e) {
      error = e.message || String(e);
    }
  }

  async function unpublishAgent() {
    try {
      const res = await apiFetch(`/agents/${encodeURIComponent(name)}/unpublish`, { method: 'POST' });
      if (!res.ok) throw new Error(res?.data?.error || `Unpublish failed (HTTP ${res.status})`);
      if (agent) {
        agent = { ...(agent || {}), is_published: false, isPublished: false };
      }
      computeFrameUrl();
      error = null;
    } catch (e) {
      error = e.message || String(e);
    }
  }

  // Remix current agent into a new one and navigate to it
  async function remixAgent() {
    try {
      const cur = String(name || '').trim();
      const suggestion = (cur && /^[a-z0-9-]+$/.test(cur)) ? `${cur}-remix` : 'new-agent';
      const next = typeof window !== 'undefined' ? window.prompt('Enter new agent name', suggestion) : null;
      if (!next) return;
      const newName = String(next).trim();
      const pattern = /^[a-z][a-z0-9-]{0,61}[a-z0-9]$/;
      if (!pattern.test(newName)) {
        error = 'Invalid name. Use ^[a-z][a-z0-9-]{0,61}[a-z0-9]$';
        return;
      }

      const res = await apiFetch(`/agents/${encodeURIComponent(name)}/remix`, {
        method: 'POST',
        body: JSON.stringify({ name: newName, code: true, secrets: true, content: true })
      });
      if (!res.ok) throw new Error(res?.data?.error || `Remix failed (HTTP ${res.status})`);
      // Navigate to the remixed agent
      goto(`/agents/${encodeURIComponent(newName)}`);
    } catch (e) {
      error = e.message || String(e);
    }
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
      // Initial iframe load
      if (frameBaseUrl) refreshFrameUrl();
    } catch (e) {
      error = e.message || String(e);
      loading = false;
    }
  });
  onDestroy(() => { stopPolling(); $appOptions.appContentClass = ''; $appOptions.appContentFullHeight = false; });
</script>

<div class="row g-3 h-100">
  <div class="col-12 col-xl-8 d-flex flex-column h-100" style="min-height: 0;">
    <div class="d-flex align-items-center gap-2 mb-2 px-3 py-2 border rounded-2 bg-body">
      <div class="fw-bold">{name}</div>
      <div>{#if agent}<span class={stateClass(stateStr)}>{stateStr}</span>{/if}</div>
      <div class="ms-auto d-flex gap-2">
        <div class="dropdown">
          <button class="btn btn-outline-secondary btn-sm dropdown-toggle" type="button" data-bs-toggle="dropdown" aria-expanded="false" aria-label="Agent menu">
            Menu
          </button>
          <ul class="dropdown-menu dropdown-menu-end">
            <li><button class="dropdown-item" on:click={remixAgent}>Remix</button></li>
          </ul>
        </div>
        <button class="btn btn-outline-secondary btn-sm" on:click={expandAllTools} aria-label="Expand all tool details" title="Expand all"><i class="bi bi-arrows-expand"></i></button>
        <button class="btn btn-outline-secondary btn-sm" on:click={collapseAllTools} aria-label="Collapse all tool details" title="Collapse all"><i class="bi bi-arrows-collapse"></i></button>
        {#if stateStr === 'slept'}
          <button class="btn btn-outline-success btn-sm" on:click={wakeAgent} aria-label="Wake agent">Wake</button>
        {:else if stateStr === 'idle' || stateStr === 'busy'}
          <button class="btn btn-outline-warning btn-sm" on:click={sleepAgent} aria-label="Put agent to sleep">Sleep</button>
        {:else}
          <button class="btn btn-outline-warning btn-sm" disabled aria-label="Put agent to sleep" title="Agent not ready">Sleep</button>
        {/if}
      </div>
    </div>
    {#if error}
      <div class="alert alert-danger py-2 small mb-2">{error}</div>
    {/if}
    {#if loading}
      <div class="flex-fill d-flex align-items-center justify-content-center border rounded-2 bg-body">
        <div class="text-body text-opacity-75 text-center p-3">
          <div class="spinner-border text-theme mb-3"></div>
          <div>Loading…</div>
        </div>
      </div>
    {:else}
      <div id="chat-body" class="flex-fill px-2 py-2 border rounded-2" style="background: transparent; overflow-y: auto; min-height: 0; height: 100%;">
        <div class="d-flex flex-column justify-content-end" style="min-height: 100%;">
        {#if messages && messages.length}
          {#each messages as m, i}
            {#if m.role === 'user'}
              <div class="d-flex mb-3 justify-content-end">
                <div class="p-2 rounded-3 bg-dark text-white" style="max-width: 80%; white-space: pre-wrap; word-break: break-word;">
                  {m.content}
                </div>
              </div>
            {:else}
              <!-- Agent side -->
              {#if isToolExec(m)}
                <!-- Compact single-line summary that toggles details for ALL tool requests -->
                <div class="d-flex mb-2 justify-content-start">
                  <details class="mt-0">
                    <summary class="small fw-500 text-body text-opacity-75" style="cursor: pointer;">
                      {toolLabel(m.metadata.tool_type)} Request {argsPreview(m)}
                    </summary>
                    <pre class="small bg-dark text-white p-2 rounded mb-0 code-wrap"><code>{JSON.stringify({ tool: m?.metadata?.tool_type || 'tool', args: (m?.metadata?.args ?? { text: m.content }) }, null, 2)}</code></pre>
                  </details>
                </div>
              {:else}
                <!-- Tool response card or regular agent message -->
                {#if isToolResult(m)}
                  <!-- Compact single-line summary that toggles details for ALL tool responses -->
                  <div class="d-flex mb-2 justify-content-start">
                    <details class="mt-0">
                      <summary class="small fw-500 text-body text-opacity-75" style="cursor: pointer;">
                        {toolLabel(m.metadata.tool_type)} Response {argsPreview(m)}
                      </summary>
                      <pre class="small bg-dark text-white p-2 rounded mb-0 code-wrap"><code>{JSON.stringify({ tool: m?.metadata?.tool_type || 'tool', args: (m?.metadata?.args ?? null), output: m.content }, null, 2)}</code></pre>
                    </details>
                  </div>
                {:else}
                  <div class="d-flex mb-3 justify-content-start">
                    <div class="text-body" style="max-width: 80%; word-break: break-word;">
                      {#if m.metadata && m.metadata.thinking}
                        <div class="small fst-italic text-body text-opacity-50 mt-1 mb-2" style="white-space: pre-wrap;">{m.metadata.thinking}</div>
                      {/if}
                      {#if m.content && m.content.trim()}
                        <div class="markdown-wrap">
                          <div class="markdown-body">{@html renderMarkdown(m.content)}</div>
                        </div>
                      {/if}
                    </div>
                  </div>
                {/if}
              {/if}
            {/if}
          {/each}
        {/if}
        </div>
      </div>
      <form class="pt-2" on:submit|preventDefault={sendMessage}>
        <div class="input-group">
          <input aria-label="Message input" class="form-control chat-no-focus" placeholder="Type a message…" bind:value={input} on:keydown={(e)=>{ if(e.key==='Enter' && !e.shiftKey){ e.preventDefault(); sendMessage(); }}} />
          <button class="btn btn-theme" aria-label="Send message" disabled={sending || !input.trim()}>Send</button>
        </div>
      </form>
    {/if}
  </div>
  <div class="col-12 col-xl-4 d-flex flex-column h-100" style="min-height: 0;">
      <div class="p-0 h-100 d-flex flex-column" style="min-height: 300px;">
        <div class="d-flex align-items-center gap-2 mb-2 px-3 py-2 border rounded-2 bg-body fw-bold">
          <span>Content</span>
          <div class="ms-auto d-flex gap-2">
            {#if agent}
              {#if agent.is_published || agent.isPublished}
                <div class="dropdown">
                  <button class="btn btn-success btn-sm fw-bold dropdown-toggle published-toggle" type="button" data-bs-toggle="dropdown" aria-expanded="false" aria-label="Published options">
                    Published
                  </button>
                  <ul class="dropdown-menu dropdown-menu-end">
                    <li>
                      <a class="dropdown-item" href={`/content/${agent.name || name}/`} target="_blank" rel="noopener noreferrer">Open Public URL ↗</a>
                    </li>
                    <li>
                      <button class="dropdown-item" on:click={publishAgent}>Publish New Version</button>
                    </li>
                    <li>
                      <button class="dropdown-item text-danger" on:click={unpublishAgent}>Unpublish</button>
                    </li>
                  </ul>
                </div>
              {:else}
                <button class="btn btn-outline-primary btn-sm" on:click={publishAgent} aria-label="Publish content">Publish</button>
              {/if}
            {/if}
            {#if stateStr === 'idle' || stateStr === 'busy'}
              <button class="btn btn-outline-secondary btn-sm" on:click={refreshFrameUrl} aria-label="Refresh content">Refresh</button>
            {/if}
          </div>
        </div>
        <div class="flex-fill border rounded-2" style="min-height: 0;">
          <div class="h-100" style="overflow: auto; min-height: 0; height: 100%;">
          {#if stateStr === 'idle' || stateStr === 'busy'}
            {#if frameUrl && contentAvailable}
              <iframe src={frameUrl} title="Agent content" style="border:0; width:100%; height:100%; opacity: {frameOpacity}; transition: opacity 200ms ease;" on:load={() => { frameOpacity = 1; }}></iframe>
            {:else if contentProbing}
              <div class="d-flex align-items-center justify-content-center h-100 p-4">
                <div class="text-center text-body text-opacity-75">
                  <div class="spinner-border text-theme mb-3"></div>
                  <div>Checking for agent content…</div>
                </div>
              </div>
            {:else}
              <div class="d-flex align-items-center justify-content-center h-100 p-4">
                <div class="text-center">
                  <div class="h4 fw-bold mb-2">No Content</div>
                  <div class="text-body text-opacity-75 mb-3">This agent has no live page to display.</div>
                  {#if frameUrl}
                    <a class="btn btn-outline-theme btn-sm" href={frameUrl} target="_blank" rel="noopener">Open in new tab</a>
                  {/if}
                </div>
              </div>
            {/if}
          {:else if stateStr === 'slept'}
            <div class="d-flex align-items-center justify-content-center h-100 p-4">
              <div class="text-center">
                <div class="h4 fw-bold mb-2">Agent is Sleeping</div>
                <div class="text-body text-opacity-75 mb-3">Wake the agent to view live content.</div>
                <button class="btn btn-outline-success btn-sm" on:click={wakeAgent} aria-label="Wake agent">Wake</button>
              </div>
            </div>
          {:else}
            <div class="d-flex align-items-center justify-content-center h-100 p-4">
              <div class="text-center">
                <div class="h4 fw-bold mb-2">Agent Not Ready</div>
                <div class="text-body text-opacity-75">This agent is initializing or has been deleted.</div>
              </div>
            </div>
          {/if}
          </div>
        </div>
      </div>
  </div>

  <style>
    :global(pre.code-wrap) { white-space: pre-wrap; word-break: break-word; overflow-wrap: anywhere; }
    /* Restore default border/background for chat input */
    /* Remove focus border and shadow on the chat input to match template behavior */
    :global(.chat-no-focus:focus) {
      outline: 0 !important;
      box-shadow: none !important;
      border-color: var(--bs-border-color) !important; /* keep neutral border on focus */
    }
    :global(.markdown-body) { white-space: normal; }
    :global(.markdown-body p) { margin-bottom: 0.5rem; }
    :global(.markdown-body pre) {
      background: #0d1117; /* dark theme for fenced blocks */
      color: #e6edf3;
      padding: 0.5rem;
      border-radius: 0.25rem;
      overflow: auto;
    }
    :global(.markdown-body pre code) {
      background: transparent !important; /* avoid white inline code bg inside pre */
      color: inherit;
      padding: 0;
      border-radius: 0;
    }
    :global(.markdown-body code) {
      background: rgba(0,0,0,0.06);
      padding: 0.1rem 0.25rem;
      border-radius: 0.2rem;
    }
    :global(.markdown-body table) { width: 100%; border-collapse: collapse; margin: 0.5rem 0; }
    :global(.markdown-body th), :global(.markdown-body td) { border: 1px solid var(--bs-border-color); padding: 0.375rem 0.5rem; }
    :global(.markdown-body thead th) { background: var(--bs-light); }
    :global(.markdown-body ul) { padding-left: 1.25rem; }
    :global(.markdown-body li) { margin: 0.125rem 0; }
    :global(.markdown-wrap) { border: 1px solid var(--bs-border-color); border-radius: 0.5rem; padding: 0.5rem 0.75rem; background: var(--bs-body-bg); }
    /* Make the Published dropdown caret arrow a bit bigger */
    :global(.published-toggle.dropdown-toggle::after) {
      border-top-width: 0.5em;
      border-right-width: 0.5em;
      border-left-width: 0.5em;
      margin-left: 0.4rem;
    }
  </style>
</div>
