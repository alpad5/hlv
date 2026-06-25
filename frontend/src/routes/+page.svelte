<script>
  import { onMount, onDestroy, tick } from 'svelte';
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
  let clockTick = 0;
  let clockTickInterval;

  // Mirror of the backend's inactivity window (30 min). A thread's effective
  // expiry is the sooner of its hard cap or last_activity + this value.
  const INACTIVITY_TTL = 30 * 60;

  // Returns a value in [0, 1] representing how much life this thread has left.
  // 1 = just posted or just replied to; 0 = about to expire.
  // _clockTick is only here to make Svelte re-run this whenever the clock advances.
  function decayFraction(thread, _clockTick) {
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
        clockTickInterval = setInterval(() => { clockTick++; }, 30_000);
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

  onDestroy(() => { ws?.close(); clearInterval(clockTickInterval); });

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

  // Opens a thread in-place: the card it lives in expands to show replies
  // and a reply box. Other cards remain visible but dimmed (see CSS).
  async function openThread(thread) {
    activeThread = thread;
    comments = await getComments(thread.id);
    // Bring the focused card into a comfortable position so its replies are
    // visible without the user having to scroll immediately. `tick()` waits
    // for Svelte to flush the DOM so the expanded card actually exists before
    // we ask the browser to scroll to it.
    await tick();
    const el = document.querySelector(`[data-thread-id="${thread.id}"]`);
    // `nearest` keeps the card put if it's already visible — only nudges
    // the viewport when the expanded card would otherwise spill off-screen.
    el?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
  }

  function closeThread() {
    activeThread = null;
    comments = [];
    commentDraft = '';
  }

  // Escape closes the focused thread — keeps parity with clicking outside.
  function handleGlobalKey(e) {
    if (e.key === 'Escape' && activeThread) closeThread();
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

  // Spanish-style elapsed time, e.g. "hace 3m". The "hace" prefix makes it
  // unambiguous that the number is time-since-post, not a time-of-day.
  function timeAgo(ts) {
    const diff = Math.floor(Date.now() / 1000 - ts);
    if (diff < 60) return `hace ${diff}s`;
    if (diff < 3600) return `hace ${Math.floor(diff / 60)}m`;
    return `hace ${Math.floor(diff / 3600)}h`;
  }

  // Returns true when fewer than 5 minutes remain on a thread's life.
  // _clockTick is a reactive dependency so this re-evaluates every 30s.
  function isNearExpiry(thread, _clockTick) {
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

<svelte:window on:keydown={handleGlobalKey} />

<!-- Browser tab title for the home page: brand mark plus tagline, so a lone tab still reads as the app. -->
<svelte:head>
  <title>hlv — hablan los vecinos</title>
</svelte:head>

<div class="app">
  <!-- Sidebar / controls + compose -->
  <aside>
    <div class="brand">hlv</div>
    <a href="/blog" class="blog-link" target="_blank" rel="noopener noreferrer">— blog</a>
    <a href="/como" class="blog-link" target="_blank" rel="noopener noreferrer">— cómo funciona</a>

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

  <!-- Feed.
       The feed is always present. When a thread is opened, its card expands
       in place to show replies and a reply box — the rest of the feed dims
       out around it (see CSS .dimmed). This keeps the visual identity of
       the conversation bubble continuous: the same card you tapped is the
       same card whose replies you now see, with the same border, the same
       drained expiry bar, the same LED. -->
  <main>
    <div class="feed" class:has-active={activeThread}>
      {#each threads as t (t.id)}
        {@const isActive = activeThread?.id === t.id}
        <!-- Active card: renders as a non-button container so it can hold
             a textarea and reply button inside it (nested buttons aren't
             allowed). Inactive cards remain real buttons for accessibility. -->
        {#if isActive}
          <article class="thread-card active" data-thread-id={t.id}>
            <button class="thread-close" on:click={closeThread} aria-label="close">
              <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
            </button>
            <p class="thread-content">{t.content}</p>
            <div class="thread-meta">
              <span>{t.comment_count} {t.comment_count === 1 ? 'respuesta' : 'respuestas'}</span>
              <span>{timeAgo(t.created_at)}</span>
            </div>

            <!-- In the active card the expiry bar moves up to sit directly under
                 the OP. It does double duty here: it shows remaining life right
                 where the eye lands, AND it acts as the visual seam separating
                 the original post from the replies below. -->
            <div class="expiry-bar inline" style="width: {decayFraction(t, clockTick) * 100}%"></div>

            <!-- Replies live inside the same card — visual continuity with the OP. -->
            {#if comments.length > 0}
              <div class="comment-list">
                {#each comments as c}
                  <div class="comment">
                    <p>{c.content}</p>
                    <span class="meta">{timeAgo(c.created_at)}</span>
                  </div>
                {/each}
              </div>
            {/if}

            <div class="comment-compose" class:no-divider={comments.length === 0}>
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

            <!-- LED stays at the bottom-right corner — it's a separate signal
                 ("about to expire") and benefits from being unmistakable. -->
            <div
              class="expiry-pulse"
              class:active={isNearExpiry(t, clockTick)}
              style="animation-delay: {pulseDelay(t.id)}s"
            ></div>
          </article>
        {:else}
          <button
            class="thread-card"
            class:dimmed={activeThread}
            data-thread-id={t.id}
            on:click={() => openThread(t)}
          >
            <p class="thread-content">{t.content}</p>
            <div class="thread-meta">
              <span>{t.comment_count} {t.comment_count === 1 ? 'respuesta' : 'respuestas'}</span>
              <span>{timeAgo(t.created_at)}</span>
            </div>
            <div class="expiry-bar" style="width: {decayFraction(t, clockTick) * 100}%"></div>
            <div
              class="expiry-pulse"
              class:active={isNearExpiry(t, clockTick)}
              style="animation-delay: {pulseDelay(t.id)}s"
            ></div>
          </button>
        {/if}
      {/each}
      {#if threads.length === 0 && location}
        <p class="empty">nada por aquí. sé el primero.</p>
      {/if}
    </div>
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
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 2px;
    background: #333;
    outline: none;
    border-radius: 1px;
    cursor: pointer;
    /* On touch devices (notably Android Chrome) the sliders sit inside a
       scrollable sidebar. Without this, the browser treats a drag as a
       scroll: the thumb moves visually but its value never sticks.
       `touch-action: none` dedicates the drag gesture to the slider. */
    touch-action: none;
  }

  input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 28px;
    height: 14px;
    border-radius: 1px;
    background: rgba(192, 64, 64, 0.9);
    cursor: pointer;
  }

  input[type="range"]::-moz-range-thumb {
    width: 28px;
    height: 14px;
    border: none;
    border-radius: 1px;
    background: rgba(192, 64, 64, 0.9);
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
    background: #181818;
    border: none;
    border-radius: 8px;
    color: #7a6218;
    padding: 10px;
    font-family: inherit;
    font-size: inherit;
    resize: none;
    width: 100%;
    outline: none;
    line-height: 1.5;
    animation: textarea-breathe 40s ease-in-out infinite;
  }

  /* Slow, faint white "breath" — like the LED pulse but white and dimmer. */
  @keyframes textarea-breathe {
    0%, 100% { box-shadow: 0 0 0 0 rgba(255, 255, 255, 0); }
    50%      { box-shadow: 0 0 24px 4px rgba(255, 255, 255, 0.12); }
  }
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

  /* Inline variant — used inside the active (expanded) card. The bar moves up
     to sit right under the OP so the user can read the remaining life next to
     the post itself, and it doubles as the seam between OP and replies. */
  .expiry-bar.inline {
    position: static;
    margin: 14px 0;
    border-radius: 1px;
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

  /* When a thread is open, other cards recede into the background. They stay
     visible so the user keeps spatial context, but lose grab so the focused
     conversation reads as the only "live" thing on screen. */
  .thread-card.dimmed {
    opacity: 0.18;
    transition: opacity 0.3s ease, transform 0.15s ease, border-color 0.15s ease;
  }
  .thread-card.dimmed:hover {
    opacity: 0.45;
    transform: none;
    background: none;
    border-color: #222;
  }

  /* Focused card — same border, same radius, same expiry bar, same LED.
     The only difference vs the resting state is that it now contains the
     replies and a reply box. No scale, no shadow trickery — the card stays
     exactly where it sat in the feed. */
  .thread-card.active {
    cursor: default;
    border-color: #333;
    background: #0f0f0f;
    padding-top: 16px;
    padding-bottom: 24px;
    transition: opacity 0.3s ease;
  }
  .thread-card.active:hover { transform: none; }

  .thread-close {
    position: absolute;
    top: 8px;
    right: 8px;
    padding: 4px;
    border: none;
    color: #444;
    background: none;
  }
  .thread-close:hover { color: #aaa; border: none; }

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

  .meta { font-size: 11px; color: #555; }

  /* Replies sit inside the active card. The seam between the OP and the replies
     is the inline expiry bar above — no extra divider needed here. */
  .comment-list {
    width: 100%;
    display: flex;
    flex-direction: column;
  }

  .comment {
    padding: 10px 0 10px 24px;
    border-left: 2px solid #1e1e1e;
    margin-left: 4px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .comment + .comment { border-top: 1px solid #141414; }

  .comment p {
    line-height: 1.5;
    color: #ccc;
    font-size: 17px;
  }

  .comment-compose {
    padding-top: 12px;
    margin-top: 4px;
    border-top: 1px solid #1e1e1e;
    display: flex;
    gap: 8px;
    align-items: flex-end;
    width: 100%;
  }

  /* When there are no replies yet, the expiry bar above is already the only
     divider in the card — drop the compose's own top border so we don't get
     two stacked separators sitting on top of each other. */
  .comment-compose.no-divider {
    border-top: none;
    padding-top: 0;
    margin-top: 0;
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
