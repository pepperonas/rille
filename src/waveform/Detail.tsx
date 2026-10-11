import { useCallback, useEffect, useRef, useState } from 'react';
import type { Deck } from '../ipc/types';
import { positionAt } from '../state/extrapolate';
import { store, useAppState } from '../state/store';
import { useAnimationFrame } from '../state/useAnimationFrame';
import { waveColors } from './colors';
import { countDraw } from './drawCounter';
import { DetailRenderer } from './gl';
import { ZOOM_SECONDS, type Peaks } from './peaks';
import { setZoom } from './zoom';
import styles from './Detail.module.css';

interface Props {
  deck: Deck;
  peaks: Peaks | null;
  bpm: number | null;
  firstBeat: number | null;
}

/** Scrolling waveform around the playhead with the beatgrid. Draws at display rate while the
 *  deck plays, otherwise once per change (seek, zoom, new peaks, resize). */
export function Detail({ deck, peaks, bpm, firstBeat }: Props) {
  const i = deck === 'a' ? 0 : 1;
  const label = deck === 'a' ? 'Deck 1' : 'Deck 2';
  const root = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const renderer = useRef<DetailRenderer | null>(null);
  const [failed, setFailed] = useState(false);
  const [size, setSize] = useState({ w: 0, h: 0 });
  const zoom = useAppState((s) => s.waveZoom);
  const playing = useAppState((s) => s.frame?.decks[i]?.playing ?? false);
  const seconds = ZOOM_SECONDS[zoom] ?? 8;

  // The renderer lives as long as its canvas (callback ref with cleanup).
  const attach = useCallback((c: HTMLCanvasElement | null) => {
    canvas.current = c;
    if (!c) return;
    try {
      renderer.current = new DetailRenderer(c);
    } catch (e) {
      console.warn('detail waveform unavailable', e);
      setFailed(true);
    }
    return () => {
      renderer.current?.dispose();
      renderer.current = null;
      canvas.current = null;
    };
  }, []);

  useEffect(() => {
    const el = root.current;
    if (!el) return;
    const measure = () => {
      const dpr = window.devicePixelRatio || 1;
      setSize({ w: Math.round(el.clientWidth * dpr), h: Math.round(el.clientHeight * dpr) });
    };
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    measure();
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const r = renderer.current;
    const el = root.current;
    if (!r || !el) return;
    r.setColors(waveColors(el));
    r.setPeaks(peaks);
  }, [peaks]);

  useEffect(() => {
    const c = canvas.current;
    if (!c) return;
    c.width = size.w;
    c.height = size.h;
  }, [size]);

  useAnimationFrame(playing, (now) => {
    const r = renderer.current;
    const { frame, receivedAt } = store.get();
    const d = frame?.decks[i];
    if (!r || !frame || !d || frame.sampleRate <= 0) return;
    const pos = positionAt(d, frame.sampleRate, receivedAt, now);
    r.draw({
      center: pos / frame.sampleRate,
      secondsVisible: seconds,
      bpm: bpm ?? 0,
      firstBeat: firstBeat ?? 0,
    });
    countDraw();
  });
  // While paused, redraw on seek; zoom, peaks and size re-render through their own state.
  useAppState((s) => {
    const d = s.frame?.decks[i];
    return d && !d.playing ? d.position : -1;
  });

  const onWheel = (e: React.WheelEvent) => {
    if (Math.abs(e.deltaY) < 1) return;
    setZoom(zoom + (e.deltaY > 0 ? 1 : -1));
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === '+' || e.key === '=') setZoom(zoom - 1);
    else if (e.key === '-') setZoom(zoom + 1);
    else return;
    e.preventDefault();
  };

  return (
    <div
      ref={root}
      className={styles.detail}
      data-peaks={peaks ? 'ready' : 'waiting'}
      tabIndex={0}
      role="img"
      aria-label={`Waveform ${label}, ${seconds} Sekunden sichtbar`}
      onWheel={onWheel}
      onKeyDown={onKeyDown}
    >
      <canvas ref={attach} className={styles.canvas} />
      <div className={styles.playhead} aria-hidden="true" />
      {!peaks && !failed && <p className={styles.note}>Waveform wird berechnet …</p>}
      {failed && <p className={styles.note}>Waveform braucht WebGL2</p>}
      <span className={`${styles.zoom} numeric`} aria-hidden="true">
        {seconds} s
      </span>
    </div>
  );
}
