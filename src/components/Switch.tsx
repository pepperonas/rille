import styles from './Switch.module.css';

interface Props {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  description?: string;
}

/** M3 switch with label (and optional supporting text). */
export function Switch({ label, checked, onChange, description }: Props) {
  return (
    <label className={styles.row}>
      <span className={styles.text}>
        <span className={styles.label}>{label}</span>
        {description && <span className={styles.description}>{description}</span>}
      </span>
      <input
        type="checkbox"
        role="switch"
        className={styles.input}
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span className={styles.track} aria-hidden="true">
        <span className={styles.thumb} />
      </span>
    </label>
  );
}
