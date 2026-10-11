import { useEffect, useRef, useState } from 'react';
import { backend } from '../ipc';
import type { Deck } from '../ipc/types';
import { positionAt } from '../state/extrapolate';
import { store, useAppState } from '../state/store';
import { useAnimationFrame } from '../state/useAnimationFrame';
import { css, waveColors } from './colors';
import { chooseLevel, columnMax, type Peaks } from './peaks';
import { countDraw } from './drawCounter';
import styles from './Overview.module.css';

interface Props {
  deck: Deck;
  peaks: Peaks | null;
}

/** The whole track at a glance. The waveform image is drawn only when the peaks or the size
 *  change; the playhead and the played part are a CSS overlay moved every frame. */
export function Overview({ deck, peaks }: Props) {
  const i = deck === 'a' ? 0 : 1;
  const root = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0, dpr: 1 });
  const frames = useAppState((s) => s.frame?.decks[i]?.frames ?? 0);
  const sampleRate = useAppState((s) => s.frame?.sampleRate ?? 0);
  const cue = useAppState((s) => s.frame?.decks[i]?.cue ?? 0);
  const playing = useAppState((s) => s.frame?.decks[i]?.playing ?? false);

  useEffect(() => {
    const el = root.current;
    if (!el) return;
    const measure = () =>
      setSize({ w: el.clientWidth, h: el.clientHeight, dpr: window.devicePixelRatio || 1 });
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    measure();
    return () => observer.disconnect();
  }, []);

  // The image.
  useEffect(() => {
    const c = canvas.current;
    const el = root.current;
    if (!c || !el || size.w === 0) return;
    c.width = Math.round(size.w * size.dpr);
    c.height = Math.round(size.h * size.dpr);
    const g = c.getContext('2d');
    if (!g) return;
    g.clearRect(0, 0, c.width, c.height);
    const seconds = sampleRate > 0 ? frames / sampleRate : 0;
    if (!peaks || seconds <= 0) return;
    const colors = waveColors(el);
    const cols = c.width;
    const spp = seconds / cols;
    const level = chooseLevel(peaks, spp);
    const mid = c.height / 2;
    const v = [0, 0, 0];
    // Low widest at the back, high on top: the bands stay readable where they overlap.
    const bands = [
      [0, colors.low, 1],
      [1, colors.mid, 0.8],
      [2, colors.high, 0.55],
    ] as const;
    for (const [band, color, scale] of bands) {
      g.fillStyle = css(color);
      for (let x = 0; x < cols; x++) {
        columnMax(peaks, level, x * spp, (x + 1) * spp, v);
        const h = ((v[band] ?? 0) / 255) * mid * scale;
        if (h > 0.25) g.fillRect(x, mid - h, 1, 2 * h);
      }
    }
    countDraw();
  }, [peaks, size, frames, sampleRate]);

  // Playhead and played part, at display rate while playing.
  useAnimationFrame(playing, (now) => {
    const { frame, receivedAt } = store.get();
    const d = frame?.decks[i];
    if (!frame || !d || !root.current) return;
    const pos = positionAt(d, frame.sampleRate, receivedAt, now);
    root.current.style.setProperty('--progress', String(d.frames ? pos / d.frames : 0));
  });
  useAppState((s) => {
    const d = s.frame?.decks[i];
    return d && !d.playing ? d.position : -1;
  });

  const seek = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0 || frames <= 0) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const at = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
    backend.deck(deck, { type: 'seek', frame: Math.round(at * frames) });
  };

  return (
    <div
      ref={root}
      className={styles.overview}
      data-peaks={peaks ? 'ready' : 'waiting'}
      style={{ '--cue': String(frames ? cue / frames : 0) } as React.CSSProperties}
      onPointerDown={seek}
      role="presentation"
      title="Klicken springt an diese Stelle"
    >
      <canvas ref={canvas} className={styles.canvas} aria-hidden="true" />
      {!peaks && <div className={styles.waiting} aria-hidden="true" />}
      <div className={styles.played} aria-hidden="true" />
      <div className={styles.cue} aria-hidden="true" />
      <div className={styles.playhead} aria-hidden="true" />
    </div>
  );
}
