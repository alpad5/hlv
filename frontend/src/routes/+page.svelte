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

  onMount(async () => {
    if (!navigator.geolocation) {
      locationError = 'Geolocation is not supported by this browser.';
      return;
    }
    navigator.geolocation.getCurrentPosition(
      async (pos) => {
        location = { lat: pos.coords.latitude, lng: pos.coords.longitude };
        threads = await getFeed(location.lat, location.lng, radius);
        ws = connectWs(location.lat, location.lng, radius, handleWsEvent);
      },
      (err) => {
        const messages = {
          1: 'Location permission denied. Please allow location access in your browser settings.',
          2: 'Location unavailable. Check your device GPS/location settings.',
          3: 'Location request timed out. Try refreshing.'
        };
        locationError = messages[err.code] ?? `Location error: ${err.message}`;
      }
    );
  });

  onDestroy(() => ws?.close());

  function handleWsEvent(event) {
    if (event.type === 'new_thread') {
      threads = [event.data, ...threads];
    } else if (event.type === 'new_comment' && activeThread?.id === event.thread_id) {
      comments = [...comments, event.data];
    }
  }

  async function submit() {
    if (!draft.trim() || !location || posting) return;
    posting = true;
    try {
      await postThread(draft.trim(), location.lat, location.lng);
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

  function expiresIn(thread) {
    const remaining = thread.expires_at - Math.floor(Date.now() / 1000);
    if (remaining <= 0) return 'expired';
    const m = Math.floor(remaining / 60);
    const s = remaining % 60;
    return m > 0 ? `${m}m` : `${s}s`;
  }

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

    <div class="controls">
      <label>
        <span>radius <strong>{radius}km</strong></span>
        <input type="range" min="1" max="20" step="1" bind:value={radius} on:change={onRadiusChange} />
      </label>

      <label class="noise-label">
        <span>noise <strong>{noise}m</strong></span>
        <input type="range" min="50" max="1000" step="50" bind:value={noise} disabled />
        <small>location fuzzing — coming soon</small>
      </label>
    </div>

    <div class="compose">
      <textarea
        bind:value={draft}
        placeholder="what's happening here?"
        rows="4"
        on:keydown={handleKey}
        maxlength="500"
      ></textarea>
      <div class="compose-footer">
        <span class="hint">⌘↵ to post</span>
        <button on:click={submit} disabled={!draft.trim() || posting || !location}>
          {posting ? '…' : 'post'}
        </button>
      </div>
    </div>

    {#if locationError}
      <p class="status error">{locationError}</p>
    {:else if !location}
      <p class="status">waiting for location…</p>
    {/if}
  </aside>

  <!-- Feed -->
  <main>
    {#if activeThread}
      <div class="thread-view">
        <button class="back" on:click={() => { activeThread = null; comments = []; }}>← back</button>
        <div class="thread-op">
          <p>{activeThread.content}</p>
          <span class="meta">{timeAgo(activeThread.created_at)} · expires in {expiresIn(activeThread)}</span>
        </div>
        <div class="comment-list">
          {#each comments as c}
            <div class="comment">
              <p>{c.content}</p>
              <span class="meta">{timeAgo(c.created_at)}</span>
            </div>
          {/each}
          {#if comments.length === 0}
            <p class="empty">no replies yet</p>
          {/if}
        </div>
        <div class="comment-compose">
          <textarea
            bind:value={commentDraft}
            placeholder="reply…"
            rows="2"
            on:keydown={handleCommentKey}
            maxlength="500"
          ></textarea>
          <button on:click={submitComment} disabled={!commentDraft.trim() || posting}>
            {posting ? '…' : 'reply'}
          </button>
        </div>
      </div>
    {:else}
      <div class="feed">
        {#each threads as t (t.id)}
          <button class="thread-card" on:click={() => openThread(t)}>
            <p class="thread-content">{t.content}</p>
            <div class="thread-meta">
              <span>{t.comment_count} {t.comment_count === 1 ? 'reply' : 'replies'}</span>
              <span>{timeAgo(t.created_at)}</span>
              <span class="expires">⏱ {expiresIn(t)}</span>
            </div>
          </button>
        {/each}
        {#if threads.length === 0 && location}
          <p class="empty">nothing nearby. be the first.</p>
        {/if}
      </div>
    {/if}
  </main>
</div>

<style>
  :global(*, *::before, *::after) { box-sizing: border-box; margin: 0; padding: 0; }
  :global(body) {
    background: #0a0a0a;
    color: #e0e0e0;
    font-family: 'Courier New', monospace;
    font-size: 14px;
    min-height: 100vh;
  }

  .app {
    display: flex;
    min-height: 100vh;
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
    font-size: 20px;
    letter-spacing: 4px;
    color: #fff;
    text-transform: lowercase;
  }

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

  input[type="range"]:disabled { opacity: 0.3; cursor: not-allowed; }

  .noise-label small {
    font-size: 11px;
    color: #444;
    font-style: italic;
  }

  .compose {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  textarea {
    background: #111;
    border: 1px solid #222;
    color: #e0e0e0;
    padding: 10px;
    font-family: inherit;
    font-size: 13px;
    resize: none;
    width: 100%;
    outline: none;
    line-height: 1.5;
  }

  textarea:focus { border-color: #444; }

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
    padding: 6px 14px;
    font-family: inherit;
    font-size: 13px;
    cursor: pointer;
    text-transform: lowercase;
  }

  button:hover:not(:disabled) { border-color: #888; }
  button:disabled { opacity: 0.3; cursor: not-allowed; }

  .status { font-size: 12px; color: #444; }
  .status.error { color: #c0392b; }

  main {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .feed { display: flex; flex-direction: column; flex: 1; }

  .thread-card {
    border: none;
    border-bottom: 1px solid #1a1a1a;
    padding: 16px 24px;
    text-align: left;
    width: 100%;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 8px;
    background: none;
    color: inherit;
    font-family: inherit;
    font-size: inherit;
  }

  .thread-card:hover { background: #0f0f0f; border-color: #252525; }

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

  .expires { color: #3a3a3a; }

  .thread-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .back {
    border: none;
    border-bottom: 1px solid #1a1a1a;
    padding: 14px 24px;
    text-align: left;
    color: #666;
    font-size: 12px;
  }

  .thread-op {
    padding: 20px 24px;
    border-bottom: 1px solid #1e1e1e;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .thread-op p { line-height: 1.6; }

  .meta { font-size: 11px; color: #555; }

  .comment-list {
    flex: 1;
    overflow-y: auto;
    padding: 8px 0;
  }

  .comment {
    padding: 12px 24px;
    border-bottom: 1px solid #141414;
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
      height: auto;
      position: static;
      border-right: none;
      border-bottom: 1px solid #1e1e1e;
    }
  }
</style>
