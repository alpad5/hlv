import adapter from '@sveltejs/adapter-static';

export default {
  kit: {
    // 404.html is the correct Cloudflare Pages SPA fallback — it's only served
    // when no static file matches, so JS/CSS assets are never intercepted.
    adapter: adapter({ fallback: '404.html' })
  }
};
