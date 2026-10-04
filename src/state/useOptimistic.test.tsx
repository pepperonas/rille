import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PENDING_TIMEOUT_MS, useOptimistic } from './useOptimistic';

describe('useOptimistic', () => {
  it('accumulates steps before the engine confirms', () => {
    const { result } = renderHook(({ v }) => useOptimistic(v), { initialProps: { v: 0.5 } });
    act(() => result.current.set(result.current.current() + 0.02));
    act(() => result.current.set(result.current.current() + 0.02));
    expect(result.current.shown).toBeCloseTo(0.54);
  });

  it('hands back to the engine value once it is confirmed', () => {
    const { result, rerender } = renderHook(({ v }) => useOptimistic(v), {
      initialProps: { v: 0.5 },
    });
    act(() => result.current.set(0.7));
    rerender({ v: 0.7 });
    rerender({ v: 0.3 }); // later change from elsewhere (controller) shows through
    expect(result.current.shown).toBe(0.3);
  });

  describe('when the engine never echoes the value', () => {
    beforeEach(() => vi.useFakeTimers());
    afterEach(() => vi.useRealTimers());

    it('gives up waiting and shows the engine again', () => {
      const { result } = renderHook(({ v }) => useOptimistic(v), { initialProps: { v: 0.5 } });
      act(() => result.current.set(0.7));
      act(() => void vi.advanceTimersByTime(PENDING_TIMEOUT_MS + 1));
      expect(result.current.shown).toBe(0.5);
      expect(result.current.current()).toBe(0.5);
    });

    it('lets a later controller change through after a double click', () => {
      // Two quick toggles end where they started, so the engine value never changes and
      // would never confirm them.
      const { result, rerender } = renderHook(({ v }) => useOptimistic(v), {
        initialProps: { v: false },
      });
      act(() => result.current.set(true));
      act(() => result.current.set(false));
      act(() => void vi.advanceTimersByTime(PENDING_TIMEOUT_MS + 1));
      rerender({ v: true }); // controller switches it on
      expect(result.current.shown).toBe(true);
    });

    it('keeps waiting while steps keep coming', () => {
      const { result } = renderHook(({ v }) => useOptimistic(v), { initialProps: { v: 0.5 } });
      for (let i = 0; i < 5; i++) {
        act(() => result.current.set(result.current.current() + 0.02));
        act(() => void vi.advanceTimersByTime(PENDING_TIMEOUT_MS / 2));
      }
      expect(result.current.shown).toBeCloseTo(0.6);
    });
  });

  it('toggles booleans reliably on double clicks', () => {
    const { result } = renderHook(({ v }) => useOptimistic(v), { initialProps: { v: false } });
    act(() => result.current.set(!result.current.current()));
    act(() => result.current.set(!result.current.current()));
    expect(result.current.shown).toBe(false);
  });
});
