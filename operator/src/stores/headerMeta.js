import { writable } from 'svelte/store';

const normalizeMeta = (meta) => ({
  model: typeof meta?.model === 'string' ? meta.model.trim() : '',
  url: typeof meta?.url === 'string' ? meta.url.trim() : ''
});

let fallbackMeta = normalizeMeta({});

export const headerMeta = writable(fallbackMeta);

export function setHeaderMeta(meta) {
  const normalized = normalizeMeta(meta);
  headerMeta.set({
    model: normalized.model || fallbackMeta.model,
    url: normalized.url || fallbackMeta.url
  });
}

export function setHeaderMetaDefaults(meta) {
  fallbackMeta = normalizeMeta(meta);
  headerMeta.set(fallbackMeta);
}

export function clearHeaderMeta() {
  headerMeta.set(fallbackMeta);
}
