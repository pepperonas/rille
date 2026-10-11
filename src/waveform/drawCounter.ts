// Counts waveform draws so a browser test can prove nothing renders while idle. Only in
// development builds; production does not touch `window`.

declare global {
  interface Window {
    __rilleWaveDraws?: number;
  }
}

export function countDraw() {
  if (import.meta.env.DEV) window.__rilleWaveDraws = (window.__rilleWaveDraws ?? 0) + 1;
}
