import { useEffect, useState } from 'react';
import { backend } from '../ipc';
import type { AudioDevice, CueSetting } from '../ipc/types';
import { useAppState } from '../state/store';
import styles from './AudioSettings.module.css';

const BUFFER_SIZES = [64, 128, 256, 512, 1024, 2048] as const;

export function AudioSettings({ open }: { open: boolean }) {
  const status = useAppState((s) => s.audio);
  const engineXruns = useAppState((s) => s.frame?.xruns ?? 0);
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
          <dd className="numeric">
            {status?.sampleRate ? `${status.sampleRate / 1000} kHz` : '–'}
          </dd>
        </div>
        <div>
          <dt>Aussetzer</dt>
          <dd className="numeric">{engineXruns + (status?.deviceXruns ?? 0)}</dd>
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

      <HeadphoneSettings devices={devices} />
    </div>
  );
}

/** Value of the headphone select: `off`, `channels34` or `device:<id>`. */
function cueValue(setting: CueSetting | undefined): string {
  if (!setting || setting.kind === 'off') return 'off';
  if (setting.kind === 'channels34') return 'channels34';
  return `device:${setting.deviceId ?? ''}`;
}

/** Headphone output: off, outputs 3/4 of the main device, or a second device. The DDJ-200 has
 *  no audio interface, so pre-listening needs one of these. */
function HeadphoneSettings({ devices }: { devices: AudioDevice[] }) {
  const status = useAppState((s) => s.audio);
  const cue = status?.cue;
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const buffer = cue?.setting.kind === 'device' ? cue.setting.bufferFrames : 256;

  const apply = async (setting: CueSetting) => {
    setBusy(true);
    setError(null);
    try {
      await backend.setCue(setting);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const choose = (value: string, bufferFrames = buffer) => {
    if (value === 'off') void apply({ kind: 'off' });
    else if (value === 'channels34') void apply({ kind: 'channels34' });
    else void apply({ kind: 'device', deviceId: value.slice(7) || null, bufferFrames });
  };

  // A second device other than the main output.
  const others = devices.filter((d) => d.id !== status?.deviceId);
  const message = error ?? cue?.error;

  return (
    <section className={styles.section} aria-label="Kopfhörer" aria-busy={busy}>
      <h3 className={styles.heading}>Kopfhörer</h3>
      <p className={styles.hint}>
        Der DDJ-200 hat keine eigene Soundkarte. Zum Vorhören brauchst du ein Gerät mit vier
        Ausgängen oder ein zweites Gerät (z. B. USB-Kopfhörer).
      </p>
      <label className={styles.field}>
        <span className={styles.label}>Kopfhörer-Ausgang</span>
        <select
          className={styles.select}
          value={cueValue(cue?.setting)}
          disabled={busy}
          onChange={(e) => choose(e.target.value)}
        >
          <option value="off">Aus</option>
          <option value="channels34">Ausgänge 3/4 des Ausgabegeräts</option>
          {others.map((d) => (
            <option key={d.id} value={`device:${d.id}`}>
              {d.name}
            </option>
          ))}
        </select>
      </label>
      {cue?.setting.kind === 'device' && (
        <label className={styles.field}>
          <span className={styles.label}>Puffer Kopfhörer</span>
          <select
            className={styles.select}
            value={buffer}
            disabled={busy}
            onChange={(e) => choose(cueValue(cue.setting), Number(e.target.value))}
          >
            {BUFFER_SIZES.map((n) => (
              <option key={n} value={n}>
                {n} Samples
              </option>
            ))}
          </select>
        </label>
      )}
      {cue?.active && (
        <dl className={styles.facts}>
          <div>
            <dt>Gerät</dt>
            <dd>{cue.deviceName ?? 'Ausgabegerät, Ausgänge 3/4'}</dd>
          </div>
          {cue.setting.kind === 'device' && (
            <>
              <div>
                <dt>Latenz</dt>
                <dd className="numeric">{`${cue.latencyMs.toFixed(1)} ms`}</dd>
              </div>
              <div>
                <dt>Aussetzer</dt>
                <dd className="numeric">{cue.underruns}</dd>
              </div>
            </>
          )}
        </dl>
      )}
      {message && (
        <p className={styles.error} role="alert">
          {message}
        </p>
      )}
    </section>
  );
}
