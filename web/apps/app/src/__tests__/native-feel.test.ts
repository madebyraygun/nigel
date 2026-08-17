import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * The desktop shell wraps this document in a webview, where a browser's
 * usual rubber-band scroll and text-selection chrome read as "this is a
 * website in a box". These are the document-level rules that keep it
 * feeling native; see docs/native-feel.md.
 */
const here = dirname(fileURLToPath(import.meta.url));
const indexHtml = readFileSync(resolve(here, '../../index.html'), 'utf8');
const style = indexHtml.slice(indexHtml.indexOf('<style>'), indexHtml.indexOf('</style>'));

describe('document-level native-feel rules', () => {
  it('disables overscroll so the document does not rubber-band', () => {
    expect(style).toMatch(/overscroll-behavior:\s*none/);
  });

  it('disables user-select so dragging chrome does not paint a selection', () => {
    expect(style).toMatch(/(?<!-webkit-)user-select:\s*none/);
    expect(style).toMatch(/-webkit-user-select:\s*none/);
  });

  it('disables the drag ghost on images', () => {
    expect(style).toMatch(/img\s*\{[^}]*-webkit-user-drag:\s*none/);
  });
});
