import { readFileSync } from 'node:fs';
import { describe, expect, it, vi } from 'vitest';

vi.mock('../ipc', () => ({ backend: { deck: vi.fn(), mixer: vi.fn(), setPadMode: vi.fn() } }));

const { blockedTarget, findBinding, SHORTCUTS } = await import('./shortcuts');
const { backend } = await import('../ipc');

const press = (code: string, extra: Partial<KeyboardEvent> = {}) => ({
  code,
  key: extra.key ?? code,
  shiftKey: extra.shiftKey ?? false,
  metaKey: extra.metaKey ?? false,
  ctrlKey: extra.ctrlKey ?? false,
  altKey: extra.altKey ?? false,
});

describe('shortcut table', () => {
  it('binds no key twice with the same modifiers', () => {
    const seen = new Set<string>();
    for (const row of SHORTCUTS) {
      for (const b of row.bindings) {
        for (const shift of b.shift === undefined ? [false, true] : [b.shift]) {
          const id = `${b.code ?? b.key}|${shift}|${!!b.meta}|${!!b.alt}`;
          expect(seen.has(id), id).toBe(false);
          seen.add(id);
        }
      }
    }
  });

  it('avoids Y and Z, which swap between QWERTZ and QWERTY', () => {
    const codes = SHORTCUTS.flatMap((r) => r.bindings.map((b) => b.code));
    expect(codes).not.toContain('KeyY');
    expect(codes).not.toContain('KeyZ');
  });

  it('lists every row in both READMEs', () => {
    for (const file of ['README.md', 'README.de.md']) {
      const text = readFileSync(file, 'utf8');
      for (const row of SHORTCUTS) {
        expect(text, `${file}: ${row.keys}`).toContain(`\`${row.keys}\``);
      }
    }
  });
});

describe('findBinding', () => {
  it('matches the physical key', () => {
    expect(findBinding(SHORTCUTS, press('KeyF'))?.row.description).toBe('Play / Pause');
  });

  it('keeps ⌘F apart from F', () => {
    expect(findBinding(SHORTCUTS, press('KeyF', { metaKey: true }))?.row.keys).toBe('⌘F');
  });

  it('ignores Ctrl and stray Alt', () => {
    expect(findBinding(SHORTCUTS, press('KeyF', { ctrlKey: true }))).toBeNull();
    expect(findBinding(SHORTCUTS, press('KeyF', { altKey: true }))).toBeNull();
  });

  it('respects shift-only and shift-less bindings', () => {
    expect(findBinding(SHORTCUTS, press('KeyF', { shiftKey: true }))).toBeNull();
    expect(findBinding(SHORTCUTS, press('KeyS', { shiftKey: true }))?.row.keys).toBe('S / ⇧S');
  });

  it('finds ? by character on any layout', () => {
    // German: Shift + ß (code Minus); US: Shift + / (code Slash).
    expect(findBinding(SHORTCUTS, press('Minus', { key: '?', shiftKey: true }))?.row.keys).toBe('?');
    expect(findBinding(SHORTCUTS, press('Slash', { key: '?', shiftKey: true }))?.row.keys).toBe('?');
  });

  it('runs the deck action with shift passed through', () => {
    findBinding(SHORTCUTS, press('KeyS'))?.binding.run(false);
    expect(backend.deck).toHaveBeenLastCalledWith('a', { type: 'sync' });
    findBinding(SHORTCUTS, press('KeyL'))?.binding.run(false);
    expect(backend.deck).toHaveBeenLastCalledWith('b', { type: 'sync' });
    findBinding(SHORTCUTS, press('Digit3'))?.binding.run(true);
    expect(backend.deck).toHaveBeenLastCalledWith('a', { type: 'hotCueClear', slot: 2 });
  });

  it('gives held keys a release', () => {
    const cue = findBinding(SHORTCUTS, press('KeyK'));
    cue?.binding.run(false);
    expect(backend.deck).toHaveBeenLastCalledWith('b', { type: 'cuePress' });
    cue?.binding.release?.();
    expect(backend.deck).toHaveBeenLastCalledWith('b', { type: 'cueRelease' });
  });

  it('clamps the crossfader', () => {
    findBinding(SHORTCUTS, press('KeyV', { shiftKey: true }))?.binding.run(true);
    expect(backend.mixer).toHaveBeenLastCalledWith({ type: 'crossfader', value: 0 });
  });
});

describe('blockedTarget', () => {
  it('stays quiet while typing, but not on switches or sliders', () => {
    const text = document.createElement('input');
    const box = document.createElement('input');
    box.type = 'checkbox';
    const button = document.createElement('button');
    expect(blockedTarget(text, false)).toBe(true);
    expect(blockedTarget(text, true)).toBe(false);
    expect(blockedTarget(box, false)).toBe(false);
    expect(blockedTarget(button, false)).toBe(false);
    expect(blockedTarget(document.createElement('textarea'), false)).toBe(true);
  });
});
