---
id: TASK-109
title: >-
  Epic: documents — proposals, estimates and agreements with approval and
  lightweight signing
status: To Do
assignee: []
created_date: '2026-08-16 04:21'
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

This epic adds **Documents** as a top-level domain beside Clients and Invoices — and it is deliberately a *mechanism*, not a business feature:

- **Kinds are operator-defined data, not a compiled-in enum.** "Proposal", "estimate" and "agreement" ship as seeded, editable defaults (the chart-of-accounts pattern), and another operator's "engagement letter" or "change order" is a row, not a fork. Nothing about any real business is compiled in, per the features-stay-general-purpose rule; every example and fixture uses the fictional cast (Acme, Cedar Systems, …).
- **Documents arrive rendered; Nigel does not draft them.** A document is filed as a finished PDF — uploaded in the browser, or filed by `nigel document add` / the HTTP API. This is the skills scaffold: the contract between a business-specific skill and Nigel is the documented CLI/API surface, so an operator's private drafting skills (which hold the wording, the rates, the clause library) plug in with zero code in this repository. The repo ships the authoring guide and one generic example skill; the business half stays in the operator's private skills directory.
- **The lifecycle is the feature.** draft → sent → accepted/declined, with withdraw terminal — statuses derived from timestamps the way invoice status is derived (`refresh_status`/`voided_at` precedent), never hand-set. Publishing reuses the invoicing machinery: a tokenized page + PDF pair at `d/{token}/…` beside the invoices' `i/{token}/…`, Mailgun send with confirmation and a step trace, void-style teardown for withdraw.
- **Signing is lightweight, recorded assent — not an e-signature platform.** The baseline needs no infrastructure: the client replies "approved" to the email and the operator records it (`nigel document accept`, or the equivalent action in the TUI/SPA) with a name, a date and a method. Online click-to-accept is optional infrastructure on the Stripe model: the published page carries a typed-name accept form posting to a small operator-deployed Worker that writes an acceptance object beside the page in R2, and `nigel document sync` pulls acceptance records — pull-based, idempotent, no webhook endpoint into Nigel, because `serve` binds localhost and localhost is not a trust boundary. Unconfigured, the form is simply absent (the PayButton live/inert/absent precedent). On acceptance the page is republished stamped with who accepted and when, form removed (the republish precedent). Nigel records assent; it makes no claim about legal enforceability.

## Design decisions stated up front

- **PDF-first ingest plus a Nigel-generated viewer page** (rather than ingesting HTML): the accept control must live on something Nigel renders, or acceptance state could not be stamped onto the page; skills and humans alike produce PDFs; and one generated wrapper page gives every document the same publish seam regardless of what produced it.
- **A Worker and pull sync rather than a public Nigel endpoint**: the invoicing subsystem already rejected webhooks for Stripe on the same grounds. The Worker is a small generic script shipped in this repo and deployed by the operator beside their existing R2 custom domain; it writes an object, nothing else.
- **No plugin/module system.** Private crates or dynamic loading would fork the self-updating public binary and turn every internal seam into a stable API. The extension surface is data (kinds, uploaded files) and the documented CLI/API contract that skills drive. If an operator ever needs private *code*, the lib+bin split already lets a private repo depend on the `nigel` library — no machinery required here.

## Sequencing

The numbering is the order for the core: data layer and kinds first, then filing, then publish/send/withdraw, then approval and signing. The HTTP API mirrors the data layer once the verbs exist; the TUI needs only the data layer, the SPA needs the API. The skills scaffold, docs and demo seed are the capstone and land last.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every subtask of this epic is Done
- [ ] #2 On demo data, a PDF produced outside Nigel can be filed against a client, previewed, sent, and carried to accepted or declined from each surface (CLI, TUI, SPA), with the acceptance record (name, date, method) readable back on all three
- [ ] #3 Nothing business-specific is compiled in or committed: document kinds are editable seeded data, every fixture and example uses the fictional cast, and ./scripts/check-no-real-data.sh passes on every commit in the push
- [ ] #4 The skills scaffold is real: docs/documents.md documents the filing contract, a generic example skill in .claude/skills/ exercises it end to end on demo data, and an operator's private skill can participate with no change to this repository
- [ ] #5 Scope is stated where it stops: signing is recorded assent with no legal-enforceability claim, online accept is optional operator infrastructure with manual accept as the baseline, and in-app drafting/templating, versioning and countersigning are explicitly out
- [ ] #6 IMPORTANT: Any PRs created from this epic must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
