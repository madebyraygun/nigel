---
id: TASK-109
title: >-
  Epic: documents — drafting, templates, versioning, approval and two-party
  signing
status: To Do
assignee: []
created_date: '2026-08-16 04:21'
updated_date: '2026-08-17 04:53'
labels:
  - epic
  - documents
  - architecture
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Nigel tracks the money side of an engagement — clients, invoices, payments, aging — but not the paper that starts one: the proposal, the estimate, the agreement. Today those are produced outside the app (by hand, or by an operator's private Claude skills) and live as loose files with no link to the client, no lifecycle, and no record of whether the client ever said yes. "Which proposals are outstanding?" has no answer the way "which invoices are unpaid?" does.

This epic adds **Documents** as a top-level domain beside Clients and Invoices — a *mechanism* with no business content compiled in:

- **Kinds are operator-defined data, not a compiled-in enum.** "Proposal", "estimate" and "agreement" ship as seeded, editable defaults (the chart-of-accounts pattern), and another operator's "engagement letter" or "change order" is a row, not a fork. Nothing about any real business is compiled in, per the features-stay-general-purpose rule; every example and fixture uses the fictional cast (Acme, Cedar Systems, …).
- **Documents are drafted in Nigel or filed as finished PDFs — both paths are first-class.** A document's source is either *drafted* — a Markdown body with `{{KEY}}` placeholders, wrapped by a per-kind template and rendered by Nigel to the page and the PDF — or *filed* — a finished PDF produced elsewhere (a drafting skill, a partner's paper, a countersigned scan) ingested by `nigel document add`, a browser upload, or the HTTP API. Templates are seeded, operator-editable data under the data dir with generic wording only in the repo (the `templates/invoice.html` precedent), so drafting ships without compiling in anyone's rates or clauses. The skills contract covers both halves: a business-specific skill may hand Nigel Markdown and let Nigel own rendering and branding, or render its own PDF and file it; either way the contract is the documented CLI/API surface and the business half (wording, rates, clause libraries) stays in the operator's private skills and data dir.
- **The lifecycle is the feature — and it ends in *executed*.** draft → sent → accepted → executed, with declined and withdrawn terminal alongside — statuses derived from timestamps the way invoice status is derived (`refresh_status`/`voided_at` precedent), never hand-set. Publishing reuses the invoicing machinery: a tokenized page + PDF pair at `d/{token}/…` beside the invoices' `i/{token}/…`, Mailgun send with confirmation and a step trace, void-style teardown for withdraw.
- **Sending freezes a version, and signatures bind to it.** A draft mutates freely; each send snapshots an immutable numbered version — content, rendered PDF, checksum. Acceptance and countersigning record *which version* was assented to, which is what keeps recorded assent meaningful once editing exists. Revising a sent document opens a new draft version to resend at the same token; the published page names the version it shows.
- **Signing is recorded, two-party assent — not an e-signature platform.** The client's acceptance comes first: manual as the zero-infrastructure baseline (`nigel document accept` with a name, a date and a method), or optional online click-to-accept — the published page carries a typed-name accept form posting to a small operator-deployed Worker that writes an acceptance object beside the page in R2, pulled by `nigel document sync` (pull-based, idempotent, no webhook endpoint into Nigel, because `serve` binds localhost and localhost is not a trust boundary). The operator's countersign (`nigel document countersign`) then carries the document to *executed*. On each signature the page is republished stamped with who signed and when (the republish precedent); once executed the accept form is gone and both parties stand on the page and the PDF. Nigel records assent; it makes no claim about legal enforceability.

## Design decisions stated up front

- **Dual-source ingest, one publish seam**: drafted documents render through a shared page/PDF decision layer (the `render_invoice` / `invoicing/document.rs` precedent — decide once, render twice); filed PDFs get the same Nigel-generated viewer wrapper. Either way the accept control lives on something Nigel renders, so acceptance state can always be stamped onto the page.
- **Markdown + per-kind templates rather than HTML authoring or structured sections**: Markdown is pleasant in `$EDITOR`, in a textarea, and for a skill to emit; the per-kind HTML template (seeded, operator-editable, `{{KEY}}` placeholders — the invoice-template pattern) owns layout and branding. Clause libraries and structured/block authoring are deliberately out of v1; they can layer onto this model later without unwinding it.
- **Prose needs pagination**: the invoice PDF renderer draws at fixed offsets with no page-break logic. Multi-page PDF rendering for prose documents is its own subtask, not a rider on an existing one.
- **A Worker and pull sync rather than a public Nigel endpoint**: the invoicing subsystem already rejected webhooks for Stripe on the same grounds. The Worker is a small generic script shipped in this repo and deployed by the operator beside their existing R2 custom domain; it writes an object, nothing else.
- **No plugin/module system.** Private crates or dynamic loading would fork the self-updating public binary and turn every internal seam into a stable API. The extension surface is data (kinds, templates, uploaded files) and the documented CLI/API contract that skills drive. If an operator ever needs private *code*, the lib+bin split already lets a private repo depend on the `nigel` library — no machinery required here.

## Sequencing

Data layer and kinds first, then filing, then drafting/templates with paginated rendering beside them, then publish/send/withdraw with versioning, then approval and countersigning: 109.1 → 109.2 → 109.9 / 109.10 → 109.3 → 109.4. The HTTP API (109.5) mirrors the data layer once the verbs exist; the TUI (109.6) needs only the data layer, the SPA (109.7) needs the API. The skills scaffold, docs and demo seed (109.8) are the capstone and land last.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every subtask of this epic is Done
- [ ] #2 On demo data, a document can be drafted in Nigel from a per-kind template, edited, sent, revised and resent, and carried to executed (client acceptance plus operator countersign) from each surface (CLI, TUI, SPA), with each signature bound to a numbered version and readable back on all three
- [ ] #3 On demo data, a PDF produced outside Nigel can still be filed against a client, previewed, sent, and carried through the same lifecycle
- [ ] #4 Nothing business-specific is compiled in or committed: document kinds and templates are editable seeded data with generic wording, every fixture and example uses the fictional cast, and ./scripts/check-no-real-data.sh passes on every commit in the push
- [ ] #5 The skills scaffold is real: docs/documents.md documents both halves of the filing contract (hand Nigel Markdown, or file a rendered PDF), a generic example skill in .claude/skills/ exercises it end to end on demo data, and an operator's private skill can participate with no change to this repository
- [ ] #6 Scope is stated where it stops: signing is recorded assent with no legal-enforceability claim, online accept is optional operator infrastructure with manual accept as the baseline, and clause libraries, structured/block authoring and third-party e-signature integration are explicitly out of v1
- [ ] #7 IMPORTANT: Any PRs created from this epic must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
