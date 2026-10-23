const MONTHS = [
  'Jan',
  'Feb',
  'Mar',
  'Apr',
  'May',
  'Jun',
  'Jul',
  'Aug',
  'Sep',
  'Oct',
  'Nov',
  'Dec',
] as const;

/**
 * `2026-10-16` as `Oct 16, 2026`, the way the invoice screens print a date.
 *
 * Fixed English month names rather than `Intl`, so a date reads the same on
 * every machine and a test can name it. Anything that is not a `YYYY-MM-DD`
 * comes back unchanged: a date the server wrote is shown, never swallowed.
 */
export function shortDate(iso: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!match) return iso;
  const month = MONTHS[Number(match[2]) - 1];
  if (!month) return iso;
  return `${month} ${Number(match[3])}, ${match[1]}`;
}
