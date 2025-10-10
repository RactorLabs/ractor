import { redirect } from '@sveltejs/kit';

export async function GET({ cookies }) {
  const past = new Date(0);
  try {
    cookies.set('ractor_token', '', { path: '/', expires: past, sameSite: 'lax' });
    cookies.set('ractor_operator', '', { path: '/', expires: past, sameSite: 'lax' });
  } catch {}
  throw redirect(302, '/login');
}

