import { css } from 'lit';

export const motionCss = css`
  :root {
    --nc-transition-fast: 120ms ease;
    --nc-transition-base: 200ms ease;
    --nc-duration-fast: 120ms;
    --nc-duration-base: 200ms;
    /* Long enough to read as a fade rather than a switch. The button's hover
       edge is the only thing that takes it: a press or a focus ring at this
       length would feel unresponsive. */
    --nc-duration-slow: 500ms;
  }

  @media (prefers-reduced-motion: reduce) {
    :root {
      --nc-transition-fast: 0ms linear;
      --nc-transition-base: 0ms linear;
      --nc-duration-fast: 0ms;
      --nc-duration-base: 0ms;
      --nc-duration-slow: 0ms;
    }
  }
`;
