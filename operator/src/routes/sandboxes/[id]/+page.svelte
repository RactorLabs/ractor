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
  $: sandboxInferenceModel =
    sandbox?.inference_model ||
    $page?.data?.globalStats?.default_inference_model ||
    '';
  let stateStr = '';
  // Task tracking state
  let tasks = [];
  let selectedTaskId = '';
  let selectedTaskSummary = null;
  let selectedTaskDetail = null;
  let showTaskDetail = false;
  let detailLoading = false;
  let detailError = '';
  const taskDetailCache = new Map();
  let expandedSteps = {};
  $: selectedTaskSummary = tasks.find((t) => t.id === selectedTaskId) || null;
  $: {
    if (showTaskDetail && selectedTaskId && !selectedTaskSummary && !selectedTaskDetail) {
      showTaskDetail = false;
    }
  }
  let displayTask = null;
  $: displayTask = selectedTaskDetail || selectedTaskSummary;
  let currentStepGroups = [];
  $: currentStepGroups = selectedTaskDetail ? groupedTaskSteps(selectedTaskDetail) : [];
  let taskOutputItemsList = [];
  let hasTaskOutputText = false;
  let hasAnyTaskOutput = false;
  $: taskOutputItemsList = selectedTaskDetail ? taskOutputItems(selectedTaskDetail) : [];
  $: hasTaskOutputText = selectedTaskDetail ? hasOutputText(selectedTaskDetail) : false;
  $: hasAnyTaskOutput = hasTaskOutputText || taskOutputItemsList.length > 0;
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
  // First onMount merged into second onMount below (line ~1175)
  let loading = true;
  let error = null;
  let input = '';
  let sending = false;
  const taskTypeOptions = [
    { value: 'NL', label: 'Natural Language', short: 'NL', description: 'Use inference to complete multi-step instructions.' },
    { value: 'SH', label: 'Shell', short: 'SH', description: 'Run the prompt as a /bin/sh command.' },
    { value: 'PY', label: 'Python', short: 'PY', description: 'Execute the prompt with python3 -c.' },
    { value: 'JS', label: 'JavaScript', short: 'JS', description: 'Execute the prompt via node -e.' }
  ];
  let taskType = 'NL';
  function taskTypeLabel(code) {
    if (!code || typeof code !== 'string') return 'Natural Language';
    const upper = code.toUpperCase();
    const found = taskTypeOptions.find((opt) => opt.value === upper);
    return found ? found.label : 'Natural Language';
  }
  function taskTypeShort(code) {
    if (!code || typeof code !== 'string') return 'NL';
    const upper = code.toUpperCase();
    const found = taskTypeOptions.find((opt) => opt.value === upper);
    return found?.short || upper;
  }
  function taskTypeClass(code) {
    if (!code || typeof code !== 'string') return 'task-type-nl';
    const upper = code.toUpperCase();
    return `task-type-${upper.toLowerCase()}`;
  }
  let pollHandle = null;
  let runtimeSeconds = 0;
  let idleDurationLabel = '';
  let topData = null;
  let statsInitialized = false;
  let statsLoading = true;
  $: taskInputDisabled =
    stateStr === 'terminating' ||
    stateStr === 'terminated';
  $: idleDurationLabel = computeIdleDuration(sandbox?.idle_from);
  $: toolUsageEntries = (() => {
    try {
      if (!topData || !topData.tool_count) return [];
      const obj = topData.tool_count;
      if (typeof obj !== 'object' || obj === null) return [];
      return Object.entries(obj)
        .map(([name, value]) => [name, Number(value) || 0])
        .filter(([tool, count]) => count > 0 && !isOutputToolName(tool))
        .sort((a, b) => b[1] - a[1]);
    } catch (_) {
      return [];
    }
  })();
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
  function fmtDuration(seconds) {
    try {
      const s = Number(seconds);
      if (!Number.isFinite(s) || s < 0) return '-';
      if (s < 60) return `${Math.floor(s)}s`;
      if (s < 3600) return `${Math.floor(s / 60)}m ${Math.floor(s % 60)}s`;
      const h = Math.floor(s / 3600);
      const m = Math.floor((s % 3600) / 60);
      return `${h}h ${m}m`;
    } catch (_) { return '-'; }
  }
  function fmtBytes(bytes) {
    try {
      const b = Number(bytes);
      if (!Number.isFinite(b) || b < 0) return '-';
      if (b < 1024) return `${b} B`;
      if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
      if (b < 1024 * 1024 * 1024) return `${(b / (1024 * 1024)).toFixed(1)} MB`;
      return `${(b / (1024 * 1024 * 1024)).toFixed(2)} GB`;
    } catch (_) { return '-'; }
  }
  function fmtPercent(value) {
    try {
      const v = Number(value);
      return Number.isFinite(v) ? `${v.toFixed(1)}%` : '-';
    } catch (_) { return '-'; }
  }
  function computeIdleDuration(idleFrom) {
    try {
      if (!idleFrom) return '';
      const start = new Date(idleFrom);
      if (Number.isNaN(start.getTime())) return '';
      const now = new Date();
      const diff = Math.floor((now.getTime() - start.getTime()) / 1000);
      if (!Number.isFinite(diff) || diff <= 0) return '';
      return fmtDuration(diff);
    } catch (_) { return ''; }
  }
  // File list kind counters for folder details
  function countKind(kind) {
    try { const k = String(kind || '').toLowerCase(); return (fmEntries || []).filter(e => String(e?.kind || '').toLowerCase() === k).length; } catch (_) { return 0; }
  }
  function countFiles() { return countKind('file'); }
  function countDirs() { try { return (fmEntries || []).filter(e => { const k = String(e?.kind || '').toLowerCase(); return k === 'dir' || k === 'directory'; }).length; } catch (_) { return 0; } }
  function countSymlinks() { return countKind('symlink'); }
  $: context_length = (() => {
    const detail = displayTask;
    if (detail && detail.context_length != null) {
      const value = Number(detail.context_length);
      return Number.isFinite(value) ? value : 0;
    }
    if (selectedTaskSummary && selectedTaskSummary.context_length != null) {
      const value = Number(selectedTaskSummary.context_length);
      return Number.isFinite(value) ? value : 0;
    }
    const latest = tasks && tasks.length ? tasks[tasks.length - 1] : null;
    if (latest && latest.context_length != null) {
      const value = Number(latest.context_length);
      return Number.isFinite(value) ? value : 0;
    }
    return 0;
  })();
  let _statsFetchedAt = 0;
  let inputEl = null; // task textarea element
  let taskListEl = null; // task list container element
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
  function openTerminateModal() { showTerminateModal = true; }
  function closeTerminateModal() { showTerminateModal = false; }
  async function confirmTerminateSandbox() {
    try {
      const cur = String(sandboxId || '').trim();
      const res = await apiFetch(`/sandboxes/${encodeURIComponent(cur)}`, { method: 'DELETE' });
      if (!res.ok) throw new Error(res?.data?.message || res?.data?.error || `Terminate failed (HTTP ${res.status})`);
      showTerminateModal = false;
      if (sandbox) {
        sandbox = { ...sandbox, state: 'terminating' };
      }
      // Refresh details so the UI shows updated state/tasks
      await fetchSandbox();
    } catch (e) {
      alert(e.message || String(e));
    }
  }

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

  async function fetchStats(force = false) {
    try {
      if (!force && Date.now() - _statsFetchedAt < 1000) return;
      if (!statsInitialized) statsLoading = true;
      const res = await apiFetch(`/sandboxes/${encodeURIComponent(sandboxId)}/stats`);
      if (res.ok && res.data) {
        topData = res.data;
        const runtime = Number(res.data?.runtime_seconds ?? 0);
        if (Number.isFinite(runtime) && runtime >= 0) runtimeSeconds = runtime;
        _statsFetchedAt = Date.now();
        statsInitialized = true;
        statsLoading = false;
      } else {
        if (!statsInitialized) statsLoading = false;
      }
    } catch (_) {
      if (!statsInitialized) statsLoading = false;
    }
  }

  async function loadTaskDetail(taskId, { force = false, showSpinner = true } = {}) {
    if (!taskId) {
      if (showSpinner) {
        detailLoading = false;
      }
      detailError = '';
      selectedTaskDetail = null;
      return;
    }
    if (!force && taskDetailCache.has(taskId)) {
      if (selectedTaskId === taskId) {
        selectedTaskDetail = taskDetailCache.get(taskId);
        detailError = '';
      }
      return;
    }
    if (showSpinner && selectedTaskId === taskId) {
      detailLoading = true;
      detailError = '';
    }
    try {
      const res = await apiFetch(`/sandboxes/${encodeURIComponent(sandboxId)}/tasks/${encodeURIComponent(taskId)}`);
      if (!res.ok) {
        throw new Error(res?.data?.message || res?.data?.error || `Failed to fetch task ${res.status}`);
      }
      taskDetailCache.set(taskId, res.data);
      if (selectedTaskId === taskId) {
        selectedTaskDetail = res.data;
        detailError = '';
      }
    } catch (e) {
      if (selectedTaskId === taskId) {
        detailError = e?.message || String(e);
        selectedTaskDetail = null;
      }
    } finally {
      if (showSpinner && selectedTaskId === taskId) {
        detailLoading = false;
      }
    }
  }

  async function fetchTasks() {
    const res = await apiFetch(`/sandboxes/${encodeURIComponent(sandboxId)}/tasks?limit=200`);
    if (!res.ok) return;
    const list = Array.isArray(res.data) ? res.data : (res.data?.tasks || []);
    tasks = list;

    if (selectedTaskId && !tasks.some((t) => t.id === selectedTaskId)) {
      selectedTaskId = '';
      selectedTaskDetail = null;
      detailError = '';
    }

    if (!selectedTaskId && tasks.length) {
      selectedTaskId = tasks[0].id;
    }

    if (!tasks.length) {
      showTaskDetail = false;
      selectedTaskDetail = null;
      detailError = '';
      return;
    }

    const summary = tasks.find((t) => t.id === selectedTaskId);
    if (!summary) {
      return;
    }

    const cached = taskDetailCache.get(summary.id);
    const needsRefresh = !cached || cached.updated_at !== summary.updated_at;
    if (showTaskDetail && selectedTaskId === summary.id) {
      await loadTaskDetail(summary.id, { force: needsRefresh, showSpinner: !cached });
    } else if (needsRefresh) {
      await loadTaskDetail(summary.id, { force: true, showSpinner: false });
    } else if (cached && selectedTaskId === summary.id) {
      selectedTaskDetail = cached;
    }
  }

  function startPolling() {
    stopPolling();
    pollHandle = setInterval(async () => {
      await fetchTasks();
      await fetchSandbox();
      await fetchStats();
    }, 1000);
  }
  function stopPolling() { if (pollHandle) { clearInterval(pollHandle); pollHandle = null; } }

  // No content preview probing; only status is shown in the panel.

  function segType(s) { return String(s?.type || '').toLowerCase(); }
  function segText(s) { return String(s?.text || ''); }
  function segTool(s) { return String(s?.tool || ''); }
  function segArgs(s) {
    try {
      if (!s || typeof s !== 'object') return null;
      const raw = s.arguments !== undefined ? s.arguments : s.args;
      if (!raw) return null;
      if (typeof raw === 'object') return raw;
      if (typeof raw === 'string') {
        const trimmed = raw.trim();
        if (!trimmed) return null;
        try {
          return JSON.parse(trimmed);
        } catch (_) {
          return { value: trimmed };
        }
      }
      return null;
    } catch (_) { return null; }
  }
  function segOutput(s) {
    try {
      const payload = s && Object.prototype.hasOwnProperty.call(s, 'result') ? s.result : s?.output;
      if (typeof payload === 'string') return payload;
      return payload;
    } catch (_) { return s?.result ?? s?.output; }
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
    if (status === 'queued') return 'Queued';
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
    if (status === 'queued') return 'bg-warning-subtle text-warning-emphasis border';
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
      const items = Array.isArray(task?.input) ? task.input : [];
      if (items.length) return items;
      const text = task?.input?.text || task?.input?.content;
      if (typeof text === 'string' && text.trim()) {
        return [{ type: 'text', content: text }];
      }
      return [];
    } catch (_) { return []; }
  }
  function taskSteps(task) {
    try {
      if (Array.isArray(task?.steps)) return task.steps;
      return [];
    } catch (_) { return []; }
  }
  function isAnalysisSegment(seg) {
    const type = segType(seg);
    const channel = String(seg?.channel || '').toLowerCase();
    return type === 'commentary' || channel === 'analysis' || channel === 'commentary';
  }

  function taskAnalysisSegments(task) {
    const segs = taskSteps(task);
    return segs.filter((s) => isAnalysisSegment(s));
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
  function normalizeOutputItem(item) {
    try {
      if (!item || typeof item !== 'object') return null;
      const copy = { ...item };
      const typ = String(copy.type || '').toLowerCase();
      if (typ === 'markdown') copy.type = 'md';
      else if (typ === 'md' || typ === 'json' || typ === 'text' || typ === 'stdout' || typ === 'stderr' || typ === 'exit_code' || typ === 'commentary') copy.type = typ;
      else copy.type = 'text';
      if (copy.type === 'json' && copy.content === undefined) {
        copy.content = null;
      }
      if (copy.type !== 'json') {
        const raw = copy.content;
        if (typeof raw === 'string') {
          copy.content = raw;
        } else if (raw === undefined || raw === null) {
          copy.content = '';
        } else {
          copy.content = String(raw);
        }
      }
      return copy;
    } catch (_) {
      return null;
    }
  }

  function taskOutputItems(task) {
    const convert = (arr) =>
      arr
        .map((item) => normalizeOutputItem(item))
        .filter((item) => item && item.content !== undefined);

    // New format: { items: [...], commentary: "..." }
    if (task?.output && typeof task.output === 'object' && Array.isArray(task.output.items)) {
      return convert(task.output.items);
    }

    // Direct array (backwards compatibility)
    if (Array.isArray(task?.output)) {
      return convert(task.output);
    }

    // Legacy format
    const legacy = task?.output;
    const items = [];
    if (legacy && typeof legacy === 'object') {
      if (typeof legacy.text === 'string' && legacy.text.trim()) {
        items.push({ type: 'md', content: legacy.text.trim() });
      }
      if (Array.isArray(legacy.content)) {
        items.push(...legacy.content);
      }
      if (Array.isArray(legacy.items)) {
        items.push(...legacy.items);
      }
    }
    return convert(items);
  }
  function outputItemType(item) {
    try { return String(item?.type || '').toLowerCase(); } catch (_) { return ''; }
  }
  function hasOutputText(task) {
    try {
      const text = task?.output?.text;
      if (typeof text !== 'string') return false;
      return text.trim().length > 0;
    } catch (_) {
      return false;
    }
  }
  function stepType(step) {
    const t = String(step?.type || '').toLowerCase();
    if (t) return t;
    const channel = String(step?.channel || '').toLowerCase();
    if (channel === 'analysis' || channel === 'commentary') return 'analysis';
    return t || 'step';
  }
  function stepLabel(step) {
    const t = stepType(step);
    if (t === 'tool_call' || t === 'tool_result') {
      return `${t.replace('_', ' ')} (${segTool(step) || 'unknown'})`;
    }
    if (t === 'analysis') return 'Analysis';
    if (t === 'final') return 'Final';
    if (t === 'cancelled') return 'Cancelled';
    return t.charAt(0).toUpperCase() + t.slice(1);
  }
  function stepBadgeClass(step) {
    const t = stepType(step);
    if (t === 'tool_call' || t === 'tool_result') return 'bg-info-subtle text-info-emphasis border';
    if (t === 'analysis') return 'bg-secondary-subtle text-secondary-emphasis border';
    if (t === 'final') return 'bg-success-subtle text-success-emphasis border';
    if (t === 'cancelled') return 'bg-danger-subtle text-danger-emphasis border';
    return 'bg-secondary-subtle text-secondary-emphasis border';
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

  function groupedTaskSteps(task) {
    try {
      const steps = taskSteps(task);
      const groups = [];
      let current = null;
      steps.forEach((step, idx) => {
        const type = stepType(step);
        if (type === 'final') {
          return;
        }
        if (type === 'tool_call') {
          if (current) {
            groups.push(current);
          }
          current = { index: idx, call: step, result: null, extras: [] };
        } else if (type === 'tool_result' && current && !current.result) {
          current.result = step;
        } else {
          if (current) {
            current.extras.push(step);
          } else {
            groups.push({ index: idx, call: null, result: null, extras: [step] });
          }
        }
      });
      if (current) {
        groups.push(current);
      }
      const mapped = groups.map((group, groupIdx) => {
        const input = group.call ? segArgs(group.call) : null;
        const inputCommentary = commentaryFromArgs(input);
        const fallbackCommentary = computeGroupCommentary(group);
        const commentary = inputCommentary || fallbackCommentary;
        const output = group.result ? segOutput(group.result) : null;
        const title = computeGroupTitle(group);
        const toolName = group.call ? segTool(group.call) : '';
        return {
          ...group,
          id: `${task?.id || 'task'}-${group.index ?? groupIdx}-${groupIdx}`,
          title,
          commentary,
          input,
          output,
          toolName,
          inputCommentary,
          fallbackCommentary
        };
      });
      return mapped.filter((group) => {
        const toolName = group.toolName || '';
        if (toolName && isOutputToolName(toolName)) return false;

        const extras = Array.isArray(group.extras) ? group.extras : [];
        const hasNonFinalExtras = extras.some((extra) => {
          const t = stepType(extra);
          return t !== 'final';
        });

        const title = typeof group.title === 'string' ? group.title.trim() : '';
        const hasNonFinalTitle = title && title.toLowerCase() !== 'final';
        const hasTool = typeof toolName === 'string' && toolName.trim().length > 0;
        const hasCommentary = typeof group.commentary === 'string' && group.commentary.trim().length > 0;
        const hasInput = hasValue(group.input);
        const hasOutput = hasValue(group.output);

        const isFinalOnly =
          !hasTool &&
          !hasCommentary &&
          !hasInput &&
          !hasOutput &&
          !hasNonFinalExtras &&
          (!title || title.toLowerCase() === 'final');

        if (isFinalOnly) return false;

        return hasTool || hasCommentary || hasInput || hasOutput || hasNonFinalExtras || hasNonFinalTitle;
      });
    } catch (_) { return []; }
  }

  function computeGroupTitle(group) {
    if (group.call) return '';
    if (group.extras && group.extras.length) {
      return stepLabel(group.extras[0]) || 'Step';
    }
    return 'Step';
  }

  function computeGroupCommentary(group) {
    try {
      if (group.result) {
        const preview = group.result?.preview;
        if (typeof preview === 'string' && preview.trim()) {
          return preview.trim();
        }
      }
      if (group.extras && group.extras.length) {
        const analysis = group.extras.find((s) => stepType(s) === 'analysis');
        if (analysis) {
          return (segText(analysis) || '').trim();
        }
      }
    } catch (_) {}
    return '';
  }

  function commentaryFromArgs(args) {
    try {
      if (!args) return '';
      const candidates = [
        args?.commentary,
        args?.input_commentary,
        args?.input?.commentary
      ];
      for (const candidate of candidates) {
        if (typeof candidate === 'string') {
          const trimmed = candidate.trim();
          if (trimmed) return trimmed;
        }
      }
    } catch (_) {}
    return '';
  }

  function toggleStepDetails(id) {
    expandedSteps = {
      ...expandedSteps,
      [id]: !expandedSteps[id]
    };
  }

  function handleToggleKey(event, id) {
    if (!event || !id) return;
    const key = event.key;
    if (key === 'Enter' || key === ' ') {
      event.preventDefault();
      toggleStepDetails(id);
    }
  }

  function hasValue(val) {
    if (val === null || val === undefined) return false;
    if (typeof val === 'string') return val.trim().length > 0;
    if (Array.isArray(val)) return val.length > 0;
    if (typeof val === 'object') return Object.keys(val).length > 0;
    return true;
  }

  function formatJson(val) {
    try {
      return JSON.stringify(val, null, 2);
    } catch (_) {
      return String(val);
    }
  }

  function openTaskDetail(taskId) {
    if (!taskId) return;
    selectedTaskId = taskId;
    selectedTaskDetail = taskDetailCache.get(taskId) || null;
    detailError = '';
    showTaskDetail = true;
    loadTaskDetail(taskId, { force: false, showSpinner: true }).catch(() => {});
  }

  function closeTaskDetail() {
    showTaskDetail = false;
    detailLoading = false;
    detailError = '';
  }

  async function createTask(e) {
    e?.preventDefault?.();
    const content = (input || '').trim();
    if (!content) return;
    sending = true;
    try {
      const res = await apiFetch(`/sandboxes/${encodeURIComponent(sandboxId)}/tasks`, {
        method: 'POST',
        body: JSON.stringify({
          input: { content: [{ type: 'text', content }] },
          task_type: taskType
        })
      });
      if (!res.ok) {
        if (res.status === 409) {
          // Refresh sandbox details so local context usage reflects backend limits
          await fetchSandbox();
        }
        throw new Error(res?.data?.message || res?.data?.error || `Send failed (HTTP ${res.status})`);
      }

      // Immediately add the task to the list for instant feedback
      const newTask = res?.data;
      if (newTask && newTask.id) {
        // Add to tasks array immediately
        tasks = [...tasks, {
          id: newTask.id,
          sandbox_id: newTask.sandbox_id,
          status: newTask.status || 'queued',
          task_type: newTask.task_type,
          input: newTask.input || [],
          output: newTask.output || { items: [] },
          context_length: newTask.context_length || 0,
          created_at: newTask.created_at || new Date().toISOString(),
          updated_at: newTask.updated_at || new Date().toISOString()
        }];

        // Stay on task list and scroll to bottom
        showTaskDetail = false;
        await tick();
        if (taskListEl) {
          taskListEl.scrollTop = taskListEl.scrollHeight;
        }

        // Refresh tasks in background to ensure consistency
        fetchTasks().catch(() => {});
        fetchSandbox().catch(() => {});
      }

      input = '';
      // Reset textarea height after clearing
      error = null;
      await tick();
      try { if (inputEl) { inputEl.style.height = ''; } } catch (_) {}
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

    // Initialize folder/file path from URL before first fetch
    try {
      const init = _getPathFromUrl();
      if (init && init.length) {
        fmSegments = init.slice(0, -1);
        fmPendingOpenFile = init[init.length - 1] || '';
      }
    } catch (_) {}

    try {
      await fetchSandbox();
      await fetchTasks();
      await fetchStats(true);
      try { await fetchFiles(true); } catch (_) {}
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
          <p class="small text-body text-opacity-75 mb-0">
            Terminating this sandbox immediately stops its runtime and cancels any in-flight tasks. This action cannot be undone.
          </p>
        </div>
        <div class="modal-footer">
          <button class="btn btn-outline-secondary" on:click={closeTerminateModal}>Cancel</button>
          <button class="btn btn-danger" on:click={confirmTerminateSandbox}>Terminate</button>
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
            {#if sandboxInferenceModel}
              <div class="small text-body-secondary mt-1">
                Model: <span class="font-monospace">{sandboxInferenceModel}</span>
              </div>
            {/if}
            {#if sandbox}
              {#if isAdmin}
                <div class="small text-body-secondary mt-1">Owner: <span class="font-monospace">{sandbox.created_by}</span></div>
              {/if}
              <div class="small text-body-secondary mt-1">
                Idle Timeout: {fmtDuration(Number(sandbox.idle_timeout_seconds ?? 0))}
              </div>
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
                  {#if stateStr === 'idle' && idleDurationLabel}
                    <span class="text-body-secondary small">for {idleDurationLabel}</span>
                  {/if}
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
        <Card class="h-100">
          <div class="card-body small">
            {#if sandbox}
              {#if sandbox.snapshot_id}
                <div class="mt-1">
                  Source Snapshot: <a href="/snapshots/{encodeURIComponent(sandbox.snapshot_id)}" class="font-monospace text-decoration-none">{sandbox.snapshot_id}</a>
                </div>
              {/if}
              {#if topData}
                <div class="mt-1">
                  <div class="d-flex align-items-center justify-content-between">
                    <div class="me-2">CPU: {fmtPercent(topData.cpu_usage_percent)}</div>
                    <div class="text-body-secondary">{topData.cpu_limit_cores} cores</div>
                  </div>
                  <div class="progress mt-1" role="progressbar" aria-valuenow={Number(topData.cpu_usage_percent)} aria-valuemin="0" aria-valuemax="100" style="height: 6px;">
                    <div class={`progress-bar ${Number(topData.cpu_usage_percent) >= 90 ? 'bg-danger' : 'bg-theme'}`} style={`width: ${Math.min(100, Number(topData.cpu_usage_percent)).toFixed(1)}%;`}></div>
                  </div>
                </div>
                <div class="mt-2">
                  <div class="d-flex align-items-center justify-content-between">
                    <div class="me-2">Memory: {fmtBytes(topData.memory_usage_bytes)}</div>
                    <div class="text-body-secondary">{fmtBytes(topData.memory_limit_bytes)} limit</div>
                  </div>
                  <div class="progress mt-1" role="progressbar" aria-valuenow={topData.memory_limit_bytes > 0 ? (topData.memory_usage_bytes / topData.memory_limit_bytes * 100) : 0} aria-valuemin="0" aria-valuemax="100" style="height: 6px;">
                    <div class={`progress-bar ${topData.memory_limit_bytes > 0 && (topData.memory_usage_bytes / topData.memory_limit_bytes * 100) >= 90 ? 'bg-danger' : 'bg-theme'}`} style={`width: ${topData.memory_limit_bytes > 0 ? Math.min(100, (topData.memory_usage_bytes / topData.memory_limit_bytes * 100)).toFixed(1) : 0}%;`}></div>
                  </div>
                </div>
              {/if}
              {#if topData}
                <div class="mt-2">
                  Tasks Completed: {fmtInt(topData.tasks_completed ?? 0)}
                  {#if topData.total_tasks !== undefined} / {fmtInt(topData.total_tasks)} total{/if}
                </div>
                <div class="mt-1">
                  Tokens:
                  <span class="font-monospace">{fmtInt(topData.tokens_prompt ?? 0)} prompt</span>
                  <span class="text-body-tertiary mx-2">|</span>
                  <span class="font-monospace">{fmtInt(topData.tokens_completion ?? 0)} completion</span>
                </div>
                <div class="mt-1">
                  Tool Calls:
                  {#if toolUsageEntries.length}
                    {#each toolUsageEntries as [tool, count], idx (tool)}
                      <span class="font-monospace">{tool}</span> × {fmtInt(count)}
                      {#if idx < toolUsageEntries.length - 1}
                        <span class="text-body-tertiary mx-2">|</span>
                      {/if}
                    {/each}
                  {:else}
                    <span class="text-body-secondary">None yet</span>
                  {/if}
                </div>
                <div class="mt-1">
                  Runtime: {fmtDuration(runtimeSeconds)}
                </div>
              {:else if statsLoading}
                <div class="mt-2 d-flex align-items-center gap-2 text-body text-opacity-75 small">
                  <span class="spinner-border spinner-border-sm text-body text-opacity-75" role="status" aria-label="Loading statistics"></span>
                  Loading statistics…
                </div>
              {:else}
                <div class="mt-2 text-body-secondary small">Statistics unavailable.</div>
              {/if}
            {:else if loading}
              <div class="d-flex align-items-center gap-2 text-body text-opacity-75">
                <span class="spinner-border spinner-border-sm" role="status" aria-hidden="true"></span>
                Loading sandbox info…
              </div>
            {:else if error}
              <div class="alert alert-danger small mb-0">Failed to load sandbox info</div>
            {:else}
              <div class="text-body text-opacity-75">No sandbox data available</div>
            {/if}
          </div>
        </Card>
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
            {#if showTaskDetail}
              <div class="task-detail flex-fill px-3 py-3 border-top" style="overflow-y: auto; min-height: 0;">
                <div class="d-flex flex-wrap align-items-center justify-content-between mb-3 gap-3">
                  <div class="d-flex align-items-start gap-3 flex-wrap">
                    <button class="btn btn-sm btn-outline-secondary" type="button" on:click={closeTaskDetail} aria-label="Back to task list">
                      <i class="bi bi-chevron-left"></i>
                    </button>
                    <div class="fw-semibold font-monospace">Task: {displayTask?.id || selectedTaskId || '-'}</div>
                  </div>
                  <div class="d-flex align-items-center flex-wrap gap-3">
                    <span class={`badge ${taskStatusBadgeClass(displayTask)}`}>{taskStatusLabel(displayTask)}</span>
                    {#if displayTask && String(displayTask?.status || '').toLowerCase() === 'processing'}
                      <button type="button" class="btn btn-sm btn-outline-danger" aria-label="Cancel this task" on:click={cancelActive}>
                        <i class="bi bi-x-circle me-1"></i>Cancel
                      </button>
                    {/if}
                  </div>
                </div>
                <div class="mb-3">
                  <span class="badge task-type-chip {taskTypeClass(displayTask?.task_type)}" title={taskTypeLabel(displayTask?.task_type)}>
                    {taskTypeShort(displayTask?.task_type)}
                  </span>
                </div>
                <section class="mb-3">
                  <h6 class="fw-semibold fs-6 mb-2">Input</h6>
                  {#if taskInputItems(displayTask).length}
                    {#each taskInputItems(displayTask) as item}
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
                {#if detailLoading}
                  <div class="d-flex align-items-center gap-2 small text-body-secondary mb-3">
                    <span class="spinner-border spinner-border-sm" role="status" aria-hidden="true"></span>
                    Loading task details…
                  </div>
                {/if}
                {#if detailError}
                  <div class="alert alert-warning small">{detailError}</div>
                {/if}
                {#if selectedTaskDetail}
                  {#if taskAnalysisSegments(selectedTaskDetail).length}
                    <section class="mb-3">
                      <h6 class="fw-semibold fs-6 mb-2">Analysis</h6>
                      {#each taskAnalysisSegments(selectedTaskDetail) as seg}
                        <div class="small fst-italic text-body text-opacity-75 mb-2" style="white-space: pre-wrap;">{segText(seg)}</div>
                      {/each}
                    </section>
                  {/if}
                {#if currentStepGroups.length}
                  <section class="mb-3">
                    <h6 class="fw-semibold fs-6 mb-2">Steps</h6>
                    <div class="list-group list-group-flush">
                      {#each currentStepGroups as group}
                        <div class="list-group-item">
                          <div class="d-flex align-items-start gap-3">
                            <div class="flex-grow-1">
                              {#if group.title}
                                <div class="fw-semibold mb-1">{group.title}</div>
                              {/if}
                              <div
                                class="d-flex align-items-center gap-2 flex-wrap step-toggle"
                                role="button"
                                tabindex="0"
                                aria-expanded={!!expandedSteps[group.id]}
                                aria-controls={`step-detail-${group.id}`}
                                on:click={() => toggleStepDetails(group.id)}
                                on:keydown={(event) => handleToggleKey(event, group.id)}
                                style="cursor: pointer;"
                              >
                                {#if group.toolName}
                                  <span class="badge bg-success-subtle text-success-emphasis border border-success-subtle rounded-pill">{group.toolName}</span>
                                {/if}
                                {#if group.commentary}
                                  <span class="small text-body-secondary text-truncate" style="max-width: 28rem;">{group.commentary}</span>
                                {:else}
                                  <span class="small text-body-secondary fst-italic">No commentary</span>
                                {/if}
                              </div>
                            </div>
                          </div>
                          {#if expandedSteps[group.id]}
                            <div class="mt-3 border-top pt-3 small" id={`step-detail-${group.id}`}>
                              <div class="mb-3">
                                <div class="fw-semibold text-uppercase mb-1">Input</div>
                                {#if hasValue(group.input)}
                                  <pre class="bg-dark text-white p-2 rounded code-wrap mb-0"><code>{formatJson(group.input)}</code></pre>
                                {:else}
                                  <div class="text-body-secondary">No input recorded.</div>
                                {/if}
                              </div>
                              <div>
                                <div class="fw-semibold text-uppercase mb-1">Output</div>
                                {#if hasValue(group.output)}
                                  {#if typeof group.output === 'string'}
                                    <pre class="bg-dark text-white p-2 rounded code-wrap mb-0"><code>{group.output}</code></pre>
                                  {:else}
                                    <pre class="bg-dark text-white p-2 rounded code-wrap mb-0"><code>{formatJson(group.output)}</code></pre>
                                  {/if}
                                {:else}
                                  <div class="text-body-secondary">No output captured.</div>
                                {/if}
                              </div>
                            </div>
                          {/if}
                        </div>
                      {/each}
                    </div>
                  </section>
                {/if}
                {#if hasAnyTaskOutput}
                  <section>
                    <h6 class="fw-semibold fs-6 mb-2">Output</h6>
                    {#if hasTaskOutputText}
                      <div class="markdown-body mb-3">
                        {@html renderMarkdown(selectedTaskDetail.output.text.trim())}
                      </div>
                    {/if}
                    {#if taskOutputItemsList.length}
                      {#each taskOutputItemsList as item}
                        {#if String(item?.type || '').toLowerCase() === 'commentary'}
                          <div class="small fst-italic text-body text-opacity-75 mb-3" style="white-space: pre-wrap;">{item.content || ''}</div>
                        {:else if String(item?.type || '').toLowerCase() === 'markdown'}
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
                    {/if}
                  </section>
                {/if}
                {:else if !detailLoading && !detailError}
                  <div class="small text-body-secondary">Task details will load shortly.</div>
                {/if}
              </div>
            {:else}
              <div class="px-3 py-2 border-bottom d-flex align-items-center justify-content-between">
                <h6 class="mb-0 small text-uppercase text-body-secondary">Task List</h6>
                <div class="small text-body-secondary">Total: {tasks.length}</div>
              </div>
              <div bind:this={taskListEl} class="task-list flex-grow-1" style="overflow-y: auto;">
                {#if loading}
                  <div class="d-flex align-items-center gap-2 text-body text-opacity-75 small px-3 py-2">
                    <span class="spinner-border spinner-border-sm" role="status" aria-hidden="true"></span>
                    Loading tasks…
                  </div>
                {:else if tasks.length}
                  <div class="list-group list-group-flush">
                    {#each tasks as task}
                      <button
                        type="button"
                        class="list-group-item list-group-item-action d-flex align-items-start justify-content-between gap-3 py-3"
                        on:click={() => openTaskDetail(task.id)}
                      >
                        <div class="text-start flex-grow-1" style="min-width: 0;">
                          <div class="mb-1">
                            <span class="badge task-type-chip {taskTypeClass(task?.task_type)}" title={taskTypeLabel(task?.task_type)}>
                              {taskTypeShort(task?.task_type)}
                            </span>
                          </div>
                          {#if taskPreview(task)}
                            <div class="small text-body text-opacity-75 task-preview">{taskPreview(task)}</div>
                          {/if}
                        </div>
                        <div class="text-end flex-shrink-0">
                          <span class={`badge ${taskStatusBadgeClass(task)}`}>{taskStatusLabel(task)}</span>
                          <div class="small text-body-secondary text-opacity-50 mt-1" style="font-size: 0.7rem;">{formatTaskTimestamp(task)}</div>
                        </div>
                      </button>
                    {/each}
                  </div>
                {:else}
                  <div class="text-body text-opacity-75 small px-3 py-2">No tasks yet.</div>
                {/if}
              </div>
            {/if}
          </div>
          <div class="px-3 pt-3 pb-3 border-top">
            {#if error}
              <div class="alert alert-danger small mb-3">{error}</div>
            {/if}
            <div class="d-flex flex-wrap align-items-center gap-2 mb-2 small text-body-secondary">
              <div>Context length: {fmtInt(context_length)} tokens</div>
            </div>
            <form class="task-form" on:submit|preventDefault={createTask}>
              <div class="mb-2">
                <div class="d-flex flex-wrap gap-2">
                  {#each taskTypeOptions as option}
                    <button
                      type="button"
                      class={`btn btn-sm ${taskType === option.value ? 'btn-dark text-white' : 'btn-outline-secondary'}`}
                      on:click={() => (taskType = option.value)}
                      disabled={taskInputDisabled}
                    >
                      {option.label}
                    </button>
                  {/each}
                </div>
              </div>
              <div class="input-group task-input-group rounded-0 shadow-none">
                <textarea
                  aria-label="Task instructions"
                  class="form-control shadow-none task-input"
                  disabled={taskInputDisabled}
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
                <button class="btn btn-theme task-action-btn" aria-label="Create task" disabled={taskInputDisabled || !input.trim()}>
                  {#if sending}
                    <span class="spinner-border spinner-border-sm" role="status" aria-hidden="true"></span>
                  {:else}
                    <i class="bi bi-plus-circle"></i>
                  {/if}
                </button>
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
  :global(.task-pane .task-list .list-group-item.active .badge:not(.task-type-chip)) {
    background-color: var(--bs-theme);
    color: var(--bs-theme-color);
  }
  :global(.task-pane .task-list .task-preview) {
    max-width: 18rem;
    display: block;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
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

  :global(.task-type-chip) {
    font-size: 0.75rem;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    font-weight: 500;
    border: 1px solid;
    padding: 0.25rem 0.75rem;
    border-radius: 0.3rem;
  }

  /* NL - Natural Language (Purple/Indigo) */
  :global(.task-type-chip.task-type-nl) {
    border-color: rgba(111, 66, 193, 0.4);
    color: rgb(111, 66, 193);
    background: rgba(111, 66, 193, 0.1);
  }

  /* SH - Shell (Green) */
  :global(.task-type-chip.task-type-sh) {
    border-color: rgba(25, 135, 84, 0.4);
    color: rgb(25, 135, 84);
    background: rgba(25, 135, 84, 0.1);
  }

  /* PY - Python (Blue) */
  :global(.task-type-chip.task-type-py) {
    border-color: rgba(13, 110, 253, 0.4);
    color: rgb(13, 110, 253);
    background: rgba(13, 110, 253, 0.1);
  }

  /* JS - JavaScript (Yellow/Amber) */
  :global(.task-type-chip.task-type-js) {
    border-color: rgba(255, 193, 7, 0.5);
    color: rgb(204, 153, 0);
    background: rgba(255, 193, 7, 0.15);
  }

  :global(.task-pane .task-list .list-group-item.active .task-type-chip) {
    border-color: rgba(var(--bs-theme-rgb), 0.5);
    color: var(--bs-theme);
    background: rgba(var(--bs-theme-rgb), 0.12);
  }

  @media (max-width: 576px) {
    :global(.task-pane .task-input) { font-size: 16px; }
  }
</style>
