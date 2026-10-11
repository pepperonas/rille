import { useEffect, useState, type RefObject } from 'react';

export interface CanvasSize {
  /** CSS pixels. */
  w: number;
  h: number;
  dpr: number;
}

/** Size of `ref` in CSS pixels plus the device pixel ratio. Follows resizes and moves between
 *  screens with different pixel density (a Retina laptop and an external monitor), which a
 *  ResizeObserver alone does not report. */
export function useCanvasSize(ref: RefObject<HTMLElement | null>): CanvasSize {
  const [size, setSize] = useState<CanvasSize>({ w: 0, h: 0, dpr: 1 });

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    let media: MediaQueryList | null = null;
    const measure = () => {
      const dpr = window.devicePixelRatio || 1;
      setSize((s) =>
        s.w === el.clientWidth && s.h === el.clientHeight && s.dpr === dpr
          ? s
          : { w: el.clientWidth, h: el.clientHeight, dpr },
      );
      // A density query matches only the current ratio; renew it after every change.
      media?.removeEventListener('change', measure);
      media = window.matchMedia(`(resolution: ${dpr}dppx)`);
      media.addEventListener('change', measure);
    };
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    measure();
    return () => {
      observer.disconnect();
      media?.removeEventListener('change', measure);
    };
  }, [ref]);

  return size;
}
