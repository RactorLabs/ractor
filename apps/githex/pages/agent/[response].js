import Head from 'next/head';
import { useEffect, useMemo, useState } from 'react';
import { marked } from 'marked';

const TERMINAL_STATUSES = new Set(['completed', 'failed', 'cancelled']);
marked.setOptions({ breaks: true });

function isTerminal(status) {
  if (!status) return false;
  return TERMINAL_STATUSES.has(String(status).toLowerCase());
}

function normalizeResponse(resp) {
  if (!resp || typeof resp !== 'object') return null;
  const segments = Array.isArray(resp.segments) ? resp.segments : [];
  const output = Array.isArray(resp.output_content) ? resp.output_content : [];
  return { ...resp, segments, output_content: output };
}

function renderOutputItems(items) {
  if (!Array.isArray(items) || items.length === 0) {
    return (
      <p className="output-panel__empty">
        No output items available for this response yet.
      </p>
    );
  }
  return items.map((item, idx) => {
    if (!item || typeof item !== 'object') return null;
    const type = (item.type || '').toLowerCase();
    const title = item.title || `Result ${idx + 1}`;
    if (type === 'markdown' || type === 'text') {
      const markdown = typeof item.content === 'string' ? item.content : '';
      const html = marked.parse(markdown || '');
      return (
        <section className="output-panel__item" key={`out-${idx}`}>
          <div className="output-panel__markdown" dangerouslySetInnerHTML={{ __html: html }} />
        </section>
      );
    }
    if (type === 'json') {
      const value = item.content ?? item;
      const formatted = typeof value === 'string' ? value : JSON.stringify(value, null, 2);
      return (
        <section className="output-panel__item" key={`out-${idx}`}>
          <h3 className="output-panel__title">{title}</h3>
          <pre className="output-panel__json">{formatted}</pre>
        </section>
      );
    }
    if (type === 'url') {
      const href = item.content || item.url || '#';
      return (
        <section className="output-panel__item" key={`out-${idx}`}>
          <h3 className="output-panel__title">{title}</h3>
          <a className="output-panel__link" href={href} target="_blank" rel="noreferrer">{href}</a>
        </section>
      );
    }
    return (
      <section className="output-panel__item" key={`out-${idx}`}>
        <h3 className="output-panel__title">{title}</h3>
        <pre className="output-panel__json">{JSON.stringify(item, null, 2)}</pre>
      </section>
    );
  });
}

export default function ResponsePage({ agentName, response: initialResponse, responseId, setupError }) {
  const normalizedInitial = useMemo(() => normalizeResponse(initialResponse), [initialResponse]);
  const [response, setResponse] = useState(normalizedInitial);
  const derivedResponseId = response?.id || responseId || null;
  const derivedAgentName = response?.agent_name || agentName || null;
  const [isPolling, setIsPolling] = useState(() => Boolean(derivedResponseId && !isTerminal((normalizedInitial?.status) || 'pending')));
  const [pollError, setPollError] = useState(null);

  useEffect(() => {
    if (!derivedAgentName || !derivedResponseId || !isPolling) return undefined;
    let cancelled = false;
    const interval = setInterval(async () => {
      try {
        const res = await fetch(`/api/raworc/responses/${encodeURIComponent(derivedAgentName)}/${encodeURIComponent(derivedResponseId)}`);
        if (!res.ok) throw new Error(`Polling failed with status ${res.status}`);
        const data = normalizeResponse(await res.json());
        if (!cancelled && data) {
          setResponse(data);
          setPollError(null);
          if (isTerminal(data.status)) {
            setIsPolling(false);
            clearInterval(interval);
          }
        }
      } catch (err) {
        if (!cancelled) {
          console.error('[GitHex] Polling error', err);
          setPollError('Temporary issue polling agent status…');
        }
      }
    }, 2500);
    return () => { cancelled = true; clearInterval(interval); };
  }, [derivedAgentName, derivedResponseId, isPolling]);

  const status = (response?.status || 'pending').toLowerCase();
  const isFailed = status === 'failed';
  const isCancelled = status === 'cancelled';
  const missingSetup = setupError || !derivedAgentName || !derivedResponseId;

  if (missingSetup) {
    return (
      <main>
        <Head>
          <title>GitHex · Response</title>
        </Head>
        <section className="hero repo-hero">
          <p className="clone-banner" aria-live="polite">
            <span className="clone-text">GitHex configuration is incomplete</span>
          </p>
          <p className="poll-error">{setupError || 'Missing mapping for response/agent or required credentials.'}</p>
        </section>
      </main>
    );
  }

  return (
    <main>
      <Head>
        <title>GitHex · Response</title>
      </Head>
      {!isTerminal(status) && (
        <section className="hero repo-hero">
          <p className="clone-banner" aria-live="polite">
            <span className="clone-text">Processing…</span>
          </p>
          {pollError && (<p className="poll-error" aria-live="polite">{pollError}</p>)}
        </section>
      )}
      {isTerminal(status) && (
        <section className="output-panel" aria-live="polite">
          {isFailed && (<p className="output-panel__error">The agent reported a failure. Check Raworc logs for more detail.</p>)}
          {isCancelled && (<p className="output-panel__error">The agent cancelled the request before completion.</p>)}
          {!isFailed && !isCancelled && renderOutputItems(response?.output_content || [])}
        </section>
      )}
    </main>
  );
}

export async function getServerSideProps(context) {
  const responseId = context?.params?.response || null;
  const adminToken = process.env.RAWORC_APPS_GITHEX_ADMIN_TOKEN;
  const raworcHost = process.env.RAWORC_HOST_URL;

  if (!responseId || !adminToken || !raworcHost) {
    return { props: { agentName: null, response: null, responseId: responseId || null, setupError: 'Missing context or credentials' } };
  }

  const base = raworcHost.endsWith('/') ? raworcHost.slice(0, -1) : raworcHost;
  const headers = {
    Authorization: `Bearer ${adminToken}`,
    Accept: 'application/json',
    'Content-Type': 'application/json',
    'User-Agent': 'raworc-githex-app'
  };

  const fs = await import('fs/promises');
  const pathMod = await import('path');
  const storageDir = pathMod.join(process.cwd(), 'storage');
  const runsPath = pathMod.join(storageDir, 'runs.json');

  let agentName = null;
  try {
    const raw = await fs.readFile(runsPath, 'utf8');
    const map = JSON.parse(raw || '{}');
    agentName = map[responseId] || null;
  } catch (_) {
    agentName = null;
  }

  // Helper: attempt to locate the agent by scanning GitHex-tagged agents
  async function probeAgentsForResponse() {
    let page = 1;
    const pageSize = 200;
    const maxPages = 10; // scan up to ~2000 agents
    while (page <= maxPages) {
      const url = `${base}/api/v0/agents?tags=githex&limit=${pageSize}&page=${page}`;
      const list = await fetch(url, { headers });
      if (!list.ok) break;
      const data = await list.json();
      const items = Array.isArray(data?.items) ? data.items : [];
      for (const a of items) {
        const name = a?.name;
        if (!name) continue;
        const r = await fetch(`${base}/api/v0/agents/${encodeURIComponent(name)}/responses/${encodeURIComponent(responseId)}`, { headers });
        if (r.ok) {
          const responseView = await r.json();
          return { agent: name, responseView };
        }
      }
      const totalPages = Number(data?.pages || 1);
      if (page >= totalPages) break;
      page += 1;
    }
    return null;
  }

  if (!agentName) {
    try {
      const found = await probeAgentsForResponse();
      if (found && found.agent && found.responseView) {
        agentName = found.agent;
        // persist mapping
        try {
          const raw = await fs.readFile(runsPath, 'utf8').catch(() => '{}');
          const map = JSON.parse(raw || '{}');
          map[responseId] = agentName;
          await fs.mkdir(storageDir, { recursive: true });
          await fs.writeFile(runsPath, JSON.stringify(map, null, 2), 'utf8');
        } catch (_) {}
        return { props: { agentName, response: found.responseView, responseId, setupError: null } };
      }
    } catch (_) {}
    return { props: { agentName: null, response: null, responseId, setupError: 'Unknown response id' } };
  }

  try {
    const resp = await fetch(`${base}/api/v0/agents/${encodeURIComponent(agentName)}/responses/${encodeURIComponent(responseId)}`, { headers });
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
    const responseView = await resp.json();
    // Refresh mapping if agent name is attached
    try {
      const raw = await fs.readFile(runsPath, 'utf8').catch(() => '{}');
      const map = JSON.parse(raw || '{}');
      map[responseId] = responseView.agent_name || agentName;
      await fs.mkdir(storageDir, { recursive: true });
      await fs.writeFile(runsPath, JSON.stringify(map, null, 2), 'utf8');
    } catch (_) {}
    return { props: { agentName: responseView.agent_name || agentName, response: responseView, responseId, setupError: null } };
  } catch (e) {
    return { props: { agentName, response: null, responseId, setupError: 'Failed to load response' } };
  }
}
