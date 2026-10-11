import { useEffect, useRef } from 'react';
import { backend } from '../ipc';
import type { Deck } from '../ipc/types';
import { formatTime, positionAt } from '../state/extrapolate';
import { store, useAppState } from '../state/store';
import { useAnimationFrame } from '../state/useAnimationFrame';
import { css, waveColors } from './colors';
import { countDraw } from './drawCounter';
import { chooseLevel, columnMax, seekTarget } from './peaks';
import type { PeaksState } from './usePeaks';
import { useCanvasSize } from './useCanvasSize';
import styles from './Overview.module.css';

/** Arrow keys move the playhead by this much (Shift: ten times as far). */
const KEY_STEP_SECONDS = 5;

interface Props {
  deck: Deck;
  peaks: PeaksState;
}

/** The whole track at a glance, also a seek control. The waveform image is drawn only when
 *  the peaks or the size change; playhead and played part are a CSS overlay. */
export function Overview({ deck, peaks }: Props) {
  const i = deck === 'a' ? 0 : 1;
  const label = deck === 'a' ? 'Deck 1' : 'Deck 2';
  const root = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const size = useCanvasSize(root);
  const frames = useAppState((s) => s.frame?.decks[i]?.frames ?? 0);
  const sampleRate = useAppState((s) => s.frame?.sampleRate ?? 0);
  const cue = useAppState((s) => s.frame?.decks[i]?.cue ?? 0);
  const playing = useAppState((s) => s.frame?.decks[i]?.playing ?? false);
  // Whole seconds, only for the screen reader value; the bar itself moves through CSS.
  const second = useAppState((s) => {
    const rate = s.frame?.sampleRate ?? 0;
    return rate > 0 ? Math.floor((s.frame?.decks[i]?.position ?? 0) / rate) : 0;
  });
  const data = peaks.status === 'ready' ? peaks.peaks : null;

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
    if (!data || seconds <= 0) return;
    const colors = waveColors(el);
    const cols = c.width;
    const spp = seconds / cols;
    const level = chooseLevel(data, spp);
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
        columnMax(data, level, x * spp, (x + 1) * spp, v);
        const h = ((v[band] ?? 0) / 255) * mid * scale;
        if (h > 0.25) g.fillRect(x, mid - h, 1, 2 * h);
      }
    }
    countDraw();
  }, [data, size, frames, sampleRate]);

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

  const seekTo = (frame: number) => backend.deck(deck, { type: 'seek', frame });

  const onPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0 || frames <= 0) return;
    const rect = e.currentTarget.getBoundingClientRect();
    seekTo(seekTarget((e.clientX - rect.left) / rect.width, frames));
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    const d = store.get().frame?.decks[i];
    if (!d || frames <= 0 || sampleRate <= 0) return;
    const step = KEY_STEP_SECONDS * sampleRate * (e.shiftKey ? 10 : 1);
    const target: Record<string, number> = {
      ArrowRight: d.position + step,
      ArrowUp: d.position + step,
      ArrowLeft: d.position - step,
      ArrowDown: d.position - step,
      Home: 0,
      End: frames,
    };
    const to = target[e.key];
    if (to === undefined) return;
    e.preventDefault();
    seekTo(seekTarget(to / frames, frames));
  };

  const note =
    peaks.status === 'waiting' ? 'waiting' : peaks.status === 'unavailable' ? 'none' : undefined;

  return (
    <div
      ref={root}
      className={styles.overview}
      data-peaks={peaks.status}
      data-note={note}
      style={{ '--cue': String(frames ? cue / frames : 0) } as React.CSSProperties}
      onPointerDown={onPointerDown}
      onKeyDown={onKeyDown}
      role="slider"
      tabIndex={0}
      aria-label={`Position ${label}`}
      aria-valuemin={0}
      aria-valuemax={Math.round(frames / Math.max(1, sampleRate))}
      aria-valuenow={second}
      aria-valuetext={formatTime(second * sampleRate, sampleRate).replace(/\.\d$/, '')}
      title="Klicken springt an diese Stelle"
    >
      <canvas ref={canvas} className={styles.canvas} aria-hidden="true" />
      {peaks.status !== 'ready' && <div className={styles.waiting} aria-hidden="true" />}
      <div className={styles.played} aria-hidden="true" />
      <div className={styles.cue} aria-hidden="true" />
      <div className={styles.playhead} aria-hidden="true" />
    </div>
  );
}
