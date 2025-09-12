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
  import { getHostUrl } from '$lib/branding.js';
  import { auth } from '$lib/auth.js';
  import Card from '/src/components/bootstrap/Card.svelte';

  let md;
  try {
    md = new MarkdownIt({ html: false, linkify: true, breaks: true }).use(taskLists, { label: true, labelAfter: true });
    // Ensure all markdown links open in a new tab (chat panel)
    if (md && md.renderer && md.renderer.rules) {
      const defaultRender = md.renderer.rules.link_open || function(tokens, idx, options, env, self) {
        return self.renderToken(tokens, idx, options);
      };
      md.renderer.rules.link_open = function(tokens, idx, options, env, self) {
        // target="_blank"
        const tgtIdx = tokens[idx].attrIndex('target');
        if (tgtIdx < 0) tokens[idx].attrPush(['target', '_blank']);
        else tokens[idx].attrs[tgtIdx][1] = '_blank';

        // rel="noopener noreferrer" (merge with existing if present)
        const relRequired = 'noopener noreferrer';
        const relIdx = tokens[idx].attrIndex('rel');
        if (relIdx < 0) {
          tokens[idx].attrPush(['rel', relRequired]);
        } else {
          const current = tokens[idx].attrs[relIdx][1] || '';
          const set = new Set(current.split(/\s+/).filter(Boolean));
          relRequired.split(' ').forEach((v) => set.add(v));
          tokens[idx].attrs[relIdx][1] = Array.from(set).join(' ');
        }

        return defaultRender(tokens, idx, options, env, self);
      };
    }
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
  // Keep document title as just the agent name (no prefix)
  $: setPageTitle(name || 'Agent');

  let agent = null;
  let stateStr = '';
  let messages = [];
  let loading = true;
  let error = null;
  let input = '';
  let sending = false;
  let pollHandle = null;
  // Content preview via agent ports has been removed.

  function stateClass(state) {
    const s = String(state || '').toLowerCase();
    if (s === 'init') return 'badge rounded-pill bg-transparent border border-secondary text-secondary';
    if (s === 'idle') return 'badge rounded-pill bg-transparent border border-success text-success';
    if (s === 'busy') return 'badge rounded-pill bg-transparent border border-warning text-warning';
    return 'badge rounded-pill bg-transparent border border-secondary text-secondary';
  }

  function stateColorClass(state) {
    const s = String(state || '').toLowerCase();
    if (s === 'idle') return 'bg-success border-success';
    if (s === 'busy') return 'bg-warning border-warning';
    if (s === 'init') return 'bg-secondary border-secondary';
    return 'bg-secondary border-secondary';
  }

  function normState(v) { return String(v || '').trim().toLowerCase(); }
  $: stateStr = normState(agent?.state);
  $: isAdmin = $auth && String($auth.type || '').toLowerCase() === 'admin';

  function isSlept() { return stateStr === 'slept'; }
  function isAwake() { return stateStr === 'idle' || stateStr === 'busy'; }
  function isInitOrDeleted() { return stateStr === 'init'; }

  // Edit tags modal state and helpers
  let showTagsModal = false;
  let tagsInput = '';
  function openEditTags() {
    const current = Array.isArray(agent?.tags) ? agent.tags : [];
    tagsInput = current.join(', ');
    showTagsModal = true;
  }
  function closeEditTags() { showTagsModal = false; }
  function parseTagsInput() {
    const parts = tagsInput.split(',').map(s => s.trim()).filter(Boolean);
    const re = /^[A-Za-z0-9]+$/;
    for (const t of parts) {
      if (!re.test(t)) throw new Error(`Invalid tag '${t}'. Tags must be alphanumeric.`);
    }
    return parts;
  }
  async function saveTags() {
    try {
      const tags = parseTagsInput();
      const res = await apiFetch(`/agents/${encodeURIComponent(name)}`, { method: 'PUT', body: JSON.stringify({ tags }) });
      if (!res.ok) throw new Error(res?.data?.message || res?.data?.error || `Update failed (HTTP ${res.status})`);
      // Update local agent tags
      agent = res.data || agent;
      if (agent && !Array.isArray(agent.tags)) agent.tags = tags;
      showTagsModal = false;
    } catch (e) {
      alert(e.message || String(e));
    }
  }

  // Remix modal state and actions
  let showRemixModal = false;
  let remixName = '';
  function openRemixModal() {
    const cur = String(name || '').trim();
    remixName = nextRemixName(cur);
    showRemixModal = true;
  }
  function closeRemixModal() { showRemixModal = false; }
  let remixError = null;
  async function confirmRemix() {
    try {
      const newName = String(remixName || '').trim();
      const pattern = /^[a-z][a-z0-9-]{0,61}[a-z0-9]$/;
      if (!pattern.test(newName)) throw new Error('Invalid name. Use ^[a-z][a-z0-9-]{0,61}[a-z0-9]$');
      const res = await apiFetch(`/agents/${encodeURIComponent(name)}/remix`, {
        method: 'POST',
        body: JSON.stringify({ name: newName, code: true, secrets: true, content: true })
      });
      if (!res.ok) {
        remixError = res?.data?.message || res?.data?.error || `Remix failed (HTTP ${res.status})`;
        return;
      }
      showRemixModal = false;
      goto(`/agents/${encodeURIComponent(newName)}`);
    } catch (e) {
      remixError = e.message || String(e);
    }
  }

  // Delete modal state and actions
  let showDeleteModal = false;
  let deleteConfirm = '';
  function openDeleteModal() { deleteConfirm = ''; showDeleteModal = true; }
  function closeDeleteModal() { showDeleteModal = false; }
  $: canConfirmDelete = String(deleteConfirm || '').trim() === String(name || '').trim();

  async function fetchAgent() {
    const res = await apiFetch(`/agents/${encodeURIComponent(name)}`);
    if (res.ok && res.data) {
      agent = res.data;
    }
    // No content frame to compute; panel shows status only.
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

  // No content preview probing; only status is shown in the panel.

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

  // Normalize metadata to an object (handles string-serialized JSON)
  function metaOf(m) {
    try {
      const v = m?.metadata;
      if (!v) return null;
      if (typeof v === 'string') {
        try { return JSON.parse(v); } catch (_) { return null; }
      }
      return v;
    } catch (_) { return null; }
  }

  // Helper: detect a tool execution message
  function isToolExec(m) {
    try { const meta = metaOf(m); return !!(meta && meta.type === 'tool_execution' && meta.tool_type); } catch (_) { return false; }
  }

  // Helper: detect a tool result message
  function isToolResult(m) {
    try { const meta = metaOf(m); return !!(meta && meta.type === 'tool_result' && meta.tool_type); } catch (_) { return false; }
  }

  // Helper: for Text Editor description like "write /path/file" => { action, path }
  function parseTextEditorDesc(desc) {
    const s = String(desc || '').trim();
    if (!s) return { action: '', path: '' };
    const firstSpace = s.indexOf(' ');
    if (firstSpace === -1) return { action: s, path: '' };
    return { action: s.slice(0, firstSpace), path: s.slice(firstSpace + 1).trim() };
  }
  function fmtSeconds(v) {
    const n = Number(v || 0);
    if (!isFinite(n) || n <= 0) return '';
    if (n < 1) return `${n.toFixed(2)}s`;
    if (n < 10) return `${n.toFixed(1)}s`;
    return `${Math.round(n)}s`;
  }

  // Format seconds as human-readable hours/minutes/seconds, e.g., 1h 5m 3s, 5m, 30s
  function fmtDuration(v) {
    let total = Number(v || 0);
    if (!isFinite(total) || total < 0) total = 0;
    const h = Math.floor(total / 3600);
    total -= h * 3600;
    const m = Math.floor(total / 60);
    const s = Math.floor(total - m * 60);
    const parts = [];
    if (h) parts.push(`${h}h`);
    if (m) parts.push(`${m}m`);
    if (s || parts.length === 0) parts.push(`${s}s`);
    return parts.join(' ');
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
      if (!res.ok) throw new Error(res?.data?.message || res?.data?.error || `Send failed (HTTP ${res.status})`);
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
      if (!res.ok) throw new Error(res?.data?.message || res?.data?.error || `Sleep failed (HTTP ${res.status})`);
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
      if (!res.ok) throw new Error(res?.data?.message || res?.data?.error || `Wake failed (HTTP ${res.status})`);
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
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ code: true, secrets: true, content: true })
      });
      if (!res.ok) throw new Error(res?.data?.message || res?.data?.error || `Publish failed (HTTP ${res.status})`);
      if (agent) {
        agent = { ...(agent || {}), is_published: true, isPublished: true };
      }
      error = null;
    } catch (e) {
      error = e.message || String(e);
    }
  }

  async function unpublishAgent() {
    try {
      const res = await apiFetch(`/agents/${encodeURIComponent(name)}/unpublish`, { method: 'POST' });
      if (!res.ok) throw new Error(res?.data?.message || res?.data?.error || `Unpublish failed (HTTP ${res.status})`);
      if (agent) {
        agent = { ...(agent || {}), is_published: false, isPublished: false };
      }
      error = null;
    } catch (e) {
      error = e.message || String(e);
    }
  }

  // Remix action: open modal instead of prompt
  function remixAgent() { openRemixModal(); }

  // Delete action: open modal instead of prompt
  function deleteAgent() { openDeleteModal(); }

  // Generate a default remix name by adding or incrementing a numeric suffix
  function nextRemixName(cur) {
    const valid = /^[a-z][a-z0-9-]{0,61}[a-z0-9]$/;
    const basic = (s) => (s && typeof s === 'string') ? s.toLowerCase().replace(/[^a-z0-9-]/g, '-') : '';
    let s = basic(cur).replace(/^-+/, '').replace(/-+$/, '');
    if (!s) return 'new-agent-1';

    // If ends with -digits, increment, else add -1
    const m = s.match(/^(.*?)-(\d+)$/);
    let candidate;
    if (m) {
      const base = m[1];
      const num = parseInt(m[2] || '0', 10) + 1;
      candidate = `${base}-${num}`;
    } else {
      candidate = `${s}-1`;
    }

    // Enforce max length 63 by trimming base if needed
    if (candidate.length > 63) {
      const parts = candidate.split('-');
      const suffix = parts.pop();
      let base = parts.join('-');
      const keep = Math.max(1, 63 - 1 - String(suffix).length); // at least 1 char + '-' + suffix
      base = base.slice(0, keep).replace(/-+$/, '');
      candidate = `${base}-${suffix}`;
    }

    // Ensure it matches the platform constraints, else fallback
    if (!valid.test(candidate)) {
      // Pad start if needed and strip invalid ending
      candidate = candidate.replace(/^[^a-z]+/, 'a').replace(/[^a-z0-9]$/, '0');
      if (!valid.test(candidate)) {
        candidate = 'new-agent-1';
      }
    }
    return candidate;
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

<!-- Edit Tags Modal -->
{#if showTagsModal}
  <div class="modal fade show" style="display: block; background: rgba(0,0,0,.3);" tabindex="-1" role="dialog" aria-modal="true">
    <div class="modal-dialog">
      <div class="modal-content">
        <div class="modal-header">
          <h5 class="modal-title">Edit Tags</h5>
          <button type="button" class="btn-close" aria-label="Close" on:click={closeEditTags}></button>
        </div>
        <div class="modal-body">
          <label class="form-label" for="edit-tags">Tags (comma-separated)</label>
          <input id="edit-tags" class="form-control" bind:value={tagsInput} placeholder="e.g. Alpha,Internal,Beta" />
          <div class="form-text">Tags must be alphanumeric only; no spaces or symbols.</div>
        </div>
        <div class="modal-footer">
          <button class="btn btn-outline-secondary" on:click={closeEditTags}>Cancel</button>
          <button class="btn btn-theme" on:click={saveTags}>Save</button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Remix Modal -->
{#if showRemixModal}
  <div class="modal fade show" style="display: block; background: rgba(0,0,0,.3);" tabindex="-1" role="dialog" aria-modal="true">
    <div class="modal-dialog">
      <div class="modal-content">
        <div class="modal-header">
          <h5 class="modal-title">Remix Agent</h5>
          <button type="button" class="btn-close" aria-label="Close" on:click={closeRemixModal}></button>
        </div>
        <div class="modal-body">
          {#if remixError}
            <div class="alert alert-danger small">{remixError}</div>
          {/if}
          <label class="form-label" for="remix-name">New Agent Name</label>
          <input
            id="remix-name"
            class="form-control"
            bind:value={remixName}
            on:keydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); confirmRemix(); } }}
          />
          <div class="form-text">Pattern: ^[a-z][a-z0-9-]{0,61}[a-z0-9]$</div>
        </div>
        <div class="modal-footer">
          <button class="btn btn-outline-secondary" on:click={closeRemixModal}>Cancel</button>
          <button class="btn btn-theme" on:click={confirmRemix}>Remix</button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Delete Modal -->
{#if showDeleteModal}
  <div class="modal fade show" style="display: block; background: rgba(0,0,0,.3);" tabindex="-1" role="dialog" aria-modal="true">
    <div class="modal-dialog">
      <div class="modal-content">
        <div class="modal-header">
          <h5 class="modal-title">Delete Agent</h5>
          <button type="button" class="btn-close" aria-label="Close" on:click={closeDeleteModal}></button>
        </div>
        <div class="modal-body">
          <p class="mb-2">Type <span class="fw-bold">{name}</span> to confirm permanent deletion.</p>
          <input class="form-control" bind:value={deleteConfirm} placeholder={name} />
        </div>
        <div class="modal-footer">
          <button class="btn btn-outline-secondary" on:click={closeDeleteModal}>Cancel</button>
          <button class="btn btn-danger" disabled={!canConfirmDelete} on:click={async () => {
            try {
              const cur = String(name || '').trim();
              const res = await apiFetch(`/agents/${encodeURIComponent(cur)}`, { method: 'DELETE' });
              if (!res.ok) throw new Error(res?.data?.message || res?.data?.error || `Delete failed (HTTP ${res.status})`);
              showDeleteModal = false;
              goto('/agents');
            } catch (e) {
              alert(e.message || String(e));
            }
          }}>Delete</button>
        </div>
      </div>
    </div>
  </div>
{/if}

<div class="row g-3 h-100">
  <div class="col-12 d-flex flex-column h-100" style="min-height: 0;">
    <!-- Header: Agent details on the left, info box on the right -->
    <div class="row g-3 mb-2">
      <div class="col-12 col-lg-7">
        <Card class="h-100">
          <div class="card-body d-flex flex-column">
            <div class="d-flex align-items-center gap-2 mb-1">
              {#if agent}
                <a class="fw-bold text-decoration-none" href={'/agents/' + encodeURIComponent(agent.name || '')}>{agent.name || '-'}</a>
              {:else}
                <div class="fw-bold">{name}</div>
              {/if}
            </div>
            <div class="small text-body text-opacity-75 flex-grow-1">{agent?.description || agent?.desc || 'No description'}</div>
            {#if isAdmin && agent}
              <div class="small text-body-secondary mt-1">Owner: <span class="font-monospace">{agent.created_by}</span></div>
            {/if}
            {#if Array.isArray(agent?.tags) && agent.tags.length}
              <div class="mt-2 d-flex flex-wrap gap-1">
                {#each agent.tags as t}
                  <span class="badge bg-secondary-subtle text-secondary-emphasis border">{t}</span>
                {/each}
              </div>
            {/if}
            <!-- In-card actions (publish, remix, sleep/wake, kebab) -->
            <div class="mt-2 d-flex align-items-center flex-wrap">
              <!-- Compact status indicator on the left -->
              <div class="d-flex align-items-center gap-2">
                {#if agent}
                  <span class={`d-inline-block rounded-circle ${stateColorClass(agent.state || agent.status)} border`} style="width: 10px; height: 10px;"></span>
                  <span class="text-uppercase small fw-bold text-body">{agent.state || agent.status || 'unknown'}</span>
                {/if}
              </div>
              <!-- Actions on the right (tight group) -->
              <div class="ms-auto d-flex align-items-center flex-wrap gap-2">
                {#if stateStr === 'slept'}
                  <button class="btn btn-outline-success btn-sm" on:click={wakeAgent} aria-label="Wake agent">Wake</button>
                {:else if stateStr === 'idle' || stateStr === 'busy'}
                  <button class="btn btn-outline-warning btn-sm" on:click={sleepAgent} aria-label="Put agent to sleep">Sleep</button>
                {/if}
                {#if agent}
                  {#if agent.is_published || agent.isPublished}
                    <div class="dropdown">
                      <button class="btn btn-success btn-sm fw-bold dropdown-toggle published-toggle" type="button" data-bs-toggle="dropdown" aria-expanded="false" aria-label="Published options">
                        Published
                      </button>
                      <ul class="dropdown-menu dropdown-menu-end">
                        <li>
                          <a class="dropdown-item" href={`${getHostUrl()}/content/${agent?.name || name}/`} target="_blank" rel="noopener noreferrer">Open Public URL ↗</a>
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
                <div class="dropdown">
                  <button class="btn btn-outline-secondary btn-sm" type="button" data-bs-toggle="dropdown" aria-expanded="false" aria-label="More actions">
                    <i class="bi bi-three-dots"></i>
                  </button>
                  <ul class="dropdown-menu dropdown-menu-end">
                    <li><button class="dropdown-item" on:click={remixAgent}>Remix</button></li>
                    <li><button class="dropdown-item" on:click={openEditTags}>Edit Tags</button></li>
                    <li><hr class="dropdown-divider" /></li>
                    <li><button class="dropdown-item text-danger" on:click={deleteAgent}>Delete</button></li>
                  </ul>
                </div>
              </div>
            </div>
          </div>
        </Card>
      </div>
      <div class="col-12 col-lg-5 d-none d-lg-block">
        {#if agent}
          <Card class="h-100">
            <div class="card-body small">
              <div>Last Activity: <span class="font-monospace">{agent.last_activity_at || '-'}</span></div>
              <div class="mt-1">Idle Timeout: {fmtDuration(agent.idle_timeout_seconds)}</div>
              <div class="mt-1">Busy Timeout: {fmtDuration(agent.busy_timeout_seconds)}</div>
              <div class="mt-2">
                Public URL:
                {#if agent.is_published || agent.isPublished}
                  <a href={`${getHostUrl()}/content/${agent?.name || name}/`} target="_blank" rel="noopener noreferrer">{getHostUrl()}/content/{agent?.name || name}/</a>
                {:else}
                  Not Published
                {/if}
              </div>
            </div>
          </Card>
        {/if}
      </div>
    </div>

    <!-- Minimize/Maximize (expand/collapse) tool details row -->
    <div class="d-flex align-items-center justify-content-end flex-wrap gap-2 mb-2">
      <div class="small text-body me-2">
        Total messages <span class="d-none d-sm-inline">(includes tool calls)</span>: {Array.isArray(messages) ? messages.length : 0}
      </div>
      <div class="d-flex align-items-center gap-2">
        <button class="btn btn-outline-secondary btn-sm" on:click={expandAllTools} aria-label="Expand all tool details" title="Expand all"><i class="bi bi-chevrons-expand"></i></button>
        <button class="btn btn-outline-secondary btn-sm" on:click={collapseAllTools} aria-label="Collapse all tool details" title="Collapse all"><i class="bi bi-chevrons-collapse"></i></button>
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
                      {toolLabel(metaOf(m)?.tool_type)} Request {argsPreview(m)}
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
                        {toolLabel(metaOf(m)?.tool_type)} Response {argsPreview(m)}
                      </summary>
                      <pre class="small bg-dark text-white p-2 rounded mb-0 code-wrap"><code>{JSON.stringify({ tool: m?.metadata?.tool_type || 'tool', args: (m?.metadata?.args ?? null), output: m.content }, null, 2)}</code></pre>
                    </details>
                  </div>
                {:else}
                  <div class="d-flex mb-3 justify-content-start">
                    <div class="text-body" style="max-width: 80%; word-break: break-word;">
                      {#if metaOf(m)?.thinking}
                        <details class="mt-1 mb-2">
                          <summary class="small text-body text-opacity-75" style="cursor: pointer;">
                            Thought {#if metaOf(m)?.thinking_seconds}for {fmtSeconds(metaOf(m)?.thinking_seconds)}{/if}
                          </summary>
                          <div class="small fst-italic text-body text-opacity-50" style="white-space: pre-wrap;">{metaOf(m)?.thinking}</div>
                        </details>
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
          <textarea
            aria-label="Message input"
            class="form-control chat-no-focus"
            placeholder="Type a message…"
            rows="2"
            style="resize: none;"
            bind:value={input}
            on:keydown={(e)=>{ if(e.key==='Enter' && !e.shiftKey){ e.preventDefault(); sendMessage(); } }}
            on:input={(e)=>{ try { e.target.style.height='auto'; e.target.style.height = Math.min(e.target.scrollHeight, 200) + 'px'; } catch(_){} }}
          ></textarea>
          <button class="btn btn-theme" aria-label="Send message" disabled={sending || !input.trim()}>Send</button>
        </div>
      </form>
    {/if}
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
