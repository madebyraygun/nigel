import { describe, it, expect } from 'vitest';
import { shortDate } from './short-date.js';

describe('shortDate', () => {
  it('prints a calendar date as month, day and year', () => {
    expect(shortDate('2026-10-16')).toBe('Oct 16, 2026');
    expect(shortDate('2026-01-05')).toBe('Jan 5, 2026');
  });

  it('passes through anything that is not a calendar date', () => {
    expect(shortDate('')).toBe('');
    expect(shortDate('2026-13-01')).toBe('2026-13-01');
    expect(shortDate('On receipt')).toBe('On receipt');
  });
});
