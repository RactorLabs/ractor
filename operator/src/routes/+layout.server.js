import { loadServerConfig } from '$lib/serverConfig.js';

export async function load({ fetch, cookies }) {
  const serverConfig = loadServerConfig();
  const hostName = serverConfig.hostName;
  const hostUrl = serverConfig.hostUrl;

  let globalStats = null;
  let inferenceProviders = serverConfig.inferenceProviders;
  const token = cookies.get('tsbx_token');
  if (token) {
    try {
      const res = await fetch('/api/v0/stats', {
        headers: {
          Authorization: `Bearer ${token}`
        }
      });
      if (res.ok) {
        globalStats = await res.json();
      }
    } catch (_) {
      // ignore stats fetch errors; fallback below
    }

    try {
      const providerRes = await fetch('/api/v0/inference/providers', {
        headers: {
          Authorization: `Bearer ${token}`
        }
      });
      if (providerRes.ok) {
        inferenceProviders = await providerRes.json();
      }
    } catch (_) {
      // ignore provider fetch errors
    }
  }

  const defaultProvider =
    inferenceProviders.find((p) => p.is_default) || inferenceProviders[0] || null;

  if (!globalStats) {
    globalStats = {
      sandboxes_total: 0,
      sandboxes_active: 0,
      sandboxes_terminated: 0,
      sandboxes_by_state: {},
      sandbox_tasks_total: 0,
      sandbox_tasks_active: 0,
      inference_name: defaultProvider?.display_name || null,
      inference_url: defaultProvider?.url || null,
      inference_models: defaultProvider ? defaultProvider.models.map((m) => m.name) : [],
      default_inference_model: defaultProvider?.default_model || null,
      captured_at: new Date().toISOString(),
      host: null
    };
  }

  return {
    hostName,
    hostUrl,
    globalStats,
    inferenceProviders
  };
}
