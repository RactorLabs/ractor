const REQUIRED_ENV_VARS = ['RACTOR_APPS_GITHEX_ADMIN_TOKEN', 'RACTOR_HOST_URL'];

function ensureEnv() {
  const missing = REQUIRED_ENV_VARS.filter((key) => !process.env[key] || process.env[key].trim() === '');
  if (missing.length) {
    throw new Error(`Missing environment variables: ${missing.join(', ')}`);
  }
}

function sanitize(segment) {
  if (!segment || typeof segment !== 'object') return null;
  try {
    return JSON.parse(JSON.stringify(segment));
  } catch (_) {
    return null;
  }
}

export default async function handler(req, res) {
  if (req.method !== 'GET') {
    res.setHeader('Allow', ['GET']);
    return res.status(405).json({ error: 'Method not allowed' });
  }

  const { agent, response } = req.query || {};
  if (!agent || !response) {
    return res.status(400).json({ error: 'Missing agent or response identifier' });
  }

  try {
    ensureEnv();
  } catch (err) {
    return res.status(500).json({ error: err.message });
  }

  const token = process.env.RACTOR_APPS_GITHEX_ADMIN_TOKEN;
  const host = process.env.RACTOR_HOST_URL.replace(/\/$/, '');
  const agentId = encodeURIComponent(Array.isArray(agent) ? agent[0] : agent);
  const responseId = encodeURIComponent(Array.isArray(response) ? response[0] : response);
  const target = `${host}/api/v0/agents/${agentId}/responses/${responseId}`;

  try {
    const upstream = await fetch(target, {
      headers: {
        Authorization: `Bearer ${token}`,
        Accept: 'application/json',
        'User-Agent': 'ractor-githex-app'
      }
    });

    const text = await upstream.text();
    let payload;
    try {
      payload = JSON.parse(text);
    } catch (_) {
      payload = { error: 'Unexpected response from Ractor API', raw: text };
    }

    if (!upstream.ok) {
      return res.status(upstream.status).json(payload);
    }

    if (Array.isArray(payload?.segments)) {
      payload.segments = payload.segments.map(sanitize).filter(Boolean);
    }

    if (Array.isArray(payload?.output_content)) {
      payload.output_content = payload.output_content.map(sanitize).filter(Boolean);
    }

    return res.status(200).json(payload);
  } catch (error) {
    console.error('[GitHex] Failed to proxy Ractor response:', error);
    return res.status(500).json({ error: 'Failed to fetch response status from Ractor' });
  }
}
