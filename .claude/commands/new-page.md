# New Page — hlv Style

Create a new SvelteKit page at `frontend/src/routes/$ARGUMENTS/+page.svelte` that follows the hlv design system.

## What to do

1. Create the route directory and `+page.svelte` file.
2. Apply the full style template below — do not improvise colours, fonts, or nav structure.
3. Fill in the page-specific content (sections, headings, body copy) based on what the user asked for.
4. Add a sidebar link from `+page.svelte` (the main app) pointing to the new route, styled the same as the existing `— blog` and `— cómo funciona` links.

## Style template

Every hlv sub-page must use this exact shell. Replace the `<title>` and page content only — keep everything else as-is.

```svelte
<svelte:head>
  <title>hlv: PAGE_NAME</title>
</svelte:head>

<div class="page">
  <nav>
    <div class="brand-group">
      <a href="/" class="brand">hlv</a>
      <span class="tagline">hablan los vecinos</span>
    </div>
    <span class="sep">/</span>
    <span class="crumb">PAGE_NAME</span>
  </nav>

  <!-- page content goes here -->
</div>

<style>
  @import url('https://fonts.googleapis.com/css2?family=DM+Mono:ital,wght@0,300;0,400;1,300&display=swap');

  :global(html, body) {
    margin: 0;
    background: #0a0a0a;
    color: #e0e0e0;
    font-family: 'DM Mono', monospace;
    font-size: 18px;
    line-height: 1.7;
  }

  .page {
    max-width: 680px;
    margin: 0 auto;
    padding: 48px 24px 80px;
  }

  /* ── nav (matches blog) ── */
  nav {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-bottom: 56px;
  }

  .brand-group {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .brand {
    font-size: 40px;
    letter-spacing: 10px;
    color: #fff;
    text-decoration: none;
    line-height: 1;
  }

  .brand:hover { color: #aaa; }

  .tagline {
    font-size: 11px;
    color: #444;
    letter-spacing: 1px;
    text-transform: lowercase;
  }

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

  /* ── content ── */
  h2 {
    font-size: 14px;
    font-weight: 400;
    letter-spacing: 2px;
    text-transform: lowercase;
    color: #9a7f28;   /* mustard — used for section labels */
    margin: 0 0 16px;
  }

  p {
    font-size: 17px;
    color: #ccc;
    line-height: 1.75;
    margin: 0 0 28px;
  }

  section {
    margin-bottom: 72px;
  }

  /* ── mobile ── */
  @media (max-width: 640px) {
    .page  { padding: 32px 16px 60px; }
    .brand { font-size: 28px; letter-spacing: 8px; }
    nav    { margin-bottom: 40px; }
  }
</style>
```

## Colour reference

| Token | Value | Use |
|---|---|---|
| Background | `#0a0a0a` | `html, body` |
| Body text | `#ccc` / `#e0e0e0` | paragraphs / global default |
| Mustard | `#9a7f28` | section labels (`h2`), accents |
| Dim | `#444` | tagline, dates, captions |
| Muted | `#555` | breadcrumb, secondary labels |
| Ghost | `#333` | separators |
| White | `#fff` | brand mark |

## Font sizes

| Element | Size |
|---|---|
| Brand mark (`hlv`) | 40px / letter-spacing 10px |
| Tagline | 11px |
| Breadcrumb | 14px |
| Section label (`h2`) | 14px |
| Body text | 17px |
| Caption / fine print | 11px |

## Notes

- SSR is disabled project-wide via `+layout.js` (`export const ssr = false`). No need to add this per-page.
- If the page needs canvas animations, follow the pattern in `frontend/src/routes/como/+page.svelte`: shared RAF loop, `onMount`/`onDestroy` lifecycle, canvas `bind:this`.
- `max-width` can be narrowed below 680px for content-heavy or animation-heavy pages (como uses 480px), but the nav, fonts, and colours must stay consistent.
