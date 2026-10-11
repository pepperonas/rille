// Waveform colours come from CSS tokens (`--wave-low/-mid/-high`, defined per deck). Canvas and
// WebGL need numbers: the browser resolves the token, a 1×1 canvas turns any colour syntax
// (`color-mix`, `oklch`, `color(srgb …)`) into RGB. Never parse colour strings by hand.

export type Rgb = [number, number, number];

export interface WaveColors {
  low: Rgb;
  mid: Rgb;
  high: Rgb;
  grid: Rgb;
}

let probe: CanvasRenderingContext2D | null | undefined;

function toRgb(css: string): Rgb {
  if (probe === undefined) {
    probe = document.createElement('canvas').getContext('2d', { willReadFrequently: true });
  }
  if (!probe) return [128, 128, 128];
  probe.clearRect(0, 0, 1, 1);
  probe.fillStyle = '#000';
  probe.fillStyle = css;
  probe.fillRect(0, 0, 1, 1);
  const [r = 0, g = 0, b = 0] = probe.getImageData(0, 0, 1, 1).data;
  return [r, g, b];
}

/** Resolve the colour tokens as seen from `el` (inside a deck, so the deck accent applies). */
export function waveColors(el: HTMLElement): WaveColors {
  const span = document.createElement('span');
  span.style.display = 'none';
  el.appendChild(span);
  const read = (token: string) => {
    span.style.color = `var(${token})`;
    return toRgb(getComputedStyle(span).color);
  };
  const colors = {
    low: read('--wave-low'),
    mid: read('--wave-mid'),
    high: read('--wave-high'),
    grid: read('--wave-grid'),
  };
  span.remove();
  return colors;
}

export const css = ([r, g, b]: Rgb, alpha = 1) => `rgb(${r} ${g} ${b} / ${alpha})`;
