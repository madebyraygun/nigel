---
id: TASK-114
title: >-
  Epic: local delivery — invoices and documents without hosting or a mail
  service
status: To Do
assignee: []
created_date: '2026-08-17 13:27'
labels:
  - epic
  - invoicing
  - documents
  - desktop
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Invoicing and documents are the only Nigel features that require cloud accounts before they work at all: R2, Mailgun, a public base URL — and for documents, optionally an accept Worker. The desktop app's premise (decision-1) is "no terminal, no infrastructure": it opens no socket and asks for no setup. **Local delivery** completes that story: a mode where sending an invoice or a document means writing the page + PDF into a local outbox folder and opening the operator's own mail client with the message pre-addressed and the PDF attached.

- **A mode is two trait impls, not a fork.** `send_invoice_traced` is already generic over `PaymentGateway`, `AssetPublisher` and `Mailer` (`gateway.rs`), and preview already proves the render pipeline runs with no network and no configuration. Local mode is a `LocalDirPublisher` writing under an outbox directory and a compose-window `Mailer` — the same guards, the same step trace, the same derived statuses. Nothing above the traits knows which mode is running.
- **The mode is chosen, never inferred.** A `delivery = "hosted" | "local"` setting. Today a send without Mailgun/R2 keys is a refusal naming the missing keys; silently converting that refusal into a different delivery mechanism would be a trap. Hosted behavior stays byte-for-byte unchanged, refusal included.
- **Attachment needs the OS, not mailto.** `mailto:` (RFC 6068) cannot attach a file, and the PDF is the deliverable. Where the platform allows, compose goes through the OS: MAPI on Windows, `NSSharingService` on macOS, `xdg-email --attach` on Linux — the desktop shell reaches all three natively. Where only mailto exists (plain CLI on a bare box), the fallback is honest: open compose pre-addressed with a short body, and reveal the PDF in the file manager beside it so attaching is one drag.
- **Sent becomes attested, and says so.** Nigel can observe that it opened a compose window; it cannot observe the operator pressing send. In local mode the trace ends at a `compose opened` step followed by an explicit "mark as sent?" confirmation; the row records the delivery method (the acceptance-record precedent: a method column, not a guess), and the docs state plainly that a local `sent_at` is operator-attested rather than observed. Declining the confirmation leaves the document or invoice a draft.
- **What local mode gives up is stated, not discovered.** No hosted page or token URL — the client experience is "PDF in inbox". No Stripe pay button. No online document acceptance (the form, the Worker, `document sync`) — manual accept/decline/countersign, the zero-infrastructure baseline, still carries a document to executed. No payment or signature republish — a local re-render of the outbox artifacts stands in. No delivery observability past compose. No recurring auto-send — recurring invoices degrade to prepare-and-prompt. `invoice sync` for Stripe payments survives when Stripe is configured: it is pull-based and needs no R2.

## Design decisions stated up front

- **The outbox mirrors the publish layout** (`outbox/i/<token>/…`, `outbox/d/<token>/…`): the pure `object_key` functions serve both modes, and anything that can render to R2 can render to disk with no second naming scheme.
- **Local re-render is the republish analog**: the state changes that republish a hosted page (a payment recorded, a signature, a withdraw) re-render the outbox artifacts instead, with the same best-effort rule — a failed re-render is a warning, never lost state.
- **A Stripe payment link pasted into the body or PDF is out of v1.** It would blur the mode boundary (a network call inside "local" send) for a feature the operator can do by hand; if wanted later it is an additive flag, not a redesign.
- **Absent capabilities are absent quietly** (the PayButton live/inert/absent precedent): in local mode the pay button and accept form simply do not render anywhere, and no surface offers an action that cannot complete.

## Sequencing

Config and the publisher first, then the compose mailer and attested sent, then local re-render, then the surfaces, then docs and demo. Invoicing local delivery depends on nothing new; the documents half lands behind epic 109's verbs and generalizes the same seams.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every subtask of this epic is Done
- [ ] #2 On demo data with delivery=local and no R2, Mailgun or Stripe keys configured, an invoice and a document can be sent (outbox written, compose opened, sent attested) and the document carried to executed, from the CLI, TUI, SPA and desktop shell
- [ ] #3 Hosted mode is byte-for-byte unchanged, including the missing-keys refusal; switching modes changes delivery mechanics only, never data semantics
- [ ] #4 The hosted-versus-local capability matrix is documented, and every capability absent in local mode is absent quietly — the control does not render, no surface offers an action that cannot complete
- [ ] #5 Nothing business-specific is committed, fixtures use the fictional cast, and ./scripts/check-no-real-data.sh passes on every commit in the push
- [ ] #6 IMPORTANT: Any PRs created from this epic must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
