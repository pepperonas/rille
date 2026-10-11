import { useCallback, useEffect, useRef, useState } from 'react';
import type { Deck } from '../ipc/types';
import { positionAt } from '../state/extrapolate';
import { store, useAppState } from '../state/store';
import { useAnimationFrame } from '../state/useAnimationFrame';
import { waveColors } from './colors';
import { countDraw } from './drawCounter';
import { DetailRenderer } from './gl';
import { ZOOM_SECONDS } from './peaks';
import type { PeaksState } from './usePeaks';
import { useCanvasSize } from './useCanvasSize';
import { setZoom, wheelSteps } from './zoom';
import styles from './Detail.module.css';

/** Renderers by canvas, so a re-attach (StrictMode) finds its renderer again. */
const renderers = new WeakMap<
  HTMLCanvasElement,
  { renderer: DetailRenderer; dispose?: ReturnType<typeof setTimeout> }
>();

interface Props {
  deck: Deck;
  peaks: PeaksState;
  bpm: number | null;
  firstBeat: number | null;
}

/** Scrolling waveform around the playhead with the beatgrid. Draws at display rate while the
 *  deck plays, otherwise once per change (seek, zoom, new peaks, resize, GPU restore). */
export function Detail({ deck, peaks, bpm, firstBeat }: Props) {
  const i = deck === 'a' ? 0 : 1;
  const label = deck === 'a' ? 'Deck 1' : 'Deck 2';
  const root = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement | null>(null);
  const renderer = useRef<DetailRenderer | null>(null);
  const wheel = useRef(0);
  const [failed, setFailed] = useState(false);
  // Bumped after a GPU context restore, so the idle deck draws again.
  const [restores, setRestores] = useState(0);
  const size = useCanvasSize(root);
  const zoom = useAppState((s) => s.waveZoom);
  const playing = useAppState((s) => s.frame?.decks[i]?.playing ?? false);
  const seconds = ZOOM_SECONDS[zoom] ?? 8;
  const data = peaks.status === 'ready' ? peaks.peaks : null;

  // The renderer lives as long as its canvas (callback ref with cleanup). Disposal waits a
  // tick: React's StrictMode detaches and re-attaches the same canvas, and disposing gives the
  // GPU context up for good – the re-attached renderer would draw into a dead context.
  const attach = useCallback((c: HTMLCanvasElement | null) => {
    canvas.current = c;
    if (!c) return;
    let entry = renderers.get(c);
    if (entry) {
      clearTimeout(entry.dispose);
    } else {
      try {
        entry = { renderer: new DetailRenderer(c, () => setRestores((n) => n + 1)) };
        renderers.set(c, entry);
      } catch (e) {
        console.info('detail waveform unavailable', e);
        setFailed(true);
        return;
      }
    }
    const mine = entry;
    renderer.current = mine.renderer;
    return () => {
      renderer.current = null;
      canvas.current = null;
      mine.dispose = setTimeout(() => {
        mine.renderer.dispose();
        renderers.delete(c);
      }, 0);
    };
  }, []);

  useEffect(() => {
    const r = renderer.current;
    const el = root.current;
    if (!r || !el) return;
    r.setColors(waveColors(el));
    r.setPeaks(data);
  }, [data]);

  useEffect(() => {
    const c = canvas.current;
    if (!c) return;
    c.width = Math.round(size.w * size.dpr);
    c.height = Math.round(size.h * size.dpr);
  }, [size]);

  useAnimationFrame(playing, (now) => {
    const r = renderer.current;
    const { frame, receivedAt } = store.get();
    const d = frame?.decks[i];
    if (!r || !frame || !d || frame.sampleRate <= 0) return;
    const pos = positionAt(d, frame.sampleRate, receivedAt, now);
    r.draw({
      center: pos / frame.sampleRate,
      duration: d.frames / frame.sampleRate,
      secondsVisible: seconds,
      bpm: bpm ?? 0,
      firstBeat: firstBeat ?? 0,
    });
    countDraw();
  });
  // While paused, redraw on seek; zoom, peaks, size and restores re-render through state.
  useAppState((s) => {
    const d = s.frame?.decks[i];
    return d && !d.playing ? d.position : -1;
  });

  // Trackpads send a stream of small deltas (with inertia): accumulate, one step per notch.
  const onWheel = (e: React.WheelEvent) => {
    const { steps, rest } = wheelSteps(wheel.current, e.deltaY, e.deltaMode);
    wheel.current = rest;
    if (steps !== 0) setZoom(zoom + steps);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === '+' || e.key === '=') setZoom(zoom - 1);
    else if (e.key === '-') setZoom(zoom + 1);
    else return;
    e.preventDefault();
  };

  const note = failed
    ? 'Waveform braucht WebGL2'
    : peaks.status === 'waiting'
      ? 'Waveform wird berechnet …'
      : peaks.status === 'unavailable'
        ? `Keine Waveform: ${peaks.reason}`
        : null;

  return (
    <div
      ref={root}
      className={styles.detail}
      data-peaks={peaks.status}
      data-restores={restores}
      tabIndex={0}
      role="group"
      aria-label={`Waveform ${label}, ${seconds} Sekunden sichtbar. Plus und Minus zoomen.`}
      onWheel={onWheel}
      onKeyDown={onKeyDown}
    >
      <canvas ref={attach} className={styles.canvas} aria-hidden="true" />
      <div className={styles.playhead} aria-hidden="true" />
      {note && <p className={styles.note}>{note}</p>}
      <span className={`${styles.zoom} numeric`} aria-hidden="true">
        {seconds} s
      </span>
    </div>
  );
}
