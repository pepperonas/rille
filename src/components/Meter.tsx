import { useRef } from 'react';
import { store, useAppState, type AppState } from '../state/store';
import { useAnimationFrame } from '../state/useAnimationFrame';
import { METER_CLIP_DB, METER_FLOOR_DB, METER_WARN_DB, meterFill } from './meterScale';
import styles from './Meter.module.css';

interface Props {
  label: string;
  /** Picks the stereo peak (linear) from the app state. Must be a stable function. */
  select: (s: AppState) => [number, number] | undefined;
}

const SEGMENTS = 24;

interface Palette {
  idle: string;
  normal: string;
  warn: string;
  clip: string;
}

function readPalette(el: HTMLElement): Palette {
  const css = getComputedStyle(el);
  const v = (name: string) => css.getPropertyValue(name).trim();
  return {
    idle: v('--md-surface-container-highest'),
    normal: v('--accent') || v('--md-primary'),
    warn: v('--md-tertiary'),
    clip: v('--md-error'),
  };
}

/** Segmented stereo peak meter on a canvas, redrawn only while levels are moving. */
export function Meter({ label, select }: Props) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const palette = useRef<Palette | null>(null);
  // Re-renders only when the meter starts or stops moving; frames in between are drawn by rAF.
  const live = useAppState((s) => {
    const p = select(s);
    return !!p && (p[0] > 0 || p[1] > 0);
  });

  useAnimationFrame(live, () => {
    const el = canvas.current;
    if (!el) return;
    draw(el, select(store.get()) ?? [0, 0], (palette.current ??= readPalette(el)));
  });

  return (
    <canvas
      ref={canvas}
      className={styles.meter}
      role="meter"
      aria-label={label}
      aria-valuemin={METER_FLOOR_DB}
      aria-valuemax={0}
      width={16}
      height={160}
    />
  );
}

function draw(el: HTMLCanvasElement, peak: [number, number], p: Palette) {
  const dpr = window.devicePixelRatio || 1;
  const w = el.clientWidth;
  const h = el.clientHeight;
  if (el.width !== Math.round(w * dpr) || el.height !== Math.round(h * dpr)) {
    el.width = Math.round(w * dpr);
    el.height = Math.round(h * dpr);
  }
  const ctx = el.getContext('2d');
  if (!ctx) return;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, h);
  const gap = 2;
  const colW = (w - gap) / 2;
  const segH = (h - gap * (SEGMENTS - 1)) / SEGMENTS;
  const warnAt = Math.round(SEGMENTS * (1 - METER_WARN_DB / METER_FLOOR_DB));
  const clipAt = Math.round(SEGMENTS * (1 - METER_CLIP_DB / METER_FLOOR_DB)) - 1;
  peak.forEach((value, ch) => {
    const lit = Math.round(meterFill(value) * SEGMENTS);
    for (let i = 0; i < SEGMENTS; i++) {
      const on = i < lit;
      ctx.fillStyle = !on ? p.idle : i >= clipAt ? p.clip : i >= warnAt ? p.warn : p.normal;
      const y = h - (i + 1) * segH - i * gap;
      ctx.beginPath();
      ctx.roundRect(ch * (colW + gap), y, colW, segH, 1);
      ctx.fill();
    }
  });
}
