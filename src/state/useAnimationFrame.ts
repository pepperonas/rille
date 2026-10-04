import { useEffect, useRef } from 'react';

/**
 * Run `draw` on every display frame while `active`. When inactive it draws once after each
 * render (so state changes still show) and otherwise costs nothing.
 */
export function useAnimationFrame(active: boolean, draw: (now: number) => void) {
  const drawRef = useRef(draw);
  useEffect(() => {
    drawRef.current = draw;
    if (!active) draw(performance.now());
  });
  useEffect(() => {
    if (!active) return;
    let id = requestAnimationFrame(function loop(now) {
      drawRef.current(now);
      id = requestAnimationFrame(loop);
    });
    return () => cancelAnimationFrame(id);
  }, [active]);
}
