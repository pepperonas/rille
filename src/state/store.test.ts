import { describe, expect, it } from 'vitest';
import {
  initialState,
  loadAccepted,
  loadFailed,
  loadRequested,
  loadWarning,
  loaded,
  unloaded,
} from './store';

describe('deck load lifecycle', () => {
  it('goes loading → ready', () => {
    let s = loadRequested(initialState, 'a', 'Track');
    s = loadAccepted(s, 'a', 7);
    s = loaded(s, {
      deck: 'a',
      trackId: 7,
      title: 'Track',
      artist: 'DJ',
      bpm: 128,
      durationSecs: 180,
    });
    expect(s.decks.a).toEqual({
      status: 'ready',
      trackId: 7,
      title: 'Track',
      artist: 'DJ',
      bpm: 128,
      durationSecs: 180,
      warning: null,
    });
    expect(s.decks.b.status).toBe('empty');
  });

  it('ignores a late result from an older request', () => {
    let s = loadRequested(initialState, 'a', 'Second');
    s = loadAccepted(s, 'a', 8);
    s = loaded(s, {
      deck: 'a',
      trackId: 7,
      title: 'First',
      artist: null,
      bpm: null,
      durationSecs: 1,
    });
    expect(s.decks.a.status).toBe('loading');
  });

  it('shows decode errors', () => {
    let s = loadRequested(initialState, 'b', 'Broken');
    s = loadFailed(s, {
      deck: 'b',
      trackId: null,
      title: 'Broken',
      message: 'Datei ist beschädigt',
    });
    expect(s.decks.b).toEqual({
      status: 'error',
      title: 'Broken',
      message: 'Datei ist beschädigt',
    });
  });

  it('a failure for a deck that is not loading changes nothing', () => {
    const s = loadFailed(initialState, { deck: 'a', trackId: 1, title: 'x', message: 'y' });
    expect(s).toBe(initialState);
  });

  it('a result arriving after eject is ignored', () => {
    let s = loadRequested(initialState, 'a', 'Track');
    s = loadAccepted(s, 'a', 3);
    s = unloaded(s, 'a');
    s = loaded(s, {
      deck: 'a',
      trackId: 3,
      title: 'Track',
      artist: null,
      bpm: null,
      durationSecs: 1,
    });
    expect(s.decks.a.status).toBe('empty');
  });

  it('a failure of an older request does not replace a newer load', () => {
    let s = loadRequested(initialState, 'a', 'New');
    s = loadAccepted(s, 'a', 9);
    s = loadFailed(s, { deck: 'a', trackId: 8, title: 'Old', message: 'kaputt' });
    expect(s.decks.a.status).toBe('loading');
  });

  it('unload empties the deck', () => {
    let s = loaded(initialState, {
      deck: 'a',
      trackId: 1,
      title: 'T',
      artist: null,
      bpm: null,
      durationSecs: 1,
    });
    s = unloaded(s, 'a');
    expect(s.decks.a.status).toBe('empty');
  });

  it('a late acceptance of an older request does not claim the newer one', () => {
    let s = loadRequested(initialState, 'a', 'First', 1);
    s = loadRequested(s, 'a', 'Second', 2);
    s = loadAccepted(s, 'a', 10, 1); // answer to the first request, arriving late
    s = loaded(s, {
      deck: 'a',
      trackId: 11,
      title: 'Second',
      artist: null,
      bpm: null,
      durationSecs: 1,
    });
    expect(s.decks.a).toMatchObject({ status: 'ready', trackId: 11 });
  });

  it('shows a decode warning on the track it belongs to', () => {
    let s = loadRequested(initialState, 'a', 'T', 1);
    s = loadAccepted(s, 'a', 5, 1);
    s = loaded(s, { deck: 'a', trackId: 5, title: 'T', artist: null, bpm: null, durationSecs: 9 });
    const stale = loadWarning(s, { deck: 'a', trackId: 4, title: 'Old', message: 'x' });
    expect(stale).toBe(s);
    s = loadWarning(s, { deck: 'a', trackId: 5, title: 'T', message: 'abgeschnitten' });
    expect(s.decks.a).toMatchObject({ status: 'ready', warning: 'abgeschnitten' });
  });
});
