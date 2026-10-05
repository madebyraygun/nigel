import { html } from "lit";
import "./wc-document-timeline.js";
import type { TimelineEvent, TimelineVersion } from "./wc-document-timeline.js";
import type { Preview } from "../../preview/types.js";

const RECIPIENTS: TimelineVersion["recipients"] = [
  { role: "signer", name: "Pat Example", email: "pat@cedar.test" },
  { role: "collaborator", name: "Sam Example", email: "sam@cedar.test" },
];

const SENT: TimelineVersion = {
  number: 1,
  checksum: "sha256:9f2c41d0b7a85e3c",
  sentAt: "2026-10-05",
  createdAt: "2026-10-04",
  recipients: RECIPIENTS,
  events: [],
};

const CHANGE: TimelineEvent = {
  kind: "change_request",
  role: null,
  name: "Sam Example",
  email: "sam@cedar.test",
  method: "online",
  at: "2026-10-05T17:04:11Z",
  typedName: null,
  ip: "203.0.113.7",
  userAgent: "Mozilla/5.0 (X11; Linux x86_64) Firefox/130.0",
  note: "Please fix the dates in section 2.\nAlso: <b>not</b> bold.",
};

const ACCEPT_ONLINE: TimelineEvent = {
  kind: "signature",
  role: "client",
  name: "Pat Example",
  email: "pat@cedar.test",
  method: "online",
  at: "2026-10-07T09:12:40Z",
  typedName: "Pat Example",
  ip: "203.0.113.7",
  userAgent: "Mozilla/5.0 (X11; Linux x86_64) Firefox/130.0",
  note: null,
};

const COUNTERSIGN: TimelineEvent = {
  kind: "signature",
  role: "countersign",
  name: "Dana Fixture",
  email: null,
  method: "manual",
  at: "2026-10-08",
  typedName: null,
  ip: null,
  userAgent: null,
  note: null,
};

const REVISED: TimelineVersion = {
  ...SENT,
  number: 2,
  checksum: "sha256:1b6e0a77c4d9f850",
  sentAt: "2026-10-06",
  createdAt: "2026-10-06",
  events: [ACCEPT_ONLINE],
};

const preview: Preview = {
  id: "wc-document-timeline",
  title: "Document timeline",
  group: "Documents",
  description:
    "One block per version: checksum, send date, recipients, and each response with its method and evidence. Notes are recipient text and render as plain text.",
  states: [
    {
      name: "draft-only",
      render: () =>
        html`<wc-document-timeline
          .versions=${[{ ...SENT, sentAt: null, recipients: [] }]}
        ></wc-document-timeline>`,
    },
    {
      name: "sent-with-change-request",
      render: () =>
        html`<wc-document-timeline
          .versions=${[{ ...SENT, events: [CHANGE] }]}
        ></wc-document-timeline>`,
    },
    {
      name: "revised-and-accepted",
      render: () =>
        html`<wc-document-timeline
          .versions=${[REVISED, { ...SENT, events: [CHANGE] }]}
        ></wc-document-timeline>`,
    },
    {
      name: "executed",
      render: () =>
        html`<wc-document-timeline
          .versions=${[
            { ...REVISED, events: [COUNTERSIGN, ACCEPT_ONLINE] },
            { ...SENT, events: [CHANGE] },
          ]}
        ></wc-document-timeline>`,
    },
    {
      name: "manual-only",
      render: () =>
        html`<wc-document-timeline
          .versions=${[
            {
              ...SENT,
              events: [
                {
                  ...ACCEPT_ONLINE,
                  method: "manual",
                  typedName: null,
                  ip: null,
                  userAgent: null,
                  at: "2026-10-06",
                },
              ],
            },
          ]}
        ></wc-document-timeline>`,
    },
  ],
};

export default preview;
