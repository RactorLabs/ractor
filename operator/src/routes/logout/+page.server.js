import { redirect } from '@sveltejs/kit';

export async function load({ cookies }) {
  try {
    // Aggressively clear cookies with matching attributes
    const past = new Date(0);
    cookies.set('ractor_token', '', { path: '/', expires: past, sameSite: 'lax' });
    cookies.set('ractor_operator', '', { path: '/', expires: past, sameSite: 'lax' });
    cookies.delete('ractor_token', { path: '/' });
    cookies.delete('ractor_operator', { path: '/' });
  } catch (_) {}
  throw redirect(302, '/login');
}
