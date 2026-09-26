// Anonymous visit counts for the early tests (roadmap B6): a random id kept in this browser, and
// what was done with the viewer. No names, no addresses; the server keeps no more than this.

export type VisitKind = 'visit' | 'digest' | 'story' | 'card' | 'prediction' | 'replay' | 'view';

const KEY = 'protogaea.visitor';

function visitor(): string | undefined {
  try {
    let id = localStorage.getItem(KEY);
    if (!id) {
      // crypto.randomUUID needs a secure context; the test server is plain HTTP.
      const bytes = crypto.getRandomValues(new Uint8Array(16));
      id = Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
      localStorage.setItem(KEY, id);
    }
    return id;
  } catch {
    return undefined;
  }
}

export function track(kind: VisitKind, detail?: string) {
  const id = visitor();
  if (!id) return;
  fetch('/v0/visits', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ visitor: id, kind, detail }),
    keepalive: true,
  }).catch(() => {});
}
