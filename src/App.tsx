import { Titlebar } from './shell/Titlebar';
import { DeckPanel } from './deck/DeckPanel';
import { MixerPanel } from './mixer/MixerPanel';
import { LibraryPanel } from './library/LibraryPanel';
import styles from './App.module.css';

export function App() {
  return (
    <div className={styles.app}>
      <Titlebar />
      <main className={styles.stage}>
        <DeckPanel deck="a" />
        <MixerPanel />
        <DeckPanel deck="b" />
      </main>
      <LibraryPanel />
    </div>
  );
}
