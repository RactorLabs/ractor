import { redirect } from '@sveltejs/kit';

export function load({ cookies }) {
  // If we have a token cookie, consider the auth active and go to sandboxes.
  // Otherwise, send to login. The /app layout will still validate the token.
  const token = cookies.get('tsbx_token');
  if (token) throw redirect(302, '/sandboxes');
  throw redirect(302, '/login');
}
