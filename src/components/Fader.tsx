import { useRef, useState } from 'react';
import type { KeyboardEvent, PointerEvent } from 'react';
import styles from './Fader.module.css';

interface Props {
  label: string;
  value: number;
  onChange: (value: number) => void;
  orientation?: 'vertical' | 'horizontal';
  /** Value restored on double click. */
  defaultValue?: number;
  /** Draw the active part from the centre (crossfader) instead of from the start. */
  bipolar?: boolean;
  /** Text for screen readers, e.g. "-6 dB". */
  valueText?: string;
}

const KEY_STEP = 0.02;
const KEY_STEP_LARGE = 0.1;

/**
 * Fader with pointer, keyboard and double-click reset. While dragging, the local value wins
 * over what the engine reports, so the handle follows the pointer without lag.
 */
export function Fader({
  label,
  value,
  onChange,
  orientation = 'vertical',
  defaultValue,
  bipolar = false,
  valueText,
}: Props) {
  const track = useRef<HTMLDivElement>(null);
  const [dragValue, setDragValue] = useState<number | null>(null);
  const shown = dragValue ?? value;
  const vertical = orientation === 'vertical';

  const valueAt = (e: PointerEvent) => {
    const rect = track.current?.getBoundingClientRect();
    if (!rect) return shown;
    const t = vertical
      ? 1 - (e.clientY - rect.top) / rect.height
      : (e.clientX - rect.left) / rect.width;
    return Math.min(1, Math.max(0, t));
  };

  const set = (v: number) => {
    const clamped = Math.min(1, Math.max(0, v));
    onChange(clamped);
    return clamped;
  };

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    e.currentTarget.focus();
    setDragValue(set(valueAt(e)));
  };
  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (dragValue === null) return;
    setDragValue(set(valueAt(e)));
  };
  const onPointerUp = () => setDragValue(null);

  const onKeyDown = (e: KeyboardEvent) => {
    const step = e.shiftKey ? KEY_STEP_LARGE : KEY_STEP;
    const up = vertical ? 'ArrowUp' : 'ArrowRight';
    const down = vertical ? 'ArrowDown' : 'ArrowLeft';
    if (e.key === up) set(shown + step);
    else if (e.key === down) set(shown - step);
    else if (e.key === 'Home') set(0);
    else if (e.key === 'End') set(1);
    else return;
    e.preventDefault();
  };

  const start = bipolar ? Math.min(shown, 0.5) : 0;
  const end = bipolar ? Math.max(shown, 0.5) : shown;

  return (
    <div
      ref={track}
      className={styles.fader}
      data-orientation={orientation}
      data-dragging={dragValue !== null || undefined}
      role="slider"
      tabIndex={0}
      aria-label={label}
      aria-orientation={orientation}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(shown * 100)}
      aria-valuetext={valueText}
      style={
        {
          '--fader-value': shown,
          '--fader-start': start,
          '--fader-end': end,
        } as React.CSSProperties
      }
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onKeyDown={onKeyDown}
      onDoubleClick={() => defaultValue !== undefined && set(defaultValue)}
    >
      <div className={styles.rail}>
        <div className={styles.fill} />
      </div>
      <div className={styles.handle} />
    </div>
  );
}
