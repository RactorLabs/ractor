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

function extractLatestCommentary(segments) {
  if (!Array.isArray(segments)) return null;
  for (let idx = segments.length - 1; idx >= 0; idx -= 1) {
    const entry = segments[idx];
    if (!entry || typeof entry !== 'object') continue;
    const type = (entry.type || '').toLowerCase();
    if (type === 'tool_call') {
      const commentary = entry?.args?.commentary || entry.commentary;
      if (commentary && typeof commentary === 'string' && !commentary.trim().startsWith('{')) {
        return commentary;
      }
    }
  }
  return null;
}

function formatCommentary(commentary) {
  if (!commentary) return null;
  const cleaned = String(commentary).replace(/\s+/g, ' ').trim();
  if (!cleaned) return null;
  if (cleaned.length <= 140) return cleaned;
  const sentences = cleaned.split(/(?<=[.!?])\s+/).filter(Boolean);
  if (sentences.length > 0) {
    const lastSentence = sentences[sentences.length - 1];
    if (lastSentence.length <= 140) return lastSentence;
  }
  return `${cleaned.slice(0, 137)}…`;
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

  const repoOwner = response?.metadata?.repository?.owner || response?.agent_metadata?.repository?.owner || null;
  const repoName = response?.metadata?.repository?.name || response?.agent_metadata?.repository?.name || null;
  const bannerText = isTerminal(status)
    ? (repoOwner && repoName ? `Roast completed for ${repoOwner}/${repoName}` : 'Roast completed')
    : formatCommentary(extractLatestCommentary(response?.segments)) || (repoOwner && repoName ? `Roasting ${repoOwner}/${repoName}…` : 'Processing…');

  return (
    <main>
      <Head>
        <title>{repoOwner && repoName ? `${repoOwner}/${repoName} · GitHex` : 'GitHex · Response'}</title>
      </Head>
      {!isTerminal(status) && (
        <section className="hero repo-hero">
          <p className="clone-banner" aria-live="polite">
            <span className="clone-text">{bannerText}</span>
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

  // No local mapping; rely on Raworc API

  // First, prefer Raworc global response lookup
  try {
    const r = await fetch(`${base}/api/v0/responses/${encodeURIComponent(responseId)}`, { headers });
    if (r.ok) {
      const responseView = await r.json();
      const agentName = responseView?.agent_name || null;
      // Enrich with agent metadata (owner/repo) if available
      let enriched = responseView;
      if (agentName) {
        try {
          const a = await fetch(`${base}/api/v0/agents/${encodeURIComponent(agentName)}`, { headers });
          if (a.ok) {
            const agentObj = await a.json();
            enriched = { ...responseView, agent_metadata: agentObj?.metadata || {} };
          }
        } catch (_) {}
      }
      return { props: { agentName, response: enriched, responseId, setupError: null } };
    }
  } catch (_) {}
  return { props: { agentName: null, response: null, responseId, setupError: 'Unknown response id' } };
}
