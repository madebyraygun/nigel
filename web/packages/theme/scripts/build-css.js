/**
 * Build the plain stylesheet from the composed token sheet.
 *
 * The Lit `CSSResult` serves components; this emits the same text as a plain
 * `.css` file for the document-level sheet the app imports (which is what puts
 * `:root` tokens and the `::part()` overrides in scope).
 */
import fs from 'fs';
import path from 'path';
import { fileURLToPath, pathToFileURL } from 'url';

const here = path.dirname(fileURLToPath(import.meta.url));
const distDir = path.join(here, '..', 'dist', 'css');

fs.mkdirSync(distDir, { recursive: true });

// The @font-face urls are relative to the stylesheet, so ../fonts/ has to
// exist next to dist/css/. Vite resolves them from here into hashed assets;
// without the copy the rules point at nothing and every browser quietly
// renders a system face instead.
fs.cpSync(path.join(here, '..', 'src', 'fonts'), path.join(here, '..', 'dist', 'fonts'), {
  recursive: true,
});

const themePath = pathToFileURL(
  path.join(here, '..', 'dist', 'themes', 'nigel.js'),
).href;
const { nigelTheme } = await import(themePath);

const banner = `/**
 * Nigel theme tokens — generated from @nigel/theme
 * Source: web/packages/theme/src/themes/nigel.ts
 */
`;

fs.writeFileSync(path.join(distDir, 'nigel.css'), banner + nigelTheme.cssText + '\n');

console.log('  ✓ dist/css/nigel.css');
console.log('  ✓ dist/fonts/');
