// Simple auth helpers using browser cookies

const TOKEN_COOKIE = 'raworc_token';
const OPERATOR_COOKIE = 'raworc_operator';

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
}

export function getToken() {
  return getCookie(TOKEN_COOKIE);
}

export function clearToken() {
  deleteCookie(TOKEN_COOKIE);
}

export function setOperatorName(name) {
  setCookie(OPERATOR_COOKIE, name, 7);
}

export function getOperatorName() {
  return getCookie(OPERATOR_COOKIE);
}

export function clearOperatorName() {
  deleteCookie(OPERATOR_COOKIE);
}

export function isAuthenticated() {
  return !!getToken();
}

