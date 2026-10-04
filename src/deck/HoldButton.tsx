import { useEffect, useRef } from 'react';
import type { KeyboardEvent, ReactNode } from 'react';

interface Props {
  className: string | undefined;
  label: string;
  /** Called with `true` on press and `false` on release, exactly once each. */
  onHold: (held: boolean) => void;
  active?: boolean;
  children: ReactNode;
}

/**
 * Button that acts while held (bend, reverse): primary pointer button, Enter/Space, and
 * released on pointer cancel, lost capture, blur or unmount so it can never stick.
 */
export function HoldButton({ className, label, onHold, active, children }: Props) {
  const held = useRef(false);
  // The latest callback, for the release on unmount.
  const onHoldRef = useRef(onHold);
  useEffect(() => {
    onHoldRef.current = onHold;
  });
  useEffect(
    () => () => {
      if (held.current) {
        held.current = false;
        onHoldRef.current(false);
      }
    },
    [],
  );
  const press = () => {
    if (held.current) return;
    held.current = true;
    onHold(true);
  };
  const release = () => {
    if (!held.current) return;
    held.current = false;
    onHold(false);
  };
  const isActivation = (e: KeyboardEvent) => e.key === 'Enter' || e.key === ' ';

  return (
    <button
      type="button"
      className={className}
      aria-label={label}
      aria-pressed={active ?? false}
      data-active={active || undefined}
      onPointerDown={(e) => {
        if (e.button !== 0) return; // a right click opens no bend or reverse
        e.currentTarget.setPointerCapture(e.pointerId);
        press();
      }}
      onPointerUp={release}
      onPointerCancel={release}
      onLostPointerCapture={release}
      onKeyDown={(e) => {
        if (!isActivation(e)) return;
        e.preventDefault();
        if (!e.repeat) press();
      }}
      onKeyUp={(e) => isActivation(e) && release()}
      onBlur={release}
    >
      {children}
    </button>
  );
}
