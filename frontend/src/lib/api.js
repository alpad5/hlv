export async function postThread(content, lat, lng) {
  const res = await fetch('/api/threads', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ content, lat, lng })
  });
  if (!res.ok) throw new Error('Failed to post');
  return res.json();
}

export async function getFeed(lat, lng, radius_km) {
  const res = await fetch(`/api/feed?lat=${lat}&lng=${lng}&radius_km=${radius_km}`);
  if (!res.ok) throw new Error('Failed to fetch feed');
  return res.json();
}

export async function postComment(threadId, content) {
  const res = await fetch(`/api/threads/${threadId}/comments`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ content })
  });
  if (!res.ok) throw new Error('Failed to post comment');
  return res.json();
}

export async function getComments(threadId) {
  const res = await fetch(`/api/threads/${threadId}/comments`);
  if (!res.ok) throw new Error('Failed to fetch comments');
  return res.json();
}

export function connectWs(lat, lng, radius_km, onEvent) {
  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
  const ws = new WebSocket(`${protocol}//${location.host}/ws`);

  ws.onopen = () => {
    ws.send(JSON.stringify({ lat, lng, radius_km }));
  };

  ws.onmessage = (e) => {
    try {
      onEvent(JSON.parse(e.data));
    } catch {}
  };

  return ws;
}
