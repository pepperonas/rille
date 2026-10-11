import { backend } from '../ipc';
import type { Deck, PadMode } from '../ipc/types';
import { store } from '../state/store';

/** Pad mode of a deck: shown in the app and sent to the controller. */
export function setPadMode(deck: Deck, mode: PadMode) {
  store.update((s) =>
    s.padMode[deck] === mode ? s : { ...s, padMode: { ...s.padMode, [deck]: mode } },
  );
  backend.setPadMode(deck, mode).catch((e: unknown) => console.error('pad mode', e));
}
