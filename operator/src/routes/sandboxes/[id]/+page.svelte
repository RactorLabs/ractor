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
  import PerfectScrollbar from '/src/components/plugins/PerfectScrollbar.svelte';
  import { getToken } from '$lib/auth.js';

  let md;
  try {
    md = new MarkdownIt({ html: false, linkify: true, breaks: true }).use(taskLists, { label: true, labelAfter: true });
    // Ensure all markdown links open in a new tab (task detail panel)
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

  // Note: 'id' param contains the sandbox ID
  let sandboxId = '';
  $: sandboxId = $page.params.id;

  let sandbox = null;
  // Update page title to show sandbox ID
  $: setPageTitle(sandbox?.id ? `Sandbox ${sandbox.id}` : 'Sandbox');
  let stateStr = '';
  // Task tracking state
  let tasks = [];
  let selectedTaskId = '';
  let selectedTask = null;
  let showTaskDetail = false;
  $: selectedTask = tasks.find((t) => t.id === selectedTaskId) || (tasks.length ? tasks[0] : null);
  $: if (showTaskDetail && !selectedTask) { showTaskDetail = false; }
  const FM_AUTO_REFRESH_COOKIE = 'tsbx_filesAutoRefresh';
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
  // Sync file path with URL (?file=seg1/seg2[/file.ext])
  function _getPathFromUrl() {
    try {
      if (typeof window === 'undefined') return [];
      const u = new URL(window.location.href);
      const p = u.searchParams.get('file');
      if (!p) return [];
      return p.split('/').filter(Boolean).map(decodeURIComponent);
    } catch (_) { return []; }
  }
  function _setPathInUrl(segs, fileName = '') {
    try {
      if (typeof window === 'undefined') return;
      const u = new URL(window.location.href);
      const parts = (Array.isArray(segs) ? segs : []).map(encodeURIComponent);
      const val = fileName ? parts.concat([encodeURIComponent(fileName)]).join('/') : parts.join('/');
      if (val) u.searchParams.set('file', val);
      else u.searchParams.delete('file');
      window.history.replaceState({}, '', u.toString());
    } catch (_) {}
  }
  let fmPendingOpenFile = '';
  onMount(async () => {
    try {
      // Initialize folder/file path from URL before first fetch
      const init = _getPathFromUrl();
      if (init && init.length) {
        fmSegments = init.slice(0, -1);
        fmPendingOpenFile = init[init.length - 1] || '';
      }
    } catch (_) {}
    try { await fetchFiles(true); } catch (_) {}
    // layout handles equal heights; no JS equalizer
  });
  let loading = true;
  let error = null;
  let input = '';
  let sending = false;
  let pollHandle = null;
  let runtimeSeconds = 0;
  let currentSandboxSeconds = 0;
  // Equalize top card heights (left Sandbox card and right Info card)
  // No JS equal-height logic; use layout-based alignment

  // ---------------- File panel state (right side) ----------------
  // Start at /sandbox/ (represented as empty relative path "")
  let fmLoading = false;
  let fmError = null;
  let fmErrorNotAvailable = false;
  let fmEntries = [];
  let fmOffset = 0;
  let fmLimit = 100;
  let fmNextOffset = null;
  let fmTotal = 0;
  let fmListKey = 0; // force remount of scroll area after list refresh
  // Maintain relative path segments under /sandbox
  let fmSegments = [];
  // Reactive full path label for toolbar (current folder only; no selection state)
  let currentFullPath = '';
  // Delete modal state (supports file or directory)
  let showDeleteFile = false;
  let deleteFileError = '';
  let fmDeleteEntry = null; // { name, kind, segs }
  function openDeleteEntry(entry) { try { deleteFileError=''; fmDeleteEntry = entry ? { name: entry.name, kind: String(entry.kind||'').toLowerCase(), segs: [...fmSegments] } : null; showDeleteFile = true; } catch(_) { showDeleteFile = true; } }
  function closeDeleteFile() { showDeleteFile = false; fmDeleteEntry = null; }
  async function confirmDeleteFile() {
    try {
      deleteFileError = '';
      const target = fmDeleteEntry || null;
      if (!target) { showDeleteFile=false; return; }
      const segs = [...(target.segs || []), target.name].filter(Boolean);
      const relEnc = segs.map(encodeURIComponent).join('/');
      // Attempt delete (not supported in read-only API; will likely fail)
      const res = await apiFetch(`/sandboxes/${encodeURIComponent(sandboxId)}/files/delete/${relEnc}`, { method: 'DELETE' });
      if (!res.ok) {
        throw new Error(res?.data?.message || res?.data?.error || 'Delete not supported');
      }
      showDeleteFile = false;
      await refreshFilesPanel({ reset: true });
    } catch (e) {
      deleteFileError = e.message || String(e);
    }
  }
  function fmPathStr() {
    try { return (fmSegments || []).map(encodeURIComponent).join('/'); } catch (_) { return ''; }
  }
  function fmDisplayPath() {
    try { return ['/sandbox'].concat(fmSegments || []).join('/'); } catch (_) { return '/sandbox'; }
  }
  function fmDisplayPathShort() {
    try { return (fmSegments && fmSegments.length) ? ('/' + (fmSegments || []).join('/')) : '/'; } catch (_) { return '/'; }
  }
  function fmCurrentFullPath(segs, fileName) {
    try {
      const base = ['/sandbox'].concat(segs || []);
      if (fileName) return base.concat([fileName]).join('/');
      return base.join('/');
    } catch (_) { return fmDisplayPath(); }
  }
  $: currentFullPath = fmCurrentFullPath(fmSegments, fmPreviewName);
  function fmIconFor(entry) {
    const k = String(entry?.kind || '').toLowerCase();
    if (k === 'dir' || k === 'directory') return 'bi bi-folder text-warning';
    if (k === 'symlink') return 'bi bi-link-45deg text-secondary';
    return 'bi bi-file-earmark text-body text-opacity-50';
  }
  function fmHumanSize(n) {
    try {
      const v = Number(n);
      if (!Number.isFinite(v)) return String(n);
      const units = ['B','KB','MB','GB'];
      let s = v; let i = 0;
      while (s >= 1024 && i < units.length-1) { s /= 1024; i++; }
      return `${s.toFixed(s >= 10 || i === 0 ? 0 : 1)} ${units[i]}`;
    } catch(_) { return String(n); }
  }

  // Unified Files panel updater: list + (optional) file details in one call
  async function refreshFilesPanel(opts = {}) {
    const { reset = true } = opts || {};
    try { await fetchFiles(!!reset); } catch (_) {}
  }
  // Guard concurrent list loads as well, to avoid stale folder/file lists
  let fmListSeq = 0;
  async function fetchFiles(reset = true) {
    const seq = ++fmListSeq;
    fmLoading = true; fmError = null;
    try {
      let path = fmPathStr();
      let url;
      if (!path) url = `/sandboxes/${encodeURIComponent(sandboxId)}/files/list?offset=${reset ? 0 : fmOffset}&limit=${fmLimit}`;
      else url = `/sandboxes/${encodeURIComponent(sandboxId)}/files/list/${path}?offset=${reset ? 0 : fmOffset}&limit=${fmLimit}`;
      const res = await apiFetch(url);
      if (seq !== fmListSeq) return; // outdated
      if (!res.ok) {
        fmError = res?.data?.message || res?.data?.error || `Failed to list (HTTP ${res.status})`;
        fmLoading = false;
        return;
      }
      const data = res.data || {};
      const entries = Array.isArray(data.entries) ? data.entries : [];
      fmTotal = Number(data.total || entries.length || 0);
      fmLimit = Number(data.limit || fmLimit);
      fmOffset = Number(data.offset || 0);
      fmNextOffset = (data.next_offset == null) ? null : Number(data.next_offset);
      fmEntries = reset ? entries : fmEntries.concat(entries);
      // Ensure the path label reflects the current folder after list refresh
      currentFullPath = fmCurrentFullPath(fmSegments, fmPreviewName);
      // If there's a pending URL segment to resolve, try once after initial list
      if (reset && fmPendingOpenFile) {
        try {
          const name = String(fmPendingOpenFile);
          const match = (fmEntries || []).find(e => String(e?.name || '') === name);
          const kind = String(match?.kind || '').toLowerCase();
          if (match && (kind === 'dir' || kind === 'directory')) {
            // It's a folder segment; descend into it and refresh
            fmSegments = [...fmSegments, name];
            fmOffset = 0;
            try { _setPathInUrl(fmSegments); } catch (_) {}
            fmPendingOpenFile = '';
            fetchFiles(true);
            return;
          }
          // Otherwise try to open as a file in the current folder
          if (name) fmShowPreview({ name, kind: 'file' });
        } catch (_) {}
        finally { fmPendingOpenFile = ''; }
      }
    } catch (e) {
      if (seq === fmListSeq) fmError = e.message || String(e);
    } finally {
      if (seq === fmListSeq) { fmLoading = false; try { fmListKey++; } catch (_) {} }
    }
  }
  function fmOpen(entry) {
    if (!entry) return;
    const k = String(entry.kind || '').toLowerCase();
    if (k === 'dir' || k === 'directory') {
      fmSegments = [...fmSegments, entry.name];
      fmOffset = 0;
      try { _setPathInUrl(fmSegments); } catch (_) {}
      fetchFiles(true);
    } else {
      fmShowPreview(entry);
    }
  }
  // Lightweight preview state (no selection/highlight)
  let fmPreviewName = '';
  let fmPreviewType = '';
  let fmPreviewText = '';
  let fmPreviewUrl = '';
  let fmPreviewLoading = false;
  let fmPreviewError = null;
  let fmPreviewSeq = 0;
  let fmPreviewAbort = null;
  function fmRevokePreviewUrl() { try { if (fmPreviewUrl) { URL.revokeObjectURL(fmPreviewUrl); } } catch (_) {} }
  function fmPreviewReset() { fmRevokePreviewUrl(); fmPreviewName=''; fmPreviewType=''; fmPreviewText=''; fmPreviewUrl=''; fmPreviewLoading=false; fmPreviewError=null; try { _setPathInUrl(fmSegments); } catch (_) {} }
  async function fmShowPreview(entry) {
    try {
      // Cancel any in-flight preview load
      try { fmPreviewAbort && fmPreviewAbort.abort(); } catch (_) {}
      const seq = ++fmPreviewSeq;
      fmPreviewAbort = (typeof AbortController !== 'undefined') ? new AbortController() : null;
      // Keep existing preview content visible while loading new content
      fmPreviewError = null; fmPreviewLoading = true;
      fmPreviewName = entry?.name || '';
      try { _setPathInUrl(fmSegments, fmPreviewName); } catch (_) {}
      const segs = [...fmSegments, fmPreviewName].filter(Boolean);
      const relEnc = segs.map(encodeURIComponent).join('/');
      const token = getToken();
      const url = `/api/v0/sandboxes/${encodeURIComponent(sandboxId)}/files/read/${relEnc}`;
      const res = await fetch(url, { headers: token ? { 'Authorization': `Bearer ${token}` } : {}, signal: fmPreviewAbort ? fmPreviewAbort.signal : undefined });
      if (!res.ok) {
        fmPreviewError = (res.status === 404 ? 'Not found' : (res.status === 413 ? 'File too large (>25MB)' : `Open failed (HTTP ${res.status})`));
      } else {
        const ct = (res.headers.get('content-type') || '').toLowerCase();
        if (ct.startsWith('image/')) {
          const blob = await res.blob();
          if (seq !== fmPreviewSeq) return; // outdated
          const newUrl = URL.createObjectURL(blob);
          const oldUrl = fmPreviewUrl;
          fmPreviewUrl = newUrl;
          fmPreviewType = 'image';
          try { if (oldUrl) URL.revokeObjectURL(oldUrl); } catch (_) {}
        } else if (ct.startsWith('text/') || ct.includes('json') || ct.includes('javascript') || ct.includes('xml') || ct.includes('yaml') || ct.includes('toml') || ct.includes('html')) {
          // Read text content fully, then swap to avoid flicker during load
          let acc = '';
          const reader = res.body && res.body.getReader ? res.body.getReader() : null;
          if (reader) {
            const decoder = new TextDecoder('utf-8');
            let done = false;
            while (!done) {
              const r = await reader.read();
              if (seq !== fmPreviewSeq) { try { reader.cancel && reader.cancel(); } catch (_) {} return; }
              done = r.done;
              if (r.value && r.value.length) {
                acc += decoder.decode(r.value, { stream: !done });
              }
            }
          } else {
            // Fallback: read all at once
            const text = await res.text();
            if (seq !== fmPreviewSeq) return;
            acc = text;
          }
          // Commit new text after fully loaded
          fmPreviewText = acc;
          fmPreviewType = 'text';
        } else {
          // Try as text anyway up to a cap
          try { const t = await res.text(); if (seq !== fmPreviewSeq) return; fmPreviewText = t; fmPreviewType = 'text'; }
          catch (_) { fmPreviewType = 'binary'; fmPreviewError = 'Preview not available for this type'; }
        }
      }
    } catch (e) {
      if (e && String(e.name || '') === 'AbortError') { /* ignore abort */ }
      else fmPreviewError = e.message || String(e);
    } finally {
      fmPreviewLoading = false;
    }
  }

  async function fmDownloadEntry(entry) {
    try {
      const segs = [...fmSegments, entry?.name].filter(Boolean);
      const relEnc = segs.map(encodeURIComponent).join('/');
      const token = getToken();
      const url = `/api/v0/sandboxes/${encodeURIComponent(sandboxId)}/files/read/${relEnc}`;
      const res = await fetch(url, { headers: token ? { 'Authorization': `Bearer ${token}` } : {} });
      if (!res.ok) { fmError = (res.status === 404 ? 'Not found' : (res.status === 413 ? 'File too large (>25MB)' : `Download failed (HTTP ${res.status})`)); return; }
      const blob = await res.blob();
      const a = document.createElement('a');
      const href = URL.createObjectURL(blob);
      a.href = href; a.download = entry?.name || 'download';
      document.body.appendChild(a); a.click(); a.remove();
      setTimeout(() => { try { URL.revokeObjectURL(href); } catch (_) {} }, 1000);
    } catch (e) { fmError = e.message || String(e); }
  }
  function fmGoUp() {
    // If a file is open, just close the preview and stay in the same folder
    if (fmPreviewName) { fmPreviewReset(); return; }
    if (fmSegments.length === 0) return; // at root; nothing to go up to
    fmSegments = fmSegments.slice(0, -1);
    fmOffset = 0;
    try { _setPathInUrl(fmSegments); } catch (_) {}
    refreshFilesPanel({ reset: true });
  }
  function fmGoRoot() {
    fmPreviewReset();
    fmSegments = [];
    fmOffset = 0;
    try { _setPathInUrl(fmSegments); } catch (_) {}
    refreshFilesPanel({ reset: true });
  }
  function fmRefresh() {
    if (fmPreviewName) {
      // Refresh the folder list in the background and re-fetch the open file
      try { fetchFiles(true); } catch (_) {}
      fmShowPreview({ name: fmPreviewName, kind: 'file' });
    } else {
      refreshFilesPanel({ reset: true });
    }
  }

  // Auto-refresh the Files panel every 5 seconds (equivalent to pressing Refresh). Default on.
  let fmAutoRefresh = true;
  let fmRefreshHandle = null;
  let fmAutoRefreshReady = false;

  function fmStopAutoRefresh() {
    try {
      if (fmRefreshHandle) {
        clearInterval(fmRefreshHandle);
      }
    } catch (_) {}
    fmRefreshHandle = null;
  }

  function fmRestartAutoRefresh() {
    fmStopAutoRefresh();
    if (!fmAutoRefresh) return;
    try {
      fmRefreshHandle = setInterval(() => {
        try {
          fmRefresh();
        } catch (_) {}
      }, 5000);
    } catch (_) {}
  }

  onMount(() => {
    try {
      if (browser) {
        const autoPref = getCookie(FM_AUTO_REFRESH_COOKIE);
        if (autoPref !== null) {
          fmAutoRefresh = autoPref === '1' || autoPref === 'true';
        }
      }
    } catch (_) {}
    fmAutoRefreshReady = true;
    fmRestartAutoRefresh();
    return () => {
      fmAutoRefreshReady = false;
      fmStopAutoRefresh();
    };
  });

  onDestroy(() => {
    fmAutoRefreshReady = false;
    fmStopAutoRefresh();
  });

  $: if (fmAutoRefreshReady) {
    try { setCookie(FM_AUTO_REFRESH_COOKIE, fmAutoRefresh ? '1' : '0', 365); } catch (_) {}
    fmRestartAutoRefresh();
  }
  function fmLoadMore() {
    if (fmNextOffset == null) return;
    fmOffset = Number(fmNextOffset);
    fetchFiles(false);
  }

  $: fmErrorNotAvailable = (() => {
    try {
      if (!fmError) return false;
      const msg = String(fmError).toLowerCase();
      return msg.includes('sandbox not available');
    } catch (_) {
      return false;
    }
  })();

  function fmtInt(n) {
    try { const v = Number(n); return Number.isFinite(v) ? v.toLocaleString() : String(n); } catch (_) { return String(n); }
  }
  function fmtPct(n) {
    try { const v = Number(n); return Number.isFinite(v) ? `${v.toFixed(1)}%` : '-'; } catch (_) { return '-'; }
  }
  // File list kind counters for folder details
  function countKind(kind) {
    try { const k = String(kind || '').toLowerCase(); return (fmEntries || []).filter(e => String(e?.kind || '').toLowerCase() === k).length; } catch (_) { return 0; }
  }
  function countFiles() { return countKind('file'); }
  function countDirs() { try { return (fmEntries || []).filter(e => { const k = String(e?.kind || '').toLowerCase(); return k === 'dir' || k === 'directory'; }).length; } catch (_) { return 0; } }
  function countSymlinks() { return countKind('symlink'); }
  const CONTEXT_SOFT_LIMIT_TOKENS = 128000; // Keep aligned with backend default
  $: contextTokensUsed = Number(sandbox?.last_context_length ?? 0);
  $: contextSoftLimit = CONTEXT_SOFT_LIMIT_TOKENS;
  $: contextUsedPercent = contextSoftLimit > 0
    ? Math.min(100, (contextTokensUsed / contextSoftLimit) * 100)
    : 0;
  let _runtimeFetchedAt = 0;
  let inputEl = null; // task textarea element
  // Content preview via sandbox ports has been removed.

  function stateClass(state) {
    const s = String(state || '').toLowerCase();
    if (s === 'initializing') return 'badge rounded-pill bg-transparent border border-info text-info';
    if (s === 'idle') return 'badge rounded-pill bg-transparent border border-success text-success';
    if (s === 'busy') return 'badge rounded-pill bg-transparent border border-warning text-warning';
    if (s === 'terminating') return 'badge rounded-pill bg-transparent border border-danger text-danger';
    if (s === 'terminated') return 'badge rounded-pill bg-transparent border border-danger text-danger';
    return 'badge rounded-pill bg-transparent border border-secondary text-secondary';
  }

  function stateColorClass(state) {
    const s = String(state || '').toLowerCase();
    if (s === 'idle') return 'bg-success border-success';
    if (s === 'busy') return 'bg-warning border-warning';
    if (s === 'initializing') return 'bg-info border-info';
    if (s === 'terminating' || s === 'terminated') return 'bg-danger border-danger';
    return 'bg-secondary border-secondary';
  }

  function stateIconClass(state) {
    const s = String(state || '').toLowerCase();
    if (s === 'terminated') return 'bi bi-power';
    if (s === 'terminating') return 'spinner-border spinner-border-sm text-danger';
    if (s === 'idle') return 'bi bi-sun';
    if (s === 'busy') return 'spinner-border spinner-border-sm';
    if (s === 'initializing') return 'spinner-border spinner-border-sm text-info';
    return 'bi bi-circle';
  }

  function normState(v) { return String(v || '').trim().toLowerCase(); }
  $: stateStr = normState(sandbox?.state);
  $: isAdmin = $auth && String($auth.type || '').toLowerCase() === 'admin';

  function isStopped() { return stateStr === 'terminated' || stateStr === 'terminating'; }
  function isActive() { return stateStr === 'idle' || stateStr === 'busy'; }
  function isUnavailable() {
    return stateStr === 'initializing' || stateStr === 'terminating' || stateStr === 'terminated';
  }

  let _lastStateStr = '';
  $: {
    const nextState = stateStr;
    if (nextState !== _lastStateStr) {
      const prevState = _lastStateStr;
      _lastStateStr = nextState;
      if (prevState) {
        try {
          fmRefresh();
        } catch (_) {}
      }
    }
  }

  // Edit tags modal state and helpers
  let showTagsModal = false;
  let tagsInput = '';
  function openEditTags() {
    const current = Array.isArray(sandbox?.tags) ? sandbox.tags : [];
    tagsInput = current.join(', ');
    showTagsModal = true;
  }
  function closeEditTags() { showTagsModal = false; }
  function parseTagsInput() {
    const parts = tagsInput.split(',').map(s => s.trim().toLowerCase()).filter(Boolean);
    const re = /^[A-Za-z0-9_\/\.\-]+$/;
    for (const t of parts) {
      if (!re.test(t)) throw new Error(`Invalid tag '${t}'. Allowed: letters, digits, '/', '-', '_', '.'.`);
    }
    return parts;
  }
  async function saveTags() {
    try {
      const tags = parseTagsInput();
      const res = await apiFetch(`/sandboxes/${encodeURIComponent(sandboxId)}`, { method: 'PUT', body: JSON.stringify({ tags }) });
      if (!res.ok) throw new Error(res?.data?.message || res?.data?.error || `Update failed (HTTP ${res.status})`);
      // Update local sandbox tags
      sandbox = res.data || sandbox;
      if (sandbox && !Array.isArray(sandbox.tags)) sandbox.tags = tags;
      showTagsModal = false;
    } catch (e) {
      alert(e.message || String(e));
    }
  }

  // Edit timeouts modal state and helpers
  let showTimeoutsModal = false;
  let idleTimeoutInput = 0;
  function openEditTimeouts() {
    const idle = Number(sandbox?.idle_timeout_seconds ?? 900);
    idleTimeoutInput = Number.isFinite(idle) && idle >= 0 ? idle : 900;
    showTimeoutsModal = true;
  }
  function closeEditTimeouts() { showTimeoutsModal = false; }
  async function saveTimeouts() {
    try {
      const idle = Math.max(0, Math.floor(Number(idleTimeoutInput || 900)));
      const body = { idle_timeout_seconds: idle };
      const res = await apiFetch(`/sandboxes/${encodeURIComponent(sandboxId)}`, { method: 'PUT', body: JSON.stringify(body) });
      if (!res.ok) throw new Error(res?.data?.message || res?.data?.error || `Update failed (HTTP ${res.status})`);
      // Update local sandbox snapshot
      sandbox = res.data || sandbox;
      if (sandbox) {
        sandbox.idle_timeout_seconds = idle;
      }
      showTimeoutsModal = false;
    } catch (e) {
      alert(e.message || String(e));
    }
  }


  // Delete modal state and actions
  let showTerminateModal = false;
  let terminateConfirm = '';
  function openTerminateModal() { terminateConfirm = ''; showTerminateModal = true; }
  function closeTerminateModal() { showTerminateModal = false; }
  $: canConfirmTerminate = String(terminateConfirm || '').trim() === String(sandbox?.id || sandboxId || '').trim();

  // Snapshot modal state and actions
  let showSnapshotModal = false;
  let snapshotError = null;
  function openSnapshotModal() {
    snapshotError = null;
    showSnapshotModal = true;
  }
  function closeSnapshotModal() { showSnapshotModal = false; }
  async function confirmCreateSnapshot() {
    try {
      snapshotError = null;
      const res = await apiFetch(`/sandboxes/${encodeURIComponent(sandboxId)}/snapshots`, {
        method: 'POST',
        body: JSON.stringify({ trigger_type: 'manual' })
      });
      if (!res.ok) {
        snapshotError = res?.data?.message || res?.data?.error || `Snapshot creation failed (HTTP ${res.status})`;
        return;
      }
      showSnapshotModal = false;
      // Redirect to snapshots page
      goto('/snapshots');
    } catch (e) {
      snapshotError = e.message || String(e);
    }
  }

  async function fetchSandbox() {
    const res = await apiFetch(`/sandboxes/${encodeURIComponent(sandboxId)}`);
    if (res.ok && res.data) {
      sandbox = res.data;
    }
    // No content frame to compute; panel shows status only.
  }

  async function fetchRuntime(force = false) {
    try {
      if (!force && Date.now() - _runtimeFetchedAt < 10000) return; // throttle to 10s
      const res = await apiFetch(`/sandboxes/${encodeURIComponent(sandboxId)}/runtime`);
      if (res.ok) {
        const v = Number(res?.data?.total_runtime_seconds ?? 0);
        if (Number.isFinite(v) && v >= 0) runtimeSeconds = v;
        const cs = Number(res?.data?.current_sandbox_seconds ?? 0);
        currentSandboxSeconds = Number.isFinite(cs) && cs >= 0 ? cs : 0;
        _runtimeFetchedAt = Date.now();
      }
    } catch (_) {}
  }

  async function fetchTasks() {
    const res = await apiFetch(`/sandboxes/${encodeURIComponent(sandboxId)}/tasks?limit=200`);
    if (res.ok) {
      const list = Array.isArray(res.data) ? res.data : (res.data?.tasks || []);
      tasks = list;
      if (!tasks.some((t) => t.id === selectedTaskId)) {
        selectedTaskId = tasks.length ? tasks[0].id : '';
      }
      if (!tasks.length) {
        showTaskDetail = false;
      }
    }
  }

  function startPolling() {
    stopPolling();
    pollHandle = setInterval(async () => {
      await fetchTasks();
      await fetchSandbox();
      await fetchRuntime();
    }, 2000);
  }
  function stopPolling() { if (pollHandle) { clearInterval(pollHandle); pollHandle = null; } }

  // No content preview probing; only status is shown in the panel.

  function segType(s) { return String(s?.type || '').toLowerCase(); }
  function segText(s) { return String(s?.text || ''); }
  function segTool(s) { return String(s?.tool || ''); }
  function segArgs(s) {
    try {
      if (!s || typeof s !== 'object') return null;
      if (s.arguments && typeof s.arguments === 'object') return s.arguments;
      if (s.args && typeof s.args === 'object') return s.args;
      return null;
    } catch (_) { return null; }
  }
  function segOutput(s) {
    try {
      const o = s?.output;
      if (typeof o === 'string') return o;
      return o;
    } catch (_) { return s?.output; }
  }
  function isOutputToolName(t) {
    try { const n = String(t || '').toLowerCase(); return n === 'output' || n === 'output_markdown' || n === 'ouput_json' || n === 'output_json'; } catch (_) { return false; }
  }
  function isOutputSeg(s) { try { return segType(s) === 'tool_result' && isOutputToolName(segTool(s)); } catch (_) { return false; } }
  function outputMarkdownOfSeg(s) {
    try {
      const out = segOutput(s);
      if (out && typeof out === 'object' && typeof out.content === 'string') return out.content;
      if (typeof out === 'string') return out;
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
  function taskStatus(task) {
    try { return String(task?.status || '').toLowerCase(); } catch (_) { return ''; }
  }
  function taskStatusLabel(task) {
    const status = taskStatus(task);
    if (status === 'pending') return 'Pending';
    if (status === 'processing') return 'Processing';
    if (status === 'completed') return 'Completed';
    if (status === 'failed') return 'Failed';
    if (status === 'cancelled') return 'Cancelled';
    return status || 'Unknown';
  }
  function taskStatusBadgeClass(task) {
    const status = taskStatus(task);
    if (status === 'completed') return 'bg-success-subtle text-success-emphasis border';
    if (status === 'processing') return 'bg-info-subtle text-info-emphasis border';
    if (status === 'failed' || status === 'cancelled') return 'bg-danger-subtle text-danger-emphasis border';
    if (status === 'pending') return 'bg-warning-subtle text-warning-emphasis border';
    return 'bg-secondary-subtle text-secondary-emphasis border';
  }
  function formatTaskTimestamp(task) {
    try {
      const created = task?.created_at ? new Date(task.created_at) : null;
      if (!created || Number.isNaN(created.getTime())) return '';
      return created.toLocaleString(undefined, { hour: '2-digit', minute: '2-digit', month: 'short', day: 'numeric' });
    } catch (_) { return ''; }
  }
  function taskInputItems(task) {
    try {
      const items = Array.isArray(task?.input_content) ? task.input_content : [];
      if (items.length) return items;
      const text = task?.input?.text || task?.input?.content;
      if (typeof text === 'string' && text.trim()) {
        return [{ type: 'text', content: text }];
      }
      return [];
    } catch (_) { return []; }
  }
  function segmentsOfTask(task) {
    try { return Array.isArray(task?.segments) ? task.segments : []; } catch (_) { return []; }
  }
  function isAnalysisSegment(seg) {
    const type = segType(seg);
    const channel = String(seg?.channel || '').toLowerCase();
    return type === 'commentary' || channel === 'analysis' || channel === 'commentary';
  }

  function taskAnalysisSegments(task) {
    const segs = segmentsOfTask(task);
    return segs.filter((s) => isAnalysisSegment(s));
  }
  function taskToolPairs(task) {
    const segs = segmentsOfTask(task);
    const pairs = [];
    for (let i = 0; i < segs.length; i++) {
      const seg = segs[i];
      if (segType(seg) === 'tool_call') {
        let result = null;
        if (i + 1 < segs.length && segType(segs[i + 1]) === 'tool_result' && segTool(segs[i + 1]) === segTool(seg)) {
          result = segs[i + 1];
        }
        const commentary = [];
        for (let j = i - 1; j >= 0; j--) {
          const prev = segs[j];
          if (isAnalysisSegment(prev)) {
            commentary.unshift(prev);
            continue;
          }
          break;
        }
        pairs.push({ call: seg, result, commentary });
      }
    }
    return pairs;
  }
  function formatToolSummary(callSeg) {
    if (!callSeg) return '';
    try {
      const tool = segTool(callSeg);
      const args = segArgs(callSeg) || {};
      if (tool === 'run_bash' && typeof args.commands === 'string') {
        const cmd = args.commands.trim();
        return cmd.includes('\n') ? cmd.split('\n')[0] + ' …' : cmd;
      }
      const json = JSON.stringify(args);
      if (!json) return '';
      return json.length > 80 ? json.slice(0, 77) + '…' : json;
    } catch (_) {
      return '';
    }
  }
  function taskOutputItems(task) {
    const direct = Array.isArray(task?.output_content) ? task.output_content : [];
    if (direct.length) return direct;
    const segs = segmentsOfTask(task);
    for (const seg of segs) {
      if (isOutputSeg(seg)) {
        const items = outputItemsOfSeg(seg);
        if (items.length) return items;
        const markdown = outputMarkdownOfSeg(seg);
        if (markdown) return [{ type: 'markdown', content: markdown }];
        const raw = segOutput(seg);
        if (raw != null) {
          if (typeof raw === 'string') return [{ type: 'markdown', content: raw }];
          return [{ type: 'json', content: raw }];
        }
      }
    }
    return [];
  }

  function taskPreview(task) {
    try {
      const items = taskInputItems(task);
      const textItem = items.find((i) => String(i?.type || '').toLowerCase() === 'text');
      const value = textItem ? String(textItem.content || '') : '';
      const normalized = value.replace(/\s+/g, ' ').trim();
      if (!normalized) return '';
      return normalized.length > 120 ? `${normalized.slice(0, 117)}…` : normalized;
    } catch (_) {
      return '';
    }
  }

  function openTaskDetail(taskId) {
    selectedTaskId = taskId;
    showTaskDetail = true;
  }

  function closeTaskDetail() {
    showTaskDetail = false;
  }

  async function createTask(e) {
    e?.preventDefault?.();
    const content = (input || '').trim();
    if (!content || sending || stateStr === 'busy') { if (stateStr === 'busy') { error = 'Sandbox is busy'; } return; }
    sending = true;
    try {
      const res = await apiFetch(`/sandboxes/${encodeURIComponent(sandboxId)}/tasks`, {
        method: 'POST',
        body: JSON.stringify({ input: { content: [{ type: 'text', content }] } })
      });
      if (!res.ok) {
        if (res.status === 409) {
          // Refresh sandbox details so local context usage reflects backend limits
          await fetchSandbox();
        }
        throw new Error(res?.data?.message || res?.data?.error || `Send failed (HTTP ${res.status})`);
      }
      input = '';
      // Reset textarea height after clearing
      error = null;
      await tick();
      try { if (inputEl) { inputEl.style.height = ''; } } catch (_) {}
      // Wait until the server-side task row appears, then update UI
      const rid = res?.data?.id;
      const deadline = Date.now() + 10000; // up to 10s
      while (Date.now() < deadline) {
        await fetchTasks();
        if (!rid || tasks.some((t) => t.id === rid)) {
          if (rid) {
            selectedTaskId = rid;
            showTaskDetail = true;
          }
          break;
        }
        await new Promise(r => setTimeout(r, 200));
      }
    } catch (e) {
      error = e.message || String(e);
      sending = false;
      return;
    }
    sending = false;
  }

  async function cancelActive() {
    try {
      const activeTask = tasks.find((t) => String(t?.status || '').toLowerCase() === 'processing');
      if (!activeTask) return;
      const res = await apiFetch(
        `/sandboxes/${encodeURIComponent(sandboxId)}/tasks/${encodeURIComponent(activeTask.id)}/cancel`,
        { method: 'POST' }
      );
      if (!res.ok) throw new Error(res?.data?.message || res?.data?.error || `Cancel failed (HTTP ${res.status})`);
      await fetchSandbox();
      await fetchTasks();
    } catch (e) {
      error = e.message || String(e);
    }
  }


  // Terminate action: open modal instead of prompt
  function terminateSandbox() { openTerminateModal(); }



  onMount(async () => {
    if (!isAuthenticated()) { goto('/login'); return; }
    $appOptions.appContentClass = 'p-3';
    // Use full-height content so the bottom row can flex to fill remaining space
    $appOptions.appContentFullHeight = true;
    try {
      await fetchSandbox();
      await fetchRuntime(true);
      await fetchTasks();
      loading = false;
      await tick();
      try { updateTopCardsHeight(); } catch (_) {}
      startPolling();
    } catch (e) {
      error = e.message || String(e);
      loading = false;
    }
  });
onDestroy(() => { stopPolling(); $appOptions.appContentClass = ''; $appOptions.appContentFullHeight = false; });
onDestroy(() => { fmRevokePreviewUrl(); });
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
            <div class="col-12">
              <label class="form-label" for="idle-timeout">Idle Timeout (seconds)</label>
              <input id="idle-timeout" type="number" min="0" step="1" class="form-control" bind:value={idleTimeoutInput} />
              <div class="form-text">Time of inactivity before sandbox is automatically terminated. Minimum 60 seconds, recommended 900 (15 minutes). Set to 0 to disable.</div>
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

<!-- Terminate Modal -->
{#if showTerminateModal}
  <div class="modal fade show" style="display: block; background: rgba(0,0,0,.3);" tabindex="-1" role="dialog" aria-modal="true">
    <div class="modal-dialog">
      <div class="modal-content">
        <div class="modal-header">
          <h5 class="modal-title">Terminate Sandbox</h5>
          <button type="button" class="btn-close" aria-label="Close" on:click={closeTerminateModal}></button>
        </div>
        <div class="modal-body">
          <p class="mb-2">Type <span class="fw-bold font-monospace">{sandbox?.id || sandboxId}</span> to confirm permanent termination.</p>
          <input class="form-control font-monospace" bind:value={terminateConfirm} placeholder={sandbox?.id || sandboxId} />
        </div>
        <div class="modal-footer">
          <button class="btn btn-outline-secondary" on:click={closeTerminateModal}>Cancel</button>
          <button class="btn btn-danger" disabled={!canConfirmTerminate} on:click={async () => {
            try {
              const cur = String(sandboxId || '').trim();
              const res = await apiFetch(`/sandboxes/${encodeURIComponent(cur)}`, { method: 'DELETE' });
              if (!res.ok) throw new Error(res?.data?.message || res?.data?.error || `Terminate failed (HTTP ${res.status})`);
              showTerminateModal = false;
              goto('/sandboxes');
            } catch (e) {
              alert(e.message || String(e));
            }
          }}>Terminate</button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Snapshot Modal -->
{#if showSnapshotModal}
  <div class="modal fade show" style="display: block; background: rgba(0,0,0,.3);" tabindex="-1" role="dialog" aria-modal="true">
    <div class="modal-dialog">
      <div class="modal-content">
        <div class="modal-header">
          <h5 class="modal-title">Create Snapshot</h5>
          <button type="button" class="btn-close" aria-label="Close" on:click={closeSnapshotModal}></button>
        </div>
        <div class="modal-body">
          {#if snapshotError}
            <div class="alert alert-danger small">{snapshotError}</div>
          {/if}
          <p class="mb-2">Create a snapshot of this sandbox's current state. You can use snapshots to create new sandboxes later.</p>
        </div>
        <div class="modal-footer">
          <button class="btn btn-outline-secondary" on:click={closeSnapshotModal}>Cancel</button>
          <button class="btn btn-theme" on:click={confirmCreateSnapshot}><i class="bi bi-camera me-1"></i>Create Snapshot</button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Two-row layout: top cards align via align-items-stretch; bottom panels flex-fill -->
<div class="d-flex flex-column h-100" style="min-height: 0;">
  <!-- Top row: equal heights via align-items-stretch and h-100 cards -->
  <div class="col-12">
    <div class="row g-3 align-items-stretch">
      <div class="col-12 col-lg-6">
        <Card class="h-100">
          <div class="card-body d-flex flex-column">
            <div class="d-flex align-items-center gap-2 mb-1">
              {#if sandbox}
                <a class="fw-bold text-decoration-none fs-22px font-monospace" href={'/sandboxes/' + encodeURIComponent(sandbox.id || '')}>{sandbox.id || '-'}</a>
              {:else}
                <div class="fw-bold fs-22px">Loading...</div>
              {/if}
            </div>
            <div class="small text-body text-opacity-75 flex-grow-1">{sandbox?.description || sandbox?.desc || 'No description'}</div>
            {#if isAdmin && sandbox}
              <div class="small text-body-secondary mt-1">Owner: <span class="font-monospace">{sandbox.created_by}</span></div>
            {/if}
            <!-- Public URL in main card -->
            
            <!-- Tags removed from detail page -->
            <!-- In-card actions (publish, stop, kebab) -->
            <div class="mt-2 d-flex align-items-center flex-wrap top-actions">
              <!-- Compact status indicator on the left -->
              <div class="d-flex align-items-center gap-2">
                {#if sandbox}
                  <i class={`${stateIconClass(sandbox.state || sandbox.status)} me-1`}></i>
                  <span class="text-uppercase small fw-bold text-body">{sandbox.state || sandbox.status || 'unknown'}</span>
                {/if}
              </div>
              <!-- Actions on the right (tight group) -->
              <div class="ms-auto d-flex align-items-center flex-wrap gap-2">
                {#if stateStr === 'idle' || stateStr === 'busy'}
                  <button class="btn btn-outline-danger btn-sm" on:click={terminateSandbox} aria-label="Terminate sandbox">
                    <i class="bi bi-power me-1"></i><span>Terminate</span>
                  </button>
                {/if}
                <div class="dropdown">
                  <button class="btn btn-outline-secondary btn-sm" type="button" data-bs-toggle="dropdown" aria-expanded="false" aria-label="More actions">
                    <i class="bi bi-three-dots"></i>
                  </button>
                  <ul class="dropdown-menu dropdown-menu-end">
                    {#if !['terminated','terminating','initializing'].includes(stateStr)}
                      <li><button class="dropdown-item" on:click={openEditTags}><i class="bi bi-tags me-2"></i>Edit Tags</button></li>
                      <li><button class="dropdown-item" on:click={openEditTimeouts}><i class="bi bi-hourglass-split me-2"></i>Edit Timeouts</button></li>
                      <li><hr class="dropdown-divider" /></li>
                    {/if}
                    <li><a class="dropdown-item" href="/snapshots?sandbox_id={sandbox?.id || sandboxId}"><i class="bi bi-images me-2"></i>View Snapshots</a></li>
                    {#if !['terminated','terminating','initializing'].includes(stateStr)}
                      <li><button class="dropdown-item" on:click={openSnapshotModal}><i class="bi bi-camera me-2"></i>Create Snapshot</button></li>
                    {/if}
                  </ul>
                </div>
              </div>
            </div>
          </div>
        </Card>
      </div>
      <div class="col-12 col-lg-6 d-none d-lg-block">
        {#if sandbox}
          <Card class="h-100">
            <div class="card-body small">
              <!-- Last Activity removed per design -->
              {#if sandbox.snapshot_id}
                <div class="mt-1">
                  Source Snapshot: <a href="/snapshots/{encodeURIComponent(sandbox.snapshot_id)}" class="font-monospace text-decoration-none">{sandbox.snapshot_id}</a>
                </div>
              {/if}
              <div class="mt-1">Idle Timeout: {fmtDuration(sandbox.idle_timeout_seconds)}</div>
              <div class="mt-1">Runtime: {fmtDuration(runtimeSeconds)}{#if currentSandboxSeconds > 0}&nbsp;(Current sandbox: {fmtDuration(currentSandboxSeconds)}){/if}</div>
              <div class="mt-2">
                <div class="d-flex align-items-center justify-content-between">
                  <div class="me-2">Context: {fmtInt(contextTokensUsed)} / {fmtInt(contextSoftLimit)} ({fmtPct(contextUsedPercent)})</div>
                </div>
                <div class="progress mt-1" role="progressbar" aria-valuenow={Number(contextUsedPercent)} aria-valuemin="0" aria-valuemax="100" style="height: 6px;">
                  <div class={`progress-bar ${Number(contextUsedPercent) >= 90 ? 'bg-danger' : 'bg-theme'}`} style={`width: ${Number(contextUsedPercent).toFixed(1)}%;`}></div>
                </div>
              </div>
            </div>
          </Card>
        {/if}
      </div>
    </div>
  </div>

  <!-- Delete File Modal -->
  {#if showDeleteFile}
    <div class="modal fade show" style="display: block; background: rgba(0,0,0,.3);" tabindex="-1" role="dialog" aria-modal="true">
      <div class="modal-dialog">
        <div class="modal-content">
          <div class="modal-header">
            <h5 class="modal-title">Delete File</h5>
            <button type="button" class="btn-close" aria-label="Close" on:click={closeDeleteFile}></button>
          </div>
          <div class="modal-body">
            {#if deleteFileError}
              <div class="alert alert-warning py-1 mb-2 small">{deleteFileError}</div>
            {/if}
            <div class="small">Are you sure you want to delete <span class="font-monospace">{(fmDeleteEntry && fmDeleteEntry.name) || '-'}</span>{#if fmDeleteEntry && fmDeleteEntry.kind === 'dir'} (folder){/if}?</div>
            <div class="text-body-secondary small mt-2">This action cannot be undone.</div>
          </div>
          <div class="modal-footer">
            <button class="btn btn-outline-secondary" on:click={closeDeleteFile}>Cancel</button>
            <button class="btn btn-danger" on:click={confirmDeleteFile}><i class="bi bi-trash me-1"></i>Delete</button>
          </div>
        </div>
      </div>
    </div>
  {/if}

  <!-- Bottom row: task and files panels -->
  <div class="row gx-3 flex-fill mt-3" style="min-height: 0; flex: 1 1 0;">
    <div class="col-12 col-lg-6 d-flex flex-column h-100" style="min-height: 0; min-width: 0;">
      <Card class="flex-fill d-flex flex-column task-pane" style="min-height: 0;">
        <div class="card-body p-0 d-flex flex-column flex-fill" style="min-height: 0;">
          <div class="flex-fill d-flex flex-column task-tracking" style="min-height: 0;">
            {#if showTaskDetail && selectedTask}
              <div class="task-detail flex-fill px-3 py-3 border-top" style="overflow-y: auto; min-height: 0;">
                <div class="d-flex flex-wrap align-items-center justify-content-between mb-3 gap-3">
                  <div class="d-flex align-items-start gap-3 flex-wrap">
                    <button class="btn btn-sm btn-outline-secondary" type="button" on:click={closeTaskDetail} aria-label="Back to task list">
                      <i class="bi bi-arrow-left-short me-1"></i>Back to task list
                    </button>
                    <div class="fw-semibold font-monospace">{selectedTask.id}</div>
                  </div>
                  <div class="d-flex align-items-center flex-wrap gap-3">
                    <span class={`badge ${taskStatusBadgeClass(selectedTask)}`}>{taskStatusLabel(selectedTask)}</span>
                  </div>
                </div>
                <section class="mb-3">
                  <h6 class="fw-semibold fs-6 mb-2">Input Prompt</h6>
                  {#if taskInputItems(selectedTask).length}
                    {#each taskInputItems(selectedTask) as item}
                      {#if String(item?.type || '').toLowerCase() === 'text'}
                        <div class="markdown-body mb-2">{@html renderMarkdown(item.content || '')}</div>
                      {:else}
                        <pre class="small bg-dark text-white p-2 rounded code-wrap mb-2"><code>{JSON.stringify(item, null, 2)}</code></pre>
                      {/if}
                    {/each}
                  {:else}
                    <div class="small text-body-secondary">No input recorded.</div>
                  {/if}
                </section>
                {#if taskAnalysisSegments(selectedTask).length}
                  <section class="mb-3">
                    <h6 class="fw-semibold fs-6 mb-2">Analysis</h6>
                    {#each taskAnalysisSegments(selectedTask) as seg}
                      <div class="small fst-italic text-body text-opacity-75 mb-2" style="white-space: pre-wrap;">{segText(seg)}</div>
                    {/each}
                  </section>
                {/if}
                <section class="mb-3">
                  <h6 class="fw-semibold fs-6 mb-2">Tool Calls</h6>
                  {#if taskToolPairs(selectedTask).length}
                    {#each taskToolPairs(selectedTask) as pair}
                      <details class="tool-call mb-2">
                        <summary class="d-flex align-items-center gap-2">
                          <span class="badge bg-secondary-subtle text-secondary-emphasis border text-uppercase">{pair.call ? segTool(pair.call) : ''}</span>
                          <span class="small text-body-secondary flex-grow-1">{formatToolSummary(pair.call)}</span>
                        </summary>
                        <div class="ps-4 mt-2">
                          {#if pair.commentary && pair.commentary.length}
                            <div class="small text-body text-opacity-75 mb-2">
                              {#each pair.commentary as seg}
                                <div class="mb-1" style="white-space: pre-wrap;">{segText(seg)}</div>
                              {/each}
                            </div>
                          {/if}
                          {#if pair.call}
                            <div class="small text-body text-opacity-75 mb-1">Command</div>
                            <pre class="small bg-dark text-white p-2 rounded code-wrap mb-2"><code>{JSON.stringify(segArgs(pair.call) || {}, null, 2)}</code></pre>
                          {/if}
                          {#if pair.result}
                            <div class="small text-body text-opacity-75 mb-1">Result</div>
                            <pre class="small bg-dark text-white p-2 rounded code-wrap mb-0"><code>{JSON.stringify(segOutput(pair.result), null, 2)}</code></pre>
                          {/if}
                        </div>
                      </details>
                    {/each}
                  {:else}
                    <div class="small text-body-secondary">No tool calls recorded.</div>
                  {/if}
                </section>
                <section>
                  <h6 class="fw-semibold fs-6 mb-2">Output</h6>
                  {#if taskOutputItems(selectedTask).length}
                    {#each taskOutputItems(selectedTask) as item}
                      {#if String(item?.type || '').toLowerCase() === 'markdown'}
                        <div class="markdown-body mb-3">{@html renderMarkdown(item.content || '')}</div>
                      {:else if String(item?.type || '').toLowerCase() === 'json'}
                        <pre class="small bg-dark text-white p-2 rounded code-wrap mb-3"><code>{JSON.stringify(item.content, null, 2)}</code></pre>
                      {:else if String(item?.type || '').toLowerCase() === 'url'}
                        <a class="d-inline-flex align-items-center gap-2 mb-2" href={item.content} target="_blank" rel="noopener noreferrer">
                          <i class="bi bi-box-arrow-up-right"></i>{item.content}
                        </a>
                      {:else}
                        <pre class="small bg-dark text-white p-2 rounded code-wrap mb-3"><code>{JSON.stringify(item, null, 2)}</code></pre>
                      {/if}
                    {/each}
                  {:else}
                    <div class="small text-body-secondary">No output available yet.</div>
                  {/if}
                </section>
              </div>
            {:else}
              <div class="px-3 py-2 border-bottom d-flex align-items-center justify-content-between">
                <h6 class="mb-0 small text-uppercase text-body-secondary">Task List</h6>
                <div class="small text-body-secondary">Total: {tasks.length}</div>
              </div>
              <div class="task-list px-3 py-2 flex-grow-1" style="overflow-y: auto;">
                {#if loading}
                  <div class="d-flex align-items-center gap-2 text-body text-opacity-75 small">
                    <span class="spinner-border spinner-border-sm" role="status" aria-hidden="true"></span>
                    Loading tasks…
                  </div>
                {:else if tasks.length}
                  <div class="list-group list-group-flush">
                    {#each tasks as task}
                      <button
                        type="button"
                        class={`list-group-item list-group-item-action d-flex align-items-start justify-content-between gap-2 ${selectedTask && selectedTask.id === task.id ? 'active' : ''}`}
                        on:click={() => openTaskDetail(task.id)}
                      >
                        <div class="text-start">
                          <div class="fw-semibold font-monospace">{task.id}</div>
                          <div class="small text-body-secondary">{formatTaskTimestamp(task)}</div>
                          {#if taskPreview(task)}
                            <div class="small text-body text-opacity-75 text-truncate">{taskPreview(task)}</div>
                          {/if}
                        </div>
                        <span class={`badge ${taskStatusBadgeClass(task)}`}>{taskStatusLabel(task)}</span>
                      </button>
                    {/each}
                  </div>
                {:else}
                  <div class="text-body text-opacity-75 small">No tasks yet.</div>
                {/if}
              </div>
            {/if}
          </div>
          <div class="px-3 pt-3 pb-3 border-top">
            {#if error}
              <div class="alert alert-danger small mb-3">{error}</div>
            {/if}
            <form class="task-form" on:submit|preventDefault={createTask}>
              <div class="input-group task-input-group rounded-0 shadow-none">
                <textarea
                  aria-label="Task instructions"
                  class="form-control shadow-none task-input"
                  disabled={sending || stateStr === 'busy' || stateStr === 'terminating' || stateStr === 'terminated' || stateStr === 'initializing'}
                  placeholder="Post a task to the sandbox…"
                  rows="3"
                  style="resize: none;"
                  bind:this={inputEl}
                  bind:value={input}
                  on:keydown={(e)=>{
                    if (e.key === 'Enter' && !e.shiftKey && !e.altKey && !e.ctrlKey && !e.metaKey) {
                      e.preventDefault();
                      createTask();
                    }
                  }}
                  on:input={(e)=>{ try { if (!e.target.value || !e.target.value.trim()) { e.target.style.height=''; return; } e.target.style.height='auto'; e.target.style.height = Math.min(e.target.scrollHeight, 200) + 'px'; } catch(_){} }}
                ></textarea>
                {#if stateStr === 'busy'}
                  <button type="button" class="btn btn-outline-danger task-action-btn" aria-label="Cancel active task" on:click={cancelActive}>
                    <i class="bi bi-stop-circle"></i>
                  </button>
                {:else}
                  <button class="btn btn-theme task-action-btn" aria-label="Create task" disabled={sending || !input.trim() || stateStr === 'terminated' || stateStr === 'terminating' || stateStr === 'initializing'}>
                    {#if sending}
                      <span class="spinner-border spinner-border-sm" role="status" aria-hidden="true"></span>
                    {:else}
                      <i class="bi bi-plus-circle"></i>
                    {/if}
                  </button>
                {/if}
              </div>
            </form>
          </div>
        </div>
      </Card>
    </div>
    <div class="col-12 col-lg-6 d-none d-lg-flex flex-column h-100" style="min-height: 0; min-width: 0;">
        <!-- Content (Files) side panel -->
        <Card class="flex-fill d-flex flex-column files-pane" style="min-height: 0;">
          {#if stateStr === 'terminated'}
            <div class="card-body p-0 d-flex flex-column flex-fill" style="min-height: 0;">
              <div class="flex-fill d-flex align-items-center justify-content-center p-3">
                <div class="text-center text-body text-opacity-75">
                  <div class="fs-5 mb-2"><i class="bi bi-power me-2"></i>Sandbox not available</div>
                  <p class="small mb-3 text-body-secondary">This sandbox has been terminated and is read-only.</p>
                </div>
              </div>
            </div>
          {:else if stateStr === 'terminating'}
            <div class="card-body p-0 d-flex flex-column flex-fill" style="min-height: 0;">
              <div class="flex-fill d-flex align-items-center justify-content-center p-3">
                <div class="text-center text-body text-opacity-75">
                  <div class="fs-5 mb-2"><span class="spinner-border spinner-border-sm me-2 overlay-spin text-danger"></span>Sandbox not available</div>
                  <p class="small mb-3 text-body-secondary">Termination is in progress. Please wait for the sandbox to shut down.</p>
                </div>
              </div>
            </div>
          {:else if stateStr === 'initializing'}
            <div class="card-body p-0 d-flex flex-column flex-fill" style="min-height: 0;">
              <div class="flex-fill d-flex align-items-center justify-content-center p-3">
                <div class="text-center text-body text-opacity-75">
                  <div class="fs-5 mb-2"><span class="spinner-border spinner-border-sm me-2 overlay-spin text-info"></span>Sandbox is initializing</div>
                </div>
              </div>
            </div>
          {:else}
            <div class="card-body p-0 d-flex flex-column flex-fill" style="min-height: 0;">
              <!-- Action bar (read-only) -->
              <div class="d-flex flex-wrap align-items-center gap-1 border-bottom px-2 py-1 small">
                <button class="btn btn-sm border-0" aria-label="Root" title="Root" on:click={fmGoRoot}><i class="bi bi-house"></i></button>
                <button class="btn btn-sm border-0" aria-label="Up" title="Up" on:click={fmGoUp} disabled={(fmSegments.length === 0 && !fmPreviewName)}><i class="bi bi-arrow-90deg-up"></i></button>
                <span class="vr mx-1"></span>
                <!-- Path on the left with spacing and an icon -->
                <div class="small text-body text-opacity-75 ms-2">
                  {#if fmPreviewName}
                    <i class="bi bi-file-earmark me-1"></i>
                  {:else}
                    <i class="bi bi-folder me-1"></i>
                  {/if}
                  {currentFullPath}
                </div>

                <div class="ms-auto d-flex align-items-center gap-2">
                  {#if fmLoading}
                    <span class="spinner-border spinner-border-sm text-body text-opacity-75" role="status" aria-label="Loading"></span>
                  {/if}
                  <button class="btn btn-sm border-0" aria-label="Refresh files" title="Refresh files" on:click={fmRefresh}>
                    <i class="bi bi-arrow-clockwise"></i>
                  </button>
                  <div class="form-check form-switch form-switch-sm m-0 d-flex align-items-center text-body-secondary">
                    <input
                      class="form-check-input"
                      type="checkbox"
                      id="files-auto-refresh"
                      bind:checked={fmAutoRefresh}
                      on:change={fmRestartAutoRefresh}
                      aria-label="Toggle auto-refresh"
                    />
                    <label class="form-check-label ms-1" for="files-auto-refresh">Auto</label>
                  </div>
                  <!-- Move items count and download/delete to the right side -->
                  {#if fmPreviewName}
                    <span class="vr"></span>
                    <button class="btn btn-sm border-0" aria-label="Download" title="Download" on:click={() => fmDownloadEntry({ name: fmPreviewName, kind: 'file' })}><i class="bi bi-download"></i></button>
                  {:else}
                    <span class="vr"></span>
                    <div class="small text-body text-opacity-75">
                      {#if Number(fmTotal || 0) > Number((fmEntries && fmEntries.length) || 0)}
                        {fmtInt((fmEntries && fmEntries.length) || 0)} of {fmtInt(fmTotal)} items
                      {:else}
                        {fmtInt(fmTotal || ((fmEntries && fmEntries.length) || 0))} items
                      {/if}
                    </div>
                  {/if}
                </div>
              </div>
              <!-- List + Details scroll region -->
              {#if fmErrorNotAvailable}
                <div class="flex-fill d-flex align-items-center justify-content-center p-3">
                  <div class="text-center text-body text-opacity-75">
                    <div class="fs-5 mb-2"><i class="bi bi-power me-2"></i>Sandbox not available</div>
                    <p class="small mb-3 text-body-secondary">This sandbox is currently unavailable. Please wait and try again once it finishes starting.</p>
                  </div>
                </div>
              {:else}
                {#if fmError}
                  <div class="alert alert-danger small m-2 py-1">{fmError}</div>
                {/if}
                <div class="d-flex flex-column flex-fill" style="min-height: 0;">
                  {#key fmListKey}
                  <PerfectScrollbar class="flex-fill">
                    {#if fmPreviewName}
                      {#if fmPreviewError}
                        <div class="p-3 small text-danger">{fmPreviewError}</div>
                      {/if}
                      {#if fmPreviewType === 'image' && fmPreviewUrl}
                        <div class="p-3"><img src={fmPreviewUrl} alt={fmPreviewName} class="img-fluid rounded border" /></div>
                      {:else if fmPreviewType === 'text'}
                        <div class="p-3"><pre class="preview-code mb-0">{fmPreviewText}</pre></div>
                      {:else if fmPreviewType === 'binary'}
                        <div class="p-3 small text-body text-opacity-75">Binary file</div>
                      {:else if !fmPreviewError}
                        <!-- Keep showing previous content until new content is ready; no loading indicator here. -->
                      {/if}
                    {:else}
                      {#if !fmEntries || fmEntries.length === 0}
                        {#if !fmLoading}
                          <div class="p-3 small text-body text-opacity-75">Empty</div>
                        {/if}
                      {:else}
                        <div class="list-group list-group-flush">
                          {#each fmEntries as e}
                            <div class="list-group-item d-flex align-items-center">
                              <button type="button" class="btn btn-link text-reset text-decoration-none text-start flex-grow-1 d-flex align-items-center p-0 file-entry-btn"
                              on:click={() => fmOpen(e)}
                              title={`${e.name} • ${e.kind} • ${e.mtime}`}
                            >
                              <i class={`${fmIconFor(e)} me-2`}></i>
                              <span class="text-truncate">{e.name}</span>
                              </button>
                              {#if String(e?.kind || '').toLowerCase() !== 'dir' && String(e?.kind || '').toLowerCase() !== 'directory'}
                                <button class="btn btn-sm btn-link text-body ms-2 p-0" title="Download" aria-label="Download" on:click|stopPropagation={() => fmDownloadEntry(e)}>
                                  <i class="bi bi-download"></i>
                                </button>
                              {/if}
                              <button class="btn btn-sm btn-link text-danger ms-2 p-0" title="Delete" aria-label="Delete" on:click|stopPropagation={() => openDeleteEntry(e)}>
                                <i class="bi bi-trash"></i>
                              </button>
                            </div>
                          {/each}
                        </div>
                        {#if fmNextOffset != null}
                          <div class="border-top p-2 d-flex align-items-center justify-content-center">
                            <button class="btn btn-sm btn-outline-secondary" on:click={fmLoadMore} disabled={fmLoading}>
                              {#if fmLoading}<span class="spinner-border spinner-border-sm me-2"></span>{/if}
                              Load more
                            </button>
                          </div>
                        {/if}
                      {/if}
                    {/if}
                  </PerfectScrollbar>
                  {/key}
                  <!-- Details & Preview bottom pane (fixed height) -->
                  <!-- Preview/Details bottom pane -->
                  <!-- No separate details pane; counts are shown in the action bar -->
                </div>
              {/if}
            </div>
          {/if}
        </Card>
      </div>
    </div>
  </div>

  <style>
  .muted-card {
    border: 1px solid var(--bs-border-color-translucent, rgba(0, 0, 0, 0.08));
    background-color: var(--bs-body-bg);
    opacity: 0.94;
  }
  .muted-card .badge {
    opacity: 0.85;
  }
  .muted-card .state-label {
    color: var(--bs-secondary-color) !important;
    font-weight: 600;
  }

  :global(.top-actions) { position: relative; z-index: 3001; }
  :global(.top-actions .dropdown-menu) { z-index: 3002 !important; }
  :global(.modal) { z-index: 2000; }
  :global(.modal-backdrop) { z-index: 1990; }
  :global(.card) { overflow: visible; }

  /* Task submission layout */
  :global(.task-pane) { position: relative; z-index: 1; }
  :global(.task-pane .task-input) {
    border: 1px solid var(--bs-border-color);
    background: var(--bs-body-bg);
    border-radius: 0;
  }
  :global(.task-pane .task-input:focus) {
    box-shadow: none;
    border-color: var(--bs-theme);
  }
  :global(.task-pane .task-action-btn) {
    width: 3rem;
    min-width: 3rem;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  :global(.task-pane .task-list .list-group-item) { cursor: pointer; }
  :global(.task-pane .task-list .list-group-item.active) {
    background-color: rgba(var(--bs-theme-rgb), .12);
    border-color: rgba(var(--bs-theme-rgb), .35);
    color: inherit;
  }
  :global(.task-pane .task-list .list-group-item.active .badge) {
    background-color: var(--bs-theme);
    color: var(--bs-theme-color);
  }
  :global(.task-pane .task-detail section + section) {
    border-top: 1px solid rgba(var(--bs-border-color-rgb), .4);
    padding-top: 1rem;
    margin-top: 1rem;
  }
  :global(.task-pane .task-detail h6) {
    font-size: 0.9rem;
    text-transform: uppercase;
    letter-spacing: 0.02em;
    color: var(--bs-body-secondary);
  }
  :global(.task-pane .markdown-body) { white-space: normal; }
  :global(.task-pane .markdown-body pre) {
    background: #0d1117;
    color: #e6edf3;
    padding: 0.5rem;
    border-radius: 0.25rem;
  }
  :global(.task-pane .markdown-body pre code) {
    background: transparent !important;
    color: inherit;
    padding: 0;
    border-radius: 0;
  }
  :global(.task-pane .markdown-body code) {
    background: rgba(0,0,0,0.06);
    padding: 0.1rem 0.25rem;
    border-radius: 0.2rem;
  }
  :global(.task-pane .markdown-body ul) { padding-left: 1.25rem; }
  :global(.task-pane .markdown-body li) { margin: 0.125rem 0; }

  :global(.markdown-body) { white-space: normal; }
  :global(.markdown-body p) { margin-bottom: 0.5rem; }
  :global(.markdown-body table) { width: 100%; border-collapse: collapse; margin: 0.5rem 0; }
  :global(.markdown-body th), :global(.markdown-body td) { border: 1px solid var(--bs-border-color); padding: 0.375rem 0.5rem; }
  :global(.markdown-body thead th) { background: var(--bs-light); }
  :global(.markdown-wrap) { border: 1px solid var(--bs-border-color); border-radius: 0.5rem; padding: 0.5rem 0.75rem; background: var(--bs-body-bg); }
  :global(pre.code-wrap) { white-space: pre-wrap; word-break: break-word; overflow-wrap: anywhere; }

  :global(.files-pane),
  :global(.files-pane *),
  :global(.files-pane *::before),
  :global(.files-pane *::after) {
      transition: none !important;
      animation: none !important;
      scroll-behavior: auto !important;
  }
  :global(.files-pane .border-bottom .vr) {
      align-self: center;
      height: 1.25rem;
  }
  :global(.files-pane .spinner-border) {
      animation: .75s linear infinite spinner-border !important;
  }
  :global(.files-pane .spinner-grow) {
      animation: .75s linear infinite spinner-grow !important;
  }
  :global(.files-pane .overlay-spin) {
      animation: .75s linear infinite spinner-border !important;
  }
  :global(.file-entry-btn),
  :global(.file-entry-btn:hover),
  :global(.file-entry-btn:focus) {
      color: var(--bs-body-color) !important;
      text-decoration: none !important;
  }

  :global(.preview-code) {
      font-family: var(--bs-font-monospace, ui-monospace, SFMono-Regular, Menlo, Consolas, "Liberation Mono", monospace);
      font-size: calc(var(--bs-body-font-size, 1rem) * .8);
      line-height: 1.4;
      font-weight: 300;
      white-space: pre-wrap;
      word-break: break-word;
      overflow-wrap: anywhere;
      background: transparent !important;
      color: var(--bs-body-color);
      border: 0;
      padding: 0;
  }

  @media (max-width: 576px) {
    :global(.task-pane .task-input) { font-size: 16px; }
  }
</style>
