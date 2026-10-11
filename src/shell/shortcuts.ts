// Keyboard shortcuts: one table drives the global handler and the `?` overview.
// Keys are matched by `KeyboardEvent.code` (the physical key), so QWERTZ and QWERTY share the
// layout; Y and Z are avoided because they swap places between the two.
import { backend } from '../ipc';
import type { Deck } from '../ipc/types';
import { nextPadMode, pressPad, setPadMode } from '../deck/padMode';
import { store } from '../state/store';
import { openDialog } from './dialogs';

export type ShortcutGroup = 'Deck 1' | 'Deck 2' | 'Mixer' | 'Allgemein';

export interface Binding {
  /** `KeyboardEvent.code`, or `key` for characters that move between layouts (`?`). */
  code?: string;
  key?: string;
  /** true: only with Shift, false: only without, undefined: either (passed to `run`). */
  shift?: boolean;
  /** ⌘ required (and then allowed in text fields). */
  meta?: boolean;
  alt?: boolean;
  /** Also works while a modal dialog is open. */
  inDialogs?: boolean;
  run: (shift: boolean) => void;
  /** Held keys: called on key up, or when the window loses focus. */
  release?: () => void;
}

export interface ShortcutRow {
  group: ShortcutGroup;
  /** As printed in the overview and the README, e.g. `F`, `⇧S`, `1 2 3 4 Q W E R`. */
  keys: string;
  description: string;
  bindings: Binding[];
}


const CROSSFADER_STEP = 0.1;

function nudgeCrossfader(direction: -1 | 1, all: boolean) {
  const current = store.get().frame?.crossfader ?? 0.5;
  const value = all ? (direction < 0 ? 0 : 1) : current + direction * CROSSFADER_STEP;
  backend.mixer({ type: 'crossfader', value: Math.min(1, Math.max(0, value)) });
}

function focus(id: string) {
  const el = document.getElementById(id);
  el?.focus();
  if (el instanceof HTMLInputElement) el.select();
}

interface DeckKeys {
  play: string;
  cue: string;
  sync: string;
  sync_label: string;
  phones: string;
  phones_label: string;
  pads: readonly string[];
  pads_label: string;
  mode: string;
  mode_label: string;
  bendDown: string;
  bendUp: string;
  bend_label: string;
}

function deckRows(deck: Deck, k: DeckKeys): ShortcutRow[] {
  const group: ShortcutGroup = deck === 'a' ? 'Deck 1' : 'Deck 2';
  return [
    {
      group,
      keys: k.play.replace('Key', ''),
      description: 'Play / Pause',
      bindings: [{ code: k.play, shift: false, run: () => backend.deck(deck, { type: 'playPause' }) }],
    },
    {
      group,
      keys: k.cue.replace('Key', ''),
      description: 'Cue (halten = Vorhören)',
      bindings: [
        {
          code: k.cue,
          shift: false,
          run: () => backend.deck(deck, { type: 'cuePress' }),
          release: () => backend.deck(deck, { type: 'cueRelease' }),
        },
      ],
    },
    {
      group,
      keys: k.sync_label,
      description: 'Sync · mit ⇧ Sync-Lock an/aus',
      bindings: [
        {
          code: k.sync,
          run: (shift) =>
            backend.deck(deck, shift ? { type: 'toggleSyncLock' } : { type: 'sync' }),
        },
      ],
    },
    {
      group,
      keys: k.phones_label,
      description: 'Kopfhörer-Cue an/aus',
      bindings: [
        {
          code: k.phones,
          shift: false,
          run: () => backend.mixer({ type: 'toggleHeadphoneCue', deck }),
        },
      ],
    },
    {
      group,
      keys: k.pads_label,
      description: 'Pads 1–8 im gewählten Modus · mit ⇧ löschen bzw. Loop verlassen',
      bindings: k.pads.map((code, index) => ({
        code,
        run: (shift: boolean) => pressPad(deck, index, shift),
      })),
    },
    {
      group,
      keys: k.mode_label,
      description: 'Pad-Modus wechseln (Hot Cue → Loop → Jump)',
      bindings: [
        {
          code: k.mode,
          shift: false,
          run: () => setPadMode(deck, nextPadMode(store.get().padMode[deck])),
        },
      ],
    },
    {
      group,
      keys: k.bend_label,
      description: 'Pitch-Bend langsamer / schneller (halten)',
      bindings: [
        {
          code: k.bendDown,
          shift: false,
          run: () => backend.deck(deck, { type: 'bend', direction: -1 }),
          release: () => backend.deck(deck, { type: 'bend', direction: 0 }),
        },
        {
          code: k.bendUp,
          shift: false,
          run: () => backend.deck(deck, { type: 'bend', direction: 1 }),
          release: () => backend.deck(deck, { type: 'bend', direction: 0 }),
        },
      ],
    },
  ];
}

export const SHORTCUTS: readonly ShortcutRow[] = [
  ...deckRows('a', {
    play: 'KeyF',
    cue: 'KeyD',
    sync: 'KeyS',
    sync_label: 'S / ⇧S',
    phones: 'KeyG',
    phones_label: 'G',
    pads: ['Digit1', 'Digit2', 'Digit3', 'Digit4', 'KeyQ', 'KeyW', 'KeyE', 'KeyR'],
    pads_label: '1 2 3 4 Q W E R',
    mode: 'Digit5',
    mode_label: '5',
    bendDown: 'KeyX',
    bendUp: 'KeyC',
    bend_label: 'X / C',
  }),
  ...deckRows('b', {
    play: 'KeyJ',
    cue: 'KeyK',
    sync: 'KeyL',
    sync_label: 'L / ⇧L',
    phones: 'KeyH',
    phones_label: 'H',
    pads: ['Digit7', 'Digit8', 'Digit9', 'Digit0', 'KeyU', 'KeyI', 'KeyO', 'KeyP'],
    pads_label: '7 8 9 0 U I O P',
    mode: 'Digit6',
    mode_label: '6',
    bendDown: 'KeyM',
    bendUp: 'Comma',
    bend_label: 'M / ,',
  }),
  {
    group: 'Mixer',
    keys: 'V / N',
    description: 'Crossfader nach links / rechts · mit ⇧ ganz',
    bindings: [
      { code: 'KeyV', run: (shift) => nudgeCrossfader(-1, shift) },
      { code: 'KeyN', run: (shift) => nudgeCrossfader(1, shift) },
    ],
  },
  {
    group: 'Mixer',
    keys: 'B',
    description: 'Crossfader in die Mitte',
    bindings: [
      {
        code: 'KeyB',
        shift: false,
        run: () => backend.mixer({ type: 'crossfader', value: 0.5 }),
      },
    ],
  },
  {
    group: 'Allgemein',
    keys: '?',
    description: 'Diese Übersicht',
    bindings: [{ key: '?', run: () => openDialog('shortcuts') }],
  },
  {
    group: 'Allgemein',
    keys: '⌘F',
    description: 'Library durchsuchen',
    bindings: [{ code: 'KeyF', meta: true, run: () => focus('library-search') }],
  },
  {
    group: 'Allgemein',
    keys: '⌘L',
    description: 'Trackliste (dort Enter, 1 / 2 = laden)',
    bindings: [{ code: 'KeyL', meta: true, run: () => focus('library-list') }],
  },
  {
    group: 'Allgemein',
    keys: '⌘,',
    description: 'Einstellungen',
    bindings: [{ code: 'Comma', meta: true, run: () => openDialog('settings') }],
  },
  {
    group: 'Allgemein',
    keys: '⌘⌥M',
    description: 'MIDI-Monitor',
    bindings: [{ code: 'KeyM', meta: true, alt: true, inDialogs: true, run: () => openDialog('monitor') }],
  },
];

export interface KeyLike {
  code: string;
  key: string;
  shiftKey: boolean;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
}

/** The binding a key press triggers, if any. */
export function findBinding(
  rows: readonly ShortcutRow[],
  e: KeyLike,
): { binding: Binding; row: ShortcutRow } | null {
  for (const row of rows) {
    for (const binding of row.bindings) {
      const keyMatches = binding.key !== undefined ? e.key === binding.key : e.code === binding.code;
      if (!keyMatches) continue;
      if (!!binding.meta !== e.metaKey || !!binding.alt !== e.altKey || e.ctrlKey) continue;
      // A character binding (`?`) needs Shift on most layouts; that is part of the character.
      if (binding.key === undefined && binding.shift !== undefined && binding.shift !== e.shiftKey) {
        continue;
      }
      return { binding, row };
    }
  }
  return null;
}

/** Where a shortcut must stay quiet: typing, or something already took the key. */
export function blockedTarget(target: EventTarget | null, meta: boolean): boolean {
  if (meta) return false;
  if (!(target instanceof Element)) return false;
  if (target.closest('input, textarea, select, [contenteditable="true"]')) {
    // Checkboxes and range inputs don't take letters; text-like inputs do.
    const input = target.closest('input');
    return !(input && (input.type === 'checkbox' || input.type === 'range'));
  }
  return false;
}
