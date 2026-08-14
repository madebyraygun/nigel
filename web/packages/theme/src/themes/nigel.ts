import { css } from 'lit';
import { colorCss, colorDarkCss } from '../tokens/color.js';
import { brandCycleKeyframes, gradientCss } from '../tokens/gradient.js';
import { typographyCss } from '../tokens/typography.js';
import { spacingCss } from '../tokens/spacing.js';
import { radiusCss } from '../tokens/radius.js';
import { shadowCss } from '../tokens/shadow.js';
import { motionCss } from '../tokens/motion.js';
import { fontFacesCss } from '../tokens/font-faces.js';
import { waContractCss } from '../tokens/wa-contract.js';
import { printCss } from '../print.js';

/**
 * The composed token sheet — the one `scripts/build-css.js` emits as
 * `dist/css/nigel.css` and the document loads.
 *
 * Order is load bearing: the `@font-face` rules first, so the family is
 * declared before any token names it, then light defaults, then the dark
 * overrides (whose higher-specificity selectors have to come later to win),
 * then the print sheet last of all — it has to win over everything, including
 * dark mode.
 *
 * There are deliberately no `::part()` rules here. A document-level rule
 * reaches one shadow boundary down, and every `wa-*` primitive in this app is
 * several boundaries deep, so those rules ship in `controlsCss` and are
 * adopted by the components that host the primitives. What this sheet delivers
 * into a component is custom properties, which inherit through boundaries on
 * their own.
 */
export const nigelTheme = css`
  ${fontFacesCss}
  ${colorCss}
  ${typographyCss}
  ${spacingCss}
  ${gradientCss}
  ${brandCycleKeyframes}
  ${radiusCss}
  ${shadowCss}
  ${motionCss}
  ${waContractCss}
  ${colorDarkCss}
  ${printCss}
`;
