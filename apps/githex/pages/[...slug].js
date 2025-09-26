import Head from 'next/head';
import { useEffect, useMemo, useState } from 'react';
import { useRouter } from 'next/router';
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

// Find the commentary closest to the last tool_result
function extractCommentaryNearLastTool(segments) {
  if (!Array.isArray(segments) || segments.length === 0) return null;
  let lastToolIdx = -1;
  for (let i = segments.length - 1; i >= 0; i -= 1) {
    const it = segments[i];
    const t = (it?.type || '').toLowerCase();
    if (t === 'tool_result') { lastToolIdx = i; break; }
  }
  if (lastToolIdx < 0) return extractLatestCommentary(segments);
  // Search backwards from the last tool_result for a commentary segment
  for (let j = lastToolIdx - 1; j >= 0; j -= 1) {
    const it = segments[j];
    const t = (it?.type || '').toLowerCase();
    if (t === 'commentary') {
      const text = it?.text || it?.content || '';
      if (typeof text === 'string' && text.trim()) return text;
    }
  }
  // Fallback to any latest commentary if none found before
  return extractLatestCommentary(segments);
}

function renderOutputItems(items) {
  if (!Array.isArray(items) || items.length === 0) {
    return (
      <p className="output-panel__empty">
        The agent did not return any roast content.
      </p>
    );
  }

  const filtered = items.filter((item) => {
    if (!item || typeof item !== 'object') return false;
    const type = (item.type || '').toLowerCase();
    if (type === 'tool_call') return false;
    const content = item.content ?? '';
    if (typeof content === 'string' && content.trim().startsWith('{"tool_call"')) {
      return false;
    }
    return true;
  });

  return filtered.map((item, index) => {
    if (!item || typeof item !== 'object') return null;
    const type = (item.type || '').toLowerCase();
    const title = item.title || `Result ${index + 1}`;

    if (type === 'markdown' || type === 'text') {
      const markdown = typeof item.content === 'string' ? item.content : '';
      if (markdown.trim().startsWith('{"tool_call"')) {
        return null;
      }
      const html = marked.parse(markdown || '');
      return (
        <section className="output-panel__item" key={`out-${index}`}>
          <div
            className="output-panel__markdown"
            dangerouslySetInnerHTML={{ __html: html }}
          />
        </section>
      );
    }

    if (type === 'json') {
      const value = item.content ?? item;
      const formatted = typeof value === 'string' ? value : JSON.stringify(value, null, 2);
      if (formatted.trim().startsWith('{"tool_call"')) {
        return null;
      }
      return (
        <section className="output-panel__item" key={`out-${index}`}>
          <h3 className="output-panel__title">{title}</h3>
          <pre className="output-panel__json">{formatted}</pre>
        </section>
      );
    }

    if (type === 'url') {
      const href = item.content || item.url || '#';
      return (
        <section className="output-panel__item" key={`out-${index}`}>
          <h3 className="output-panel__title">{title}</h3>
          <a className="output-panel__link" href={href} target="_blank" rel="noreferrer">
            {href}
          </a>
        </section>
      );
    }

    return (
      <section className="output-panel__item" key={`out-${index}`}>
        <h3 className="output-panel__title">{title}</h3>
        <pre className="output-panel__json">{JSON.stringify(item, null, 2)}</pre>
      </section>
    );
  });
}

function sanitizeSlug(value) {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9-]+/g, '-')
    .replace(/^-+/, '')
    .replace(/-+$/, '')
    .slice(0, 48) || 'repo';
}

function createAgentName(owner, name) {
  const ownerPart = sanitizeSlug(owner);
  const namePart = sanitizeSlug(name);
  const suffix = Math.random().toString(36).slice(2, 8);
  const base = `githex-${ownerPart}-${namePart}-${suffix}`;
  return base.slice(0, 64).replace(/-+$/, '') || `githex-${suffix}`;
}

export default function RepoPage({
  owner,
  name,
  repoUrl,
  agentName,
  response: initialResponse,
  responseId: initialResponseId,
  setupError
}) {
  const router = useRouter();
  const normalizedInitial = useMemo(() => normalizeResponse(initialResponse), [initialResponse]);
  const [response, setResponse] = useState(normalizedInitial);
  const derivedResponseId = response?.id || initialResponseId || null;
  const derivedAgentName = response?.agent_name || agentName || null;
  const [isPolling, setIsPolling] = useState(() => Boolean(derivedResponseId && !isTerminal((normalizedInitial?.status) || 'pending')));
  const [pollError, setPollError] = useState(null);

  // Single-route mode: keep URL as /owner/repo; no response id in URL

  useEffect(() => {
    if (!derivedAgentName || !derivedResponseId || !isPolling) {
      return undefined;
    }

    let cancelled = false;
    const interval = setInterval(async () => {
      try {
        const res = await fetch(`/api/raworc/responses/${encodeURIComponent(derivedAgentName)}/${encodeURIComponent(derivedResponseId)}`);
        if (!res.ok) {
          throw new Error(`Polling failed with status ${res.status}`);
        }
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

    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [derivedAgentName, derivedResponseId, isPolling]);

  const status = (response?.status || 'pending').toLowerCase();
  const commentary = useMemo(() => {
    if (isTerminal(status)) return null;
    return extractLatestCommentary(response?.segments);
  }, [status, response?.segments]);

  const bannerText = isTerminal(status)
    ? `Roast completed for ${owner}/${name}`
    : formatCommentary(commentary) || `Roasting ${owner}/${name}…`;

 const outputItems = useMemo(() => {
   if (!isTerminal(status)) return [];
   const items = Array.isArray(response?.output_content) ? response.output_content : [];
   if (items.length > 0) return items;
   // Fallback: show commentary nearest to last tool item
   const near = extractCommentaryNearLastTool(response?.segments);
   if (near && typeof near === 'string' && near.trim().length > 0) {
     return [{ type: 'markdown', title: 'Result', content: near }];
   }
   return [];
 }, [response?.output_content, response?.segments, status]);

  const isFailed = status === 'failed';
  const isCancelled = status === 'cancelled';
  const missingSetup = setupError || !derivedAgentName || !derivedResponseId;

  if (missingSetup) {
    return (
      <main>
        <Head>
          <title>{`${owner}/${name} · GitHex`}</title>
        </Head>
        <section className="hero repo-hero">
          <p className="clone-banner" aria-live="polite">
            <span className="clone-text">GitHex configuration is incomplete</span>
          </p>
          <p className="poll-error">
            {setupError || 'Required Raworc credentials are missing. Set RAWORC_HOST_URL and RAWORC_APPS_GITHEX_ADMIN_TOKEN before using GitHex.'}
          </p>
        </section>
      </main>
    );
  }

  return (
    <main>
      <Head>
        <title>{`${owner}/${name} · GitHex`}</title>
      </Head>
      {!isTerminal(status) && (
        <section className="hero repo-hero">
          <p className="clone-banner" aria-live="polite">
            <span className="clone-text">{bannerText}</span>
          </p>
          {pollError && (
            <p className="poll-error" aria-live="polite">{pollError}</p>
          )}
        </section>
      )}

      {isTerminal(status) && (
        <section className="output-panel" aria-live="polite">
          {isFailed && (
            <p className="output-panel__error">
              The agent reported a failure while roasting {owner}/{name}. Check Raworc logs for more detail.
            </p>
          )}
          {isCancelled && (
            <p className="output-panel__error">
              The agent cancelled the request before completion. Try again later.
            </p>
          )}
          {!isFailed && !isCancelled && renderOutputItems(outputItems)}
        </section>
      )}
    </main>
  );
}

export async function getServerSideProps(context) {
  const { params, query } = context;
  const slug = Array.isArray(params?.slug) ? params.slug : [];

  if (slug.length < 2) {
    return {
      redirect: {
        destination: '/',
        permanent: false
      }
    };
  }

  const [owner, name] = slug;
  const repoUrl = `https://github.com/${encodeURIComponent(owner)}/${encodeURIComponent(name)}`;

  try {
    const githubRes = await fetch(`https://api.github.com/repos/${encodeURIComponent(owner)}/${encodeURIComponent(name)}`, {
      headers: {
        'User-Agent': 'raworc-githex-app',
        Accept: 'application/vnd.github+json'
      }
    });

    if (!githubRes.ok) {
      throw new Error(`GitHub responded with ${githubRes.status}`);
    }

    const repoInfo = await githubRes.json();
    if (repoInfo?.private) {
      throw new Error('Repository is private');
    }
  } catch (error) {
    const repoLabel = encodeURIComponent(`${owner}/${name}`);
    return {
      redirect: {
        destination: `/?error=repo_inaccessible&repo=${repoLabel}`,
        permanent: false
      }
    };
  }

  const adminToken = process.env.RAWORC_APPS_GITHEX_ADMIN_TOKEN;
  const raworcHost = process.env.RAWORC_HOST_URL;

  if (!adminToken || !raworcHost) {
    return {
      props: {
        owner,
        name,
        repoUrl,
        agentName: null,
        response: null,
        responseId: null,
        setupError: 'Required Raworc credentials are missing.'
      }
    };
  }

  const base = raworcHost.endsWith('/') ? raworcHost.slice(0, -1) : raworcHost;
  const headers = {
    Authorization: `Bearer ${adminToken}`,
    Accept: 'application/json',
    'Content-Type': 'application/json',
    'User-Agent': 'raworc-githex-app'
  };

  // No local storage; rely on Raworc API for response resolution

  // No response id in URL — resolve or create response internally

  // Either reuse an existing agent by tag or create a fresh one,
  // then resolve/create a response and render on this page
  const tagValue = `${owner}/${name}`;
  // Try to find existing agent by tag
  try {
    const listRes = await fetch(`${base}/api/v0/agents?tags=${encodeURIComponent(tagValue)}&limit=1`, { headers });
    if (listRes.ok) {
      const page = await listRes.json();
      const found = Array.isArray(page.items) && page.items.length ? page.items[0] : null;
      if (found && found.name) {
        // Try to use the most recent response; if none, create a new one
        let chosen = null;
        try {
          const cntRes = await fetch(`${base}/api/v0/agents/${encodeURIComponent(found.name)}/responses/count`, { headers });
          if (cntRes.ok) {
            const cntObj = await cntRes.json();
            const total = Number(cntObj?.count || 0);
            if (total > 0) {
              const batchOffset = Math.max(0, total - 10);
              const lastBatch = await fetch(`${base}/api/v0/agents/${encodeURIComponent(found.name)}/responses?limit=10&offset=${batchOffset}`, { headers });
              if (lastBatch.ok) {
                const list = await lastBatch.json();
                if (Array.isArray(list) && list.length > 0) {
                  // Prefer latest completed with output items;
                  // otherwise latest with any commentary; else latest entry
                  const completedWithItems = list.filter(r => String(r?.status||'').toLowerCase()==='completed' && Array.isArray(r?.output_content) && r.output_content.length>0);
                  if (completedWithItems.length) {
                    chosen = completedWithItems[completedWithItems.length-1];
                  } else {
                    const withCommentary = list.filter(r => Array.isArray(r?.segments) && r.segments.some(s => String(s?.type||'').toLowerCase()==='commentary' && typeof (s?.text ?? s?.content) === 'string' && String((s?.text ?? s?.content)).trim().length>0));
                    chosen = withCommentary.length ? withCommentary[withCommentary.length-1] : list[list.length-1];
                  }
                }
              }
            }
          }
        } catch (_) {}
        if (chosen) {
          const resp = await fetch(`${base}/api/v0/responses/${encodeURIComponent(chosen.id)}`, { headers });
          if (!resp.ok) return { notFound: true };
          const responseView = await resp.json();
          return {
            props: {
              owner,
              name,
              repoUrl,
              agentName: found.name,
              response: responseView,
              responseId: responseView.id,
              setupError: null
            }
          };
        }
        // No responses found; create a new response for the existing agent
        const messageBody = {
          input: {
            content: [
              { type: 'text', content: `Clone ${repoUrl}. After cloning, produce an unfiltered roast of this repository: call out poor structure, questionable decisions, missing docs, flaky scripts, or any other red flags. Be witty but factual, cite evidence, and respond strictly in Markdown.` }
            ]
          }
        };
        const responseRes = await fetch(`${base}/api/v0/agents/${encodeURIComponent(found.name)}/responses`, { method: 'POST', headers, body: JSON.stringify(messageBody) });
        if (!responseRes.ok) throw new Error('Failed to enqueue response');
        const response = await responseRes.json();
        return {
          props: {
            owner,
            name,
            repoUrl,
            agentName: found.name,
            response,
            responseId: response.id,
            setupError: null
          }
        };
      }
    }
  } catch (e) {
    console.warn('[GitHex] Agent reuse check failed:', e);
  }

  const agentName = createAgentName(owner, name);

  try {
    const agentPayload = {
      name: agentName,
      description: `GitHex roast agent for ${owner}/${name}`,
      tags: ['githex', 'roast', tagValue],
      metadata: {
        source: 'githex',
        repository: {
          owner,
          name,
          url: repoUrl
        }
      },
      instructions:
        'You are a no-nonsense repository roasting agent. Clone the assigned repository, inspect its structure, configuration, and scripts, and craft a witty yet evidence-based critique pointing out flaws or red flags.'
    };

    const createAgentRes = await fetch(`${base}/api/v0/agents`, {
      method: 'POST',
      headers,
      body: JSON.stringify(agentPayload)
    });

    if (!createAgentRes.ok) {
      const text = await createAgentRes.text();
      console.error('[GitHex] Failed to create agent:', createAgentRes.status, text);
      throw new Error('Failed to create agent');
    }

    const messageBody = {
      input: {
        content: [
          {
            type: 'text',
            content: `Clone ${repoUrl}. After cloning, produce an unfiltered roast of this repository: call out poor structure, questionable decisions, missing docs, flaky scripts, or any other red flags. Be witty but factual, cite evidence, and respond strictly in Markdown.`
          }
        ]
      }
    };

    const responseRes = await fetch(`${base}/api/v0/agents/${encodeURIComponent(agentName)}/responses`, {
      method: 'POST',
      headers,
      body: JSON.stringify(messageBody)
    });

    if (!responseRes.ok) {
      const text = await responseRes.text();
      console.error('[GitHex] Failed to enqueue response:', responseRes.status, text);
      throw new Error('Failed to enqueue response');
    }

    const response = await responseRes.json();
    return {
      props: {
        owner,
        name,
        repoUrl,
        agentName,
        response,
        responseId: response.id,
        setupError: null
      }
    };
  } catch (error) {
    console.error('[GitHex] Error preparing agent workflow:', error);
    const repoLabel = encodeURIComponent(`${owner}/${name}`);
    return {
      redirect: {
        destination: `/?error=repo_inaccessible&repo=${repoLabel}`,
        permanent: false
      }
    };
  }
}
