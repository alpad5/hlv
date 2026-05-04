<script>
  import { onMount, onDestroy } from 'svelte';

  let canvas;
  let raf;

  const W = 280;
  const H = 280;
  const CELL = 40;

  // Animation state
  let phase = 'pause'; // pause | moving | arrived
  let pauseTimer = null;

  let dot = { x: 0, y: 0 };
  let startPt = { x: 0, y: 0 };
  let targetPt = { x: 0, y: 0 };

  let moveStart = 0;
  const MOVE_DURATION = 700;   // ms for the snap animation
  const ARRIVE_HOLD = 1800;    // ms to pause after arriving
  const DEPART_HOLD = 600;     // ms to pause before next move

  function randomOffGridPoint() {
    // Pick a cell that's fully inside the canvas, then place the dot
    // somewhere in the inner 60% of that cell so it's visibly off-grid.
    const cols = Math.floor(W / CELL) - 1;
    const rows = Math.floor(H / CELL) - 1;
    const cx = (Math.floor(Math.random() * cols) + 0.5) * CELL;
    const cy = (Math.floor(Math.random() * rows) + 0.5) * CELL;
    const ox = (Math.random() * 0.5 + 0.15) * CELL * (Math.random() < 0.5 ? 1 : -1);
    const oy = (Math.random() * 0.5 + 0.15) * CELL * (Math.random() < 0.5 ? 1 : -1);
    return { x: cx + ox, y: cy + oy };
  }

  function nearestGridPoint(pt) {
    return {
      x: Math.round(pt.x / CELL) * CELL,
      y: Math.round(pt.y / CELL) * CELL,
    };
  }

  // Cubic ease-out: fast start, decelerates into the target.
  function easeOut(t) {
    return 1 - Math.pow(1 - t, 3);
  }

  function beginMove() {
    startPt = randomOffGridPoint();
    targetPt = nearestGridPoint(startPt);
    dot = { ...startPt };
    phase = 'pause';
    pauseTimer = setTimeout(() => {
      moveStart = performance.now();
      phase = 'moving';
    }, DEPART_HOLD);
  }

  function draw(ctx, now) {
    ctx.clearRect(0, 0, W, H);

    // Grid lines
    ctx.strokeStyle = 'rgba(255,255,255,0.07)';
    ctx.lineWidth = 1;
    for (let x = 0; x <= W; x += CELL) {
      ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, H); ctx.stroke();
    }
    for (let y = 0; y <= H; y += CELL) {
      ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(W, y); ctx.stroke();
    }

    // Grid intersections — faint crosshairs
    ctx.strokeStyle = 'rgba(255,255,255,0.12)';
    ctx.lineWidth = 1;
    for (let x = 0; x <= W; x += CELL) {
      for (let y = 0; y <= H; y += CELL) {
        const s = 3;
        ctx.beginPath(); ctx.moveTo(x - s, y); ctx.lineTo(x + s, y); ctx.stroke();
        ctx.beginPath(); ctx.moveTo(x, y - s); ctx.lineTo(x, y + s); ctx.stroke();
      }
    }

    // Animate dot position
    if (phase === 'moving') {
      const elapsed = now - moveStart;
      const t = Math.min(elapsed / MOVE_DURATION, 1);
      const e = easeOut(t);
      dot = {
        x: startPt.x + (targetPt.x - startPt.x) * e,
        y: startPt.y + (targetPt.y - startPt.y) * e,
      };
      if (t >= 1) {
        dot = { ...targetPt };
        phase = 'arrived';
        pauseTimer = setTimeout(beginMove, ARRIVE_HOLD);
      }
    }

    // Dashed line from start to target (only while in motion or just arrived)
    if (phase === 'moving' || phase === 'arrived') {
      ctx.save();
      ctx.setLineDash([3, 4]);
      ctx.strokeStyle = 'rgba(255,255,255,0.15)';
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(startPt.x, startPt.y);
      ctx.lineTo(targetPt.x, targetPt.y);
      ctx.stroke();
      ctx.restore();

      // Ghost dot at original position
      ctx.beginPath();
      ctx.arc(startPt.x, startPt.y, 3, 0, Math.PI * 2);
      ctx.fillStyle = 'rgba(255,255,255,0.15)';
      ctx.fill();
    }

    // Dot
    const arrived = phase === 'arrived';
    ctx.beginPath();
    ctx.arc(dot.x, dot.y, arrived ? 4.5 : 4, 0, Math.PI * 2);
    ctx.fillStyle = arrived ? 'rgba(255,255,255,0.95)' : 'rgba(255,255,255,0.75)';
    ctx.fill();
  }

  function loop(now) {
    const ctx = canvas?.getContext('2d');
    if (ctx) draw(ctx, now);
    raf = requestAnimationFrame(loop);
  }

  onMount(() => {
    beginMove();
    raf = requestAnimationFrame(loop);
  });

  onDestroy(() => {
    cancelAnimationFrame(raf);
    clearTimeout(pauseTimer);
  });
</script>

<svelte:head>
  <title>qhlv: cómo funciona</title>
</svelte:head>

<div class="page">
  <header>
    <a href="/" class="back">hlv</a>
    <span class="sep">·</span>
    <span class="title">cómo funciona</span>
  </header>

  <section>
    <h2>ajuste a la cuadrícula</h2>
    <p>
      tu ubicación exacta nunca se almacena. antes de guardar cualquier
      mensaje, las coordenadas se ajustan al cruce de cuadrícula más
      cercano — una precisión de ~100m.
    </p>
    <div class="canvas-wrap">
      <canvas bind:this={canvas} width={W} height={H}></canvas>
    </div>
    <p class="caption">el punto blanco es donde escribiste. se mueve al cruce más cercano.</p>
  </section>
</div>

<style>
  @import url('https://fonts.googleapis.com/css2?family=DM+Mono:ital,wght@0,300;0,400;1,300&display=swap');

  :global(body) {
    margin: 0;
    background: #111;
    color: #ccc;
    font-family: 'DM Mono', monospace;
    font-size: 14px;
    line-height: 1.7;
  }

  .page {
    max-width: 480px;
    margin: 0 auto;
    padding: 48px 24px 80px;
  }

  header {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin-bottom: 56px;
    font-size: 13px;
  }

  .back {
    color: #fff;
    text-decoration: none;
    font-size: 20px;
    letter-spacing: 6px;
  }

  .back:hover { color: #aaa; }

  .sep { color: #333; }

  .title {
    color: #555;
    letter-spacing: 1px;
  }

  section {
    margin-bottom: 72px;
  }

  h2 {
    font-size: 12px;
    font-weight: 400;
    letter-spacing: 2px;
    text-transform: lowercase;
    color: #666;
    margin: 0 0 16px;
  }

  p {
    color: #888;
    margin: 0 0 28px;
    font-size: 13px;
  }

  .canvas-wrap {
    display: flex;
    justify-content: flex-start;
  }

  canvas {
    display: block;
  }

  .caption {
    margin-top: 14px;
    font-size: 11px;
    color: #444;
    margin-bottom: 0;
  }
</style>
