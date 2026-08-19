---
id: TASK-117
title: QFX/OFX importer
status: To Do
assignee: []
created_date: '2026-08-19 14:43'
labels:
  - importer
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Nigel reads CSV and XLSX statements only (uploads::ALLOWED_EXTENSIONS is csv/xlsx/xls). Most US banks also export QFX (Quicken's OFX dialect), and for some institutions it is the cleanest export on offer — stable field structure, unambiguous dates and amounts, and a per-transaction FITID that makes duplicate detection exact instead of heuristic.

Add a QFX/OFX importer as an ImporterKind variant per docs/importers.md (enum dispatch, no plugin registry). OFX 1.x is SGML-flavored (no closing tags) and OFX 2.x is XML; a parser must accept both, since banks ship either. Surface the format everywhere formats already appear: detection, the --format flag, the imports/formats endpoint, and the upload allow-list — which ripples into the SPA's extension list (kept in wc-dropzone's DEFAULT_EXTENSIONS after TASK-33.3 lands; the native dialog filter follows ALLOWED_EXTENSIONS automatically).

Sequencing: build on the import-integrity fixes (TASK-50/51/52) so the new parser inherits the malformed-row record and atomic sequence rather than retrofitting them.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A QFX 1.x (SGML) and an OFX 2.x (XML) fixture statement both import via detection alone, with dates, signed amounts and descriptions correct
- [ ] #2 FITID is used for row-level duplicate detection within and across files for this format
- [ ] #3 qfx and ofx extensions are accepted by upload and staging allow-lists and offered by the web dropzone and the desktop dialog filter
- [ ] #4 Malformed SGML/XML rows follow the malformed-row record from TASK-52, not a silent drop
- [ ] #5 docs/importers.md gains the format's entry and fixtures use the fictional cast
<!-- AC:END -->
