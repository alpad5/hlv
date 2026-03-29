# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

**hlv** — anonymous, ephemeral, location-based messaging. No accounts, no history. Open it, post a message, see what people nearby are saying. Threads die if ignored.

## Running locally

Three processes required:

```bash
# 1. Redis
redis-server --daemonize yes

# 2. Backend (Rust)
cd backend && ~/.cargo/bin/cargo run

# 3. Frontend (SvelteKit)
cd frontend && npm run dev
```

Frontend: `http://localhost:5173` — Backend: `http://localhost:3000`

## Backend (Rust + axum)

```
backend/src/
  main.rs          # server boot, router, AppState
  models.rs        # Thread, Comment, request/response types
  store.rs         # all Redis operations
  geo.rs           # coordinate fuzzing (grid snap + Gaussian jitter) and haversine distance
  ws.rs            # WebSocket handler, client registry, broadcast logic
  routes/
    threads.rs     # POST /threads, GET /feed
    comments.rs    # POST /threads/:id/comments, GET /threads/:id/comments
```

**API:**
- `POST /threads` — body: `{content, lat, lng}`
- `GET /feed?lat=&lng=&radius_km=` — threads within radius
- `POST /threads/:id/comments` — body: `{content}`
- `GET /threads/:id/comments`
- `WS /ws` — connect then send `{lat, lng, radius_km}` to register; server pushes `new_thread` and `new_comment` events

**Redis schema** (no SQL, Redis only):
- `thread:{id}` → hash with TTL
- `thread:{id}:comments` → list with TTL
- `feed:geo` → GEO set (stale entries filtered by checking if thread key exists)

**Expiry rules:**
- Inactivity TTL: 15 min, reset on each comment
- Hard cap: 60 min from `created_at`
- TTL on comment = `min(15min, expires_at - now)`, computed atomically in a Lua script in `store.rs`

**Location privacy — two layers applied at post time:**
1. Grid snap to ~100m
2. Gaussian jitter, default sigma = 300m (user-controllable sigma is a planned feature)
Stored coordinates are already fuzzed — raw location is never persisted.

**WebSocket flow:**
- Client connects → sends `{lat, lng, radius_km}` → registered in in-memory `ClientMap`
- On new thread/comment: haversine check against all clients → push to those within radius
- On disconnect: removed from map

## Frontend (SvelteKit)

```
frontend/src/
  lib/api.js              # fetch + WebSocket wrappers
  routes/+page.svelte     # entire UI (single page)
  routes/+layout.js       # SSR disabled (browser-only, needs geolocation)
```

Single-page app, SSR off. All state lives in `+page.svelte`. Layout: sidebar (controls + compose) + main feed. Clicking a thread opens it in-place with comment list and reply box.

**Radius slider:** 1–20km in 1km steps, stored client-side, sent to WS on change.
**Noise slider:** wired up in UI but disabled — backend sigma is hardcoded at 300m until this feature is built out.

## Planned features

- User-controlled location noise (expose sigma via UI slider → send to backend)
- Thread expiry countdown that updates in real time
- PWA icons / installable manifest
