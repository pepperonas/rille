import { useEffect } from 'react';
import { Titlebar } from './shell/Titlebar';
import { NoAudioNotice } from './shell/NoAudioNotice';
import { DeckPanel } from './deck/DeckPanel';
import { MixerPanel } from './mixer/MixerPanel';
import { LibraryPanel } from './library/LibraryPanel';
import { connectBackend } from './state/connect';
import { useShortcuts } from './shell/useShortcuts';
import styles from './App.module.css';

export function App() {
  useShortcuts();
  useEffect(() => {
    let cleanup: (() => void) | undefined;
    let cancelled = false;
    connectBackend().then(
      (off) => (cancelled ? off() : (cleanup = off)),
      (e: unknown) => console.error('backend connection failed', e),
    );
    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, []);

  return (
    <div className={styles.app}>
      <Titlebar />
      <main className={styles.stage}>
        <NoAudioNotice />
        <DeckPanel deck="a" />
        <MixerPanel />
        <DeckPanel deck="b" />
      </main>
      <LibraryPanel />
    </div>
  );
}
