import { store } from '../state/store';
import { clampZoom } from './peaks';

/** Set the detail waveform zoom step (shared by both decks). */
export function setZoom(z: number) {
  store.update((s) => {
    const waveZoom = clampZoom(z);
    return waveZoom === s.waveZoom ? s : { ...s, waveZoom };
  });
}
