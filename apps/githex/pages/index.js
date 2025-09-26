import { useEffect, useState } from 'react';
import { useRouter } from 'next/router';

function extractDestination(value) {
  const trimmed = value.trim();
  if (!trimmed) return null;

  let pathCandidate = trimmed;
  try {
    const parsed = new URL(trimmed);
    if (parsed.protocol !== 'https:' || parsed.hostname.toLowerCase() !== 'github.com') {
      return null;
    }
    pathCandidate = parsed.pathname;
  } catch (_) {
    if (/^https?:/i.test(trimmed)) {
      return null;
    }
    const normalized = trimmed
      .replace(/^https?:\/\/githex\.com\//i, '')
      .replace(/^https?:\/\/github\.com\//i, '')
      .replace(/^githex\.com\//i, '')
      .replace(/^github\.com\//i, '');
    pathCandidate = normalized.startsWith('/') ? normalized : `/${normalized}`;
  }

  const clean = pathCandidate.replace(/^\/+|\/+$/g, '');
  const segments = clean.split('/');

  if (segments.length === 2 && segments[0] && segments[1]) {
    return `/${segments[0]}/${segments[1]}`;
  }

  return null;
}

export default function Home() {
  const router = useRouter();
  const [input, setInput] = useState('');
  const [error, setError] = useState(null);

  useEffect(() => {
    if (!router.isReady) {
      return;
    }

    const query = router.query || {};
    const errorCode = Array.isArray(query.error) ? query.error[0] : query.error;
    const repo = Array.isArray(query.repo) ? query.repo[0] : query.repo;

    if (errorCode === 'repo_inaccessible') {
      const decodedRepo = repo ? decodeURIComponent(repo) : 'repository';
      setError(`Repository ${decodedRepo} is not accessible. Only public repositories can be reviewed for now.`);
      if (repo) {
        setInput(decodedRepo);
      }
    } else {
      setError(null);
    }
  }, [router.isReady, router.query]);

  async function handleSubmit(event) {
    event.preventDefault();
    const destination = extractDestination(input);
    if (!destination) {
      setError('Enter a repository as user/repo or a valid GitHub URL.');
      return;
    }
    setError(null);
    setInput('');
    await router.push(destination);
  }

  return (
    <main>
      <section className="hero">
        <h1>GitHex</h1>

        <form className="input-row" onSubmit={handleSubmit}>
          <input
            type="text"
            placeholder="Enter user/repo or a GitHub URL"
            value={input}
            onChange={(event) => setInput(event.target.value)}
            aria-label="Repository"
            autoFocus
          />
          <button className="button" type="submit">
            Review Repo
          </button>
        </form>
        {error && <p className="form-error">{error}</p>}
      </section>
    </main>
  );
}
