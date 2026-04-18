// In dev, VITE_API_BASE is unset and Vite proxies /api/* → localhost:3000.
// In production, VITE_API_BASE = "https://backend-production-fa211.up.railway.app"
// and we call the backend directly (no /api prefix needed).
const API_BASE = import.meta.env.VITE_API_BASE ?? '';

// Build a full URL for HTTP endpoints.
// Dev:  url('/threads') → '/api/threads'  (Vite proxy strips /api)
// Prod: url('/threads') → 'https://backend.../threads'
function url(path) {
  return API_BASE ? `${API_BASE}${path}` : `/api${path}`;
}

// Build the WebSocket URL, deriving wss:// from the API base when set.
function wsUrl() {
  if (API_BASE) {
    return `${API_BASE.replace(/^https/, 'wss').replace(/^http/, 'ws')}/ws`;
  }
  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${protocol}//${location.host}/ws`;
}

export async function postThread(content, lat, lng, noise_sigma) {
  const res = await fetch(url('/threads'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ content, lat, lng, noise_sigma })
  });
  if (!res.ok) throw new Error('Failed to post');
  return res.json();
}

export async function getFeed(lat, lng, radius_km) {
  const res = await fetch(url(`/feed?lat=${lat}&lng=${lng}&radius_km=${radius_km}`));
  if (!res.ok) throw new Error('Failed to fetch feed');
  return res.json();
}

export async function postComment(threadId, content) {
  const res = await fetch(url(`/threads/${threadId}/comments`), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ content })
  });
  if (!res.ok) throw new Error('Failed to post comment');
  return res.json();
}

export async function getComments(threadId) {
  const res = await fetch(url(`/threads/${threadId}/comments`));
  if (!res.ok) throw new Error('Failed to fetch comments');
  return res.json();
}

export function connectWs(lat, lng, radius_km, onEvent) {
  const ws = new WebSocket(wsUrl());

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
