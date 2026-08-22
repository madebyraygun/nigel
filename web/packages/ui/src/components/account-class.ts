/**
 * The accounting classes every account and category carries, in the order the
 * CLI and the TUI offer them.
 *
 * Mirrors `AccountClass` in `crates/nigel-core/src/db.rs`, which is where the
 * set is defined and where the `CHECK` constraint enforces it. Kept here rather
 * than derived from the API because no endpoint publishes it — a select has to
 * name its options.
 *
 * These five words are the whole vocabulary a user sees. No debit, no credit:
 * classification is structure, and the labels stay the plain words.
 */
export const ACCOUNT_CLASSES = [
  'asset',
  'liability',
  'equity',
  'revenue',
  'expense',
] as const;

export type AccountClassValue = (typeof ACCOUNT_CLASSES)[number];

const ACCOUNT_CLASS_LABELS: Record<string, string> = {
  asset: 'Asset',
  liability: 'Liability',
  equity: 'Equity',
  revenue: 'Revenue',
  expense: 'Expense',
};

/**
 * The human name for a class. A value from outside the vocabulary falls back
 * to itself rather than to a guess, so a database written by some other tool
 * still reads honestly.
 */
export function accountClassLabel(value: string): string {
  return ACCOUNT_CLASS_LABELS[value] ?? value;
}
