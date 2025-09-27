import { ImageResponse } from '@vercel/og';

export const config = {
  runtime: 'experimental-edge'
};

const WIDTH = 1200;
const HEIGHT = 630;

function toText(value, fallback) {
  if (value === undefined || value === null) return fallback;
  const trimmed = String(value).trim();
  return trimmed.length ? trimmed : fallback;
}

export default function handler(req) {
  const url = req.nextUrl || new URL(req.url);
  const { searchParams, pathname } = url;
  const segments = pathname.split('/').filter(Boolean);
  const owner = toText(segments[segments.length - 2], 'acme');
  const repo = toText(segments[segments.length - 1], 'hyperdrive');
  const language = toText(searchParams.get('language'), 'TypeScript');
  const statsLine = toText(searchParams.get('stats'), '');

  const ownerLine = `${owner}/`;

  return new ImageResponse(
    (
      <div
        style={{
          width: WIDTH,
          height: HEIGHT,
          display: 'flex',
          flexDirection: 'column',
          justifyContent: 'space-between',
          padding: '72px 96px',
          boxSizing: 'border-box',
          color: '#eef3f9',
          backgroundImage:
            'radial-gradient(circle at 20% 20%, rgba(127, 255, 212, 0.18), transparent 55%), radial-gradient(circle at 80% 15%, rgba(97, 218, 251, 0.12), transparent 60%), linear-gradient(135deg, #0b0f16 0%, #101b2c 48%, #05080f 100%)'
        }}
      >
        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          <span
            style={{
              fontSize: 56,
              fontWeight: 700,
              letterSpacing: '0.07em',
              backgroundImage: 'linear-gradient(120deg, #7fffd4, #61dafb, #7fffd4)',
              backgroundSize: '220% 100%',
              color: 'transparent',
              backgroundClip: 'text'
            }}
          >
            GitHex
          </span>
          <span style={{ color: 'rgba(238, 243, 249, 0.9)', fontSize: 34, fontWeight: 600 }}>GitHub Repo Explainer</span>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 28 }}>
          <span style={{ color: '#62f3d6', fontSize: 63, fontWeight: 750 }}>{ownerLine}{repo}</span>
          <span style={{ fontSize: 36, fontWeight: 650, color: '#eef3f9', letterSpacing: '0.02em' }}>Language: {language}</span>
          {statsLine && (
            <span style={{ fontSize: 30, fontWeight: 550, color: 'rgba(238, 243, 249, 0.88)' }}>{statsLine}</span>
          )}
        </div>
      </div>
    ),
    {
      width: WIDTH,
      height: HEIGHT
    }
  );
}
