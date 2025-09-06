import { redirect } from '@sveltejs/kit';

export async function load({ cookies }) {
  try {
    // Aggressively clear cookies with matching attributes
    const past = new Date(0);
    cookies.set('raworc_token', '', { path: '/', expires: past, sameSite: 'lax' });
    cookies.set('raworc_operator', '', { path: '/', expires: past, sameSite: 'lax' });
    cookies.delete('raworc_token', { path: '/' });
    cookies.delete('raworc_operator', { path: '/' });
  } catch (_) {}
  throw redirect(302, '/login');
}
