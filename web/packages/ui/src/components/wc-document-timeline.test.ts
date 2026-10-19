import { describe, it, expect, afterEach } from "vitest";
import "./wc-document-timeline.js";
import type {
  TimelineEvent,
  TimelineVersion,
  WcDocumentTimeline,
} from "./wc-document-timeline.js";
import { describePreviewA11y } from "../../preview/axe-suite.js";
import preview from "./wc-document-timeline.preview.js";

const V1: TimelineVersion = {
  number: 1,
  checksum: "sha256:aa",
  sentAt: "2026-10-05",
  createdAt: "2026-10-05",
  recipients: [
    { role: "signer", name: "Pat Example", email: "pat@cedar.test" },
    { role: "collaborator", name: "Sam Example", email: "sam@cedar.test" },
  ],
  events: [],
};
const ACCEPT: TimelineEvent = {
  kind: "signature",
  role: "client",
  name: "Pat Example",
  email: "pat@cedar.test",
  method: "manual",
  at: "2026-10-06",
  typedName: null,
  ip: null,
  userAgent: null,
  note: null,
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
  userAgent: "UA",
  note: "Fix the dates",
};

async function mount(versions: TimelineVersion[]): Promise<WcDocumentTimeline> {
  const el = document.createElement("wc-document-timeline");
  el.versions = versions;
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe("wc-document-timeline", () => {
  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("a_note_with_markup_renders_as_text", async () => {
    const el = await mount([
      {
        ...V1,
        events: [
          { ...CHANGE, note: '<img src=x onerror="alert(1)">\nline two' },
        ],
      },
    ]);
    const note = el.shadowRoot!.querySelector(".note")!;
    expect(note.querySelector("img")).toBeNull();
    expect(note.textContent).toContain("<img src=x");
  });

  it("shows the evidence of an online acceptance and none for a manual one", async () => {
    const el = await mount([
      {
        ...V1,
        events: [
          {
            ...ACCEPT,
            method: "online",
            typedName: "Pat Example",
            ip: "203.0.113.7",
            userAgent: "UA",
          },
          {
            ...ACCEPT,
            role: "countersign",
            name: "Sam Example",
            method: "manual",
          },
        ],
      },
    ]);
    const events = [...el.shadowRoot!.querySelectorAll("[data-event]")];
    expect(events[0].querySelector("[data-evidence]")?.textContent).toContain(
      "203.0.113.7",
    );
    expect(events[1].querySelector("[data-evidence]")).toBeNull();
  });

  it("marks an unsent version as a draft", async () => {
    const el = await mount([
      { ...V1, sentAt: null, recipients: [], events: [] },
    ]);
    expect(el.shadowRoot!.textContent).toContain("Draft — not sent");
  });

  it("words each version and event", async () => {
    const el = await mount([
      { ...V1, events: [ACCEPT, { ...ACCEPT, role: "countersign" }, CHANGE] },
    ]);
    const text = el.shadowRoot!.textContent!.replace(/\s+/g, " ");
    expect(text).toContain("Version 1");
    expect(text).toContain("Sent 2026-10-05");
    expect(el.shadowRoot!.querySelector("code")?.textContent).toBe("sha256:aa");
    expect(text).toContain("Accepted by Pat Example — manual, 2026-10-06");
    expect(text).toContain("Countersigned by Pat Example");
    expect(text).toContain(
      "Changes requested by Sam Example — online, 2026-10-05T17:04:11Z",
    );
    expect(text).toContain("signer");
    expect(text).toContain("sam@cedar.test");
  });
});

describePreviewA11y(preview);
