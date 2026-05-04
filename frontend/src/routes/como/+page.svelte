<script>
  import { onMount, onDestroy } from 'svelte';

  // Shared color language across all three animations:
  // white  = raw real location
  // purple = snapped to privacy grid
  // amber  = final published position (noise applied)
  const WHITE  = [255, 255, 255];
  const PURPLE = [130, 105, 200];
  const AMBER  = [195, 145,  55];

  function rgba([r, g, b], a) { return `rgba(${r},${g},${b},${a})`; }
  function lerpColor([r1,g1,b1], [r2,g2,b2], t) {
    return [
      Math.round(r1 + (r2 - r1) * t),
      Math.round(g1 + (g2 - g1) * t),
      Math.round(b1 + (b2 - b1) * t),
    ];
  }

  // ─── grid-snap animation ───────────────────────────────────────────────────

  let snapCanvas;

  const W = 200;
  const H = 200;
  const CELL = W / 3;

  let snapPhase = 'pause';
  let snapDot = { x: 0, y: 0 };
  let snapStart = { x: 0, y: 0 };
  let snapTarget = { x: 0, y: 0 };
  let snapMoveStart = 0;
  let snapTimer = null;

  const MOVE_DURATION = 700;
  const ARRIVE_HOLD  = 1800;
  const DEPART_HOLD  = 600;

  function randomOffGridPoint() {
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

  function easeOut(t) { return 1 - Math.pow(1 - t, 3); }

  function beginSnap() {
    snapStart  = randomOffGridPoint();
    snapTarget = nearestGridPoint(snapStart);
    snapDot    = { ...snapStart };
    snapPhase  = 'pause';
    snapTimer  = setTimeout(() => {
      snapMoveStart = performance.now();
      snapPhase = 'moving';
    }, DEPART_HOLD);
  }

  function drawSnap(ctx, now) {
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

    // Grid crosshairs
    ctx.strokeStyle = 'rgba(255,255,255,0.12)';
    for (let x = 0; x <= W; x += CELL) {
      for (let y = 0; y <= H; y += CELL) {
        const s = 3;
        ctx.beginPath(); ctx.moveTo(x - s, y); ctx.lineTo(x + s, y); ctx.stroke();
        ctx.beginPath(); ctx.moveTo(x, y - s); ctx.lineTo(x, y + s); ctx.stroke();
      }
    }

    // ~100m label — faint, bottom-right cell
    ctx.font = '9px "DM Mono", monospace';
    ctx.fillStyle = 'rgba(255,255,255,0.18)';
    ctx.textAlign = 'right';
    ctx.textBaseline = 'bottom';
    ctx.fillText('~100m', W - 6, H - 6);

    // Animate dot
    if (snapPhase === 'moving') {
      const t = Math.min((now - snapMoveStart) / MOVE_DURATION, 1);
      const e = easeOut(t);
      snapDot = {
        x: snapStart.x + (snapTarget.x - snapStart.x) * e,
        y: snapStart.y + (snapTarget.y - snapStart.y) * e,
      };
      if (t >= 1) {
        snapDot = { ...snapTarget };
        snapPhase = 'arrived';
        snapTimer = setTimeout(beginSnap, ARRIVE_HOLD);
      }
    }

    // Dashed trail + ghost
    if (snapPhase === 'moving' || snapPhase === 'arrived') {
      ctx.save();
      ctx.setLineDash([3, 4]);
      ctx.strokeStyle = 'rgba(255,255,255,0.15)';
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(snapStart.x, snapStart.y);
      ctx.lineTo(snapTarget.x, snapTarget.y);
      ctx.stroke();
      ctx.restore();

      ctx.beginPath();
      ctx.arc(snapStart.x, snapStart.y, 3, 0, Math.PI * 2);
      ctx.fillStyle = 'rgba(255,255,255,0.15)';
      ctx.fill();
    }

    // Dot
    const arrived = snapPhase === 'arrived';
    ctx.beginPath();
    ctx.arc(snapDot.x, snapDot.y, arrived ? 4.5 : 4, 0, Math.PI * 2);
    ctx.fillStyle = arrived ? rgba(PURPLE, 0.95) : rgba(WHITE, 0.75);
    ctx.fill();
  }

  // ─── gaussian noise animation ──────────────────────────────────────────────

  let noiseCanvas;

  const NW = 200;
  const NH = 200;
  const CENTER = { x: NW / 2, y: NH / 2 };
  // Sigma controlled by the slider (in metres). Mapped to canvas pixels for drawing.
  let noiseSigma = 300;
  $: sigmaPx = Math.min(8 + (noiseSigma / 1000) * 72, 85);

  let particles = [];
  let lastSpawn = 0;
  const SPAWN_INTERVAL = 1100; // ms between new dots

  function gaussian(sigma) {
    // Box-Muller: produces a normally distributed value
    const u = Math.max(1e-10, Math.random());
    const v = Math.random();
    return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v) * sigma;
  }

  function spawnParticle(now) {
    particles.push({
      x: CENTER.x,
      y: CENTER.y,
      tx: CENTER.x + gaussian(sigmaPx),
      ty: CENTER.y + gaussian(sigmaPx),
      phase: 'moving',
      born: now,
      moveDuration: 600,
      holdDuration: 1000,
      fadeDuration: 700,
    });
  }

  function drawNoise(ctx, now) {
    ctx.clearRect(0, 0, NW, NH);

    // Spawn new particle
    if (now - lastSpawn > SPAWN_INTERVAL) {
      spawnParticle(now);
      lastSpawn = now;
    }

    // Possible-landing area — purple tint, scales with slider
    ctx.beginPath();
    ctx.arc(CENTER.x, CENTER.y, sigmaPx, 0, Math.PI * 2);
    ctx.fillStyle = rgba(PURPLE, 0.05);
    ctx.fill();
    ctx.strokeStyle = rgba(PURPLE, 0.28);
    ctx.lineWidth = 1;
    ctx.stroke();

    // Label tracks the circle edge
    ctx.font = '9px "DM Mono", monospace';
    ctx.fillStyle = rgba(PURPLE, 0.35);
    ctx.textAlign = 'left';
    ctx.textBaseline = 'middle';
    const labelX = Math.min(CENTER.x + sigmaPx + 5, NW - 36);
    ctx.fillText(`~${noiseSigma}m`, labelX, CENTER.y);

    // Snapped position — purple crosshair + dot (this is the grid-snapped point)
    const cs = 5;
    ctx.strokeStyle = rgba(PURPLE, 0.65);
    ctx.lineWidth = 1;
    ctx.beginPath(); ctx.moveTo(CENTER.x - cs, CENTER.y); ctx.lineTo(CENTER.x + cs, CENTER.y); ctx.stroke();
    ctx.beginPath(); ctx.moveTo(CENTER.x, CENTER.y - cs); ctx.lineTo(CENTER.x, CENTER.y + cs); ctx.stroke();
    ctx.beginPath();
    ctx.arc(CENTER.x, CENTER.y, 2.5, 0, Math.PI * 2);
    ctx.fillStyle = rgba(PURPLE, 0.85);
    ctx.fill();

    // Update + draw particles
    particles = particles.filter(p => {
      const age = now - p.born;
      const moveEnd = p.moveDuration;
      const holdEnd = moveEnd + p.holdDuration;
      const fadeEnd = holdEnd + p.fadeDuration;

      if (age > fadeEnd) return false;

      let alpha, x, y;

      if (age < moveEnd) {
        const t = easeOut(age / moveEnd);
        x = p.x + (p.tx - p.x) * t;
        y = p.y + (p.ty - p.y) * t;
        alpha = age / moveEnd * 0.7;
      } else if (age < holdEnd) {
        x = p.tx; y = p.ty;
        alpha = 0.7;
      } else {
        x = p.tx; y = p.ty;
        alpha = 0.7 * (1 - (age - holdEnd) / p.fadeDuration);
      }

      // Published position — amber
      ctx.beginPath();
      ctx.arc(x, y, 3, 0, Math.PI * 2);
      ctx.fillStyle = rgba(AMBER, alpha);
      ctx.fill();

      return true;
    });
  }

  // ─── combined animation (map + snap + noise) ──────────────────────────────

  let combCanvas;
  const CW = 200;
  const CH = 200;
  const CC = CW / 3; // privacy grid cell size

  // Hand-placed city blocks — irregular sizes and densities so it reads
  // as a real neighbourhood, not a mathematical grid.
  // Dense small blocks top-left, larger sparse blocks bottom-right,
  // a wide boulevard gap in the middle.
  const CITY_BLOCKS = [
    // Dense top-left neighbourhood
    { x: 2,  y: 2,  w: 34, h: 24 },
    { x: 2,  y: 30, w: 15, h: 20 },
    { x: 21, y: 30, w: 15, h: 20 },
    { x: 2,  y: 54, w: 34, h: 22 },
    { x: 40, y: 2,  w: 20, h: 44 },
    { x: 40, y: 50, w: 20, h: 26 },
    { x: 64, y: 2,  w: 18, h: 30 },
    { x: 64, y: 36, w: 18, h: 40 },
    // Top-right — fewer, larger blocks
    { x: 86,  y: 2,  w: 44, h: 54 },
    { x: 86,  y: 60, w: 20, h: 16 },
    { x: 110, y: 60, w: 20, h: 16 },
    { x: 134, y: 2,  w: 64, h: 74 },
    // Boulevard gap: y ≈ 78–94 (the wide street running across)
    // Bottom-left — medium density
    { x: 2,   y: 94,  w: 38, h: 46 },
    { x: 2,   y: 144, w: 38, h: 54 },
    { x: 44,  y: 94,  w: 26, h: 40 },
    { x: 44,  y: 138, w: 26, h: 60 },
    { x: 74,  y: 94,  w: 18, h: 22 },
    { x: 74,  y: 120, w: 18, h: 18 },
    { x: 74,  y: 142, w: 18, h: 56 },
    // Bottom-right — large sparse blocks
    { x: 96,  y: 94,  w: 50, h: 60 },
    { x: 96,  y: 158, w: 50, h: 40 },
    { x: 150, y: 94,  w: 48, h: 104 },
  ];

  // Noise sigma for this animation (fixed, no slider here)
  const COMB_SIGMA = CC * 0.32;

  // Phase durations (ms)
  const C_DUR = {
    appear:   500,
    waiting:  400,
    snapping: 800,
    at_snap:  900,
    noising:  800,
    at_noise: 1600,
    fading:   500,
  };
  const C_PHASE_ORDER = Object.keys(C_DUR);

  let combPhase = 'appear';
  let combPhaseBorn = 0;
  let combRaw       = { x: 0, y: 0 };
  let combSnapped   = { x: 0, y: 0 };
  let combPublished = { x: 0, y: 0 };

  function combNearestGrid(pt) {
    return {
      x: Math.round(pt.x / CC) * CC,
      y: Math.round(pt.y / CC) * CC,
    };
  }

  function combBegin(now) {
    // Pick a raw point well inside the canvas, away from edges
    const m = 28;
    combRaw = {
      x: m + Math.random() * (CW - m * 2),
      y: m + Math.random() * (CH - m * 2),
    };
    combSnapped = combNearestGrid(combRaw);
    // Clamp published point so it stays inside the canvas
    combPublished = {
      x: Math.max(8, Math.min(CW - 8, combSnapped.x + gaussian(COMB_SIGMA))),
      y: Math.max(8, Math.min(CH - 8, combSnapped.y + gaussian(COMB_SIGMA))),
    };
    combPhase = 'appear';
    combPhaseBorn = now;
  }

  function drawCombMap(ctx) {
    ctx.lineWidth = 0.5;
    for (const b of CITY_BLOCKS) {
      ctx.fillStyle = 'rgba(255,255,255,0.045)';
      ctx.fillRect(b.x, b.y, b.w, b.h);
      ctx.strokeStyle = 'rgba(255,255,255,0.09)';
      ctx.strokeRect(b.x + 0.5, b.y + 0.5, b.w - 1, b.h - 1);
    }
  }

  function drawCombGrid(ctx, alpha) {
    ctx.strokeStyle = `rgba(255,255,255,${alpha})`;
    ctx.lineWidth = 1;
    for (let x = 0; x <= CW; x += CC) {
      ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, CH); ctx.stroke();
    }
    for (let y = 0; y <= CH; y += CC) {
      ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(CW, y); ctx.stroke();
    }
    // Crosshairs at intersections
    ctx.strokeStyle = `rgba(255,255,255,${(alpha * 1.6).toFixed(3)})`;
    for (let x = 0; x <= CW; x += CC) {
      for (let y = 0; y <= CH; y += CC) {
        const s = 3;
        ctx.beginPath(); ctx.moveTo(x - s, y); ctx.lineTo(x + s, y); ctx.stroke();
        ctx.beginPath(); ctx.moveTo(x, y - s); ctx.lineTo(x, y + s); ctx.stroke();
      }
    }
  }

  function drawComb(ctx, now) {
    ctx.clearRect(0, 0, CW, CH);

    const age = now - combPhaseBorn;
    const dur = C_DUR[combPhase];
    const t   = Math.min(age / dur, 1);

    // Advance to next phase when current one finishes
    if (t >= 1) {
      const idx = C_PHASE_ORDER.indexOf(combPhase);
      if (idx < C_PHASE_ORDER.length - 1) {
        combPhase = C_PHASE_ORDER[idx + 1];
        combPhaseBorn = now;
      } else {
        combBegin(now);
      }
      return;
    }

    // Map backdrop — always visible
    drawCombMap(ctx);

    // Privacy grid — fades in as the dot snaps, stays visible after
    const gridAlpha =
      combPhase === 'appear'   ? 0.02 :
      combPhase === 'waiting'  ? 0.02 + easeOut(t) * 0.02 :
      combPhase === 'snapping' ? 0.04 + easeOut(t) * 0.02 :
      0.05;
    drawCombGrid(ctx, gridAlpha);

    // Compute current dot position and color
    let dotX, dotY, dotAlpha;

    if (combPhase === 'appear') {
      dotX = combRaw.x; dotY = combRaw.y;
      dotAlpha = easeOut(t);
    } else if (combPhase === 'waiting') {
      dotX = combRaw.x; dotY = combRaw.y;
      dotAlpha = 1;
    } else if (combPhase === 'snapping') {
      const e = easeOut(t);
      dotX = combRaw.x + (combSnapped.x - combRaw.x) * e;
      dotY = combRaw.y + (combSnapped.y - combRaw.y) * e;
      dotAlpha = 1;
    } else if (combPhase === 'at_snap') {
      dotX = combSnapped.x; dotY = combSnapped.y;
      dotAlpha = 1;
    } else if (combPhase === 'noising') {
      const e = easeOut(t);
      dotX = combSnapped.x + (combPublished.x - combSnapped.x) * e;
      dotY = combSnapped.y + (combPublished.y - combSnapped.y) * e;
      dotAlpha = 1;
    } else {
      // at_noise or fading
      dotX = combPublished.x; dotY = combPublished.y;
      dotAlpha = combPhase === 'fading' ? 1 - easeOut(t) : 1;
    }

    // Ghost dot at raw position once dot has snapped away (white)
    const showRawGhost = ['at_snap', 'noising', 'at_noise', 'fading'].includes(combPhase);
    if (showRawGhost) {
      const ga = combPhase === 'fading' ? (1 - easeOut(t)) * 0.25 : 0.25;
      ctx.beginPath();
      ctx.arc(combRaw.x, combRaw.y, 3, 0, Math.PI * 2);
      ctx.fillStyle = rgba(WHITE, ga);
      ctx.fill();
    }

    // Ghost dot at snapped position once noise has moved the dot away (purple)
    if (['noising', 'at_noise'].includes(combPhase)) {
      ctx.beginPath();
      ctx.arc(combSnapped.x, combSnapped.y, 3, 0, Math.PI * 2);
      ctx.fillStyle = rgba(PURPLE, 0.28);
      ctx.fill();
    }

    // Dashed trail from raw → snapped
    if (['at_snap', 'noising', 'at_noise'].includes(combPhase)) {
      ctx.save();
      ctx.setLineDash([3, 4]);
      ctx.strokeStyle = rgba(WHITE, 0.12);
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(combRaw.x, combRaw.y);
      ctx.lineTo(combSnapped.x, combSnapped.y);
      ctx.stroke();
      ctx.restore();
    }

    // Dot color: white → purple (snapping), purple (at_snap), purple → amber (noising), amber
    let dotColor;
    if (combPhase === 'appear' || combPhase === 'waiting') {
      dotColor = rgba(WHITE, dotAlpha);
    } else if (combPhase === 'snapping') {
      dotColor = rgba(lerpColor(WHITE, PURPLE, easeOut(t)), dotAlpha);
    } else if (combPhase === 'at_snap') {
      dotColor = rgba(PURPLE, dotAlpha);
    } else if (combPhase === 'noising') {
      dotColor = rgba(lerpColor(PURPLE, AMBER, easeOut(t)), dotAlpha);
    } else {
      dotColor = rgba(AMBER, dotAlpha);
    }

    ctx.beginPath();
    ctx.arc(dotX, dotY, 4, 0, Math.PI * 2);
    ctx.fillStyle = dotColor;
    ctx.fill();
  }

  // ─── radius animation ─────────────────────────────────────────────────────

  let radiusCanvas;
  const RW = 200;
  const RH = 200;
  const R_CENTER = { x: RW / 2, y: RH / 2 };

  let feedRadius = 5; // km, driven by slider
  $: radiusPx = 14 + (feedRadius / 10) * 76;

  // Stable scatter of message dots — generated once on mount
  let feedDots = [];

  function generateFeedDots() {
    const margin = 10;
    feedDots = Array.from({ length: 22 }, () => ({
      x: margin + Math.random() * (RW - margin * 2),
      y: margin + Math.random() * (RH - margin * 2),
    }));
  }

  function drawRadius(ctx) {
    ctx.clearRect(0, 0, RW, RH);

    // Radius fill + border
    ctx.beginPath();
    ctx.arc(R_CENTER.x, R_CENTER.y, radiusPx, 0, Math.PI * 2);
    ctx.fillStyle = rgba(AMBER, 0.04);
    ctx.fill();
    ctx.strokeStyle = rgba(AMBER, 0.28);
    ctx.lineWidth = 1;
    ctx.stroke();

    // Message dots — amber inside radius, dim outside
    for (const dot of feedDots) {
      const inside = Math.hypot(dot.x - R_CENTER.x, dot.y - R_CENTER.y) <= radiusPx;
      ctx.beginPath();
      ctx.arc(dot.x, dot.y, 2.5, 0, Math.PI * 2);
      ctx.fillStyle = inside ? rgba(AMBER, 0.85) : rgba(WHITE, 0.1);
      ctx.fill();
    }

    // User position at center — white
    ctx.beginPath();
    ctx.arc(R_CENTER.x, R_CENTER.y, 3.5, 0, Math.PI * 2);
    ctx.fillStyle = rgba(WHITE, 0.9);
    ctx.fill();
  }

  // ─── shared RAF loop ───────────────────────────────────────────────────────

  let raf;

  function loop(now) {
    const sc = snapCanvas?.getContext('2d');
    if (sc) drawSnap(sc, now);

    const nc = noiseCanvas?.getContext('2d');
    if (nc) drawNoise(nc, now);

    const cc = combCanvas?.getContext('2d');
    if (cc) drawComb(cc, now);

    const rc = radiusCanvas?.getContext('2d');
    if (rc) drawRadius(rc);

    raf = requestAnimationFrame(loop);
  }

  onMount(() => {
    beginSnap();
    lastSpawn = performance.now() - SPAWN_INTERVAL;
    combBegin(performance.now());
    generateFeedDots();
    raf = requestAnimationFrame(loop);
  });

  onDestroy(() => {
    cancelAnimationFrame(raf);
    clearTimeout(snapTimer);
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
      <div class="glass-frame">
        <canvas bind:this={snapCanvas} width={W} height={H}></canvas>
      </div>
    </div>
    <p class="caption">blanco: posición real — morado: ajustado a la cuadrícula.</p>
  </section>

  <section>
    <h2>ruido gaussiano</h2>
    <p>
      después del ajuste, se añade un desplazamiento aleatorio con
      distribución normal — sigma ~300m por defecto. cada mensaje
      publicado desde el mismo lugar llega a una posición diferente.
    </p>
    <div class="canvas-wrap">
      <div class="glass-frame">
        <canvas bind:this={noiseCanvas} width={NW} height={NH}></canvas>
      </div>
    </div>
    <div class="noise-control">
      <span class="noise-label">precisión</span>
      <input type="range" min="50" max="1000" step="50" bind:value={noiseSigma} />
      <span class="noise-value">{noiseSigma}m</span>
    </div>
    <p class="slider-note">al mover el slider de precisión, el área posible del mensaje cambia.</p>
    <p class="caption">morado: posición ajustada a la cuadrícula — amarillo: posición publicada.</p>
  </section>

  <section>
    <h2>en conjunto</h2>
    <p>
      primero el ajuste a la cuadrícula, luego el ruido. la posición
      final nunca coincide con la real — ni siquiera el servidor sabe
      exactamente dónde estás.
    </p>
    <div class="canvas-wrap">
      <div class="glass-frame">
        <canvas bind:this={combCanvas} width={CW} height={CH}></canvas>
      </div>
    </div>
    <p class="caption">blanco: posición real — morado: ajustada a la cuadrícula — amarillo: publicada.</p>
  </section>

  <section>
    <h2>radio de búsqueda</h2>
    <p>
      cuando abres la app, ves los mensajes publicados dentro de un radio
      alrededor de tu posición. tú decides qué tan lejos mirar. para leer,
      se usa tu ubicación real — no se guarda, solo se usa para calcular
      qué mensajes caen dentro del radio.
    </p>
    <div class="canvas-wrap">
      <div class="glass-frame">
        <canvas bind:this={radiusCanvas} width={RW} height={RH}></canvas>
      </div>
    </div>
    <div class="noise-control">
      <span class="noise-label">radio</span>
      <input type="range" min="1" max="10" step="1" bind:value={feedRadius} />
      <span class="noise-value">{feedRadius}km</span>
    </div>
    <p class="caption">blanco: tu posición real (no almacenada) — amarillo: mensajes dentro del radio.</p>
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
    justify-content: center;
  }

  .glass-frame {
    display: inline-block;
    border-radius: 14px;
    border: 2px solid rgba(255, 255, 255, 0.14);
    box-shadow:
      inset 0 2px 0 rgba(255, 255, 255, 0.08),
      inset 0 -2px 0 rgba(255, 255, 255, 0.03);
    overflow: hidden;
    line-height: 0;
  }

  canvas {
    display: block;
  }

  .noise-control {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    margin-top: 16px;
  }

  .noise-label {
    font-size: 10px;
    color: #444;
    letter-spacing: 1px;
    min-width: 52px;
    text-align: right;
  }

  .noise-value {
    font-size: 10px;
    color: #555;
    min-width: 36px;
  }

  input[type='range'] {
    -webkit-appearance: none;
    appearance: none;
    width: 100px;
    height: 1px;
    background: #333;
    outline: none;
    border-radius: 1px;
  }

  input[type='range']::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.5);
    cursor: pointer;
  }

  input[type='range']::-moz-range-thumb {
    width: 10px;
    height: 10px;
    border: none;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.5);
    cursor: pointer;
  }

  .slider-note {
    margin-top: 10px;
    font-size: 11px;
    color: #555;
    text-align: center;
    margin-bottom: 0;
    font-style: italic;
  }

  .caption {
    margin-top: 14px;
    font-size: 11px;
    color: #444;
    margin-bottom: 0;
    text-align: center;
  }
</style>
