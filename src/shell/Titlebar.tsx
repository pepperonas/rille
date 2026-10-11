import { useEffect, useRef, useState } from 'react';
import { MidiMonitor } from '../dev/MidiMonitor';
import { SettingsDialog } from '../settings/SettingsDialog';
import { useAppState } from '../state/store';
import { AboutDialog } from './AboutDialog';
import { registerDialog } from './dialogs';
import { ShortcutsDialog } from './ShortcutsDialog';
import styles from './Titlebar.module.css';

/** Title bar overlay: the native traffic lights sit on top-left, the rest is a drag region. */
export function Titlebar() {
  const about = useRef<HTMLDialogElement>(null);
  const settings = useRef<HTMLDialogElement>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const monitor = useRef<HTMLDialogElement>(null);
  const shortcuts = useRef<HTMLDialogElement>(null);
  const [monitorOpen, setMonitorOpen] = useState(false);
  const audio = useAppState((s) => s.audio);
  const controller = useAppState((s) => s.controller);
  const openSettings = () => {
    if (!settings.current?.open) settings.current?.showModal();
    setSettingsOpen(true);
  };
  const openMonitor = () => {
    settings.current?.close();
    if (!monitor.current?.open) monitor.current?.showModal();
    setMonitorOpen(true);
  };

  const openShortcuts = () => {
    if (!shortcuts.current?.open) shortcuts.current?.showModal();
  };

  // Shortcuts and empty states open these dialogs by name.
  useEffect(() => {
    const offs = [
      registerDialog('settings', openSettings),
      registerDialog('monitor', openMonitor),
      registerDialog('shortcuts', openShortcuts),
    ];
    return () => offs.forEach((off) => off());
  }, []);

  return (
    <header className={styles.bar} data-tauri-drag-region>
      <span className={styles.wordmark} data-tauri-drag-region>
        rille
      </span>
      <button
        type="button"
        className={styles.status}
        data-ok={audio?.connected || undefined}
        onClick={openSettings}
        title={audio?.error ?? undefined}
      >
        <span className={styles.dot} aria-hidden="true" />
        {audio === null
          ? 'Audio startet …'
          : audio.connected
            ? `${audio.deviceName ?? 'Audio'} · ${audio.latencyMs.toFixed(1)} ms`
            : 'Kein Audio-Gerät'}
      </button>
      <button
        type="button"
        className={styles.status}
        data-ok={controller?.connected || undefined}
        data-neutral
        onClick={openSettings}
      >
        <span className={styles.dot} aria-hidden="true" />
        {controller?.connected ? (controller.name ?? 'DDJ-200') : 'Kein Controller'}
      </button>
      <button
        type="button"
        className={styles.iconButton}
        aria-label="Tastenkürzel"
        title="Tastenkürzel (?)"
        onClick={openShortcuts}
      >
        <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
          <path
            fill="currentColor"
            d="M4 5h16a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2Zm0 2v10h16V7H4Zm2 2h2v2H6V9Zm3 0h2v2H9V9Zm3 0h2v2h-2V9Zm3 0h3v2h-3V9ZM6 12h2v2H6v-2Zm3 0h6v2H9v-2Zm7 0h2v2h-2v-2Z"
          />
        </svg>
      </button>
      <button
        type="button"
        className={styles.iconButton}
        aria-label="Einstellungen"
        onClick={openSettings}
      >
        <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
          <path
            fill="currentColor"
            d="M19.4 13a7.7 7.7 0 0 0 0-2l2.1-1.6-2-3.5-2.5 1a7.4 7.4 0 0 0-1.7-1L15 3h-4l-.4 2.7a7.4 7.4 0 0 0-1.7 1l-2.5-1-2 3.5L6.6 11a7.7 7.7 0 0 0 0 2l-2.1 1.6 2 3.5 2.5-1c.5.4 1.1.7 1.7 1L11 21h4l.4-2.7c.6-.3 1.2-.6 1.7-1l2.5 1 2-3.5-2.2-1.8ZM13 15.5a3.5 3.5 0 1 1 0-7 3.5 3.5 0 0 1 0 7Z"
          />
        </svg>
      </button>
      <button
        type="button"
        className={styles.iconButton}
        aria-label="Über rille"
        onClick={() => about.current?.showModal()}
      >
        <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
          <path
            fill="currentColor"
            d="M11 17h2v-6h-2v6Zm1-8a1 1 0 1 0 0-2 1 1 0 0 0 0 2Zm0 13a10 10 0 1 1 0-20 10 10 0 0 1 0 20Z"
          />
        </svg>
      </button>
      <AboutDialog ref={about} />
      <ShortcutsDialog ref={shortcuts} />
      <SettingsDialog
        ref={settings}
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        onOpenMonitor={openMonitor}
      />
      <MidiMonitor ref={monitor} open={monitorOpen} onClose={() => setMonitorOpen(false)} />
    </header>
  );
}
