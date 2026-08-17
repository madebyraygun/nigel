---
id: decision-2
title: 'Licensing: stay MIT with guardrails — Fair Source evaluated and deferred'
date: '2026-08-17 17:38'
status: accepted
---
## Context

Pre-launch is the one moment a license change is free: the repository is public but
unannounced, has no known users, and every line is the work of one author, so any of the
three postures was available at no cost. Once a community exists, moving away from MIT
becomes the HashiCorp story in miniature — so the choice had to be made deliberately now,
not inherited.

Three postures were evaluated against the product strategy in `docs/product/foundation.md`
(paid signed builds over an MIT core, plus a hosted tier):

- **MIT (status quo).** Anyone may rebuild, redistribute, even sell builds; only trademark
  keeps clones from wearing the name. In exchange: the strongest possible launch story and
  distribution channel for a product with no marketing budget, a verifiable trust story
  ("read the code") that *is* the positioning against cloud bookkeeping, and frictionless
  contribution — which matters most for the long tail of bank importers.
- **Fair Source (FSL-1.1-MIT).** Source stays public; commercial substitutes (sold builds,
  hosted clones) are barred per version for two years; each version converts to plain MIT
  automatically at two years. Real gains: the paywall gets legal teeth and the bus-factor
  promise becomes contractual. Real costs: the OSI label is lost, a CLA becomes mandatory
  from the first patch, FOSS-only distros are out, and every launch conversation spends
  oxygen on licensing instead of the product.
- **Fully proprietary.** Full pricing power and clean exit optionality, at the cost of the
  audit story, the bus-factor answer, and the only free distribution channel. The
  configuration with the weakest track record in this category for an unknown solo product.

The deciding observations: the threat MIT actually exposes (commercial rebuilds and hosted
clones) is near zero at Nigel's scale and years away from mattering, while FSL's costs are
paid immediately, in the launch window; and sole authorship means the FSL/proprietary
option survives indefinitely for future versions — provided no outside contribution is
ever merged without a rights grant.

## Decision

Nigel stays MIT, with three guardrails that keep every other door open:

1. **CLA from the first external contribution.** Every outside contributor signs the
   individual contributor license grant in `.github/cla/CLA.md`, enforced by the CLA
   Assistant Lite workflow (`.github/workflows/cla.yml`). This preserves clean chain of
   title and the unilateral right to relicense future versions. Contributions remain
   MIT-licensed in this repository, always — the CLA changes who may relicense, never
   what contributors receive.
2. **Trademark before announcement.** The Nigel name and mark are registered before any
   public launch, and the trademark policy ships with epic 115.2. Under MIT the mark is
   the only thing keeping unofficial builds from wearing the name; it does the work a
   restrictive license would otherwise do.
3. **Named reopening triggers.** The Fair Source evaluation is deferred, not rejected.
   It reopens if: a commercial rebuild or hosted Nigel clone actually appears; acquisition
   diligence demands title consolidation beyond what the CLA provides; or the paid-build
   model changes materially. If reopened, FSL-1.1-MIT is the pre-selected alternative
   (two-year automatic MIT conversion, applied to future versions only), and this record
   is the public evidence that the option was reserved before launch rather than sprung
   on a community.

## Consequences

- `docs/product/foundation.md` promise 1 ("The source is MIT, forever") stands as written.
- `CONTRIBUTING.md` explains the CLA and the reason for it in plain terms; the workflow
  blocks merges of unsigned external PRs and allowlists bots.
- Epic 115.2 (licensing) proceeds unchanged: merchant of record, signed keys, updater
  gating, trademark policy — trademark now carries known weight.
- A future licensing change, if a trigger fires, applies only to versions released after
  it; everything published under MIT remains MIT irrevocably.
