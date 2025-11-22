export async function load({ fetch, cookies }) {
  const hostName = process.env.TSBX_HOST_NAME || 'TSBX';
  const hostUrl = (process.env.TSBX_HOST_URL || 'http://localhost').replace(/\/$/, '');
  const envInferenceName = (process.env.TSBX_INFERENCE_NAME || '').trim();
  const envInferenceUrl = (process.env.TSBX_INFERENCE_URL || '').trim();
  const envInferenceModels = (process.env.TSBX_INFERENCE_MODELS || '')
    .split(',')
    .map((s) => s.trim())
    .filter((s) => s.length);

  let globalStats = null;
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
      // Ignore stats fetch errors; UI will fall back to env values
    }
  }

  if (!globalStats) {
    globalStats = {
      sandboxes_total: 0,
      sandboxes_active: 0,
      sandboxes_terminated: 0,
      sandboxes_by_state: {},
      inference_name: envInferenceName || null,
      inference_url: envInferenceUrl || null,
      inference_models: envInferenceModels,
      default_inference_model: envInferenceModels[0] || null,
      captured_at: new Date().toISOString()
    };
  } else {
    if ((!globalStats.inference_models || !globalStats.inference_models.length) && envInferenceModels.length) {
      globalStats.inference_models = envInferenceModels;
    }
    if (!globalStats.default_inference_model && envInferenceModels.length) {
      globalStats.default_inference_model = envInferenceModels[0];
    }
    if (!globalStats.inference_url && envInferenceUrl) {
      globalStats.inference_url = envInferenceUrl;
    }
    if (!globalStats.inference_name && envInferenceName) {
      globalStats.inference_name = envInferenceName;
    }
  }

  return {
    hostName,
    hostUrl,
    globalStats
  };
}
