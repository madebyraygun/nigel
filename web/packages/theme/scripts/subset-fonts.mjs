/**
 * Regenerate `src/fonts/*.woff2` from the upstream IBM Plex Mono release.
 *
 * Run by hand, never by the build and never in CI — the output is committed,
 * so a checkout needs no font tooling. Re-run only when the glyph inventory
 * below changes.
 *
 *   cd web/packages/theme
 *   npm pack @ibm/plex-mono@2.5.0 && tar xzf ibm-plex-mono-2.5.0.tgz
 *   npx --yes subset-font@2.5.0 --help >/dev/null   # warms the cache
 *   node scripts/subset-fonts.mjs ./package ./src/fonts
 *   rm -rf package ibm-plex-mono-2.5.0.tgz
 *
 * `subset-font` is invoked through npx rather than declared as a devDependency
 * for the same reason: nothing about a normal install or build should need it.
 *
 * Known gap, and not one subsetting causes: IBM Plex Mono has no glyph for
 * ✗ ⟳ ◑ ● ◆ ▲ ⊘ ◻ (checked against the *complete* upstream font, not the
 * subset). Those fall back per-glyph wherever the UI draws them — see the
 * Typefaces section of web/README.md.
 */
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import subsetFont from 'subset-font';

/**
 * Ranges, not the exact glyphs in use. Client names and bank descriptions are
 * real-world text, and a subset that drops "é" renders half a name in a
 * fallback face, mid-word — which looks worse than bundling no font at all.
 */
const RANGES = [
  [0x0000, 0x00ff], // Basic Latin + Latin-1 Supplement (carries ×)
  [0x0100, 0x017f], // Latin Extended-A
  [0x2000, 0x206f], // General Punctuation — — – … “ ” ’ · •
  [0x20a0, 0x20bf], // Currency Symbols
  [0x2122, 0x2122], // ™
  [0x2190, 0x2193], // ← ↑ → ↓
  [0x2212, 0x2212], // − (minus, not hyphen)
  [0x2713, 0x2713], // ✓
  [0xfffd, 0xfffd], // replacement character
];

const WEIGHTS = [
  ['Regular', 400],
  ['Medium', 500],
  ['SemiBold', 600],
];

const [pkgDir = './package', outDir = './src/fonts'] = process.argv.slice(2);

let text = '';
for (const [lo, hi] of RANGES) {
  for (let cp = lo; cp <= hi; cp++) text += String.fromCodePoint(cp);
}

mkdirSync(outDir, { recursive: true });

for (const [name, weight] of WEIGHTS) {
  const src = readFileSync(join(pkgDir, `fonts/complete/woff2/IBMPlexMono-${name}.woff2`));
  const out = await subsetFont(src, text, { targetFormat: 'woff2' });
  const path = join(outDir, `ibm-plex-mono-${weight}.woff2`);
  writeFileSync(path, out);
  console.log(`${weight}: ${src.length} -> ${out.length} bytes  ${path}`);
}
