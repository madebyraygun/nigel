// Composed theme (Lit CSSResult — primary export)
export { nigelTheme } from './themes/index.js';

// Token modules — re-exported for callers that want category-level access
export { NIGEL_PALETTE, gradientCss } from './tokens/gradient.js';
export { colorCss, colorDarkCss } from './tokens/color.js';
export { typographyCss } from './tokens/typography.js';
export { spacingCss } from './tokens/spacing.js';
export { radiusCss } from './tokens/radius.js';
export { shadowCss } from './tokens/shadow.js';
export { motionCss } from './tokens/motion.js';
export { controlsCss } from './controls.js';

// The one behaviour module in an otherwise CSS-only package: the writer for
// the light/dark class contract tokens/color.ts defines.
export {
  COLOR_MODES,
  COLOR_MODE_STORAGE_KEY,
  DARK_CLASS,
  LIGHT_CLASS,
  applyMode,
  darkModeQuery,
  initColorMode,
  readMode,
  resolveMode,
  writeMode,
  type ColorMode,
  type ResolvedMode,
} from './color-mode.js';
export { printCss } from './print.js';
