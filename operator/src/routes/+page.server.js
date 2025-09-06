import { redirect } from '@sveltejs/kit';

export function load({ cookies }) {
  // If we have a token cookie, consider the session active and go to agents.
  // Otherwise, send to login. The /app layout will still validate the token.
  const token = cookies.get('raworc_token');
  if (token) throw redirect(302, '/agents');
  throw redirect(302, '/login');
}
