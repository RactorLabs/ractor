import { writable } from 'svelte/store';

let nextId = 1;

export const toasts = writable([]);

export function pushToast({ title = '', message = '', variant = 'info', timeout = 4000 } = {}) {
  const id = nextId++;
  const item = { id, title, message, variant, timeout };
  toasts.update((list) => [...list, item]);
  if (timeout && timeout > 0) {
    setTimeout(() => removeToast(id), timeout);
  }
  return id;
}

export function removeToast(id) {
  toasts.update((list) => list.filter((t) => t.id !== id));
}

export const toast = {
  info: (message, title = '') => pushToast({ message, title, variant: 'info' }),
  success: (message, title = '') => pushToast({ message, title, variant: 'success' }),
  warning: (message, title = '') => pushToast({ message, title, variant: 'warning' }),
  error: (message, title = '') => pushToast({ message, title, variant: 'danger' }),
};

