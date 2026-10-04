import { backend } from '../ipc';
import { Switch } from '../components/Switch';
import { useAppState } from '../state/store';
import styles from './AudioSettings.module.css';

export function ControllerSettings({ onOpenMonitor }: { onOpenMonitor: () => void }) {
  const controller = useAppState((s) => s.controller);
  return (
    <div className={styles.settings}>
      <p className={styles.status}>
        {controller?.connected
          ? `Verbunden: ${controller.name ?? 'DDJ-200'}`
          : 'Kein DDJ-200 verbunden. Per USB anschließen – rille erkennt ihn automatisch.'}
      </p>
      <Switch
        label="Vinyl-Modus"
        description="Jog-Teller berühren und drehen scratcht. Aus: der Teller biegt nur das Tempo."
        checked={controller?.vinylMode ?? true}
        onChange={(on) => void backend.setVinylMode(on)}
      />
      <button type="button" className={styles.link} onClick={onOpenMonitor}>
        MIDI-Monitor öffnen (⌘⌥M)
      </button>
    </div>
  );
}
