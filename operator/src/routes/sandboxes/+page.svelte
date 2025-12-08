<script>
  import { onMount, onDestroy } from 'svelte';
  import { goto } from '$app/navigation';
  import { auth, getOperatorName, isAuthenticated } from '$lib/auth.js';
  import { apiFetch } from '$lib/api/client.js';
  import Card from '/src/components/bootstrap/Card.svelte';
  import ApexCharts from '/src/components/plugins/ApexCharts.svelte';
  import { setPageTitle } from '$lib/utils.js';
  import { getHostName as getBrandHostName, getHostUrl as getBrandHostUrl } from '$lib/branding.js';

  export let data;

  setPageTitle('Sandboxes');

  const numberFormatter = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 });
  const byteFormatter = new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 });
  const historyLimit = 36;
  const BYTES_IN_GIB = 1024 ** 3;

  let stats = data?.globalStats || null;
let runtimeHostUrl = null;
let runtimeHostName = null;
$: hostDisplayUrl = (() => {
    if (runtimeHostUrl) return runtimeHostUrl;
    if (data?.hostUrl) {
      const fromConfig = String(data.hostUrl).replace(/\/$/, '');
      if (fromConfig) return fromConfig;
    }
    return getBrandHostUrl();
  })();
$: hostDisplayName = (() => {
    const configName = data?.hostName && String(data.hostName).trim();
    if (configName) return configName;
    const metricsName = host?.hostname && String(host.hostname).trim();
    if (metricsName) return metricsName;
    const runtimeName = runtimeHostName && runtimeHostName !== 'localhost' ? runtimeHostName : null;
    if (runtimeName) return runtimeName;
    try {
      const parsed = new URL(hostDisplayUrl || 'http://localhost');
      if (parsed.hostname && parsed.hostname !== 'localhost') return parsed.hostname;
    } catch (_) {}
    return getBrandHostName();
  })();
  let statsError = null;
  let statsLoading = false;
  let statsLastUpdated = stats?.captured_at ? new Date(stats.captured_at) : null;
  let cpuHistory = [];
  let memoryHistory = [];
  let statsPollHandle = null;
  let historySeeded = false;
  let host = stats?.host || null;

  let loading = true;
  let error = null;
  let sandboxes = [];
  $: activeSandboxes = sandboxes.filter(s => {
    const state = String(s?.state || '').toLowerCase();
    return state !== 'terminated' && state !== 'deleted';
  });
  $: terminatedSandboxes = sandboxes.filter(s => {
    const state = String(s?.state || '').toLowerCase();
    return state === 'terminated';
  });
  $: host = stats?.host || null;
  $: memoryPercent = host ? Math.round((Number(host.memory_used_percent || 0)) * 10) / 10 : 0;
  $: cpuPercent = host ? Math.round((Number(host.cpu_percent || 0)) * 10) / 10 : 0;
  $: stateBreakdown = Object.entries(stats?.sandboxes_by_state || {}).map(([state, count]) => ({
    state,
    count: Number(count) || 0
  })).sort((a, b) => b.count - a.count);
  // Filters + pagination
  let q = '';
  let stateFilter = '';
  let currentStateTab = 'active';
  $: currentStateTab = stateFilter === 'terminated' ? 'terminated' : 'active';
  let tagsText = '';
  let limit = 30;
  let pageNum = 1; // 1-based
  let total = 0;
  let pages = 1;
  let operatorName = '';
  $: isAdmin = $auth && String($auth.type || '').toLowerCase() === 'admin';

  function formatNumber(value) {
    const numeric = Number(value);
    if (!Number.isFinite(numeric)) return '0';
    return numberFormatter.format(Math.round(numeric));
  }

  function clampPercent(value) {
    if (!Number.isFinite(value)) return 0;
    return Math.min(100, Math.max(0, Number(value)));
  }

  function formatBytes(bytes) {
    const numeric = Number(bytes);
    if (!Number.isFinite(numeric) || numeric <= 0) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
    let idx = 0;
    let current = numeric;
    while (current >= 1024 && idx < units.length - 1) {
      current /= 1024;
      idx += 1;
    }
    return `${byteFormatter.format(current)} ${units[idx]}`;
  }

  function formatDuration(seconds) {
    const total = Number(seconds);
    if (!Number.isFinite(total) || total <= 0) return '—';
    const days = Math.floor(total / 86400);
    const hours = Math.floor((total % 86400) / 3600);
    const minutes = Math.floor((total % 3600) / 60);
    if (days > 0) return `${days}d ${hours}h`;
    if (hours > 0) return `${hours}h ${minutes}m`;
    if (minutes > 0) return `${minutes}m`;
    return `${Math.floor(total)}s`;
  }

  function formatRelativeTime(date) {
    if (!date) return 'never';
    const diff = Date.now() - date.getTime();
    if (diff < 2000) return 'just now';
    if (diff < 60000) return `${Math.round(diff / 1000)}s ago`;
    if (diff < 3600000) return `${Math.round(diff / 60000)}m ago`;
    if (diff < 86400000) return `${Math.round(diff / 3600000)}h ago`;
    return date.toLocaleString();
  }

  function formatPercent(value) {
    const numeric = Number(value);
    if (!Number.isFinite(numeric)) return '0%';
    return `${numeric.toFixed(1)}%`;
  }

  function formatLoadAverages() {
    if (!host) return '—';
    const one = (host.load_avg_1m ?? 0).toFixed(2);
    const five = (host.load_avg_5m ?? 0).toFixed(2);
    const fifteen = (host.load_avg_15m ?? 0).toFixed(2);
    return `${one} / ${five} / ${fifteen}`;
  }

  function formatStateLabel(name) {
    if (!name) return 'Unknown';
    const str = String(name);
    return str.charAt(0).toUpperCase() + str.slice(1);
  }

  function ensureHistorySeeded(snapshot = stats) {
    if (historySeeded || !snapshot?.host) return;
    historySeeded = true;
    const ts = snapshot?.captured_at ? Date.parse(snapshot.captured_at) : Date.now();
    cpuHistory = [{
      x: ts,
      y: clampPercent(snapshot.host.cpu_percent || 0)
    }];
    memoryHistory = [{
      x: ts,
      y: Number(snapshot.host.memory_used_bytes || 0)
    }];
  }

  function recordCpuSample(value, timestamp = Date.now()) {
    if (!Number.isFinite(value)) return;
    const clamped = clampPercent(value);
    cpuHistory = [
      ...cpuHistory.slice(-(historyLimit - 1)),
      { x: timestamp, y: Number(clamped.toFixed(2)) }
    ];
  }

  function recordMemorySample(bytesUsed, timestamp = Date.now()) {
    if (!Number.isFinite(bytesUsed)) return;
    const normalized = Math.max(0, Number(bytesUsed));
    memoryHistory = [
      ...memoryHistory.slice(-(historyLimit - 1)),
      { x: timestamp, y: normalized }
    ];
  }

  function applyStatsSnapshot(snapshot, trackHistory = true) {
    if (!snapshot) return;
    stats = snapshot;
    statsLastUpdated = snapshot?.captured_at ? new Date(snapshot.captured_at) : new Date();
    if (!snapshot.host) return;
    const seededBefore = historySeeded;
    ensureHistorySeeded(snapshot);
    if (!trackHistory || !seededBefore) return;
    const ts = statsLastUpdated ? statsLastUpdated.getTime() : Date.now();
    recordCpuSample(snapshot.host.cpu_percent, ts);
    recordMemorySample(snapshot.host.memory_used_bytes, ts);
  }

  async function refreshStats(trackHistory = true) {
    try {
      statsLoading = true;
      const res = await apiFetch('/stats');
      if (!res.ok) {
        statsError = res?.data?.message || `Failed to load host stats (HTTP ${res.status})`;
        return;
      }
      statsError = null;
      applyStatsSnapshot(res.data, trackHistory);
    } catch (e) {
      statsError = e?.message || String(e);
    } finally {
      statsLoading = false;
    }
  }

  function startStatsPolling() {
    stopStatsPolling();
    statsPollHandle = setInterval(async () => {
      try { await refreshStats(true); } catch (_) {}
    }, 8000);
  }

  function stopStatsPolling() {
    if (statsPollHandle) {
      clearInterval(statsPollHandle);
      statsPollHandle = null;
    }
  }

  ensureHistorySeeded(stats);

  $: cpuChartOptions = {
    chart: {
      type: 'area',
      toolbar: { show: false },
      animations: { easing: 'easeinout', speed: 250 }
    },
    dataLabels: { enabled: false },
    stroke: { curve: 'smooth', width: 2 },
    fill: { type: 'gradient', gradient: { shadeIntensity: 0.35, opacityFrom: 0.45, opacityTo: 0.05 } },
    series: [
      {
        name: 'CPU %',
        data: cpuHistory.map((point) => [point.x, Number(point.y.toFixed(2))])
      }
    ],
    xaxis: { type: 'datetime', labels: { datetimeUTC: false } },
    yaxis: {
      min: 0,
      max: 100,
      tickAmount: 5,
      labels: {
        formatter: (val) => {
          const numeric = Number(val);
          return Number.isFinite(numeric) ? `${Math.round(numeric)}%` : '-';
        }
      }
    },
    tooltip: {
      x: { format: 'HH:mm:ss' },
      y: {
        formatter: (val) => {
          const numeric = Number(val);
          return Number.isFinite(numeric) ? `${numeric.toFixed(1)}%` : '-';
        }
      }
    },
    colors: ['#0d6efd']
  };

  $: memoryChartMax = host?.memory_total_bytes ? host.memory_total_bytes / BYTES_IN_GIB : null;
  $: memoryChartOptions = {
    chart: {
      type: 'area',
      toolbar: { show: false },
      animations: { easing: 'easeinout', speed: 250 }
    },
    dataLabels: { enabled: false },
    stroke: { curve: 'smooth', width: 2 },
    fill: { type: 'gradient', gradient: { shadeIntensity: 0.25, opacityFrom: 0.45, opacityTo: 0.05 } },
    series: [
      {
        name: 'Used (GiB)',
        data: memoryHistory.map((point) => [
          point.x,
          Number(((point.y || 0) / BYTES_IN_GIB).toFixed(2))
        ])
      }
    ],
    xaxis: { type: 'datetime', labels: { datetimeUTC: false } },
    yaxis: {
      min: 0,
      max: memoryChartMax ? Number((memoryChartMax * 1.05).toFixed(2)) : undefined,
      labels: {
        formatter: (val) => {
          const numeric = Number(val);
          return Number.isFinite(numeric) ? `${numeric.toFixed(1)} GiB` : '-';
        }
      }
    },
    tooltip: {
      x: { format: 'HH:mm:ss' },
      y: {
        formatter: (val) => {
          const numeric = Number(val);
          return Number.isFinite(numeric) ? `${numeric.toFixed(2)} GiB` : '-';
        }
      }
    },
    colors: ['#20c997']
  };

  function stateIconClass(state) {
    const s = String(state || '').toLowerCase();
    if (s === 'terminated') return 'bi bi-power';
    if (s === 'terminating') return 'spinner-border spinner-border-sm text-danger';
    if (s === 'deleted') return 'bi bi-trash';
    if (s === 'idle') return 'bi bi-sun';
    if (s === 'busy') return 'spinner-border spinner-border-sm';
    if (s === 'initializing') return 'spinner-border spinner-border-sm text-info';
    return 'bi bi-circle';
  }

  function buildQuery() {
    const params = new URLSearchParams();
    if (q && q.trim().length) params.set('q', q.trim());
    if (stateFilter && stateFilter.trim().length) params.set('state', stateFilter.trim());
    if (tagsText && tagsText.trim().length) {
      const tags = tagsText.split(',').map(t => t.trim().toLowerCase()).filter(Boolean);
      if (tags.length) params.set('tags', tags.join(','));
    }
    if (limit) params.set('limit', String(limit));
    if (pageNum) params.set('page', String(pageNum));
    return params.toString();
  }

  async function fetchSandboxes() {
    const qs = buildQuery();
    const res = await apiFetch(`/sandboxes?${qs}`);
    if (!res.ok) {
      error = res?.data?.message || `Failed to load sandboxes (HTTP ${res.status})`;
      loading = false;
      return;
    }
    const data = res.data || {};
    sandboxes = Array.isArray(data.items) ? data.items : [];
    total = Number(data.total || 0);
    limit = Number(data.limit || limit);
    const offset = Number(data.offset || 0);
    pageNum = Number(data.page || (limit ? (Math.floor(offset / limit) + 1) : 1));
    pages = Number(data.pages || (limit ? Math.max(1, Math.ceil(total / limit)) : 1));
  }

  let showTerminateModal = false;
  let terminateSandboxTarget = null;

  function openTerminateModal(sandbox) {
    terminateSandboxTarget = sandbox;
    showTerminateModal = true;
  }

  function closeTerminateModal() {
    showTerminateModal = false;
    terminateSandboxTarget = null;
  }

  async function confirmTerminateSandbox() {
    if (!terminateSandboxTarget) return;
    const res = await apiFetch(`/sandboxes/${encodeURIComponent(terminateSandboxTarget.id)}`, { method: 'DELETE' });
    if (!res.ok) {
      error = res?.data?.message || 'Termination failed';
      return;
    }
    closeTerminateModal();
    await fetchSandboxes();
  }

  // Edit Timeouts modal state and actions
  let showTimeoutsModal = false;
  let idleTimeoutInput = 900;
  let currentSandbox = null;
  function openEditTimeouts(sandbox) {
    currentSandbox = sandbox;
    const idle = Number(sandbox?.idle_timeout_seconds ?? 900);
    idleTimeoutInput = Number.isFinite(idle) && idle >= 0 ? idle : 900;
    showTimeoutsModal = true;
  }
  function closeEditTimeouts() { showTimeoutsModal = false; currentSandbox = null; }
  async function saveTimeouts() {
    if (!currentSandbox) return;
    try {
      const idle = Math.max(0, Math.floor(Number(idleTimeoutInput || 900)));
      const body = { idle_timeout_seconds: idle };
      const res = await apiFetch(`/sandboxes/${encodeURIComponent(currentSandbox.id)}`, { method: 'PUT', body: JSON.stringify(body) });
      if (!res.ok) throw new Error(res?.data?.message || res?.data?.error || `Update failed (HTTP ${res.status})`);
      showTimeoutsModal = false;
      currentSandbox = null;
      await fetchSandboxes();
    } catch (e) {
      alert(e.message || String(e));
    }
  }

  // Snapshot modal state and actions
  let showSnapshotModal = false;
  let snapshotError = null;
  function openSnapshotModal(sandbox) {
    currentSandbox = sandbox;
    snapshotError = null;
    showSnapshotModal = true;
  }
  function closeSnapshotModal() { showSnapshotModal = false; currentSandbox = null; }
  async function confirmCreateSnapshot() {
    if (!currentSandbox) return;
    try {
      snapshotError = null;
      const res = await apiFetch(`/sandboxes/${encodeURIComponent(currentSandbox.id)}/snapshots`, {
        method: 'POST',
        body: JSON.stringify({ trigger_type: 'manual' })
      });
      if (!res.ok) {
        snapshotError = res?.data?.message || res?.data?.error || `Snapshot creation failed (HTTP ${res.status})`;
        return;
      }
      showSnapshotModal = false;
      currentSandbox = null;
      // Redirect to snapshots page
      goto('/snapshots');
    } catch (e) {
      snapshotError = e.message || String(e);
    }
  }

  let pollHandle = null;
  function startPolling() {
    stopPolling();
    const filtersActive = (q && q.trim()) || (tagsText && tagsText.trim()) || (stateFilter && stateFilter.trim());
    if (!filtersActive && pageNum === 1) {
      pollHandle = setInterval(async () => { try { await fetchSandboxes(); } catch (_) {} }, 3000);
    }
  }
  function stopPolling() {
    if (pollHandle) { clearInterval(pollHandle); pollHandle = null; }
  }

  function syncUrl() {
    try {
      const qs = buildQuery();
      const url = qs ? `/sandboxes?${qs}` : '/sandboxes';
      goto(url, { replaceState: true, keepfocus: true, noScroll: true });
    } catch (_) {}
  }

  async function applyFilters() {
    pageNum = 1;
    syncUrl();
    loading = true;
    await fetchSandboxes();
    loading = false;
    startPolling();
  }

  function setStateTab(tab) {
    const newFilter = tab === 'terminated' ? 'terminated' : '';
    if (stateFilter === newFilter) return;
    stateFilter = newFilter;
    applyFilters();
  }

  onMount(async () => {
    if (!isAuthenticated()) {
      goto('/login');
      return;
    }
    try {
      runtimeHostUrl = window?.location?.origin?.replace(/\/$/, '') || null;
      runtimeHostName = window?.location?.hostname || null;
    } catch (_) {
      runtimeHostUrl = null;
      runtimeHostName = null;
    }
    try { operatorName = getOperatorName() || ''; } catch (_) { operatorName = ''; }
    // Seed from URL
    try {
      const sp = new URLSearchParams(location.search || '');
      q = sp.get('q') || '';
      stateFilter = sp.get('state') || '';
      const t = [...sp.getAll('tags[]'), ...sp.getAll('tags')];
      tagsText = t && t.length ? t.join(',') : '';
      limit = Number(sp.get('limit') || 30);
      pageNum = Number(sp.get('page') || 1);
    } catch (_) {}
    await Promise.all([fetchSandboxes(), refreshStats(true)]);
    loading = false;
    startPolling();
    startStatsPolling();
  });

  onDestroy(() => { stopPolling(); stopStatsPolling(); });
</script>

<div class="container-xxl">
  <div class="row justify-content-center">
    <div class="col-12 col-xxl-10">
      {#if isAdmin}
      <div class="mb-4">
        <div class="d-flex align-items-center flex-wrap gap-2 mb-2">
          <div>
            <div class="fw-bold fs-20px">Host Overview</div>
            <div class="text-body text-opacity-75 small">
              Last updated {statsLastUpdated ? formatRelativeTime(statsLastUpdated) : 'never'}
            </div>
          </div>
          <div class="ms-auto d-flex align-items-center flex-wrap gap-2">
            {#if statsError}
              <div class="text-danger small">{statsError}</div>
            {/if}
            <button class="btn btn-outline-secondary btn-sm" type="button" disabled={statsLoading} on:click={() => refreshStats(true)}>
              {#if statsLoading}
                <span class="spinner-border spinner-border-sm me-1" role="status" aria-hidden="true"></span>
              {:else}
                <i class="bi bi-arrow-repeat me-1"></i>
              {/if}
              Refresh
            </button>
          </div>
        </div>
        <div class="row g-3">
          <div class="col-12 col-xl-4">
            <Card class="h-100">
              <div class="card-body">
                <div class="stat-label mb-1">Host</div>
                <div class="fs-24px fw-bold">{hostDisplayName}</div>
                <div class="text-body text-opacity-75 small">{hostDisplayUrl}</div>
                <div class="text-body text-opacity-75 small mb-3">Uptime {formatDuration(host?.uptime_seconds)}</div>
                <div class="d-flex justify-content-between small text-body text-opacity-75 mb-1">
                  <span>Load (1/5/15m)</span>
                  <span class="fw-semibold">{formatLoadAverages()}</span>
                </div>
                <div class="d-flex justify-content-between small text-body text-opacity-75">
                  <span>CPU Cores</span>
                  <span class="fw-semibold">{host?.cpu_cores ?? '—'}</span>
                </div>
              </div>
            </Card>
          </div>
          <div class="col-12 col-xl-4">
                <Card class="h-100">
                  <div class="card-body">
                    <div class="stat-label mb-1">Sandboxes</div>
                    <div class="d-flex align-items-baseline gap-2">
                      <div class="fs-32px fw-bold">{formatNumber(stats?.sandboxes_active || 0)}</div>
                      <div class="text-body text-opacity-75">active</div>
                    </div>
                    <div class="text-body text-opacity-75 small mb-3">
                      {formatNumber(stats?.sandboxes_total || 0)} total • {formatNumber(stats?.sandboxes_terminated || 0)} terminated
                    </div>
                    <div class="d-flex flex-column gap-1">
                      {#if stateBreakdown.length}
                        {#each stateBreakdown.slice(0,4) as sb}
                          <div class="d-flex justify-content-between small text-body text-opacity-75">
                            <span>{formatStateLabel(sb.state)}</span>
                            <span class="fw-semibold">{formatNumber(sb.count)}</span>
                          </div>
                        {/each}
                      {:else}
                        <div class="small text-body text-opacity-75">No sandboxes yet.</div>
                      {/if}
                    </div>
                  </div>
                </Card>
          </div>
          <div class="col-12 col-xl-4">
            <Card class="h-100">
              <div class="card-body">
                <div class="stat-label mb-1">Tasks &amp; Memory</div>
                <div class="d-flex align-items-baseline gap-2">
                  <div class="fs-32px fw-bold">{formatNumber(stats?.sandbox_tasks_active || 0)}</div>
                  <div class="text-body text-opacity-75">in flight</div>
                </div>
                <div class="text-body text-opacity-75 small mb-3">{formatNumber(stats?.sandbox_tasks_total || 0)} total tasks</div>
                <div class="d-flex justify-content-between small text-body text-opacity-75 mb-1">
                  <span>Memory</span>
                  <span class="fw-semibold">{formatPercent(memoryPercent)}</span>
                </div>
                <div class="progress memory-progress" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={memoryPercent || 0}>
                  <div class="progress-bar bg-theme" style={`width: ${Math.min(100, memoryPercent || 0)}%`}></div>
                </div>
                <div class="d-flex justify-content-between text-body-secondary small mt-1">
                  <span>{formatBytes(host?.memory_used_bytes || 0)} used</span>
                  <span>{formatBytes(host?.memory_total_bytes || 0)} total</span>
                </div>
              </div>
            </Card>
          </div>
        </div>
        <div class="row g-3 mt-1">
          <div class="col-12 col-xl-6">
            <Card class="h-100 chart-card">
              <div class="card-body">
                <div class="d-flex align-items-center justify-content-between mb-2">
                  <div>
                    <div class="stat-label mb-1">CPU Utilization</div>
                    <div class="fs-24px fw-bold">{formatPercent(cpuPercent)}</div>
                  </div>
                  <div class="text-body text-opacity-75 small">{host?.cpu_cores ?? '—'} cores</div>
                </div>
                <ApexCharts height="180px" options={cpuChartOptions} />
              </div>
            </Card>
          </div>
          <div class="col-12 col-xl-6">
            <Card class="h-100 chart-card">
              <div class="card-body">
                <div class="d-flex align-items-center justify-content-between mb-2">
                  <div>
                    <div class="stat-label mb-1">Memory Usage</div>
                    <div class="fs-24px fw-bold">{formatBytes(host?.memory_used_bytes || 0)}</div>
                  </div>
                  <div class="text-body text-opacity-75 small">{formatBytes(host?.memory_total_bytes || 0)} total</div>
                </div>
                <ApexCharts height="180px" options={memoryChartOptions} />
              </div>
            </Card>
          </div>
        </div>
      </div>
      {/if}
{#if isAdmin}
  <div class="alert alert-info d-flex align-items-center" role="alert">
    <div>
      You are logged in as <strong>{operatorName || 'admin'}</strong>. Please create a token here and use the system as a user.
      <a href="/tokens" class="ms-1">Open Tokens</a>
    </div>
  </div>
{/if}
<div class="d-flex align-items-center flex-wrap gap-2 mb-2">
  <div class="fw-bold fs-20px">Sandboxes</div>
  <div class="ms-auto d-flex align-items-center gap-2">
    <a href="/sandboxes/start" class="btn btn-outline-theme btn-sm"><i class="bi bi-plus me-1"></i>Start Sandbox</a>
  </div>

  <!-- Filters row -->
  <div class="w-100"></div>
  <div class="w-100 mb-2">
    <form class="row g-2" on:submit|preventDefault={applyFilters}>
      <div class="col-12 col-md-6">
        <div class="input-group input-group-sm flex-nowrap">
          <span class="input-group-text bg-body-secondary border-0"><i class="bi bi-search"></i></span>
          <input class="form-control" placeholder="Search by ID, description, or owner" bind:value={q} name="q" autocapitalize="none" />
        </div>
      </div>
      <div class="col-12 col-md-4 col-lg-3">
        <div class="input-group input-group-sm flex-nowrap">
          <span class="input-group-text bg-body-secondary border-0"><i class="bi bi-tags"></i></span>
          <input class="form-control" placeholder="tags,comma,separated" bind:value={tagsText} name="tags" autocapitalize="none" />
        </div>
      </div>
      <div class="col-12 col-md-auto">
        <button type="submit" class="btn btn-outline-secondary btn-sm w-100"><i class="bi bi-funnel me-1"></i>Apply Filter</button>
      </div>
      <!-- Desktop total aligned to far right -->
      <div class="col-12 col-md-auto ms-md-auto d-none d-md-flex align-items-center">
        <div class="small text-body text-opacity-75">{total} total</div>
      </div>
      <div class="col-12 d-md-none">
        <div class="small text-body text-opacity-75">{total} total</div>
      </div>
    </form>
  </div>
  <div class="w-100 mb-2">
    <ul class="nav nav-pills gap-2 flex-wrap small">
      <li class="nav-item">
        <button
          type="button"
          class="nav-link rounded-pill px-3 py-1"
          class:active={currentStateTab === 'active'}
          aria-current={currentStateTab === 'active' ? 'page' : undefined}
          on:click={() => setStateTab('active')}
        >
          Active ({activeSandboxes.length})
        </button>
      </li>
      <li class="nav-item">
        <button
          type="button"
          class="nav-link rounded-pill px-3 py-1"
          class:active={currentStateTab === 'terminated'}
          aria-current={currentStateTab === 'terminated' ? 'page' : undefined}
          on:click={() => setStateTab('terminated')}
        >
          Terminated ({terminatedSandboxes.length})
        </button>
      </li>
    </ul>
  </div>
</div>

<div>
        {#if loading}
          <div class="d-flex align-items-center justify-content-center" style="min-height: 30vh;">
            <div class="text-center text-body text-opacity-75">
              <div class="spinner-border text-theme mb-3"></div>
              <div>Loading sandboxes…</div>
            </div>
          </div>
        {:else if error}
          <div class="alert alert-danger small">{error}</div>
        {:else if !sandboxes || sandboxes.length === 0}
          <div class="text-body text-opacity-75">No sandboxes found.</div>
          <div class="mt-3">
            <a href="/sandboxes/start" class="btn btn-outline-theme"><i class="bi bi-plus me-1"></i>Start your first sandbox</a>
          </div>
        {:else}
          {#if currentStateTab === 'active'}
            {#if activeSandboxes.length}
              <div class="row g-3">
                {#each activeSandboxes as a (a.id)}
                  <div class="col-12 col-md-6">
                    <Card class="h-100 muted-card">
                      <div class="card-body d-flex flex-column">
                        <div class="d-flex align-items-center gap-2 mb-1">
                          <a class="fw-bold text-decoration-none fs-18px font-monospace" href={'/sandboxes/' + encodeURIComponent(a.id || '')}>{a.id || '-'}</a>
                        </div>
                        <div class="small text-body text-opacity-75 flex-grow-1 text-truncate" title={a.description || a.desc || ''}>{a.description || a.desc || 'No description'}</div>
                        {#if isAdmin}
                          <div class="small text-body-secondary mt-1">Owner: <span class="font-monospace">{a.created_by}</span></div>
                        {/if}

                        <div class="mt-2 d-flex flex-wrap gap-1">
                          {#if Array.isArray(a.tags) && a.tags.length}
                            {#each a.tags as t}
                              <span class="badge bg-secondary-subtle text-secondary-emphasis border">{t}</span>
                            {/each}
                          {:else}
                            <span class="text-body-secondary small">No tags</span>
                          {/if}
                        </div>
                        <!-- In-card actions: status on left, buttons on right -->
                        <div class="mt-2 d-flex align-items-center flex-wrap">
                          <div class="d-flex align-items-center gap-2">
                            <i class={`${stateIconClass(a.state || a.status)} me-1`}></i>
                            <span class="text-uppercase small fw-bold text-body state-label">{a.state || a.status || 'unknown'}</span>
                          </div>
                          <div class="ms-auto d-flex align-items-center flex-wrap gap-2 list-actions">
                            <button class="btn btn-outline-secondary btn-sm" on:click={() => goto('/sandboxes/' + encodeURIComponent(a.id))} aria-label="Open sandbox">
                              <i class="bi bi-box-arrow-up-right me-1"></i><span>Open</span>
                            </button>
                            {#if ['idle','busy'].includes(String(a.state||'').toLowerCase())}
                              <button class="btn btn-outline-danger btn-sm" on:click|preventDefault={() => openTerminateModal(a)} aria-label="Terminate sandbox">
                                <i class="bi bi-power text-danger me-1"></i><span>Terminate</span>
                              </button>
                            {/if}
                          </div>
                        </div>
                      </div>
                    </Card>
                  </div>
                {/each}
              </div>
            {:else}
              <div class="text-body text-opacity-75 small">No active sandboxes.</div>
            {/if}
          {:else}
            {#if terminatedSandboxes.length}
              <div class="row g-3">
                {#each terminatedSandboxes as a (a.id)}
                  <div class="col-12 col-md-6">
                    <Card class="h-100">
                      <div class="card-body d-flex flex-column">
                        <div class="d-flex align-items-center gap-2 mb-1">
                          <a class="fw-bold text-decoration-none fs-18px font-monospace" href={'/sandboxes/' + encodeURIComponent(a.id || '')}>{a.id || '-'}</a>
                        </div>
                        <div class="small text-body text-opacity-75 flex-grow-1 text-truncate" title={a.description || a.desc || ''}>{a.description || a.desc || 'No description'}</div>
                        {#if isAdmin}
                          <div class="small text-body-secondary mt-1">Owner: <span class="font-monospace">{a.created_by}</span></div>
                        {/if}

                        <div class="mt-2 d-flex flex-wrap gap-1">
                          {#if Array.isArray(a.tags) && a.tags.length}
                            {#each a.tags as t}
                              <span class="badge bg-secondary-subtle text-secondary-emphasis border">{t}</span>
                            {/each}
                          {:else}
                            <span class="text-body-secondary small">No tags</span>
                          {/if}
                        </div>
                        <!-- In-card actions: status on left, buttons on right -->
                        <div class="mt-2 d-flex align-items-center flex-wrap">
                          <div class="d-flex align-items-center gap-2">
                            <i class={`${stateIconClass(a.state || a.status)} me-1`}></i>
                            <span class="text-uppercase small fw-bold text-body state-label">{a.state || a.status || 'unknown'}</span>
                          </div>
                          <div class="ms-auto d-flex align-items-center flex-wrap gap-2 list-actions">
                            <button class="btn btn-outline-secondary btn-sm" on:click={() => goto('/sandboxes/' + encodeURIComponent(a.id))} aria-label="Open sandbox">
                              <i class="bi bi-box-arrow-up-right me-1"></i><span>Open</span>
                            </button>
                          </div>
                        </div>
                      </div>
                    </Card>
                  </div>
                {/each}
              </div>
            {:else}
              <div class="text-body text-opacity-75 small">No terminated sandboxes.</div>
            {/if}
          {/if}
{#if pages > 1}
          <div class="d-flex align-items-center justify-content-center mt-3 gap-1">
            <button class="btn btn-sm btn-outline-secondary" disabled={pageNum <= 1} on:click={async () => { pageNum = Math.max(1, pageNum-1); syncUrl(); loading = true; await fetchSandboxes(); loading = false; startPolling(); }}>Prev</button>
            {#each Array(pages) as _, idx}
              {#if Math.abs((idx+1) - pageNum) <= 2 || idx === 0 || idx+1 === pages}
                <button class={`btn btn-sm ${idx+1===pageNum ? 'btn-theme' : 'btn-outline-secondary'}`} on:click={async () => { pageNum = idx+1; syncUrl(); loading = true; await fetchSandboxes(); loading = false; startPolling(); }}>{idx+1}</button>
              {:else if Math.abs((idx+1) - pageNum) === 3}
                <span class="px-1">…</span>
              {/if}
            {/each}
            <button class="btn btn-sm btn-outline-secondary" disabled={pageNum >= pages} on:click={async () => { pageNum = Math.min(pages, pageNum+1); syncUrl(); loading = true; await fetchSandboxes(); loading = false; startPolling(); }}>Next</button>
          </div>
          {/if}
        {/if}
    </div>
  </div>
</div>
</div>

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

{#if showTerminateModal}
  <div class="modal fade show" style="display: block; background: rgba(0,0,0,.3);" tabindex="-1" role="dialog" aria-modal="true">
    <div class="modal-dialog">
      <div class="modal-content">
        <div class="modal-header">
          <h5 class="modal-title">Terminate Sandbox</h5>
          <button type="button" class="btn-close" aria-label="Close" on:click={closeTerminateModal}></button>
        </div>
        <div class="modal-body">
          <p class="small text-body text-opacity-75 mb-2">
            Sandbox <span class="font-monospace">{terminateSandboxTarget?.id}</span> will shut down immediately. Any running tasks will be cancelled and the runtime will be destroyed.
          </p>
          <div class="alert alert-warning small mb-0">
            This action cannot be undone.
          </div>
        </div>
        <div class="modal-footer">
          <button type="button" class="btn btn-outline-secondary" on:click={closeTerminateModal}>Cancel</button>
          <button type="button" class="btn btn-danger" on:click={confirmTerminateSandbox}>
            Terminate
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Create Snapshot Modal -->
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

<style>
  :global(.card) { overflow: visible; }
  :global(.list-actions) { position: relative; z-index: 3001; isolation: isolate; }
  :global(.card .card-arrow) { z-index: 0; pointer-events: none; }
  .text-truncate { display: block; }
  :global(.modal) { z-index: 2000; }
  :global(.modal-backdrop) { z-index: 1990; }
  :global(.muted-card) {
    background-color: var(--bs-body-bg);
    opacity: 0.94;
  }
  :global(.muted-card .badge) {
    opacity: 0.85;
  }
  :global(.muted-card .state-label) {
    color: var(--bs-secondary-color) !important;
    font-weight: 600;
  }
  .stat-label {
    font-size: 0.75rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--bs-secondary-color);
  }
  .memory-progress {
    height: 8px;
  }
  :global(.chart-card .card-body) {
    min-height: 260px;
  }
</style>
