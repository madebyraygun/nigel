import { confirmDialog } from '@nigel/ui';
import { signal } from '../mixins/signal-watcher.js';

/**
 * Edits a screen holds that are not stored yet, and what to ask before they
 * are thrown away.
 *
 * A screen with a draft sets this while the draft differs from what is stored
 * and clears it when it does not, or when it goes away. `nigel-app` asks
 * `confirmDiscard` before anything that would remount the screen — a route
 * change from the sidebar, the palette or the address bar, or Refresh.
 * Closing the window is the screen's own `beforeunload`, since no dialog of
 * ours can run then.
 */
export interface UnsavedChanges {
  /** The question, e.g. "Discard unsaved letterhead changes?". */
  heading: string;
  /** What is lost, said under it. */
  message: string;
}

const pending = signal<UnsavedChanges | null>(null);

export function unsavedChanges(): UnsavedChanges | null {
  return pending.get();
}

export function setUnsavedChanges(changes: UnsavedChanges | null): void {
  pending.set(changes);
}

/**
 * Whether it is fine to throw the pending edits away: true at once when there
 * are none, otherwise the answer to the confirm dialog. A yes clears the
 * record, so the screen's own teardown has nothing left to ask about.
 */
export async function confirmDiscard(): Promise<boolean> {
  const changes = pending.get();
  if (!changes) return true;
  const confirmed = await confirmDialog({
    heading: changes.heading,
    message: changes.message,
    confirmLabel: 'Discard',
    cancelLabel: 'Keep editing',
    variant: 'danger',
  });
  if (confirmed) pending.set(null);
  return confirmed;
}
