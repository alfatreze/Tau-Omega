<script lang="ts">
  // Placeholder live preview: a plain <canvas> animation driven by synthetic
  // (not real) audio-level data, reacting to the selected meter's current
  // parameter values in real time. This is NOT tau-alpha's real preview
  // stack (`docs/METER_MODULE_SPEC.md` section 7: tau_fb.js/tau_theme.js/
  // tau_audio.js/tau_ballistics.js, matching the device's own RGB565
  // quantisation and ballistics exactly) -- that stack does not exist to
  // vendor yet. This exists so the UI structure (a live-updating preview
  // panel, reacting to the property editor) is real and testable now, ready
  // to have its drawing internals swapped for the real stack later without
  // touching the surrounding page.
  import { onMount, onDestroy } from 'svelte';
  import type { MeterSchema } from './schema';

  export let meter: MeterSchema;
  export let values: Record<string, number | boolean>;

  let canvas: HTMLCanvasElement;
  let frame: number;
  const WIDTH = 400;
  const HEIGHT = 110;

  // Smoothed per-band levels, persisted across frames so attack/release
  // params have something to act on (matches the real ballistics idea,
  // "instant attack / eased release", without claiming to be it).
  let bandLevels: number[] = [];

  function synthLevel(band: number, t: number): number {
    // A handful of unrelated frequencies so bands don't move in lockstep.
    const f = 0.6 + band * 0.37;
    return 0.5 + 0.5 * Math.sin(t * f + band);
  }

  function drawBars(ctx: CanvasRenderingContext2D, t: number) {
    const bands = Math.max(1, Number(values.bands ?? 16));
    const attackMs = Number(values.attack_ms ?? 20);
    const releaseMs = Number(values.release_ms ?? 220);
    const peakOn = Boolean(values.peak_on ?? true);
    if (bandLevels.length !== bands) bandLevels = new Array(bands).fill(0);
    const barWidth = WIDTH / bands;
    ctx.clearRect(0, 0, WIDTH, HEIGHT);
    for (let i = 0; i < bands; i++) {
      const target = synthLevel(i, t);
      // Faster attack, slower release -- the one ballistics idea every real
      // meter shares, approximated with a simple per-frame lerp rate.
      const rate = target > bandLevels[i] ? 1000 / Math.max(1, attackMs) : 1000 / Math.max(1, releaseMs);
      bandLevels[i] += (target - bandLevels[i]) * Math.min(1, rate * (1 / 60));
      const h = bandLevels[i] * (HEIGHT - 14);
      const x = i * barWidth + 1;
      ctx.fillStyle = '#8fd67a';
      ctx.fillRect(x, HEIGHT - h, barWidth - 2, h);
      if (peakOn) {
        ctx.fillStyle = '#e8ecec';
        ctx.fillRect(x, HEIGHT - h - 4, barWidth - 2, 2);
      }
    }
  }

  function drawScope(ctx: CanvasRenderingContext2D, t: number) {
    const trail = Number(values.trail ?? 25) / 100;
    const smooth = Number(values.smooth ?? 40) / 100;
    // A non-zero trail leaves a fading ghost instead of a hard clear.
    ctx.fillStyle = `rgba(16, 22, 23, ${1 - trail * 0.85})`;
    ctx.fillRect(0, 0, WIDTH, HEIGHT);
    ctx.beginPath();
    const points = 96;
    for (let i = 0; i <= points; i++) {
      const x = (i / points) * WIDTH;
      const phase = t * 2 + i * (0.25 - smooth * 0.15);
      const y = HEIGHT / 2 + Math.sin(phase) * (HEIGHT / 2 - 8) * (0.6 + 0.4 * Math.sin(t * 0.7));
      if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
    }
    ctx.strokeStyle = '#8fd67a';
    ctx.lineWidth = 2;
    ctx.stroke();
  }

  function drawGeneric(ctx: CanvasRenderingContext2D, t: number) {
    // A generic stand-in for meters with no bars/scope shape yet (e.g.
    // Chladni) -- concentric rings reacting to `tiles`, just enough to show
    // the preview panel is alive and parameter-reactive.
    const tiles = Math.max(1, Number(values.tiles ?? 4));
    ctx.fillStyle = '#101617';
    ctx.fillRect(0, 0, WIDTH, HEIGHT);
    ctx.strokeStyle = '#8fd67a';
    ctx.lineWidth = 1.5;
    for (let i = 1; i <= tiles; i++) {
      const r = (i / tiles) * (HEIGHT / 2 - 4) + 4 * Math.sin(t + i);
      ctx.beginPath();
      ctx.arc(WIDTH / 2, HEIGHT / 2, Math.abs(r), 0, Math.PI * 2);
      ctx.stroke();
    }
  }

  function tick() {
    const ctx = canvas?.getContext('2d');
    if (ctx) {
      const t = performance.now() / 1000;
      if (meter.params.some((p) => p.key === 'bands')) drawBars(ctx, t);
      else if (meter.params.some((p) => p.key === 'smooth' || p.key === 'trail')) drawScope(ctx, t);
      else drawGeneric(ctx, t);
    }
    frame = requestAnimationFrame(tick);
  }

  onMount(() => {
    frame = requestAnimationFrame(tick);
  });
  onDestroy(() => {
    if (frame) cancelAnimationFrame(frame);
  });
</script>
<div class="meter-preview">
  <canvas bind:this={canvas} width={WIDTH} height={HEIGHT}></canvas>
  <p class="meter-preview-caption">Placeholder preview — synthetic data, not tau-alpha's real rendering stack.</p>
</div>
<style>
  .meter-preview { display: grid; gap: 6px; }
  .meter-preview canvas { width: 100%; height: 110px; border-radius: 9px; border: 1px solid #344244; background: #101617; }
  .meter-preview-caption { margin: 0; font-size: 11px; color: #718180; }
</style>
