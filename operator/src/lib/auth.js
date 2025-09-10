// Simple auth helpers using browser cookies + a reactive auth store
import { writable } from 'svelte/store';

const TOKEN_COOKIE = 'raworc_token';
const OPERATOR_COOKIE = 'raworc_operator';
const PRINCIPAL_COOKIE = 'raworc_principal_type';

export const auth = writable({ token: null, name: null, type: null });

export function setCookie(name, value, days = 7) {
  const d = new Date();
  d.setTime(d.getTime() + days * 24 * 60 * 60 * 1000);
  const expires = `expires=${d.toUTCString()}`;
  const path = 'path=/';
  const sameSite = 'SameSite=Lax';
  document.cookie = `${name}=${encodeURIComponent(value)}; ${expires}; ${path}; ${sameSite}`;
}

export function getCookie(name) {
  const match = document.cookie.match(new RegExp('(?:^|; )' + name.replace(/([.$?*|{}()\[\]\\\/\+^])/g, '\\$1') + '=([^;]*)'));
  return match ? decodeURIComponent(match[1]) : null;
}

export function deleteCookie(name) {
  document.cookie = `${name}=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/; SameSite=Lax`;
}

export function setToken(token) {
  setCookie(TOKEN_COOKIE, token, 7);
  try { auth.update((s) => ({ ...s, token })); } catch (_) {}
}

export function getToken() {
  return getCookie(TOKEN_COOKIE);
}

export function clearToken() {
  deleteCookie(TOKEN_COOKIE);
  try { auth.update((s) => ({ ...s, token: null })); } catch (_) {}
}

export function setOperatorName(name) {
  setCookie(OPERATOR_COOKIE, name, 7);
  try { auth.update((s) => ({ ...s, name })); } catch (_) {}
}

export function getOperatorName() {
  return getCookie(OPERATOR_COOKIE);
}

export function clearOperatorName() {
  deleteCookie(OPERATOR_COOKIE);
  try { auth.update((s) => ({ ...s, name: null })); } catch (_) {}
}

export function setPrincipalType(t) {
  if (!t) return;
  setCookie(PRINCIPAL_COOKIE, t, 7);
  try { auth.update((s) => ({ ...s, type: t })); } catch (_) {}
}

export function getPrincipalType() {
  return getCookie(PRINCIPAL_COOKIE);
}

export function clearPrincipalType() {
  deleteCookie(PRINCIPAL_COOKIE);
  try { auth.update((s) => ({ ...s, type: null })); } catch (_) {}
}

export function isAuthenticated() {
  return !!getToken();
}

export function logoutClientSide() {
  try {
    clearToken();
    clearOperatorName();
    clearPrincipalType();
  } catch (_) {}
}

// Initialize store from cookies (client-side only)
export function initAuthFromCookies() {
  try {
    if (typeof document === 'undefined') return;
    const token = getCookie(TOKEN_COOKIE);
    // Only surface name when a valid token exists to avoid stale name after logout
    const name = token ? getCookie(OPERATOR_COOKIE) : null;
    const type = token ? getCookie(PRINCIPAL_COOKIE) : null;
    auth.set({ token: token || null, name: name || null, type: type || null });
  } catch (_) {}
}
