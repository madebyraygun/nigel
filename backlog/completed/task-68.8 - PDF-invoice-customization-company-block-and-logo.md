---
id: TASK-68.8
title: 'PDF invoice customization: company block and logo'
status: Done
assignee: []
created_date: '2026-08-08 01:02'
updated_date: '2026-08-11 04:07'
labels:
  - invoicing
dependencies: []
parent_task_id: TASK-68
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Deferred out of 68.3 by design review: pdf.rs has no template, only imperative layout, so "customizable" means either a structured settings shape (company block, typography, logo path — with its own validation and export) or an HTML-to-PDF dependency decision (headless browser / Typst / Weasyprint), which carries real weight in a single-static-binary tool. This task owns that decision. Minimum bar: company name from metadata renders in the PDF header (parity with the HTML {{COMPANY}} from 68.3); logo support decided here.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The PDF carries the operator's company identity without rebuilding the binary
- [x] #2 The customization mechanism's shape (settings vs HTML-to-PDF) is decided and documented
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
PR #194 merged. The invoice PDF renders the operator's company name in its header and Info title, from the same Branding resolution as the HTML (single-source property pinned by test). Mechanism decided and documented: data-driven within printpdf, no HTML-to-PDF dependency; logo explicitly deferred on measured grounds (nine transitive crates via printpdf/embedded_images plus an upstream SMask width/height bug for non-square images) with the rationale in docs/invoicing.md. Tests gained real PDF text assertions via printpdf's re-exported lopdf extract_text — no new dependency.
<!-- SECTION:FINAL_SUMMARY:END -->
