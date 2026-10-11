import { backend } from '../ipc';
import type { Deck, PadMode } from '../ipc/types';
import { store } from '../state/store';
import { JUMP_BEATS, LOOP_BEATS, PAD_MODES } from './beat';

/** Pad mode of a deck: shown in the app and sent to the controller. */
export function setPadMode(deck: Deck, mode: PadMode) {
  store.update((s) =>
    s.padMode[deck] === mode ? s : { ...s, padMode: { ...s.padMode, [deck]: mode } },
  );
  backend.setPadMode(deck, mode).catch((e: unknown) => console.error('pad mode', e));
}

/** The pad mode after `mode` (Hot Cue → Loop → Jump → Hot Cue). */
export function nextPadMode(mode: PadMode): PadMode {
  const i = PAD_MODES.findIndex((m) => m.mode === mode);
  return PAD_MODES[(i + 1) % PAD_MODES.length]?.mode ?? 'hotCue';
}

/** A pad pressed in the app (click or key). Shift deletes the cue or leaves the loop. */
export function pressPad(deck: Deck, index: number, shift: boolean) {
  const mode = store.get().padMode[deck];
  if (mode === 'hotCue') {
    backend.deck(
      deck,
      shift ? { type: 'hotCueClear', slot: index } : { type: 'hotCue', slot: index },
    );
  } else if (mode === 'beatLoop') {
    const beats = LOOP_BEATS[index] ?? 1;
    backend.deck(deck, shift ? { type: 'loopExit' } : { type: 'beatLoop', beats });
  } else {
    backend.deck(deck, { type: 'beatJump', beats: JUMP_BEATS[index] ?? 1 });
  }
}
