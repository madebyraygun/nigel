import { LitElement, html, css, nothing } from "lit";
import { customElement, property } from "lit/decorators.js";

export interface TimelineRecipient {
  role: "signer" | "collaborator";
  name: string;
  email: string;
}

export interface TimelineEvent {
  kind: "signature" | "change_request";
  role: "client" | "countersign" | null;
  name: string;
  email: string | null;
  method: "online" | "manual";
  at: string;
  typedName: string | null;
  ip: string | null;
  userAgent: string | null;
  note: string | null;
}

export interface TimelineVersion {
  number: number;
  checksum: string;
  sentAt: string | null;
  createdAt: string;
  recipients: TimelineRecipient[];
  events: TimelineEvent[];
}

function eventVerb(event: TimelineEvent): string {
  if (event.kind === "change_request") return "Changes requested";
  return event.role === "countersign" ? "Countersigned" : "Accepted";
}

@customElement("wc-document-timeline")
export class WcDocumentTimeline extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    section {
      padding: var(--wa-space-m, 12px) 0;
      border-top: 1px solid var(--wa-color-border);
    }

    section:first-child {
      border-top: 0;
    }

    h3 {
      margin: 0 0 var(--wa-space-2xs, 4px);
      font-size: var(--wa-font-size-m, 15px);
    }

    code {
      font-family: var(--wa-font-family-mono, monospace);
      font-size: var(--wa-font-size-s, 13px);
      overflow-wrap: anywhere;
    }

    .meta,
    .evidence {
      margin: 0;
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
      overflow-wrap: anywhere;
    }

    ul {
      margin: var(--wa-space-xs, 6px) 0;
      padding: 0;
      list-style: none;
      font-size: var(--wa-font-size-s, 13px);
    }

    ul.events {
      font-size: var(--wa-font-size-m, 15px);
    }

    li.event {
      margin: var(--wa-space-xs, 6px) 0;
    }

    .role {
      color: var(--wa-color-muted);
      text-transform: capitalize;
    }

    .note {
      margin: var(--wa-space-2xs, 4px) 0 0;
      padding-left: var(--wa-space-s, 8px);
      border-left: 2px solid var(--wa-color-border);
      white-space: pre-wrap;
      overflow-wrap: anywhere;
    }
  `;

  /** Newest first. */
  @property({ attribute: false })
  versions: TimelineVersion[] = [];

  private renderEvent(event: TimelineEvent) {
    const evidence =
      event.method === "online"
        ? [
            event.typedName ? `Typed name ${event.typedName}` : null,
            event.ip ? `IP ${event.ip}` : null,
            event.userAgent ? `Agent ${event.userAgent}` : null,
          ].filter((part) => part !== null)
        : [];
    return html`
      <li class="event" data-event>
        <div>
          ${eventVerb(event)} by ${event.name} — ${event.method}, ${event.at}
        </div>
        ${
          evidence.length > 0
            ? html`<p class="evidence" data-evidence>
                ${evidence.join(" · ")}
              </p>`
            : nothing
        }
        ${event.note ? html`<p class="note">${event.note}</p>` : nothing}
      </li>
    `;
  }

  private renderVersion(version: TimelineVersion) {
    const id = `version-${version.number}`;
    return html`
      <section aria-labelledby=${id}>
        <h3 id=${id}>Version ${version.number}</h3>
        <p class="meta">
          <code>${version.checksum}</code>
          · ${version.sentAt ? `Sent ${version.sentAt}` : "Draft — not sent"}
        </p>
        ${
          version.recipients.length > 0
            ? html`<ul class="recipients">
                ${version.recipients.map(
                  (r) =>
                    html`<li>
                      <span class="role">${r.role}</span> ${r.name} ${r.email}
                    </li>`,
                )}
              </ul>`
            : nothing
        }
        ${
          version.events.length > 0
            ? html`<ul class="events">
                ${version.events.map((event) => this.renderEvent(event))}
              </ul>`
            : nothing
        }
      </section>
    `;
  }

  render() {
    return html`${this.versions.map((version) => this.renderVersion(version))}`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "wc-document-timeline": WcDocumentTimeline;
  }
}
