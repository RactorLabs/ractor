export async function load({ fetch, cookies }) {
  const hostName = process.env.TSBX_HOST_NAME || 'TSBX';
  const hostUrl = (process.env.TSBX_HOST_URL || 'http://localhost').replace(/\/$/, '');
  const envInferenceModel = (process.env.TSBX_INFERENCE_MODEL || '').trim();
  const envInferenceUrl = (process.env.TSBX_INFERENCE_URL || '').trim();

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

  const inferenceModel = (globalStats && globalStats.inference_model) || envInferenceModel;
  const inferenceUrl = (globalStats && globalStats.inference_url) || envInferenceUrl;

  return {
    hostName,
    hostUrl,
    inferenceModel,
    inferenceUrl,
    globalStats
  };
}
