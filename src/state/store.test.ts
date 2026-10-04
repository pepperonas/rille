import { describe, expect, it } from 'vitest';
import {
  initialState,
  loadAccepted,
  loadFailed,
  loadRequested,
  loaded,
  unloaded,
} from './store';

describe('deck load lifecycle', () => {
  it('goes loading → ready', () => {
    let s = loadRequested(initialState, 'a', 'Track');
    s = loadAccepted(s, 'a', 7);
    s = loaded(s, { deck: 'a', trackId: 7, title: 'Track', durationSecs: 180 });
    expect(s.decks.a).toEqual({ status: 'ready', trackId: 7, title: 'Track', durationSecs: 180 });
    expect(s.decks.b.status).toBe('empty');
  });

  it('ignores a late result from an older request', () => {
    let s = loadRequested(initialState, 'a', 'Second');
    s = loadAccepted(s, 'a', 8);
    s = loaded(s, { deck: 'a', trackId: 7, title: 'First', durationSecs: 1 });
    expect(s.decks.a.status).toBe('loading');
  });

  it('shows decode errors', () => {
    let s = loadRequested(initialState, 'b', 'Broken');
    s = loadFailed(s, { deck: 'b', title: 'Broken', message: 'Datei ist beschädigt' });
    expect(s.decks.b).toEqual({ status: 'error', title: 'Broken', message: 'Datei ist beschädigt' });
  });

  it('a failure for a deck that is not loading changes nothing', () => {
    const s = loadFailed(initialState, { deck: 'a', title: 'x', message: 'y' });
    expect(s).toBe(initialState);
  });

  it('unload empties the deck', () => {
    let s = loaded(initialState, { deck: 'a', trackId: 1, title: 'T', durationSecs: 1 });
    s = unloaded(s, 'a');
    expect(s.decks.a.status).toBe('empty');
  });
});
