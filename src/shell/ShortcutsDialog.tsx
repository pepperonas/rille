import type { Ref } from 'react';
import { SHORTCUTS, type ShortcutGroup } from './shortcuts';
import styles from './ShortcutsDialog.module.css';

const GROUPS: ShortcutGroup[] = ['Deck 1', 'Deck 2', 'Mixer', 'Allgemein'];

/** `?`: every keyboard shortcut, generated from the same table the handler uses. */
export function ShortcutsDialog({ ref }: { ref: Ref<HTMLDialogElement> }) {
  return (
    <dialog ref={ref} className={styles.dialog} aria-labelledby="shortcuts-title">
      <h2 id="shortcuts-title" className={styles.title}>
        Tastenkürzel
      </h2>
      <div className={styles.groups}>
        {GROUPS.map((group) => (
          <section key={group} className={styles.group} aria-label={group}>
            <h3 className={styles.heading}>{group}</h3>
            <dl className={styles.list}>
              {SHORTCUTS.filter((r) => r.group === group).map((r) => (
                <div key={r.keys} className={styles.row}>
                  <dt className={styles.keys}>
                    {r.keys.split(' / ').map((k, i) => (
                      <span key={k}>
                        {i > 0 && <span className={styles.or}> / </span>}
                        {k.split(' ').map((key) => (
                          <kbd key={key} className={styles.kbd}>
                            {key}
                          </kbd>
                        ))}
                      </span>
                    ))}
                  </dt>
                  <dd className={styles.description}>{r.description}</dd>
                </div>
              ))}
            </dl>
          </section>
        ))}
      </div>
      <p className={styles.hint}>
        Kürzel ruhen, solange ein Textfeld oder ein Dialog den Fokus hat. Alles andere erreichst du
        mit Tab; Regler reagieren auf die Pfeiltasten.
      </p>
      <form method="dialog" className={styles.actions}>
        <button type="submit" className={styles.textButton}>
          Schließen
        </button>
      </form>
    </dialog>
  );
}
