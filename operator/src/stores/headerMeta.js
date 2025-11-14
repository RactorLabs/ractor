import { writable } from 'svelte/store';

export const headerMeta = writable({
  model: '',
  url: ''
});

export function setHeaderMeta(meta) {
  headerMeta.set({
    model: meta?.model || '',
    url: meta?.url || ''
  });
}

export function clearHeaderMeta() {
  headerMeta.set({ model: '', url: '' });
}
