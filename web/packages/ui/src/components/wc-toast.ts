import { LitElement, html, css, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import { repeat } from 'lit/directives/repeat.js';

export type NcToastVariant = 'info' | 'success' | 'danger';

/** Optional action button on a toast (e.g. "Undo" after a destructive edit). */
export interface NcToastAction {
  label: string;
  onClick: () => void;
}

export interface NcToastDetail {
  message: string;
  variant?: NcToastVariant;
  /**
   * Auto-dismiss after N ms. Defaults to 4000, or 8000 with an action so there
   * is time to read and click. Zero or negative disables auto-dismiss.
   */
  duration?: number;
  action?: NcToastAction;
}

export const NC_TOAST_EVENT = 'nc-toast';

const DEFAULT_DURATION_MS = 4000;
const DEFAULT_ACTION_DURATION_MS = 8000;

/**
 * How many toasts share the corner. Past this the oldest goes, so a burst of
 * events cannot grow the column until it runs out of viewport.
 */
export const MAX_VISIBLE_TOASTS = 3;

interface QueuedToast {
  id: number;
  detail: NcToastDetail;
}

/**
 * Typed dispatcher for the toast bus. Use this instead of constructing a raw
 * CustomEvent — a raw event would accept any detail shape, this enforces it at
 * the call site.
 */
export function dispatchNcToast(target: EventTarget, detail: NcToastDetail): void {
  target.dispatchEvent(
    new CustomEvent<NcToastDetail>(NC_TOAST_EVENT, {
      detail,
      bubbles: true,
      composed: true,
    }),
  );
}

declare global {
  interface HTMLElementEventMap {
    'nc-toast': CustomEvent<NcToastDetail>;
  }
}

/**
 * The single aria-live region that terminates the toast bus.
 *
 * Listens on `window` rather than on a parent element: toast events are
 * composed and bubbling, so a window listener also catches toasts dispatched
 * from inside a wa-dialog's top layer or from any component that is not nested
 * under the app shell. Anchoring the listener to a host element loses those.
 */
@customElement('wc-toast')
export class WcToast extends LitElement {
  static styles = css`
    :host {
      /* The region is fixed-position; the host itself takes no space. */
      display: contents;
      --nc-toast-gutter: var(--wa-space-l, 16px);
      --nc-toast-max-width: 360px;
    }

    /*
     * Pinned to the bottom-right corner of the viewport.
     *
     * Both inline insets are set rather than one inset plus a translate: the
     * region spans the viewport minus its gutters, so a percentage width
     * inside it is viewport-relative and no offset can carry a toast past an
     * edge. The corner also keeps clear of the app chrome that owns the
     * others — the sidebar on the left, the header on the top.
     */
    .region {
      position: fixed;
      inset-block: auto var(--nc-toast-gutter);
      inset-inline: var(--nc-toast-gutter);
      z-index: 11000;
      display: flex;
      flex-direction: column;
      align-items: flex-end;
      gap: var(--wa-space-s, 8px);
      pointer-events: none;
      /* Reset the UA popover styles so the region flows the same way whether
       * or not it is currently in the top layer. */
      border: 0;
      padding: 0;
      margin: 0;
      background: transparent;
      color: inherit;
      overflow: visible;
      inline-size: auto;
      block-size: auto;
    }

    .region:not(:popover-open) {
      /* The region must stay rendered for the aria-live subscription to hold,
       * but a UA hides closed popovers by default. */
      display: flex;
    }

    .toast {
      pointer-events: auto;
      display: flex;
      align-items: center;
      gap: var(--wa-space-l, 16px);
      padding: 10px 16px;
      border-radius: var(--wa-radius-md, 10px);
      font-family: var(--wa-font-family-sans);
      font-size: var(--wa-font-size-base, 14px);
      font-weight: var(--wa-font-weight-medium, 500);
      box-shadow: var(--wa-shadow-lg, 0 12px 32px rgb(0 0 0 / 25%));
      animation: toast-in var(--nc-duration-fast, 120ms) ease;
      /* A consistently dark chip in both themes: the toast paints over cards
       * and dialogs that already use --wa-color-surface, so reusing surface
       * here would make it disappear into them. */
      background: #1f1f28;
      color: #ece9f5;
      border: 1px solid rgb(255 255 255 / 10%);
      /* 100% is the region, which is the viewport minus its gutters, so a long
       * message wraps inside the chip instead of widening it off-screen. */
      max-width: min(var(--nc-toast-max-width), 100%);
      box-sizing: border-box;
    }

    .message {
      min-width: 0;
      overflow-wrap: anywhere;
    }

    .toast[data-variant='success'] {
      background: var(--wa-color-success);
      border-color: var(--wa-color-success);
      color: #ffffff;
    }

    .toast[data-variant='danger'] {
      background: var(--wa-color-danger);
      border-color: var(--wa-color-danger);
      color: #ffffff;
    }

    .action {
      background: transparent;
      border: 0;
      color: inherit;
      font: inherit;
      font-weight: var(--wa-font-weight-bold, 600);
      text-decoration: underline;
      text-underline-offset: 3px;
      cursor: pointer;
      padding: 4px 8px;
      border-radius: var(--wa-radius-sm, 6px);
      white-space: nowrap;
      flex-shrink: 0;
    }

    .action:hover,
    .action:focus-visible {
      background: rgb(255 255 255 / 15%);
    }

    @keyframes toast-in {
      from {
        opacity: 0;
        transform: translateY(8px);
      }
      to {
        opacity: 1;
        transform: translateY(0);
      }
    }

    @media (prefers-reduced-motion: reduce) {
      .toast {
        animation: none;
      }
    }

    /* Screen chrome, not part of the report. This lives here rather than in
       @nigel/theme's print sheet because a rule that hides an element has to
       be in the tree that element is in, and every wc-* here sits inside
       nigel-app's shadow root where a document rule cannot reach it. */
    @media print {
      :host {
        display: none;
      }
    }
  `;

  /**
   * Seeds toasts on first render, one or several. Previews and tests use this;
   * the app does not.
   */
  @property({ attribute: false })
  initial: NcToastDetail | NcToastDetail[] | null = null;

  @state()
  private toasts: QueuedToast[] = [];

  private timers = new Map<number, ReturnType<typeof setTimeout>>();

  private nextId = 0;

  private promotedId: number | null = null;

  connectedCallback(): void {
    super.connectedCallback();
    window.addEventListener(NC_TOAST_EVENT, this.handleToast as EventListener);
    if (this.initial) {
      for (const detail of Array.isArray(this.initial) ? this.initial : [this.initial]) {
        this.show(detail);
      }
    }
  }

  disconnectedCallback(): void {
    window.removeEventListener(NC_TOAST_EVENT, this.handleToast as EventListener);
    this.clearTimers();
    this.toasts = [];
    this.promotedId = null;
    super.disconnectedCallback();
  }

  /** Show a toast directly, bypassing the event bus. */
  show(detail: NcToastDetail): void {
    if (typeof detail?.message !== 'string' || detail.message.length === 0) {
      console.error('[wc-toast] ignored a toast with no message:', detail);
      return;
    }
    const id = this.nextId++;
    const next = [...this.toasts, { id, detail }];
    for (const dropped of next.splice(0, next.length - MAX_VISIBLE_TOASTS)) {
      this.clearTimer(dropped.id);
    }
    this.toasts = next;

    const fallback = detail.action ? DEFAULT_ACTION_DURATION_MS : DEFAULT_DURATION_MS;
    const duration = detail.duration ?? fallback;
    if (duration > 0) {
      this.timers.set(
        id,
        setTimeout(() => this.drop(id), duration),
      );
    }
  }

  /** Dismiss every visible toast. */
  dismiss(): void {
    this.clearTimers();
    this.toasts = [];
  }

  /** Named for what it takes off the stack — `remove` is HTMLElement's. */
  private drop(id: number): void {
    this.clearTimer(id);
    this.toasts = this.toasts.filter((toast) => toast.id !== id);
  }

  private clearTimer(id: number): void {
    const timer = this.timers.get(id);
    if (timer !== undefined) {
      clearTimeout(timer);
      this.timers.delete(id);
    }
  }

  private clearTimers(): void {
    for (const timer of this.timers.values()) clearTimeout(timer);
    this.timers.clear();
  }

  private handleToast = (event: Event): void => {
    const detail = (event as CustomEvent<Partial<NcToastDetail>>).detail;
    if (!detail || typeof detail.message !== 'string' || detail.message.length === 0) {
      console.error('[wc-toast] ignored an event with an invalid detail:', detail);
      return;
    }
    this.show(detail as NcToastDetail);
  };

  /**
   * Promote the region into the browser's top layer so it paints above
   * wa-dialog, which uses native showModal(). The top layer is a stack ordered
   * by open time, so a toast arriving while the popover is already open has to
   * hide and re-show to get back above a dialog opened in between. Only a new
   * toast triggers that: re-showing on every render would restart the entry
   * animation of the toasts already in the column.
   */
  protected updated(): void {
    const region = this.shadowRoot?.querySelector<HTMLElement>('[data-toast-region]');
    if (!region || typeof region.showPopover !== 'function') return;
    const newest = this.toasts.at(-1)?.id ?? null;
    try {
      if (newest !== null) {
        if (newest === this.promotedId && region.matches(':popover-open')) return;
        if (region.matches(':popover-open')) region.hidePopover();
        region.showPopover();
        this.promotedId = newest;
      } else {
        if (region.matches(':popover-open')) region.hidePopover();
        this.promotedId = null;
      }
    } catch (error) {
      console.warn('[wc-toast] could not sync the popover state:', error);
    }
  }

  private runAction(toast: QueuedToast): void {
    const action = toast.detail.action;
    if (!action) return;
    try {
      action.onClick();
    } catch (error) {
      console.error('[wc-toast] the toast action threw:', error);
    }
    this.drop(toast.id);
  }

  render() {
    const danger = this.toasts.some((toast) => toast.detail.variant === 'danger');
    return html`
      <div
        class="region"
        data-toast-region
        popover="manual"
        role=${danger ? 'alert' : 'status'}
        aria-live=${danger ? 'assertive' : 'polite'}
        aria-atomic="true"
      >
        ${repeat(
          this.toasts,
          (toast) => toast.id,
          (toast) => html`
            <div class="toast" data-variant=${toast.detail.variant ?? 'info'}>
              <span class="message">${toast.detail.message}</span>
              ${toast.detail.action
                ? html`<button
                    type="button"
                    class="action"
                    data-toast-action
                    @click=${() => this.runAction(toast)}
                  >
                    ${toast.detail.action.label}
                  </button>`
                : nothing}
            </div>
          `,
        )}
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-toast': WcToast;
  }
}
