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

function parseTopics(raw) {
  if (!raw) return [];
  return String(raw)
    .split(',')
    .map((entry) => entry.trim())
    .filter(Boolean)
    .slice(0, 4);
}

export default function handler(req) {
  const url = req.nextUrl || new URL(req.url);
  const { searchParams, pathname } = url;
  const segments = pathname.split('/').filter(Boolean);
  const owner = toText(segments[segments.length - 2], 'acme');
  const repo = toText(segments[segments.length - 1], 'hyperdrive');
  const language = toText(searchParams.get('language'), 'TypeScript');
  const topics = parseTopics(searchParams.get('topics'));

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

        <div style={{ display: 'flex', flexDirection: 'column', gap: 20 }}>
          <span style={{ color: '#62f3d6', fontSize: 63, fontWeight: 750 }}>{ownerLine}{repo}</span>
          <div style={{ display: 'flex', alignItems: 'center', gap: 26, flexWrap: 'wrap' }}>
            <span style={{ fontSize: 30, fontWeight: 600, color: '#eef3f9' }}>Language: {language}</span>

            {topics.length > 0 && (
              <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 16 }}>
                {topics.map((topic) => (
                  <span
                    key={topic}
                    style={{
                      padding: '10px 18px',
                      borderRadius: 999,
                      background: 'rgba(127, 255, 212, 0.2)',
                      color: '#7fffd4',
                      fontSize: 22,
                      fontWeight: 600
                    }}
                  >
                    {topic}
                  </span>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
    ),
    {
      width: WIDTH,
      height: HEIGHT
    }
  );
}
