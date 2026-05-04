# qhlv protocol spec

**qhlv** — *que hablan los vecinos* — is a minimal open protocol for ephemeral, location-aware messaging. No accounts. No history. Messages exist near a place, for a while, then disappear.

Any server that implements this spec is a qhlv node. Any client that speaks this API can build an app on top of it.

---

## Core concepts

**Thread** — a message posted from a location. Has a TTL. Belongs to a geographic area, not a user.

**Comment** — a reply to a thread. Resets the thread's inactivity timer, keeping it alive.

**Feed** — the set of threads visible from a given position within a given radius.

---

## Location model

Clients send their real coordinates. The server applies two layers of fuzzing before storing anything:

1. **Grid snap** — coordinates are snapped to a ~100m grid.
2. **Gaussian jitter** — random offset drawn from a normal distribution, default σ = 300m (configurable).

Raw coordinates are never persisted. All stored positions are already fuzzed.

---

## Expiry model

Every thread has two timers:

| Timer | Default | Behaviour |
|---|---|---|
| Inactivity TTL | 15 min | Reset to full on each new comment |
| Hard cap | 60 min | Set at creation, never extended |

Effective TTL = `min(inactivity_ttl, hard_cap - age)`. When either expires, the thread is gone.

---

## HTTP API

### POST /threads

Post a new thread.

**Request body**
```json
{
  "content": "string",
  "lat": number,
  "lng": number,
  "noise_sigma": number   // optional, metres; server clamps to [0, max_sigma]
}
```

**Response `201`**
```json
{
  "id": "string",
  "content": "string",
  "lat": number,
  "lng": number,
  "created_at": "ISO 8601",
  "expires_at": "ISO 8601",
  "comment_count": 0
}
```

---

### GET /feed

Fetch threads within a radius of a position.

**Query params**

| Param | Type | Description |
|---|---|---|
| `lat` | number | Observer latitude |
| `lng` | number | Observer longitude |
| `radius_km` | number | Search radius in kilometres |

**Response `200`** — array of thread objects (same shape as POST /threads response), ordered newest first.

---

### POST /threads/:id/comments

Reply to a thread. Resets the thread's inactivity TTL.

**Request body**
```json
{ "content": "string" }
```

**Response `201`**
```json
{
  "id": "string",
  "thread_id": "string",
  "content": "string",
  "created_at": "ISO 8601"
}
```

Returns `404` if the thread has expired or does not exist.

---

### GET /threads/:id/comments

Fetch all comments on a thread.

**Response `200`** — array of comment objects, ordered oldest first.

Returns `404` if the thread has expired or does not exist.

---

## WebSocket API

**Endpoint:** `GET /ws` — upgrade to WebSocket.

### Registration message (client → server)

After connecting, the client sends one message to register its viewport:

```json
{
  "lat": number,
  "lng": number,
  "radius_km": number
}
```

The client can re-send this at any time to update its viewport (e.g. user moves or changes radius).

### Server events (server → client)

**`new_thread`** — a thread was posted within the client's radius.
```json
{
  "type": "new_thread",
  "thread": { /* thread object */ }
}
```

**`new_comment`** — a comment was posted on a thread the client can see.
```json
{
  "type": "new_comment",
  "thread_id": "string",
  "comment": { /* comment object */ }
}
```

**`thread_expired`** — a thread within the client's radius has expired and should be removed from the feed.
```json
{
  "type": "thread_expired",
  "thread_id": "string"
}
```

---

## Server configuration

A compliant qhlv node exposes the following as environment variables or a config file:

| Parameter | Default | Description |
|---|---|---|
| `INACTIVITY_TTL_SECS` | 900 | Seconds of silence before a thread expires |
| `HARD_CAP_SECS` | 3600 | Maximum thread lifetime from creation |
| `DEFAULT_SIGMA_M` | 300 | Default Gaussian jitter in metres |
| `MAX_SIGMA_M` | 1000 | Upper bound on client-requested sigma |
| `MAX_CONTENT_LEN` | 300 | Maximum character length for threads and comments |
| `MAX_RADIUS_KM` | 10 | Maximum feed radius a client may request |

---

## Conformance

A server is a valid qhlv node if it:

- Implements all five HTTP endpoints above
- Implements the WebSocket protocol above
- Applies two-layer location fuzzing before storing any coordinate
- Enforces both the inactivity TTL and the hard cap
- Never returns or logs raw client coordinates
