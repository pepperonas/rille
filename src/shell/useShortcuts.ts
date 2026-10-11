import { useEffect } from 'react';
import { blockedTarget, findBinding, SHORTCUTS } from './shortcuts';

/** The global keyboard handler. Held keys (cue, bend) let go on key up or when the window
 *  loses focus, so nothing stays pressed after ⌘Tab. */
export function useShortcuts() {
  useEffect(() => {
    const held = new Map<string, () => void>();
    const releaseAll = () => {
      const releases = [...held.values()];
      held.clear();
      releases.forEach((r) => r());
    };
    const onDown = (e: KeyboardEvent) => {
      if (e.defaultPrevented) return;
      const hit = findBinding(SHORTCUTS, e);
      if (!hit) return;
      const { binding } = hit;
      if (blockedTarget(e.target, !!binding.meta)) return;
      if (!binding.inDialogs && document.querySelector('dialog[open]')) return;
      e.preventDefault();
      if (e.repeat || held.has(e.code)) return;
      binding.run(e.shiftKey);
      if (binding.release) held.set(e.code, binding.release);
    };
    const onUp = (e: KeyboardEvent) => {
      const release = held.get(e.code);
      if (!release) return;
      held.delete(e.code);
      release();
    };
    const onVisibility = () => {
      if (document.visibilityState === 'hidden') releaseAll();
    };
    window.addEventListener('keydown', onDown);
    window.addEventListener('keyup', onUp);
    window.addEventListener('blur', releaseAll);
    document.addEventListener('visibilitychange', onVisibility);
    return () => {
      releaseAll();
      window.removeEventListener('keydown', onDown);
      window.removeEventListener('keyup', onUp);
      window.removeEventListener('blur', releaseAll);
      document.removeEventListener('visibilitychange', onVisibility);
    };
  }, []);
}
