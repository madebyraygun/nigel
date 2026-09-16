---
id: TASK-117
title: QFX/OFX/QBO importer
status: To Do
assignee: []
created_date: '2026-08-19 14:43'
updated_date: '2026-09-16 17:05'
labels:
  - importer
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Nigel reads CSV and XLSX statements only (uploads::ALLOWED_EXTENSIONS is csv/xlsx/xls). Most US banks also offer an OFX-family download, and for some institutions it is the cleanest export on offer — stable field structure, unambiguous dates and amounts, and a per-transaction FITID that makes duplicate detection exact instead of heuristic.

Three extensions, one format family, one parser:

- **.ofx** — Open Financial Exchange itself.
- **.qfx** — Quicken's dialect.
- **.qbo** — QuickBooks WebConnect, the dialect many banks label 'QuickBooks' on their download page and the one some institutions offer in place of a usable CSV.

All three are the same container. OFX 1.x is SGML-flavored (no closing tags) and OFX 2.x is XML; a parser must accept both, since banks ship either, and .qbo files are usually the 1.x form. The dialects differ in the signon and header blocks rather than the transaction list: .qfx and .qbo carry Intuit-namespaced tags (INTU.BID, INTU.USERID) that a reader must tolerate and ignore, and some issuers ship a .qbo whose account-type block differs from the .qfx the same bank produces. STMTTRN and FITID are common to all three, so detection keys on the OFX structure rather than the extension, and one ImporterKind variant covers the family.

Add it as an ImporterKind variant per docs/importers.md (enum dispatch, no plugin registry). Surface the family everywhere formats already appear: detection, the --format flag, the imports/formats endpoint, and the upload allow-list — which ripples into the SPA's extension list (kept in wc-dropzone's DEFAULT_EXTENSIONS after TASK-33.3 lands; the native dialog filter follows ALLOWED_EXTENSIONS automatically).

Sequencing: build on the import-integrity fixes (TASK-50/51/52) so the new parser inherits the malformed-row record and atomic sequence rather than retrofitting them.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A QFX 1.x (SGML), a QBO WebConnect and an OFX 2.x (XML) fixture statement all import via detection alone, with dates, signed amounts and descriptions correct
- [ ] #2 Detection keys on the OFX structure, not the file extension, so a renamed or mislabeled file of any of the three still imports
- [ ] #3 Intuit-namespaced signon and header tags are tolerated and ignored rather than treated as malformed
- [ ] #4 FITID is used for row-level duplicate detection within and across files for this format
- [ ] #5 qfx, ofx and qbo extensions are accepted by upload and staging allow-lists and offered by the web dropzone and the desktop dialog filter
- [ ] #6 Malformed SGML/XML rows follow the malformed-row record from TASK-52, not a silent drop
- [ ] #7 docs/importers.md gains the format's entry, naming all three extensions, and fixtures use the fictional cast
<!-- AC:END -->
