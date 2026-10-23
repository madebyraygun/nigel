import { describe, it, expect, afterEach, vi } from 'vitest';
import { confirmDiscard, setUnsavedChanges, unsavedChanges } from './unsaved.js';

const DRAFT = { heading: 'Discard unsaved letterhead changes?', message: 'Not saved.' };

describe('unsaved changes', () => {
  afterEach(() => {
    setUnsavedChanges(null);
    vi.restoreAllMocks();
  });

  it('lets anything through when nothing is unsaved, without asking', async () => {
    const ui = await import('@nigel/ui');
    const confirm = vi.spyOn(ui, 'confirmDialog');
    expect(await confirmDiscard()).toBe(true);
    expect(confirm).not.toHaveBeenCalled();
  });

  it('asks in the destructive voice and keeps the record on a no', async () => {
    const ui = await import('@nigel/ui');
    const confirm = vi.spyOn(ui, 'confirmDialog').mockResolvedValue(false);
    setUnsavedChanges(DRAFT);

    expect(await confirmDiscard()).toBe(false);
    expect(confirm).toHaveBeenCalledWith(
      expect.objectContaining({ heading: DRAFT.heading, variant: 'danger' }),
    );
    expect(unsavedChanges()).toEqual(DRAFT);
  });

  it('clears the record on a yes', async () => {
    const ui = await import('@nigel/ui');
    vi.spyOn(ui, 'confirmDialog').mockResolvedValue(true);
    setUnsavedChanges(DRAFT);

    expect(await confirmDiscard()).toBe(true);
    expect(unsavedChanges()).toBeNull();
  });
});
