import { html } from 'lit';
import './wc-wordmark.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-wordmark',
  title: 'Wordmark',
  group: 'Brand',
  description:
    "Nigel's ASCII wordmark, per-character spans sharing one gradient. The art is the TUI's LOGO and a parity test pins the two together.",
  layout: 'stack',
  states: [
    { name: 'static', render: () => html`<wc-wordmark></wc-wordmark>` },
    { name: 'animated', render: () => html`<wc-wordmark animated></wc-wordmark>` },
    {
      name: 'revealing',
      render: () => html`<wc-wordmark animated .reveal=${0.45}></wc-wordmark>`,
    },
    {
      name: 'reduced-motion',
      render: () => html`<wc-wordmark animated reduced-motion .reveal=${0.2}></wc-wordmark>`,
    },
    {
      name: 'on-dark',
      background: 'inverse',
      render: () => html`<wc-wordmark animated></wc-wordmark>`,
    },
    { name: 'diagonal', render: () => html`<wc-wordmark diagonal></wc-wordmark>` },
    {
      name: 'diagonal on dark',
      background: 'inverse',
      render: () => html`<wc-wordmark diagonal></wc-wordmark>`,
    },
    {
      name: 'diagonal reduced motion',
      render: () => html`<wc-wordmark diagonal animated reduced-motion></wc-wordmark>`,
    },
    {
      name: 'labelled',
      render: () => html`<wc-wordmark label="Nigel — bookkeeping"></wc-wordmark>`,
    },
  ],
};

export default preview;
