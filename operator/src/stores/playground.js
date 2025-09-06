import { writable } from 'svelte/store';

const STORAGE_KEY = 'raworc_playground';

function createPlaygroundStore() {
  let initial = { token: '', remember: true };
  if (typeof window !== 'undefined') {
    try {
      const saved = sessionStorage.getItem(STORAGE_KEY) || localStorage.getItem(STORAGE_KEY);
      if (saved) initial = { ...initial, ...JSON.parse(saved) };
    } catch (_) {}
  }
  const store = writable(initial);
  if (typeof window !== 'undefined') {
    store.subscribe((val) => {
      try {
        const data = JSON.stringify({ token: val.token || '', remember: !!val.remember });
        if (val.remember) {
          localStorage.setItem(STORAGE_KEY, data);
          sessionStorage.removeItem(STORAGE_KEY);
        } else {
          sessionStorage.setItem(STORAGE_KEY, data);
          localStorage.removeItem(STORAGE_KEY);
        }
      } catch (_) {}
    });
  }
  return store;
}

export const playground = createPlaygroundStore();

export function clearPlaygroundToken() {
  try {
    if (typeof window !== 'undefined') {
      sessionStorage.removeItem(STORAGE_KEY);
      localStorage.removeItem(STORAGE_KEY);
    }
  } catch (_) {}
  playground.set({ token: '', remember: false });
}
