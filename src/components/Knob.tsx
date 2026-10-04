import { useEffect, useRef, useState } from 'react';
import type { KeyboardEvent, PointerEvent } from 'react';
import { useOptimistic } from '../state/useOptimistic';
import { arcPath, dragValue, valueToAngle, wheelStep } from './knobGeometry';
import styles from './Knob.module.css';

interface Props {
  label: string;
  /** Short caption under the knob (e.g. "HI"). */
  caption: string;
  value: number;
  onChange: (value: number) => void;
  /** Restored by double click; also the origin of the value arc. */
  defaultValue?: number;
  /** Greyed out (e.g. EQ band killed) but still operable. */
  muted?: boolean;
  valueText?: string;
  /** Makes the caption a toggle button (EQ kill). */
  captionToggle?: { label: string; pressed: boolean; onToggle: () => void };
}

const SIZE = 40;
const R = 16;
const KEY_STEP = 0.02;

/** Rotary control: drag vertically (Shift = fine), wheel, arrows, double click = default. */
export function Knob({
  label,
  caption,
  value,
  onChange,
  defaultValue = 0.5,
  muted,
  valueText,
  captionToggle,
}: Props) {
  const drag = useRef<{ y: number; start: number } | null>(null);
  const [local, setLocal] = useState<number | null>(null);
  // Steps (keys, wheel) build on the last value sent, not on a snapshot that may lag behind.
  const sent = useOptimistic(value);
  const shown = local ?? sent.shown;
  const set = (v: number) => {
    const c = Math.min(1, Math.max(0, v));
    sent.set(c);
    onChange(c);
    return c;
  };
  const setRef = useRef(set);
  useEffect(() => {
    setRef.current = set;
  });
  const knobRef = useRef<HTMLDivElement>(null);

  // Native, non-passive wheel listener: React's onWheel cannot preventDefault, and the mixer
  // column would scroll along with the knob.
  useEffect(() => {
    const el = knobRef.current;
    if (!el) return;
    const onWheel = (e: globalThis.WheelEvent) => {
      e.preventDefault();
      setRef.current(sent.current() + wheelStep(e.deltaY, e.deltaMode, e.shiftKey));
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    return () => el.removeEventListener('wheel', onWheel);
    // `sent.current` reads a ref and is stable in behaviour.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    e.currentTarget.focus();
    drag.current = { y: e.clientY, start: sent.current() };
    setLocal(shown);
  };
  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d) return;
    setLocal(set(dragValue(d.start, e.clientY - d.y, e.shiftKey)));
  };
  const end = () => {
    drag.current = null;
    setLocal(null);
  };
  const onKeyDown = (e: KeyboardEvent) => {
    const step = e.shiftKey ? KEY_STEP * 5 : KEY_STEP;
    const base = sent.current();
    if (e.key === 'ArrowUp' || e.key === 'ArrowRight') set(base + step);
    else if (e.key === 'ArrowDown' || e.key === 'ArrowLeft') set(base - step);
    else if (e.key === 'Home') set(0);
    else if (e.key === 'End') set(1);
    else if (e.key === 'Delete' || e.key === 'Backspace') set(defaultValue);
    else return;
    e.preventDefault();
  };

  const angle = valueToAngle(shown);
  const origin = valueToAngle(defaultValue);

  return (
    <div className={styles.wrap} data-muted={muted || undefined}>
      <div
        ref={knobRef}
        className={styles.knob}
        role="slider"
        tabIndex={0}
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(shown * 100)}
        aria-valuetext={valueText}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={end}
        onPointerCancel={end}
        onKeyDown={onKeyDown}
        onDoubleClick={() => set(defaultValue)}
      >
        <svg viewBox={`0 0 ${SIZE} ${SIZE}`} aria-hidden="true">
          <path className={styles.track} d={arcPath(SIZE / 2, SIZE / 2, R, -135, 135)} />
          <path className={styles.value} d={arcPath(SIZE / 2, SIZE / 2, R, origin, angle)} />
          <g style={{ transform: `rotate(${angle}deg)`, transformOrigin: '50% 50%' }}>
            <circle className={styles.cap} cx={SIZE / 2} cy={SIZE / 2} r={R - 5} />
            <line className={styles.pointer} x1={SIZE / 2} y1={SIZE / 2 - 4} x2={SIZE / 2} y2={SIZE / 2 - R + 6} />
          </g>
        </svg>
      </div>
      {captionToggle ? (
        <button
          type="button"
          className={styles.captionButton}
          aria-label={captionToggle.label}
          aria-pressed={captionToggle.pressed}
          title={captionToggle.label}
          onClick={captionToggle.onToggle}
        >
          {caption}
        </button>
      ) : (
        <span className={styles.caption}>{caption}</span>
      )}
    </div>
  );
}
