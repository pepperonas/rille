import { useEffect, useState } from 'react';
import { backend } from '../ipc';
import type { AudioDevice } from '../ipc/types';
import { useAppState } from '../state/store';
import styles from './AudioSettings.module.css';

const BUFFER_SIZES = [64, 128, 256, 512, 1024, 2048] as const;

export function AudioSettings({ open }: { open: boolean }) {
  const status = useAppState((s) => s.audio);
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    backend.audioDevices().then(setDevices, (e: unknown) => setError(String(e)));
  }, [open]);

  const apply = async (deviceId: string | null, buffer: number) => {
    setBusy(true);
    setError(null);
    try {
      await backend.audioOpen(deviceId, buffer);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const currentDevice = status?.deviceId ?? '';
  const currentBuffer = status?.requestedBuffer ?? 256;

  return (
    <div className={styles.settings} aria-busy={busy}>
      <label className={styles.field}>
        <span className={styles.label}>Ausgabegerät</span>
        <select
          className={styles.select}
          value={currentDevice}
          disabled={busy}
          onChange={(e) => void apply(e.target.value || null, currentBuffer)}
        >
          {!devices.some((d) => d.id === currentDevice) && (
            <option value={currentDevice}>{status?.deviceName ?? 'Kein Gerät'}</option>
          )}
          {devices.map((d) => (
            <option key={d.id} value={d.id}>
              {d.name}
              {d.isDefault ? ' (Standard)' : ''}
              {d.maxChannels > 2 ? ` · ${d.maxChannels} Kanäle` : ''}
            </option>
          ))}
        </select>
      </label>

      <label className={styles.field}>
        <span className={styles.label}>Puffergröße</span>
        <select
          className={styles.select}
          value={currentBuffer}
          disabled={busy}
          onChange={(e) => void apply(status?.deviceId ?? null, Number(e.target.value))}
        >
          {BUFFER_SIZES.map((n) => (
            <option key={n} value={n}>
              {n} Samples
            </option>
          ))}
        </select>
      </label>

      <dl className={styles.facts}>
        <div>
          <dt>Latenz</dt>
          <dd className="numeric">
            {status?.connected ? `${status.latencyMs.toFixed(1)} ms` : '–'}
          </dd>
        </div>
        <div>
          <dt>Abtastrate</dt>
          <dd className="numeric">{status?.sampleRate ? `${status.sampleRate / 1000} kHz` : '–'}</dd>
        </div>
        <div>
          <dt>Puffer aktiv</dt>
          <dd className="numeric">
            {status?.connected ? (status.bufferFrames ?? 'vom Gerät gewählt') : '–'}
          </dd>
        </div>
      </dl>

      {(error ?? status?.error) && (
        <p className={styles.error} role="alert">
          {error ?? status?.error}
        </p>
      )}
    </div>
  );
}
