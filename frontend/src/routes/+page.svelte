<script>
  import { onMount, onDestroy } from 'svelte';
  import { postThread, postComment, getFeed, getComments, connectWs } from '$lib/api.js';

  let threads = [];
  let draft = '';
  let radius = 5;
  let noise = 300;
  let location = null;
  let ws = null;
  let activeThread = null;
  let commentDraft = '';
  let comments = [];
  let posting = false;
  let locationError = null;
  let retrying = false;

  // Incremented every 30s so that decay bar widths re-evaluate reactively.
  let tick = 0;
  let tickInterval;

  // Mirror of the backend's inactivity window (30 min). A thread's effective
  // expiry is the sooner of its hard cap or last_activity + this value.
  const INACTIVITY_TTL = 30 * 60;

  // Returns a value in [0, 1] representing how much life this thread has left.
  // 1 = just posted or just replied to; 0 = about to expire.
  // _tick is only here to make Svelte re-run this whenever the clock advances.
  function decayFraction(thread, _tick) {
    const now = Date.now() / 1000;
    const effectiveExpiry = Math.min(thread.expires_at, thread.last_activity + INACTIVITY_TTL);
    const remaining = effectiveExpiry - now;
    return Math.max(0, Math.min(1, remaining / INACTIVITY_TTL));
  }

  // Asks the browser for the user's location and initialises the feed and
  // WebSocket connection once granted. Safe to call more than once — e.g.
  // when the user taps the retry button after an initial denial.
  async function requestLocation() {
    locationError = null;
    retrying = true;

    // If the browser has permanently blocked location (not just dismissed the
    // prompt), getCurrentPosition would fire the error callback immediately
    // with no visible prompt. Detect that early and show actionable copy instead.
    if (navigator.permissions) {
      const perm = await navigator.permissions.query({ name: 'geolocation' });
      if (perm.state === 'denied') {
        retrying = false;
        locationError = 'El acceso a la ubicación está bloqueado en tu navegador. Para continuar, restablece el permiso de ubicación en la configuración del sitio e intenta de nuevo.';
        return;
      }
    }

    navigator.geolocation.getCurrentPosition(
      async (pos) => {
        retrying = false;
        location = { lat: pos.coords.latitude, lng: pos.coords.longitude };
        threads = await getFeed(location.lat, location.lng, radius);
        ws = connectWs(location.lat, location.lng, radius, handleWsEvent);
        tickInterval = setInterval(() => { tick++; }, 30_000);
      },
      () => {
        retrying = false;
        locationError = 'Suena horrible, pero necesitamos que permitas que el servicio obtenga tu ubicación a través del navegador. Esta ubicación no será almacenada ni asociada a ti de ninguna forma.';
      }
    );
  }

  onMount(() => {
    if (!navigator.geolocation) {
      locationError = 'Suena horrible, pero necesitamos que permitas que el servicio obtenga tu ubicación a través del navegador. Esta ubicación no será almacenada ni asociada a ti de ninguna forma.';
      return;
    }
    requestLocation();
  });

  onDestroy(() => { ws?.close(); clearInterval(tickInterval); });

  function handleWsEvent(event) {
    if (event.type === 'new_thread') {
      threads = [event.data, ...threads];
    } else if (event.type === 'new_comment') {
      // Update reply count and reset last_activity so the decay bar refills.
      threads = threads.map(t =>
        t.id === event.thread_id
          ? { ...t, comment_count: t.comment_count + 1, last_activity: Math.floor(Date.now() / 1000) }
          : t
      );
      // Also append the comment if this thread is currently open
      if (activeThread?.id === event.thread_id) {
        comments = [...comments, event.data];
      }
    } else if (event.type === 'thread_expired') {
      // Remove the expired thread from the feed. If it's currently open,
      // drop back to the feed — the thread is gone and there's nothing to show.
      threads = threads.filter(t => t.id !== event.thread_id);
      if (activeThread?.id === event.thread_id) {
        activeThread = null;
        comments = [];
      }
    }
  }

  async function submit() {
    if (!draft.trim() || !location || posting) return;
    posting = true;
    try {
      await postThread(draft.trim(), location.lat, location.lng, noise);
      draft = '';
    } finally {
      posting = false;
    }
  }

  async function openThread(thread) {
    activeThread = thread;
    comments = await getComments(thread.id);
  }

  async function submitComment() {
    if (!commentDraft.trim() || posting) return;
    posting = true;
    try {
      await postComment(activeThread.id, commentDraft.trim());
      commentDraft = '';
    } finally {
      posting = false;
    }
  }

  async function onRadiusChange() {
    if (!location) return;
    threads = await getFeed(location.lat, location.lng, radius);
    ws?.send(JSON.stringify({ lat: location.lat, lng: location.lng, radius_km: radius }));
  }

  function timeAgo(ts) {
    const diff = Math.floor(Date.now() / 1000 - ts);
    if (diff < 60) return `${diff}s`;
    if (diff < 3600) return `${Math.floor(diff / 60)}m`;
    return `${Math.floor(diff / 3600)}h`;
  }

  // Returns true when fewer than 5 minutes remain on a thread's life.
  // _tick is a reactive dependency so this re-evaluates every 30s.
  function isNearExpiry(thread, _tick) {
    const now = Date.now() / 1000;
    const effectiveExpiry = Math.min(thread.expires_at, thread.last_activity + INACTIVITY_TTL);
    return (effectiveExpiry - now) < 5 * 60;
  }

  // Derives a stable phase offset from the thread ID so each card's
  // pulse is staggered rather than all blinking in sync.
  function pulseDelay(id) {
    let h = 0;
    for (const c of id) h = (h * 31 + c.charCodeAt(0)) & 0xffff;
    return -(h % 20);
  }

  // --- Location preview ---
  // Mirrors the two-layer fuzzing the backend applies at post time
  // (grid snap + Gaussian jitter), so the user can see roughly where
  // their message will appear — not their exact location.

  const GRID_METERS = 100;
  const METERS_PER_DEG_LAT = 111_320;

  // Box-Muller transform: converts two uniform random numbers into one
  // Gaussian-distributed value with mean 0 and standard deviation 1.
  function gaussianRandom() {
    let u, v;
    do { u = Math.random(); } while (u === 0);
    do { v = Math.random(); } while (v === 0);
    return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v);
  }

  // Replicate the backend fuzz: snap to ~100m grid, then add Gaussian jitter.
  // Returns [lat, lng] of the approximate posted location.
  function computeFuzzPreview(lat, lng, sigma) {
    const gridLat = GRID_METERS / METERS_PER_DEG_LAT;
    const gridLng = GRID_METERS / (METERS_PER_DEG_LAT * Math.cos(lat * Math.PI / 180));
    const snappedLat = Math.round(lat / gridLat) * gridLat;
    const snappedLng = Math.round(lng / gridLng) * gridLng;
    const jitterLat = gaussianRandom() * sigma / METERS_PER_DEG_LAT;
    const jitterLng = gaussianRandom() * sigma / (METERS_PER_DEG_LAT * Math.cos(snappedLat * Math.PI / 180));
    return [snappedLat + jitterLat, snappedLng + jitterLng];
  }

  // Recomputes whenever the noise slider or location changes, giving a live preview.
  $: previewCoords = location ? computeFuzzPreview(location.lat, location.lng, noise) : null;

  function handleKey(e) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) submit();
  }

  function handleCommentKey(e) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) submitComment();
  }
</script>

<div class="app">
  <!-- Sidebar / controls + compose -->
  <aside>
    <div class="brand">hlv</div>
    <a href="/blog" class="blog-link" target="_blank" rel="noopener noreferrer">— blog</a>

    <div class="controls">
      <label>
        <span>radio <strong>{radius}km</strong></span>
        <input type="range" min="1" max="10" step="1" bind:value={radius} on:change={onRadiusChange} />
      </label>

      <label class="noise-label">
        <span>precisión <strong>{noise}m</strong></span>
        <input type="range" min="50" max="1000" step="50" bind:value={noise} />
        {#if previewCoords}
          <small class="location-preview">publicando cerca de {previewCoords[0].toFixed(4)}, {previewCoords[1].toFixed(4)}</small>
        {/if}
      </label>
    </div>

    <div class="compose">
      <textarea
        bind:value={draft}
        placeholder="¿qué se dice?"
        rows="4"
        on:keydown={handleKey}
        maxlength="300"
      ></textarea>
      <div class="compose-footer">
        <span class="hint">⌘↵ to post</span>
        <button on:click={submit} disabled={!draft.trim() || posting || !location} aria-label="post">
          {#if posting}…{:else}<svg xmlns="http://www.w3.org/2000/svg" width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="2" y1="12" x2="19" y2="12"/><polyline points="12 5 19 12 12 19"/></svg>{/if}
        </button>
      </div>
    </div>

    {#if locationError}
      <div class="location-error">
        <p class="status error">{locationError}</p>
        <button class="retry-btn" on:click={requestLocation} disabled={retrying}>
          {retrying ? '…' : 'intentar de nuevo'}
        </button>
      </div>
    {:else if !location}
      <p class="status">waiting for location…</p>
    {/if}
  </aside>

  <!-- Feed -->
  <main>
    {#if activeThread}
      <div class="thread-view">
        <button class="back" on:click={() => { activeThread = null; comments = []; }} aria-label="back">
          <svg xmlns="http://www.w3.org/2000/svg" width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="22" y1="12" x2="5" y2="12"/><polyline points="12 19 5 12 12 5"/></svg>
        </button>
        <div class="thread-op">
          <p>{activeThread.content}</p>
          <span class="meta">{timeAgo(activeThread.created_at)}</span>
        </div>
        <div class="comment-list">
          {#each comments as c}
            <div class="comment">
              <p>{c.content}</p>
              <span class="meta">{timeAgo(c.created_at)}</span>
            </div>
          {/each}
        </div>
        <div class="comment-compose">
          <textarea
            bind:value={commentDraft}
            placeholder="        ← respondo aquí"
            rows="2"
            on:keydown={handleCommentKey}
            maxlength="300"
          ></textarea>
          <button on:click={submitComment} disabled={!commentDraft.trim() || posting} aria-label="reply">
            {#if posting}…{:else}<svg xmlns="http://www.w3.org/2000/svg" width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 10 4 15 9 20"/><path d="M20 2v9a4 4 0 0 1-4 4H2"/></svg>{/if}
          </button>
        </div>
      </div>
    {:else}
      <div class="feed">
        {#each threads as t (t.id)}
          <button class="thread-card" on:click={() => openThread(t)}>
            <p class="thread-content">{t.content}</p>
            <div class="thread-meta">
              <span>{t.comment_count} {t.comment_count === 1 ? 'respuesta' : 'respuestas'}</span>
              <span>{timeAgo(t.created_at)}</span>
            </div>
            <!-- Decay bar: drains as the thread approaches expiry, refills on reply -->
            <div class="expiry-bar" style="width: {decayFraction(t, tick) * 100}%"></div>
            <!-- Pulse light: dormant square that wakes up in the last 5 minutes -->
            <div
              class="expiry-pulse"
              class:active={isNearExpiry(t, tick)}
              style="animation-delay: {pulseDelay(t.id)}s"
            ></div>
          </button>
        {/each}
        {#if threads.length === 0 && location}
          <p class="empty">nada por aquí. sé el primero.</p>
        {/if}
      </div>
    {/if}
  </main>
</div>

<style>
  :global(*, *::before, *::after) { box-sizing: border-box; margin: 0; padding: 0; }
  :global(html, body) { overflow-x: hidden; }
  :global(body) {
    background: #0a0a0a;
    color: #e0e0e0;
    font-family: 'DM Mono', monospace;
    font-size: 22px;
    min-height: 100vh;
  }

  .app {
    display: flex;
    min-height: 100vh;
    width: 100%;
  }

  aside {
    width: 280px;
    min-width: 280px;
    border-right: 1px solid #1e1e1e;
    padding: 24px 20px;
    display: flex;
    flex-direction: column;
    gap: 24px;
    position: sticky;
    top: 0;
    height: 100vh;
    overflow-y: auto;
  }

  .brand {
    font-size: 40px;
    letter-spacing: 10px;
    color: #fff;
    text-transform: lowercase;
    line-height: 1;
  }

  .blog-link {
    font-size: 11px;
    color: #444;
    text-decoration: none;
    letter-spacing: 1px;
    text-transform: lowercase;
    margin-top: -16px;
  }

  .blog-link:hover { color: #888; }

  .controls {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    color: #888;
  }

  label span { font-size: 12px; text-transform: uppercase; letter-spacing: 1px; }
  label strong { color: #e0e0e0; }

  input[type="range"] {
    width: 100%;
    accent-color: #e0e0e0;
    cursor: pointer;
  }

  .location-preview {
    font-size: 11px;
    color: #444;
    font-style: italic;
    line-height: 1.4;
  }

  .compose {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  textarea {
    background: #111;
    border: 1px solid #222;
    color: #7a6218;
    padding: 10px;
    font-family: inherit;
    font-size: inherit;
    resize: none;
    width: 100%;
    outline: none;
    line-height: 1.5;
  }

  textarea:focus { border-color: #444; }
  .compose textarea::placeholder { color: #564812; opacity: 1; font-family: 'DM Mono', monospace; font-size: 17px; font-style: italic; }
  .comment-compose textarea::placeholder { color: #564812; opacity: 0.6; font-size: 17px; font-style: italic; }

  .compose-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .hint { font-size: 11px; color: #444; }

  button {
    background: none;
    border: 1px solid #333;
    color: #e0e0e0;
    padding: 6px 10px;
    font-family: inherit;
    font-size: 13px;
    cursor: pointer;
    text-transform: lowercase;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  button:hover:not(:disabled) { border-color: #888; }
  button:disabled { opacity: 0.3; cursor: not-allowed; }

  button { color: #666; }
  button:hover:not(:disabled) { color: #aaa; }

  .status { font-size: 12px; color: #444; }
  .status.error { color: #7a6218; font-style: italic; line-height: 1.6; }

  .location-error { display: flex; flex-direction: column; gap: 8px; }

  .retry-btn {
    align-self: flex-start;
    font-size: 11px;
    color: #555;
    border-color: #2a2a2a;
    padding: 3px 8px;
  }

  .retry-btn:hover:not(:disabled) { color: #9a7f28; border-color: #9a7f28; }

  main {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .feed { display: flex; flex-direction: column; flex: 1; padding: 8px 12px; gap: 8px; }

  .thread-card {
    border: 1px solid #222;
    border-radius: 10px;
    padding: 16px 20px;
    text-align: left;
    width: 100%;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    justify-content: flex-start;
    align-items: flex-start;
    gap: 8px;
    background: none;
    color: inherit;
    font-family: inherit;
    font-size: inherit;
    transition: transform 0.15s ease, border-color 0.15s ease, background 0.15s ease;
    /* Needed so the decay bar clips to the card's rounded corners */
    position: relative;
    overflow: hidden;
  }

  /* Thin mustard line that drains left-to-right as the thread approaches expiry.
     Sits flush against the bottom edge of the card, outside the normal flow. */
  .expiry-bar {
    position: absolute;
    bottom: 0;
    left: 0;
    height: 3px;
    background: #9a7f28;
    transition: width 1s ease;
    border-radius: 0 0 0 10px;
  }

  /* LED housing: always visible as a dark red square, like plastic over an unlit bulb. */
  .expiry-pulse {
    position: absolute;
    bottom: 8px;
    right: 8px;
    width: 1%;
    aspect-ratio: 2 / 1;
    background: #7a1a1a;
    opacity: 0.35;
  }

  /* LED lit: bright red with a soft glow, pulses slowly in the last 5 minutes. */
  @keyframes expiry-pulse {
    0%, 100% { opacity: 0.8; box-shadow: 0 0 3px 1px rgba(192, 64, 64, 0.5); }
    50%       { opacity: 1.0; box-shadow: 0 0 6px 2px rgba(192, 64, 64, 0.8); }
  }

  .expiry-pulse.active {
    background: #c04040;
    animation: expiry-pulse 20s ease-in-out infinite;
  }

  .thread-card:hover {
    background: #0f0f0f;
    border-color: #444;
    transform: scale(1.012);
  }

  .thread-content {
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .thread-meta {
    display: flex;
    gap: 16px;
    font-size: 11px;
    color: #555;
  }

  .thread-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    overflow-y: auto;
    width: 100%;
  }

  .back {
    border: 1px solid #1e1e1e;
    border-radius: 8px;
    margin: 12px 16px;
    width: calc(100% - 32px);
    padding: 8px 20px;
    text-align: left;
    color: #666;
    font-size: 12px;
    justify-content: flex-start;
  }

  .thread-op {
    padding: 20px 24px;
    border-bottom: 1px solid #1e1e1e;
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
    text-align: left;
  }

  .thread-op p { line-height: 1.6; }

  .meta { font-size: 11px; color: #555; }

  .comment-list {
    padding: 8px 0;
    width: 100%;
  }

  .comment {
    padding: 12px 24px 12px 40px;
    border-bottom: 1px solid #141414;
    border-left: 2px solid #1e1e1e;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .comment p { line-height: 1.5; color: #ccc; }

  .comment-compose {
    border-top: 1px solid #1e1e1e;
    padding: 12px 24px;
    display: flex;
    gap: 8px;
    align-items: flex-end;
  }

  .comment-compose textarea { flex: 1; }
  .comment-compose button { white-space: nowrap; }

  .empty { padding: 40px 24px; color: #333; font-style: italic; }

  /* Mobile */
  @media (max-width: 640px) {
    .app { flex-direction: column; }

    aside {
      width: 100%;
      min-width: 0;
      height: auto;
      position: static;
      border-right: none;
      border-bottom: 1px solid #1e1e1e;
      padding: 16px;
      gap: 16px;
    }

    .brand {
      font-size: 28px;
      letter-spacing: 8px;
    }

    .hint { display: none; }

    .compose-footer { justify-content: flex-end; }

    main {
      flex: 1;
      min-height: 50vh;
    }
  }
</style>
