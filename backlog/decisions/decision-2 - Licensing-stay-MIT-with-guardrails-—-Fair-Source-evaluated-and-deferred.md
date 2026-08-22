---
id: decision-2
title: 'Licensing: stay MIT with guardrails — Fair Source evaluated and deferred'
date: '2026-08-17 17:38'
status: accepted
---
## Context

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

## Decision

Nigel stays MIT, with two guardrails that keep every other door open:

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

Fair Source is deferred, not rejected. Sole authorship plus the CLA keeps FSL-1.1-MIT
available as the pre-selected alternative for future versions, should a commercial
rebuild appear or the paid-build model change materially. Anything already published
under MIT stays MIT irrevocably.

## Consequences

- `docs/product/foundation.md` promise 1 ("The source is MIT, forever") stands as written.
- `CONTRIBUTING.md` explains the CLA and the reason for it in plain terms; the workflow
  blocks merges of unsigned external PRs and allowlists bots.
- Epic 115.2 (licensing) proceeds unchanged: merchant of record, signed keys, updater
  gating, trademark policy — trademark now carries known weight.
