import { redirect } from '@sveltejs/kit';

export async function load({ cookies }) {
  try {
    // Aggressively clear cookies with matching attributes
    const past = new Date(0);
    cookies.set('tsbx_token', '', { path: '/', expires: past, sameSite: 'lax' });
    cookies.set('tsbx_operator', '', { path: '/', expires: past, sameSite: 'lax' });
    cookies.delete('tsbx_token', { path: '/' });
    cookies.delete('tsbx_operator', { path: '/' });
  } catch (_) {}
  throw redirect(302, '/login');
}
