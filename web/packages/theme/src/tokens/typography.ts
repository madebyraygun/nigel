import { css } from 'lit';

/**
 * IBM Plex Mono throughout, bundled into the binary.
 *
 * The browser is meant to read as the same product as the terminal, and the
 * app has almost no prose to lose by it — the longest strings anywhere are
 * two-sentence guardrail explanations and empty states. A second face for
 * those would cost a second token and a per-component judgement about which
 * side of the line each string falls on, which is the drift the token system
 * exists to prevent.
 *
 * **Bundled, not fetched.** `nigel serve` is one binary that may be running
 * with no network at all; the faces are committed under `../fonts/`, declared
 * by `font-faces.ts`, and served from the same binary. The system mono stack
 * stays behind the bundled family, so a face that somehow fails to load still
 * aligns money columns rather than falling back to something proportional.
 *
 * `--wa-font-family-sans` keeps its name. It means "the primary UI face", and
 * the name is read by every component and by Web Awesome's own internals;
 * renaming it to say `mono` would be a rename of the whole token vocabulary
 * to record one decision that could be reversed.
 */
export const typographyCss = css`
  :root {
    --wa-font-family-sans: 'IBM Plex Mono', ui-monospace, SFMono-Regular,
      'SF Mono', Menlo, Consolas, monospace;
    --wa-font-family-mono: 'IBM Plex Mono', ui-monospace, SFMono-Regular,
      'SF Mono', Menlo, Consolas, monospace;
    --wa-font-size-s: 12px;
    --wa-font-size-base: 14px;
    --wa-font-size-lg: 16px;
    --wa-font-size-xl: 20px;
    --wa-font-size-2xl: 26px;
    --wa-font-weight-normal: 400;
    --wa-font-weight-medium: 500;
    --wa-font-weight-bold: 600;
    --wa-line-height: 1.5;

    /* Now the same stack as everything else, and kept anyway: it names an
       intent — figures that have to align in a column — which a UI that ever
       moves back to a proportional face would need again. */
    --nc-font-money: var(--wa-font-family-mono);
  }
`;
