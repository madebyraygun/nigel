import { css } from 'lit';

export const motionCss = css`
  :root {
    --nc-transition-fast: 120ms ease;
    --nc-transition-base: 200ms ease;
    --nc-duration-fast: 120ms;
    --nc-duration-base: 200ms;
    /* A switch sliding between off and on. */
    --nc-transition-switch: 150ms ease;
    /* The sidebar folding to its rail and back. */
    --nc-transition-sidebar: 180ms ease;
  }

  @media (prefers-reduced-motion: reduce) {
    :root {
      --nc-transition-fast: 0ms linear;
      --nc-transition-base: 0ms linear;
      --nc-duration-fast: 0ms;
      --nc-duration-base: 0ms;
      --nc-transition-switch: 0ms linear;
      --nc-transition-sidebar: 0ms linear;
    }
  }
`;
