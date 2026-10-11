// Dialogs that keyboard shortcuts and empty states open. The title bar owns them and registers
// their openers here.
export type DialogName = 'settings' | 'shortcuts' | 'monitor';

const openers = new Map<DialogName, () => void>();

export function registerDialog(name: DialogName, open: () => void): () => void {
  openers.set(name, open);
  return () => {
    if (openers.get(name) === open) openers.delete(name);
  };
}

export function openDialog(name: DialogName) {
  openers.get(name)?.();
}
