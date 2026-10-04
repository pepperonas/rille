import { act, renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { useOptimistic } from './useOptimistic';

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

  it('toggles booleans reliably on double clicks', () => {
    const { result } = renderHook(({ v }) => useOptimistic(v), { initialProps: { v: false } });
    act(() => result.current.set(!result.current.current()));
    act(() => result.current.set(!result.current.current()));
    expect(result.current.shown).toBe(false);
  });
});
