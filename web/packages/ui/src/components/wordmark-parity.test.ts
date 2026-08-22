import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { WORDMARK_ART } from './wc-wordmark.js';

/**
 * The web wordmark is meant to *be* the TUI's wordmark, not a redrawing of it,
 * which is `palette-parity.test.ts`'s claim about the colours applied to the
 * shape. This reads the Rust source and fails if the two drift apart.
 */
const here = dirname(fileURLToPath(import.meta.url));
const effectsRs = resolve(here, '../../../../../crates/nigel/src/effects.rs');

function logoFromRust(): string[] {
  const source = readFileSync(effectsRs, 'utf8');
  const start = source.indexOf('pub const LOGO');
  expect(start, 'LOGO const not found in crates/nigel/src/effects.rs').toBeGreaterThan(-1);
  const end = source.indexOf('];', start);
  return [...source.slice(start, end).matchAll(/^\s*r"(.*)",$/gm)].map((m) => m[1]);
}

describe('wordmark parity with crates/nigel/src/effects.rs', () => {
  it('reads a non-empty logo out of the Rust source', () => {
    expect(logoFromRust().length).toBeGreaterThan(0);
  });

  it('matches WORDMARK_ART exactly, in order', () => {
    expect(logoFromRust()).toEqual([...WORDMARK_ART]);
  });
});
