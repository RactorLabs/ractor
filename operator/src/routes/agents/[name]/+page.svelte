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
  // Chat rendering derived from Responses
  let chat = [];
  // Toggle display of thinking (analysis/commentary) text; persisted via cookie
  let showThinking = false;
  const SHOW_THINKING_COOKIE = 'raworc_showThinking';
  let thinkingPrefLoaded = false;
  // Toggle display of tool calls/results; persisted via cookie
  let showTools = true;
  const SHOW_TOOLS_COOKIE = 'raworc_showTools';
  let toolsPrefLoaded = false;
  function getCookie(name) {
    try {
      const value = `; ${document.cookie}`;
      const parts = value.split(`; ${name}=`);
      if (parts.length === 2) return decodeURIComponent(parts.pop().split(';').shift());
    } catch (_) {}
    return null;
  }
  function setCookie(name, value, days) {
    try {
      const d = new Date();
      d.setTime(d.getTime() + (days * 24 * 60 * 60 * 1000));
      const expires = `; expires=${d.toUTCString()}`;
      const secure = (typeof location !== 'undefined' && location.protocol === 'https:') ? '; Secure' : '';
      document.cookie = `${name}=${encodeURIComponent(value || '')}${expires}; path=/; SameSite=Lax${secure}`;
    } catch (_) {}
  }
  onMount(() => {
    try {
      if (browser) {
        const v = getCookie(SHOW_THINKING_COOKIE);
        if (v !== null) showThinking = v === '1' || v === 'true';
        const t = getCookie(SHOW_TOOLS_COOKIE);
        if (t !== null) showTools = t === '1' || t === 'true';
      }
    } catch (_) {}
    thinkingPrefLoaded = true;
    toolsPrefLoaded = true;
  });
  onMount(async () => {
    try {
      await fetchContextUsage();
    } catch (_) {}
  });
  $: if (browser && thinkingPrefLoaded) {
    setCookie(SHOW_THINKING_COOKIE, showThinking ? '1' : '0', 365);
  }
  $: if (browser && toolsPrefLoaded) {
    setCookie(SHOW_TOOLS_COOKIE, showTools ? '1' : '0', 365);
  }
  let loading = true;
  let error = null;
  let input = '';
  let sending = false;
  let pollHandle = null;
  let runtimeSeconds = 0;
  let currentSessionSeconds = 0;

  // Context usage state
  let ctx = null; // raw response { soft_limit_tokens, used_tokens_estimated, used_percent, cutoff_at, measured_at }
  let ctxLoading = false;
  let contextFull = false;
  // When compacting context, block UI interactions
  let isCompacting = false;
  function fmtInt(n) {
    try { const v = Number(n); return Number.isFinite(v) ? v.toLocaleString() : String(n); } catch (_) { return String(n); }
  }
  function fmtPct(n) {
    try { const v = Number(n); return Number.isFinite(v) ? `${v.toFixed(1)}%` : '-'; } catch (_) { return '-'; }
  }
  async function fetchContextUsage() {
    try {
      ctxLoading = true;
      const res = await apiFetch(`/agents/${encodeURIComponent(name)}/context`);
      if (res.ok) {
        ctx = res.data || null;
        // Update banner flag if over soft limit
        const used = Number(ctx?.used_tokens_estimated || 0);
        const limit = Number(ctx?.soft_limit_tokens || 100000);
        contextFull = used >= limit;
      }
    } catch (_) { /* ignore */ }
    ctxLoading = false;
  }
  async function clearContext() {
    try {
      const res = await apiFetch(`/agents/${encodeURIComponent(name)}/context/clear`, { method: 'POST' });
      if (!res.ok) throw new Error(res?.data?.message || res?.data?.error || `Clear failed (HTTP ${res.status})`);
      error = null;
      contextFull = false;
      await fetchContextUsage();
      await fetchResponses();
      await tick();
      scrollToBottom();
    } catch (e) {
      error = e.message || String(e);
    }
  }
  async function compactContext() {
    try {
      isCompacting = true;
      ctxLoading = true;
      const res = await apiFetch(`/agents/${encodeURIComponent(name)}/context/compact`, { method: 'POST' });
      if (!res.ok) throw new Error(res?.data?.message || res?.data?.error || `Compact failed (HTTP ${res.status})`);
      error = null;
      contextFull = false;
      await fetchContextUsage();
      await fetchResponses();
      await tick();
      scrollToBottom();
    } catch (e) {
      error = e.message || String(e);
    } finally {
      isCompacting = false;
      ctxLoading = false;
    }
  }
  let _runtimeFetchedAt = 0;
  let inputEl = null; // chat textarea element
  // Content preview via agent ports has been removed.
  // Details (analysis + tool calls/results) visibility controlled via toggles

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

  function stateIconClass(state) {
    const s = String(state || '').toLowerCase();
    if (s === 'slept') return 'fas fa-moon';
    if (s === 'idle') return 'fas fa-sun';
    if (s === 'busy') return 'fas fa-circle-notch fa-spin';
    if (s === 'init') return 'fas fa-spinner fa-spin';
    return 'fas fa-circle-dot';
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
    const parts = tagsInput.split(',').map(s => s.trim().toLowerCase()).filter(Boolean);
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

  // Edit timeouts modal state and helpers
  let showTimeoutsModal = false;
  let idleTimeoutInput = 0;
  let busyTimeoutInput = 0;
  function openEditTimeouts() {
    const idle = Number(agent?.idle_timeout_seconds ?? 0);
    const busy = Number(agent?.busy_timeout_seconds ?? 0);
    idleTimeoutInput = Number.isFinite(idle) && idle >= 0 ? idle : 0;
    busyTimeoutInput = Number.isFinite(busy) && busy >= 0 ? busy : 0;
    showTimeoutsModal = true;
  }
  function closeEditTimeouts() { showTimeoutsModal = false; }
  async function saveTimeouts() {
    try {
      const idle = Math.max(0, Math.floor(Number(idleTimeoutInput || 0)));
      const busy = Math.max(0, Math.floor(Number(busyTimeoutInput || 0)));
      const body = { idle_timeout_seconds: idle, busy_timeout_seconds: busy };
      const res = await apiFetch(`/agents/${encodeURIComponent(name)}`, { method: 'PUT', body: JSON.stringify(body) });
      if (!res.ok) throw new Error(res?.data?.message || res?.data?.error || `Update failed (HTTP ${res.status})`);
      // Update local agent snapshot
      agent = res.data || agent;
      if (agent) {
        agent.idle_timeout_seconds = idle;
        agent.busy_timeout_seconds = busy;
      }
      showTimeoutsModal = false;
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

  // Sleep modal state and actions
  let showSleepModal = false;
  let sleepDelayInput = 5;
  let sleepNoteInput = '';
  function openSleepModal() {
    sleepDelayInput = 5;
    sleepNoteInput = '';
    showSleepModal = true;
  }
  function closeSleepModal() { showSleepModal = false; }
  async function confirmSleep() {
    const d = Math.max(5, Math.floor(Number(sleepDelayInput || 5)));
    showSleepModal = false;
    await sleepAgent(d, sleepNoteInput);
  }

  async function fetchAgent() {
    const res = await apiFetch(`/agents/${encodeURIComponent(name)}`);
    if (res.ok && res.data) {
      agent = res.data;
    }
    // No content frame to compute; panel shows status only.
  }

  async function fetchRuntime(force = false) {
    try {
      if (!force && Date.now() - _runtimeFetchedAt < 10000) return; // throttle to 10s
      const res = await apiFetch(`/agents/${encodeURIComponent(name)}/runtime`);
      if (res.ok) {
        const v = Number(res?.data?.total_runtime_seconds ?? 0);
        if (Number.isFinite(v) && v >= 0) runtimeSeconds = v;
        const cs = Number(res?.data?.current_session_seconds ?? 0);
        currentSessionSeconds = Number.isFinite(cs) && cs >= 0 ? cs : 0;
        _runtimeFetchedAt = Date.now();
      }
    } catch (_) {}
  }

  async function fetchResponses() {
    const res = await apiFetch(`/agents/${encodeURIComponent(name)}/responses?limit=200`);
    if (res.ok) {
      const list = Array.isArray(res.data) ? res.data : (res.data?.responses || []);
      // Only auto-stick if near bottom before refresh
      let shouldStick = true;
      try {
        const el = typeof document !== 'undefined' ? document.getElementById('chat-body') : null;
        if (el) {
          const delta = el.scrollHeight - el.scrollTop - el.clientHeight;
          shouldStick = delta < 80;
        }
      } catch (_) {}
      // Transform responses into synthetic chat bubbles to reuse rendering
      const transformed = [];
      for (const r of list) {
        const inputText = r?.input?.text || '';
        const inputContent = Array.isArray(r?.input_content) ? r.input_content : null;
        if (inputContent && inputContent.length > 0) {
          // Render first text item as user bubble; future: support richer inputs
          const firstText = inputContent.find((it) => String(it?.type || '').toLowerCase() === 'text' && typeof it?.content === 'string' && it.content.trim());
          if (firstText) {
            transformed.push({ role: 'user', content: firstText.content, id: r.id + ':in' });
          } else if (inputText && inputText.trim()) {
            transformed.push({ role: 'user', content: inputText, id: r.id + ':in' });
          }
        } else if (inputText && inputText.trim()) {
          transformed.push({ role: 'user', content: inputText, id: r.id + ':in' });
        }
        const items = Array.isArray(r?.segments) ? r.segments : [];
        const contentText = '';
        const meta = { type: 'composite_step', in_progress: String(r?.status || '').toLowerCase() === 'processing' };
        const outputContent = Array.isArray(r?.output_content) ? r.output_content : [];
        transformed.push({
          role: 'agent',
          id: r.id + ':out',
          content: contentText,
          metadata: meta,
          content_json: { composite: { segments: items }, output_content: outputContent },
        });
      }
      chat = transformed;
      await tick();
      if (shouldStick) scrollToBottom();
    }
  }

  function startPolling() {
    stopPolling();
    pollHandle = setInterval(async () => {
      await fetchResponses();
      await fetchContextUsage();
      await fetchAgent();
      await fetchRuntime();
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
    // Show the tool name exactly as it appears
    return String(t ?? '');
  }

  // Composite helpers (content_json.composite.segments)
  function hasComposite(m) {
    try { return Array.isArray(m?.content_json?.composite?.segments) && m.content_json.composite.segments.length > 0; } catch (_) { return false; }
  }
  function segmentsOf(m) { try { return Array.isArray(m?.content_json?.composite?.segments) ? m.content_json.composite.segments : []; } catch (_) { return []; } }
  function segType(s) { return String(s?.type || '').toLowerCase(); }
  function segChannel(s) { return String(s?.channel || '').toLowerCase(); }
  function segText(s) { return String(s?.text || ''); }
  function segContent(s) { return String(s?.content || ''); }
  function segTool(s) { return String(s?.tool || ''); }
  function segArgs(s) { try { return (s && typeof s.args === 'object') ? s.args : null; } catch(_) { return null; } }
  function segOutput(s) {
    try {
      const o = s?.output;
      if (typeof o === 'string') { return o; }
      return o; // object/array/null
    } catch (_) { return s?.output; }
  }

  // Output_* helpers
  function isOutputToolName(t) {
    try { const n = String(t || '').toLowerCase(); return n === 'output' || n === 'output_markdown' || n === 'ouput_json' || n === 'output_json'; } catch (_) { return false; }
  }
  function isOutputSeg(s) { try { return segType(s) === 'tool_result' && isOutputToolName(segTool(s)); } catch (_) { return false; } }
  function outputMarkdownOfSeg(s) {
    try {
      const out = segOutput(s);
      if (out && typeof out === 'object' && typeof out.content === 'string') return out.content;
      if (typeof out === 'string') return out; // fallback if tool returned string
      return '';
    } catch (_) { return ''; }
  }
  function outputItemsOfSeg(s) {
    try {
      const out = segOutput(s);
      if (out && typeof out === 'object' && Array.isArray(out.items)) return out.items;
      return [];
    } catch (_) { return []; }
  }
  function typeBadge(t) {
    try { const n = String(t || '').toLowerCase();
      if (n === 'markdown') return 'Markdown';
      if (n === 'json') return 'JSON';
      if (n === 'url') return 'URL';
      return n;
    } catch (_) { return String(t || ''); }
  }

  // Parse helpers for rare top-level tool_result card content
  function parseJsonSafe(s) {
    try { return JSON.parse(String(s ?? '')); } catch (_) { return null; }
  }
  function outputMarkdownFromTopCard(m) {
    try {
      const p = parseJsonSafe(m?.content);
      if (p && typeof p === 'object' && typeof p.content === 'string') return p.content;
      return '';
    } catch (_) { return ''; }
  }
  function outputJsonFromTopCard(m) {
    try {
      const p = parseJsonSafe(m?.content);
      if (p && typeof p === 'object' && Object.prototype.hasOwnProperty.call(p, 'data')) return p.data;
      return undefined;
    } catch (_) { return undefined; }
  }
  function parsedItemsFromTopCard(m) {
    try {
      const p = parseJsonSafe(m?.content);
      if (p && Array.isArray(p.items)) return p.items;
      return [];
    } catch (_) { return []; }
  }

  // Summary-mode: show tool output exactly as-is (no parsing or wrapping)
  function summaryOutputText(s) {
    try {
      const o = s?.output;
      if (o == null) return '';
      if (typeof o === 'string') return o;
      try { return JSON.stringify(o); } catch (_) { return String(o); }
    } catch (_) { return ''; }
  }
  function segNote(s) {
    try { return String(s?.note || '').trim(); } catch (_) { return ''; }
  }
  function hasSleptSeg(m) {
    try { return segmentsOf(m).some((x) => segType(x) === 'slept'); } catch(_) { return false; }
  }
  function sleptNoteFrom(m) {
    try {
      const s = segmentsOf(m).find((x) => segType(x) === 'slept');
      return s ? segNote(s) : '';
    } catch(_) { return ''; }
  }
  function sleptRuntimeFrom(m) {
    try {
      const s = segmentsOf(m).find((x) => segType(x) === 'slept');
      const v = s && s.runtime_seconds != null ? Number(s.runtime_seconds) : NaN;
      return Number.isFinite(v) && v >= 0 ? v : 0;
    } catch(_) { return 0; }
  }
  function hasWokeSeg(m) {
    try { return segmentsOf(m).some((x) => segType(x) === 'woke'); } catch(_) { return false; }
  }
  function wokeNoteFrom(m) {
    try {
      const s = segmentsOf(m).find((x) => segType(x) === 'woke');
      return s ? segNote(s) : '';
    } catch(_) { return ''; }
  }
  function hasCancelledSeg(m) {
    try { return segmentsOf(m).some((x) => segType(x) === 'cancelled'); } catch(_) { return false; }
  }
  function cancelledReasonFrom(m) {
    try {
      const s = segmentsOf(m).find((x) => segType(x) === 'cancelled');
      return s ? String(s?.reason || '').trim() : '';
    } catch(_) { return ''; }
  }
  function dateOnly(s) {
    try {
      const d = new Date(String(s || ''));
      if (isNaN(d)) return '';
      return d.toISOString().slice(0, 10); // YYYY-MM-DD
    } catch (_) { return ''; }
  }
  function hasContextClearedSeg(m) {
    try { return segmentsOf(m).some((x) => segType(x) === 'context_cleared'); } catch(_) { return false; }
  }
  function contextClearedAt(m) {
    try {
      const s = segmentsOf(m).find((x) => segType(x) === 'context_cleared');
      const raw = String(s?.cutoff_at || '').trim();
      return dateOnly(raw);
    } catch(_) { return ''; }
  }
  function hasContextCompactedSeg(m) {
    try { return segmentsOf(m).some((x) => segType(x) === 'context_compacted'); } catch(_) { return false; }
  }
  function hasFinalSeg(m) {
    try { return segmentsOf(m).some((x) => segType(x) === 'final'); } catch(_) { return false; }
  }
  function segToolTitle(s) {
    try {
      const t = segTool(s).toLowerCase();
      const a = segArgs(s) || {};
      const shortPath = (p) => { try { const s = String(p || '').trim(); return s.startsWith('/agent/') ? s.slice(7) : s; } catch(_) { return ''; } };
      if (t === 'run_bash') {
        const cmd = a.commands || a.command || a.cmd || '';
        const cwd = a.exec_dir || a.cwd || a.workdir || '';
        const title = [cmd ? truncate(String(cmd), 80) : '', cwd ? shortPath(cwd) : ''].filter(Boolean).join(' • ');
        return title || '(run_bash)';
      }
      if (t === 'open_file') {
        const p = a.path || '';
        const sl = a.start_line != null ? Number(a.start_line) : null;
        const el = a.end_line != null ? Number(a.end_line) : null;
        const range = (sl || el) ? ` [${sl || ''}${el ? ':' + el : ''}]` : '';
        return `${shortPath(p)}${range}` || '(open_file)';
      }
      if (t === 'create_file') {
        const p = a.path || '';
        const bytes = (a.content && typeof a.content === 'string') ? a.content.length : null;
        const meta = bytes != null ? ` (${bytes} bytes)` : '';
        return `${shortPath(p)}${meta}` || '(create_file)';
      }
      if (t === 'str_replace') {
        const p = a.path || '';
        const oldStr = typeof a.old_str === 'string' ? truncate(a.old_str, 30) : '';
        const newStr = typeof a.new_str === 'string' ? truncate(a.new_str, 30) : '';
        const many = a.many ? ' (all)' : '';
        const pair = (oldStr || newStr) ? ` ${oldStr} → ${newStr}` : '';
        return `${shortPath(p)}${pair}${many}` || '(str_replace)';
      }
      if (t === 'insert') {
        const p = a.path || '';
        const line = a.insert_line != null ? `:${a.insert_line}` : '';
        const len = (a.content && typeof a.content === 'string') ? a.content.length : null;
        const meta = len != null ? ` (+${len})` : '';
        return `${shortPath(p)}${line}${meta}` || '(insert)';
      }
      if (t === 'remove_str') {
        const p = a.path || '';
        const len = (a.content && typeof a.content === 'string') ? a.content.length : null;
        const many = a.many ? ' (all)' : '';
        const meta = len != null ? ` (-${len})` : '';
        return `${shortPath(p)}${meta}${many}` || '(remove_str)';
      }
      if (t === 'find_filecontent') {
        const p = a.path || '';
        const rgx = typeof a.regex === 'string' ? ` /${truncate(a.regex, 40)}/` : '';
        return `${shortPath(p)}${rgx}` || '(find_filecontent)';
      }
      if (t === 'find_filename') {
        const p = a.path || '';
        const glob = typeof a.glob === 'string' ? ` ${truncate(a.glob, 40)}` : '';
        return `${shortPath(p)}${glob}` || '(find_filename)';
      }
      if (t === 'publish_agent') {
        const note = typeof a.note === 'string' && a.note.trim() ? ` (${truncate(a.note, 50)})` : '';
        return `publish${note}`;
      }
      if (t === 'sleep_agent') {
        const d = a.delay_seconds != null ? Number(a.delay_seconds) : null;
        const note = typeof a.note === 'string' && a.note.trim() ? ` (${truncate(a.note, 50)})` : '';
        return `sleep${d ? ` in ${d}s` : ''}${note}`;
      }
      if (t === 'create_plan') {
        const title = typeof a.title === 'string' ? a.title : '';
        const n = Array.isArray(a.tasks) ? a.tasks.length : 0;
        const label = title ? ` ${truncate(title, 50)}` : '';
        return `create_plan${label}${n ? ` (${n} tasks)` : ''}`;
      }
      if (t === 'add_task') {
        const task = typeof a.task === 'string' ? ` ${truncate(a.task, 60)}` : '';
        return `add_task${task}`;
      }
      if (t === 'complete_task') {
        const id = a.task_id != null ? ` #${a.task_id}` : '';
        const vp = Array.isArray(a.verify_paths) ? ` (verify ${a.verify_paths.length})` : '';
        const vu = typeof a.verify_url === 'string' && a.verify_url ? ' (verify url)' : '';
        const force = a.force ? ' [force]' : '';
        return `complete${id}${vp || vu ? `${vp}${vu}` : ''}${force}`;
      }
      if (t === 'clear_plan') {
        return 'clear_plan';
      }
      const json = JSON.stringify(a);
      return json && json.length > 80 ? json.slice(0, 77) + '…' : (json || '(args)');
    } catch (_) { return ''; }
  }

  // In-progress helpers
  function isInProgress(m) {
    try { const meta = metaOf(m); return !!(meta && meta.in_progress === true); } catch (_) { return false; }
  }
  function truncate(s, max = 80) {
    try {
      const str = String(s || '').trim();
      if (!str) return '';
      return str.length > max ? str.slice(0, max - 1) + '…' : str;
    } catch (_) { return ''; }
  }
  // Multiline truncation for large tool outputs
  function truncateMultiline(s, maxChars = 1200, maxLines = 24) {
    try {
      const str = String(s || '');
      if (!str) return '';
      let truncated = false;
      let lines = str.split('\n');
      if (lines.length > maxLines) { lines = lines.slice(0, maxLines); truncated = true; }
      let text = lines.join('\n');
      if (text.length > maxChars) { text = text.slice(0, maxChars); truncated = true; }
      return truncated ? (text + '\n… truncated') : text;
    } catch (_) { return String(s || ''); }
  }

  // Track which tool-result segments are expanded (full output) and which <details> are open
  let expandedSegments = new Set();
  let openedSegments = new Set();
  let expandAll = false; // when true, force-open all <details> blocks and full segment views
  function segKey(m, j) { try { return `${m?.id || ''}:${j}`; } catch (_) { return `${j}`; } }
  function expandSeg(key) {
    try {
      const s = new Set(expandedSegments); s.add(key); expandedSegments = s;
      if (!expandAll) { const o = new Set(openedSegments); o.add(key); openedSegments = o; }
    } catch (_) {}
  }
  function collapseSeg(key) {
    try {
      const s = new Set(expandedSegments); s.delete(key); expandedSegments = s;
      if (!expandAll) { const o = new Set(openedSegments); o.delete(key); openedSegments = o; }
    } catch (_) {}
  }
  function onToggleDetails(key, isOpen) {
    try {
      const o = new Set(openedSegments);
      if (isOpen) { o.add(key); } else { o.delete(key); }
      openedSegments = o;
    } catch (_) {}
  }
  // In-progress preview and label removed; always render full details in the feed.

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

  // (old text_editor helper removed)
  function fmtSeconds(v) {
    const n = Number(v || 0);
    if (!isFinite(n) || n <= 0) return '';
    if (n < 1) return `${n.toFixed(2)}s`;
    if (n < 10) return `${n.toFixed(1)}s`;
    return `${Math.round(n)}s`;
  }

  // Expand/Collapse all details across visible composite segments
  function allSegmentKeys() {
    try {
      const keys = [];
      for (const m of (chat || [])) {
        if (!hasComposite(m)) continue;
        const segs = segmentsOf(m);
        for (let j = 0; j < segs.length; j++) {
          keys.push(segKey(m, j));
        }
      }
      return keys;
    } catch (_) { return []; }
  }
  function expandAllDetails() {
    try {
      const s = new Set(expandedSegments);
      for (const k of allSegmentKeys()) s.add(k);
      expandedSegments = s;
      expandAll = true;
    } catch (_) {}
  }
  function collapseAllDetails() {
    try {
      expandedSegments = new Set();
      openedSegments = new Set();
      expandAll = false;
    } catch (_) {}
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
  // Expand/Collapse controls removed; Show Details toggle controls visibility.

  // Helper: compact args preview for tool summaries
  function argsPreview(m) {
    try {
      const t = String(m?.metadata?.tool_type || '').toLowerCase();
      const a = m?.metadata?.args;
      if (!a || typeof a !== 'object') return '';
      if (t === 'run_bash') {
        const cmd = a.commands || a.command || a.cmd || '';
        const cwd = a.exec_dir || a.cwd || a.workdir || '';
        const parts = [];
        if (cmd) parts.push(String(cmd).trim().slice(0, 80));
        if (cwd) parts.push(String(cwd).trim());
        if (!parts.length) return '';
        return `(${parts.join(' • ')})`;
      }
      const json = JSON.stringify(a);
      if (!json) return '';
      const short = json.length > 80 ? json.slice(0, 77) + '…' : json;
      return `(${short})`;
    } catch (_) { return ''; }
  }

  async function sendMessage(e) {
    e?.preventDefault?.();
    if (isCompacting) { return; }
    const content = (input || '').trim();
    if (!content || sending || stateStr === 'busy') { if (stateStr === 'busy') { error = 'Agent is busy'; } return; }
    sending = true;
    try {
      const res = await apiFetch(`/agents/${encodeURIComponent(name)}/responses`, {
        method: 'POST',
        body: JSON.stringify({ input: { content: [{ type: 'text', content }] } })
      });
      if (!res.ok) {
        if (res.status === 409) {
          // Soft limit reached; show friendly banner and fetch latest usage
          contextFull = true;
          await fetchContextUsage();
        }
        throw new Error(res?.data?.message || res?.data?.error || `Send failed (HTTP ${res.status})`);
      }
      input = '';
      // Reset textarea height back to default (2 rows) after clearing
      await tick();
      try { if (inputEl) { inputEl.style.height = ''; } } catch (_) {}
      // Wait until the server-side response row appears, then update UI
      const rid = res?.data?.id;
      const deadline = Date.now() + 10000; // up to 10s
      while (Date.now() < deadline) {
        await fetchResponses();
        const expectIn = rid ? `${rid}:in` : null;
        if (!rid || (chat && chat.some(m => m.id === expectIn))) {
          break;
        }
        await new Promise(r => setTimeout(r, 200));
      }
      await tick();
      scrollToBottom();
    } catch (e) {
      error = e.message || String(e);
      sending = false;
      return;
    }
    sending = false;
  }

  async function sleepAgent(delaySeconds = 5, note = '') {
    try {
      const body = { delay_seconds: delaySeconds };
      const t = String(note || '').trim();
      if (t) body['note'] = t;
      const res = await apiFetch(`/agents/${encodeURIComponent(name)}/sleep`, { method: 'POST', body: JSON.stringify(body) });
      if (!res.ok) throw new Error(res?.data?.message || res?.data?.error || `Sleep failed (HTTP ${res.status})`);
      // Do not optimistically flip state; let polling update when controller sleeps it
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

  async function cancelActive() {
    try {
      const res = await apiFetch(`/agents/${encodeURIComponent(name)}/cancel`, { method: 'POST' });
      if (!res.ok) throw new Error(res?.data?.message || res?.data?.error || `Cancel failed (HTTP ${res.status})`);
      await fetchAgent();
      await fetchResponses();
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
      await fetchRuntime(true);
      await fetchResponses();
      await tick();
      scrollToBottom();
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

<!-- Edit Timeouts Modal -->
{#if showTimeoutsModal}
  <div class="modal fade show" style="display: block; background: rgba(0,0,0,.3);" tabindex="-1" role="dialog" aria-modal="true">
    <div class="modal-dialog">
      <div class="modal-content">
        <div class="modal-header">
          <h5 class="modal-title">Edit Timeouts</h5>
          <button type="button" class="btn-close" aria-label="Close" on:click={closeEditTimeouts}></button>
        </div>
        <div class="modal-body">
          <div class="row g-3">
            <div class="col-12 col-md-6">
              <label class="form-label" for="idle-timeout">Idle Timeout (seconds)</label>
              <input id="idle-timeout" type="number" min="0" step="1" class="form-control" bind:value={idleTimeoutInput} />
              <div class="form-text">Time of inactivity before auto-sleep. 0 disables idle timeout.</div>
            </div>
            <div class="col-12 col-md-6">
              <label class="form-label" for="busy-timeout">Busy Timeout (seconds)</label>
              <input id="busy-timeout" type="number" min="0" step="1" class="form-control" bind:value={busyTimeoutInput} />
              <div class="form-text">Max time to stay busy before reset. 0 disables busy timeout.</div>
            </div>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn btn-outline-secondary" on:click={closeEditTimeouts}>Cancel</button>
          <button class="btn btn-theme" on:click={saveTimeouts}>Save</button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Sleep Modal -->
{#if showSleepModal}
  <div class="modal fade show" style="display: block; background: rgba(0,0,0,.3);" tabindex="-1" role="dialog" aria-modal="true">
    <div class="modal-dialog">
      <div class="modal-content">
        <div class="modal-header">
          <h5 class="modal-title">Sleep Agent</h5>
          <button type="button" class="btn-close" aria-label="Close" on:click={closeSleepModal}></button>
        </div>
        <div class="modal-body">
          <label class="form-label" for="sleep-delay">Sleep in (seconds)</label>
          <input id="sleep-delay" type="number" min="5" step="1" class="form-control" bind:value={sleepDelayInput} />
          <div class="form-text">Minimum 5 seconds. The agent will go to sleep after this delay.</div>
          <div class="mt-3">
            <label class="form-label" for="sleep-note">Note (optional)</label>
            <input id="sleep-note" type="text" class="form-control" bind:value={sleepNoteInput} placeholder="e.g., Taking a break" />
            <div class="form-text">Shown alongside the sleep marker in chat.</div>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn btn-outline-secondary" on:click={closeSleepModal}>Cancel</button>
          <button class="btn btn-warning" on:click={confirmSleep}><i class="fas fa-moon me-1"></i>Sleep</button>
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
  <!-- No page overlay during compaction; only disable action controls -->
  <div class="col-12 d-flex flex-column h-100" style="min-height: 0;">
    <!-- Header: Agent details on the left, info box on the right -->
    <div class="row g-3 mb-2">
      <div class="col-12 col-lg-7">
        <Card class="h-100">
          <div class="card-body d-flex flex-column">
            <div class="d-flex align-items-center gap-2 mb-1">
              {#if agent}
                <a class="fw-bold text-decoration-none" href={'/agents/' + encodeURIComponent(agent.name || '')}>{agent.name || '-'}</a>
                {#if agent.is_published || agent.isPublished}
                  <a class="small ms-1 text-decoration-none text-body-secondary" href={`${getHostUrl()}/content/${agent?.name || name}/`} target="_blank" rel="noopener noreferrer">(public link)</a>
                {/if}
              {:else}
                <div class="fw-bold">{name}</div>
              {/if}
            </div>
            <div class="small text-body text-opacity-75 flex-grow-1">{agent?.description || agent?.desc || 'No description'}</div>
            {#if isAdmin && agent}
              <div class="small text-body-secondary mt-1">Owner: <span class="font-monospace">{agent.created_by}</span></div>
            {/if}
            <!-- Public URL in main card -->
            
            <div class="mt-2 d-flex flex-wrap gap-1">
              {#if Array.isArray(agent?.tags) && agent.tags.length}
                {#each agent.tags as t}
                  <span class="badge bg-secondary-subtle text-secondary-emphasis border">{t}</span>
                {/each}
              {:else}
                <span class="text-body-secondary small">No tags</span>
              {/if}
            </div>
            <!-- In-card actions (publish, remix, sleep/wake, kebab) -->
            <div class="mt-2 d-flex align-items-center flex-wrap">
              <!-- Compact status indicator on the left -->
              <div class="d-flex align-items-center gap-2">
                {#if agent}
                  <i class={`${stateIconClass(agent.state || agent.status)} me-1`}></i>
                  <span class="text-uppercase small fw-bold text-body">{agent.state || agent.status || 'unknown'}</span>
                {/if}
              </div>
              <!-- Actions on the right (tight group) -->
              <div class="ms-auto d-flex align-items-center flex-wrap gap-2">
                {#if stateStr === 'slept'}
                  <button class="btn btn-outline-success btn-sm" on:click={wakeAgent} aria-label="Wake agent">
                    <i class="fas fa-sun me-1"></i><span>Wake</span>
                  </button>
                {:else if stateStr === 'idle' || stateStr === 'busy'}
                  <button class="btn btn-outline-warning btn-sm" on:click={openSleepModal} aria-label="Put agent to sleep">
                    <i class="fas fa-moon me-1"></i><span>Sleep</span>
                  </button>
                {/if}
                {#if agent}
                  {#if agent.is_published || agent.isPublished}
                    <div class="dropdown">
                      <button class="btn btn-success btn-sm fw-bold dropdown-toggle published-toggle" type="button" data-bs-toggle="dropdown" aria-expanded="false" aria-label="Published options">
                        <i class="fas fa-globe me-1"></i><span>Published</span>
                      </button>
                      <ul class="dropdown-menu dropdown-menu-end">
                        <li>
                          <a class="dropdown-item" href={`${getHostUrl()}/content/${agent?.name || name}/`} target="_blank" rel="noopener noreferrer"><i class="fas fa-up-right-from-square me-2"></i>Open Public URL</a>
                        </li>
                        <li>
                          <button class="dropdown-item" on:click={publishAgent}><i class="fas fa-cloud-arrow-up me-2"></i>Publish New Version</button>
                        </li>
                        <li>
                          <button class="dropdown-item text-danger" on:click={unpublishAgent}><i class="fas fa-eye-slash me-2"></i>Unpublish</button>
                        </li>
                      </ul>
                    </div>
                  {:else}
                    <button class="btn btn-outline-primary btn-sm" on:click={publishAgent} aria-label="Publish content">
                      <i class="fas fa-cloud-arrow-up me-1"></i><span>Publish</span>
                    </button>
                  {/if}
                {/if}
                <div class="dropdown">
                  <button class="btn btn-outline-secondary btn-sm" type="button" data-bs-toggle="dropdown" aria-expanded="false" aria-label="More actions">
                    <i class="bi bi-three-dots"></i>
                  </button>
                  <ul class="dropdown-menu dropdown-menu-end">
                    <li><button class="dropdown-item" on:click={remixAgent}><i class="fas fa-code-branch me-2"></i>Remix</button></li>
                    <li><button class="dropdown-item" on:click={openEditTags}><i class="fas fa-tags me-2"></i>Edit Tags</button></li>
                    <li><button class="dropdown-item" on:click={openEditTimeouts}><i class="fas fa-hourglass-half me-2"></i>Edit Timeouts</button></li>
                    <li><hr class="dropdown-divider" /></li>
                    <li><button class="dropdown-item text-danger" on:click={deleteAgent}><i class="fas fa-trash me-2"></i>Delete</button></li>
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
              <!-- Last Activity removed per design -->
              <div class="mt-1">Idle Timeout: {fmtDuration(agent.idle_timeout_seconds)}</div>
              <div class="mt-1">Busy Timeout: {fmtDuration(agent.busy_timeout_seconds)}</div>
              <div class="mt-1">Runtime: {fmtDuration(runtimeSeconds)}{#if currentSessionSeconds > 0}&nbsp;(Current session: {fmtDuration(currentSessionSeconds)}){/if}</div>
              <div class="mt-2">
                <div class="d-flex align-items-center justify-content-between">
                  <div class="me-2">Context: {fmtInt(ctx?.used_tokens_estimated || 0)} / {fmtInt(ctx?.soft_limit_tokens || 100000)} ({fmtPct(ctx?.used_percent || 0)})</div>
                  <!-- Clear moved to toolbar next to Compact -->
                </div>
                <div class="progress mt-1" role="progressbar" aria-valuenow={Number(ctx?.used_percent || 0)} aria-valuemin="0" aria-valuemax="100" style="height: 6px;">
                  <div class="progress-bar {Number(ctx?.used_percent || 0) >= 90 ? 'bg-danger' : 'bg-theme'}" style={`width: ${Math.min(100, Number(ctx?.used_percent || 0)).toFixed(1)}%;`}></div>
                </div>
                <!-- Removed 'Since' line for context cutoff display -->
              </div>
              <div class="mt-2">
                <!-- Public URL moved to main card -->
              </div>
            </div>
          </Card>
        {/if}
      </div>
    </div>

    <!-- Toolbar -->
    <div class="d-flex align-items-center flex-wrap gap-2 mb-2">
      <button class="btn btn-sm btn-outline-secondary" on:click|preventDefault={compactContext} disabled={isCompacting || ctxLoading} title="Compact context with LLM summary">Compact</button>
      <button class="btn btn-sm btn-outline-secondary" on:click|preventDefault={clearContext} disabled={isCompacting || ctxLoading} title="Clear context and reset history window">Clear</button>
      <button class="btn btn-sm btn-outline-secondary" title="Expand all details" on:click={expandAllDetails} aria-label="Expand all">
        <i class="fas fa-angles-down"></i>
      </button>
      <button class="btn btn-sm btn-outline-secondary" title="Collapse all details" on:click={collapseAllDetails} aria-label="Collapse all">
        <i class="fas fa-angles-up"></i>
      </button>
      <div class="form-check form-switch" title="Toggle display of thinking (analysis/commentary)">
        <input class="form-check-input" type="checkbox" id="toggle-thinking" bind:checked={showThinking} />
        <label class="form-check-label small" for="toggle-thinking">🧠</label>
      </div>
      <div class="form-check form-switch" title="Toggle display of tool calls/results">
        <input class="form-check-input" type="checkbox" id="toggle-tools" bind:checked={showTools} />
        <label class="form-check-label small" for="toggle-tools">🛠️</label>
      </div>
    </div>

    {#if error}
      <div class="alert alert-danger py-2 small mb-2">{error}</div>
    {/if}
    {#if contextFull}
      <div class="alert alert-warning py-2 small mb-2 d-flex align-items-center justify-content-between" role="alert">
        <div>
          <i class="bi bi-exclamation-triangle me-2"></i>
          Context is full — clear it to continue.
        </div>
        <div class="ms-2">
          <button class="btn btn-sm btn-outline-secondary" on:click|preventDefault={clearContext} disabled={isCompacting || ctxLoading} title="Clear context and reset history window">Clear Context</button>
        </div>
      </div>
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
        {#if chat && chat.length}
          {#each chat as m, i}
            {#if m.role === 'user'}
              <div class="d-flex mb-3 justify-content-end">
                <div class="p-2 rounded-3 bg-dark text-white" style="max-width: 80%; white-space: pre-wrap; word-break: break-word;">
                  {m.content}
                </div>
              </div>
            {:else}
              <!-- Agent side -->
              {#if hasComposite(m)}
                <!-- Composite rendering: thinking, tool calls/results, final in one message -->
                <div class="mb-3">
                  <div class="d-flex justify-content-start">
                    <div class="text-body" style="max-width: 80%; word-break: break-word;">
                    {#each segmentsOf(m) as s, j}
                      {#if (segType(s) === 'commentary' || segChannel(s) === 'analysis' || segChannel(s) === 'commentary')}
                        {#if showThinking}
                          <div class="small fst-italic text-body text-opacity-50 mb-2" style="white-space: pre-wrap;">{segText(s)}</div>
                        {/if}
                      {:else if segType(s) === 'compact_summary'}
                        {#if segContent(s)}
                          <div class="markdown-wrap mt-2 mb-2">
                            <div class="markdown-body">{@html renderMarkdown(segContent(s))}</div>
                          </div>
                        {/if}
                      
                      {:else if segType(s) === 'tool_call'}
                        {#if showTools}
                        <!-- Optional commentary from args.commentary -->
                        {#if typeof (segArgs(s)?.commentary) === 'string' && String(segArgs(s)?.commentary).trim()}
                          <div class="small text-body mb-1" style="white-space: pre-wrap;">{String(segArgs(s).commentary).trim()}</div>
                        {/if}
                        <!-- Combine tool call + immediate tool result if next segment matches -->
                        {#if j + 1 < segmentsOf(m).length && segType(segmentsOf(m)[j+1]) === 'tool_result' && segTool(segmentsOf(m)[j+1]) === segTool(s)}
                          <div class="d-flex mb-1 justify-content-start">
                            <details class="mt-0" open={expandAll || openedSegments.has(segKey(m, j+1))} on:toggle={(e) => onToggleDetails(segKey(m, j+1), e.currentTarget.open)}>
                              <summary class="small fw-500 text-body text-opacity-75" style="cursor: pointer;">
                                <span class="badge rounded-pill bg-transparent border text-body text-opacity-75 me-2 px-2 py-1" style="font-size: .7rem;">{toolLabel(segTool(s))}</span>
                                <span class="text-body-secondary">{segToolTitle(s)}</span>
                              </summary>
                              <div class="small text-body">
                                <div class="text-body text-opacity-75 mb-1">Args</div>
                                <pre class="small bg-dark text-white p-2 rounded code-wrap mb-2"><code>{JSON.stringify({ tool: segTool(s), args: segArgs(s) }, null, 2)}</code></pre>
                                <div class="text-body text-opacity-75 mb-1">Result</div>
                                {#if isInProgress(m) || expandAll || expandedSegments.has(segKey(m, j+1))}
                                  <pre class="small bg-dark text-white p-2 rounded code-wrap mb-1"><code>{JSON.stringify({ output: segOutput(segmentsOf(m)[j+1]) }, null, 2)}</code></pre>
                                  {#if !isInProgress(m) && !expandAll}
                                    <button class="btn btn-link btn-sm p-0" on:click={() => collapseSeg(segKey(m, j+1))}>Show less</button>
                                  {/if}
                                {:else}
                                  <pre class="small bg-dark text-white p-2 rounded code-wrap mb-1"><code>{truncateMultiline(summaryOutputText(segmentsOf(m)[j+1]))}</code></pre>
                                  <button class="btn btn-link btn-sm p-0" on:click={() => expandSeg(segKey(m, j+1))}>Show more</button>
                                {/if}
                              </div>
                            </details>
                          </div>
                        {:else}
                          <!-- Unpaired tool call -->
                          <div class="d-flex mb-1 justify-content-start">
                            <details class="mt-0" open={expandAll}>
                              <summary class="small fw-500 text-body text-opacity-75" style="cursor: pointer;">
                                <span class="badge rounded-pill bg-transparent border text-body text-opacity-75 me-2 px-2 py-1" style="font-size: .7rem;">{toolLabel(segTool(s))}</span>
                                <span class="text-body-secondary">{segToolTitle(s)}</span>
                              </summary>
                              <pre class="small bg-dark text-white p-2 rounded mb-0 code-wrap"><code>{JSON.stringify({ tool: segTool(s), args: segArgs(s) }, null, 2)}</code></pre>
                            </details>
                          </div>
                        {/if}
                        {/if}
                      {:else if segType(s) === 'tool_result'}
                        {#if showTools}
                        <!-- Orphan tool result (no preceding call) -->
                        {#if !(j > 0 && segType(segmentsOf(m)[j-1]) === 'tool_call' && segTool(segmentsOf(m)[j-1]) === segTool(s))}
                          <div class="d-flex mb-1 justify-content-start">
                            <details class="mt-0" open={expandAll || openedSegments.has(segKey(m, j))} on:toggle={(e) => onToggleDetails(segKey(m, j), e.currentTarget.open)}>
                              <summary class="small fw-500 text-body text-opacity-75" style="cursor: pointer;">
                                <span class="badge rounded-pill bg-transparent border text-body text-opacity-75 me-2 px-2 py-1" style="font-size: .7rem;">{toolLabel(segTool(s))}</span>
                                <span class="text-body-secondary">Result</span>
                              </summary>
                              {#if isInProgress(m) || expandedSegments.has(segKey(m, j))}
                                <pre class="small bg-dark text-white p-2 rounded mb-1 code-wrap"><code>{JSON.stringify({ tool: segTool(s), output: segOutput(s) }, null, 2)}</code></pre>
                                {#if !isInProgress(m) && !expandAll}
                                  <button class="btn btn-link btn-sm p-0" on:click={() => collapseSeg(segKey(m, j))}>Show less</button>
                                {/if}
                              {:else}
                                <pre class="small bg-dark text-white p-2 rounded mb-1 code-wrap"><code>{truncateMultiline(summaryOutputText(s))}</code></pre>
                                <button class="btn btn-link btn-sm p-0" on:click={() => expandSeg(segKey(m, j))}>Show more</button>
                              {/if}
                            </details>
                          </div>
                        {/if}
                        {/if}
                        <!-- Always render output_* content inline during processing -->
                        {#if isOutputSeg(s)}
                          <Card class="mt-2 mb-2">
                            <div class="card-body py-2">
                              {#each outputItemsOfSeg(s) as it, k}
                                <details class="mb-2" open={true}>
                                  <summary class="small fw-500 text-body mb-1" style="cursor: pointer;">
                                    <span class="badge rounded-pill bg-transparent border me-2 px-2 py-1" style="font-size: .7rem;">{typeBadge(it?.type)}</span>
                                    {#if typeof it?.title === 'string' && it.title.trim()}<span class="text-body-secondary">{it.title}</span>{/if}
                                  </summary>
                                  {#if String(it?.type || '').toLowerCase() === 'markdown'}
                                    {#if typeof it?.content === 'string' && it.content.trim()}
                                      <div class="markdown-wrap mt-1 mb-2">
                                        <div class="markdown-body">{@html renderMarkdown(it.content)}</div>
                                      </div>
                                    {/if}
                                  {:else if String(it?.type || '').toLowerCase() === 'json'}
                                    <pre class="small bg-dark text-white p-2 rounded mb-1 code-wrap"><code>{JSON.stringify(it?.content, null, 2)}</code></pre>
                                  {:else if String(it?.type || '').toLowerCase() === 'url'}
                                    {#if typeof it?.content === 'string' && it.content.trim()}
                                      <a class="small" href={it.content} target="_blank" rel="noopener noreferrer">{it.content}</a>
                                    {/if}
                                  {/if}
                                </details>
                              {/each}
                          </div>
                          </Card>
                        {/if}
                        
                      {:else if segType(s) === 'slept'}
                        <!-- handled below as a full-width marker -->
                      {/if}
                    {/each}
                    </div>
                  </div>
                  {#if hasSleptSeg(m)}
                  <div class="d-flex align-items-center text-body mt-3">
                    <hr class="flex-grow-1 my-0" style="border-top: 2px dotted currentColor;" />
                    <span class="px-2 small">Slept{#if sleptNoteFrom(m)}&nbsp;({sleptNoteFrom(m)}){/if}{#if sleptRuntimeFrom(m)}&nbsp;-&nbsp;Runtime: {fmtDuration(sleptRuntimeFrom(m))}{/if}</span>
                    <hr class="flex-grow-1 my-0" style="border-top: 2px dotted currentColor;" />
                  </div>
                  {/if}
                  {#if hasCancelledSeg(m)}
                  <div class="d-flex align-items-center text-body mt-3">
                    <hr class="flex-grow-1 my-0" style="border-top: 2px dotted currentColor;" />
                    <span class="px-2 small">Cancelled{#if cancelledReasonFrom(m)}&nbsp;({cancelledReasonFrom(m)}){/if}</span>
                    <hr class="flex-grow-1 my-0" style="border-top: 2px dotted currentColor;" />
                  </div>
                  {/if}
                  {#if hasWokeSeg(m)}
                  <div class="d-flex align-items-center text-body mt-3">
                    <hr class="flex-grow-1 my-0" style="border-top: 2px dotted currentColor;" />
                    <span class="px-2 small">Woke up{#if wokeNoteFrom(m)}&nbsp;({wokeNoteFrom(m)}){/if}</span>
                    <hr class="flex-grow-1 my-0" style="border-top: 2px dotted currentColor;" />
                  </div>
                  {/if}
                  {#if hasContextClearedSeg(m)}
                  <div class="d-flex align-items-center text-body mt-3">
                    <hr class="flex-grow-1 my-0" style="border-top: 2px dotted currentColor;" />
                    <span class="px-2 small">Context Cleared</span>
                    <hr class="flex-grow-1 my-0" style="border-top: 2px dotted currentColor;" />
                  </div>
                  {/if}
                  {#if hasContextCompactedSeg(m)}
                  <div class="d-flex align-items-center text-body mt-3">
                    <hr class="flex-grow-1 my-0" style="border-top: 2px dotted currentColor;" />
                    <span class="px-2 small">Context Compacted</span>
                    <hr class="flex-grow-1 my-0" style="border-top: 2px dotted currentColor;" />
                  </div>
                  {/if}
                  {#if !hasFinalSeg(m) && m.content && m.content.trim()}
                  <div class="markdown-wrap mt-2">
                    <div class="markdown-body">{@html renderMarkdown(m.content)}</div>
                  </div>
                  {/if}
                </div>
              {:else}
              {#if isToolExec(m) && showTools}
                <!-- Compact single-line summary that toggles details for ALL tool requests -->
                <div class="d-flex mb-2 justify-content-start">
                  <details class="mt-0" open={expandAll}>
                    <summary class="small fw-500 text-body text-opacity-75" style="cursor: pointer;">
                      {toolLabel(metaOf(m)?.tool_type)} Request {argsPreview(m)}
                    </summary>
                    <pre class="small bg-dark text-white p-2 rounded mb-0 code-wrap"><code>{JSON.stringify({ tool: m?.metadata?.tool_type || 'tool', args: (m?.metadata?.args ?? { text: m.content }) }, null, 2)}</code></pre>
                  </details>
                </div>
              {:else}
                <!-- Tool response card or regular agent message -->
                {#if isToolResult(m) && showTools}
                  <!-- Compact single-line summary that toggles details for ALL tool responses -->
                  <div class="d-flex mb-2 justify-content-start">
                    <details class="mt-0" open={expandAll}>
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
                        {#if showThinking}
                          <div class="small fst-italic text-body text-opacity-50 mb-2" style="white-space: pre-wrap;">{metaOf(m)?.thinking}</div>
                        {/if}
                      {/if}
                      {#if m.content && m.content.trim()}
                        <div class="markdown-wrap mt-1">
                          <div class="markdown-body">{@html renderMarkdown(m.content)}</div>
                        </div>
                      {/if}
                      {#if Array.isArray(m?.content_json?.output_content) && m.content_json.output_content.length > 0}
                        <Card class="mt-2 mb-2">
                          <div class="card-body py-2">
                            {#each m.content_json.output_content as it}
                              <details class="mb-2" open={expandAll}>
                                <summary class="small fw-500 text-body mb-1" style="cursor: pointer;">
                                  <span class="badge rounded-pill bg-transparent border me-2 px-2 py-1" style="font-size: .7rem;">{typeBadge(it?.type)}</span>
                                  {#if typeof it?.title === 'string' && it.title.trim()}<span class="text-body-secondary">{it.title}</span>{/if}
                                </summary>
                                {#if String(it?.type || '').toLowerCase() === 'markdown'}
                                  {#if typeof it?.content === 'string' && it.content.trim()}
                                    <div class="markdown-wrap mt-1 mb-2">
                                      <div class="markdown-body">{@html renderMarkdown(it.content)}</div>
                                    </div>
                                  {/if}
                                {:else if String(it?.type || '').toLowerCase() === 'json'}
                                  <pre class="small bg-dark text-white p-2 rounded mb-1 code-wrap"><code>{JSON.stringify(it?.content, null, 2)}</code></pre>
                                {:else if String(it?.type || '').toLowerCase() === 'url'}
                                  {#if typeof it?.content === 'string' && it.content.trim()}
                                    <a class="small" href={it.content} target="_blank" rel="noopener noreferrer">{it.content}</a>
                                  {/if}
                                {/if}
                              </details>
                            {/each}
                          </div>
                        </Card>
                      {/if}
                    </div>
                  </div>
                {/if}
                <!-- If this is an output_* tool result card, render content inline during processing regardless of showTools -->
                {#if isToolResult(m)}
                  {#if (String(metaOf(m)?.tool_type || '').toLowerCase() === 'output')}
                    {#if m.content && m.content.trim()}
                      {#if parsedItemsFromTopCard(m).length > 0}
                        <Card class="mt-2 mb-2">
                          <div class="card-body py-2">
                            {#each parsedItemsFromTopCard(m) as it}
                              <details class="mb-2" open={expandAll}>
                                <summary class="small fw-500 text-body mb-1" style="cursor: pointer;">
                                  <span class="badge rounded-pill bg-transparent border me-2 px-2 py-1" style="font-size: .7rem;">{typeBadge(it?.type)}</span>
                                  {#if typeof it?.title === 'string' && it.title.trim()}<span class="text-body-secondary">{it.title}</span>{/if}
                                </summary>
                                {#if String(it?.type || '').toLowerCase() === 'markdown'}
                                  {#if typeof it?.content === 'string' && it.content.trim()}
                                    <div class="markdown-wrap mt-1 mb-2">
                                      <div class="markdown-body">{@html renderMarkdown(it.content)}</div>
                                    </div>
                                  {/if}
                                {:else if String(it?.type || '').toLowerCase() === 'json'}
                                  <pre class="small bg-dark text-white p-2 rounded mb-1 code-wrap"><code>{JSON.stringify(it?.content, null, 2)}</code></pre>
                                {:else if String(it?.type || '').toLowerCase() === 'url'}
                                  {#if typeof it?.content === 'string' && it.content.trim()}
                                    <a class="small" href={it.content} target="_blank" rel="noopener noreferrer">{it.content}</a>
                                  {/if}
                                {/if}
                              </details>
                            {/each}
                          </div>
                        </Card>
                      {/if}
                    {/if}
                  {/if}
                  
                {/if}
              {/if}
              {/if}
            {/if}
          {/each}
        {/if}
        {#if stateStr === 'busy'}
          <div class="d-flex mb-2 justify-content-start">
            <div class="small text-body-secondary d-flex align-items-center gap-2 px-2 py-1 rounded-2 border bg-body-tertiary">
              <span class="spinner-border spinner-border-sm" role="status" aria-hidden="true"></span>
              <span>Working...</span>
            </div>
          </div>
        {/if}
        </div>
      </div>

      <form class="pt-2" on:submit|preventDefault={sendMessage}>
        <div class="input-group chat-input-wrap">
          <textarea
            aria-label="Message input"
            class="form-control chat-input chat-no-zoom"
            disabled={isCompacting || stateStr === 'busy'}
            placeholder="Type a message…"
            rows="2"
            style="resize: none;"
            bind:this={inputEl}
            bind:value={input}
            on:keydown={(e)=>{
              // Send only on plain Enter (no modifiers). Allow Shift/Alt/Ctrl/Meta + Enter to insert newline.
              if (!isCompacting && e.key === 'Enter' && !e.shiftKey && !e.altKey && !e.ctrlKey && !e.metaKey) {
                e.preventDefault();
                sendMessage();
              }
            }}
            on:input={(e)=>{ try { if (!e.target.value || !e.target.value.trim()) { e.target.style.height=''; return; } e.target.style.height='auto'; e.target.style.height = Math.min(e.target.scrollHeight, 200) + 'px'; } catch(_){} }}
          ></textarea>
          {#if stateStr === 'busy'}
            <button type="button" class="btn btn-danger" aria-label="Cancel active" on:click={cancelActive}>
              <i class="fas fa-stop"></i>
            </button>
          {:else}
            <button class="btn btn-theme" aria-label="Send message" disabled={isCompacting || sending || !input.trim()}>
              {#if sending}
                <span class="spinner-border spinner-border-sm" role="status" aria-hidden="true"></span>
              {:else}
                <i class="fas fa-paper-plane"></i>
              {/if}
            </button>
          {/if}
        </div>
      </form>
    {/if}
  </div>

  <style>
    :global(pre.code-wrap) { white-space: pre-wrap; word-break: break-word; overflow-wrap: anywhere; }
    /* Chat input container adopts border; textarea is borderless */
    :global(.chat-input-wrap) {
      border: 1px solid var(--bs-border-color);
      border-radius: .5rem;
      background: var(--bs-body-bg);
      overflow: hidden; /* clip inner corners */
    }
    :global(.chat-input-wrap:focus-within) {
      border-color: rgba(var(--bs-theme-rgb), .5);
      box-shadow: 0 0 0 .25rem rgba(var(--bs-theme-rgb), .15);
    }
    :global(textarea.chat-input) {
      border: 0 !important;
      outline: 0 !important;
      box-shadow: none !important;
      background: transparent !important;
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
    /* Ensure dropdown menus overlay adjacent buttons and are not clipped */
    :global(.dropdown-menu) { z-index: 5000; }
    :global(.card) { overflow: visible; }
    /* Prevent iOS Safari from zooming the chat textarea on focus (needs >=16px) */
    @media (max-width: 576px) {
      :global(textarea.chat-no-zoom) { font-size: 16px; }
    }
  </style>
 </div>
