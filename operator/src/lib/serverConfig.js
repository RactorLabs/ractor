import fs from 'fs';
import os from 'os';
import path from 'path';

function expandHome(p) {
  if (!p) return p;
  if (p === '~') return os.homedir();
  if (p.startsWith('~/')) {
    return path.join(os.homedir(), p.slice(2));
  }
  return p;
}

function normalizeModel(model) {
  if (!model || typeof model.name !== 'string') {
    return null;
  }
  const name = model.name.trim();
  if (!name) {
    return null;
  }
  const displayName = (model.display_name || name).trim() || name;
  return { name, display_name: displayName };
}

function normalizeProvider(raw) {
  if (!raw || typeof raw.name !== 'string' || typeof raw.url !== 'string') {
    return null;
  }
  const name = raw.name.trim();
  const url = raw.url.trim();
  if (!name || !url) {
    return null;
  }
  const models = Array.isArray(raw.models)
    ? raw.models.map(normalizeModel).filter(Boolean)
    : [];
  if (!models.length) {
    return null;
  }
  const defaultModel =
    typeof raw.default_model === 'string'
      ? (models.find((m) => m.name.toLowerCase() === raw.default_model.trim().toLowerCase()) || models[0]).name
      : models[0].name;

  return {
    name,
    display_name: name,
    url,
    models,
    default_model: defaultModel,
    is_default: false
  };
}

function normalizeProviders(rawProviders, defaultProviderName) {
  const providers = [];
  if (Array.isArray(rawProviders)) {
    for (const raw of rawProviders) {
      const normalized = normalizeProvider(raw);
      if (normalized) {
        providers.push(normalized);
      }
    }
  }

  if (providers.length) {
    const targetName = typeof defaultProviderName === 'string' ? defaultProviderName.trim().toLowerCase() : '';
    let matched = false;
    providers.forEach((provider, idx) => {
      if (targetName && provider.name.toLowerCase() === targetName) {
        provider.is_default = true;
        matched = true;
      } else {
        provider.is_default = false;
      }
    });
    if (!matched) {
      providers.forEach((provider, idx) => {
        provider.is_default = idx === 0;
      });
    }
  }

  return providers;
}

export function loadServerConfig() {
  if (typeof process === 'undefined' || !process.versions?.node) {
    return {
      hostName: 'TSBX',
      hostUrl: 'http://localhost',
      inferenceProviders: []
    };
  }

  const candidate =
    (process.env.TSBX_CONFIG_PATH && process.env.TSBX_CONFIG_PATH.trim()) ||
    path.join(os.homedir(), '.tsbx', 'tsbx.json');
  const resolved = path.resolve(expandHome(candidate));

  try {
    const raw = fs.readFileSync(resolved, 'utf8');
    const parsed = JSON.parse(raw);
    const hostName = (parsed?.host?.name || 'TSBX').trim() || 'TSBX';
    const hostUrl = String(parsed?.host?.url || 'http://localhost')
      .trim()
      .replace(/\/$/, '') || 'http://localhost';

    return {
      hostName,
      hostUrl,
      inferenceProviders: normalizeProviders(parsed?.inference?.providers, parsed?.inference?.default_provider)
    };
  } catch (_) {
    return {
      hostName: 'TSBX',
      hostUrl: 'http://localhost',
      inferenceProviders: []
    };
  }
}
