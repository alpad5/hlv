<script>
  import { posts } from '$lib/posts.js';

  // Format ISO date string as "21 de abril de 2026"
  function formatDate(iso) {
    return new Date(iso).toLocaleDateString('es-ES', { year: 'numeric', month: 'long', day: 'numeric' });
  }
</script>

<svelte:head>
  <title>hlv — blog</title>
</svelte:head>

<div class="blog">
  <nav>
    <a href="/" class="brand">hlv</a>
    <span class="sep">/</span>
    <span class="crumb">blog</span>
  </nav>

  <div class="posts">
    {#each posts as post, i}
      <article id={post.slug}>
        <h2>{post.title}</h2>
        <time datetime={post.date}>{formatDate(post.date)}</time>
        <div class="body">
          {#each post.body as para}
            <p>{para}</p>
          {/each}
        </div>
      </article>
      {#if i < posts.length - 1}
        <hr />
      {/if}
    {/each}
  </div>
</div>

<style>
  :global(*, *::before, *::after) { box-sizing: border-box; margin: 0; padding: 0; }
  :global(html, body) {
    background: #0a0a0a;
    color: #e0e0e0;
    font-family: 'DM Mono', monospace;
    font-size: 18px;
    min-height: 100vh;
  }

  .blog {
    max-width: 680px;
    margin: 0 auto;
    padding: 48px 24px 80px;
  }

  nav {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-bottom: 56px;
  }

  .brand {
    font-size: 40px;
    letter-spacing: 10px;
    color: #fff;
    text-decoration: none;
    line-height: 1;
  }

  .brand:hover { color: #aaa; }

  .sep {
    font-size: 20px;
    color: #333;
  }

  .crumb {
    font-size: 14px;
    color: #555;
    letter-spacing: 2px;
    text-transform: lowercase;
  }

  .posts {
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  article {
    padding: 40px 0;
  }

  h2 {
    font-size: 20px;
    font-weight: 400;
    color: #9a7f28;
    letter-spacing: 1px;
    margin-bottom: 8px;
    text-transform: lowercase;
  }

  time {
    display: block;
    font-size: 11px;
    color: #444;
    letter-spacing: 1px;
    text-transform: uppercase;
    margin-bottom: 28px;
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .body p {
    line-height: 1.75;
    color: #ccc;
    font-size: 16px;
  }

  hr {
    border: none;
    border-top: 1px solid #1a1a1a;
  }

  @media (max-width: 640px) {
    .blog { padding: 32px 16px 60px; }

    .brand { font-size: 28px; letter-spacing: 8px; }

    nav { margin-bottom: 40px; }

    article { padding: 32px 0; }
  }
</style>
