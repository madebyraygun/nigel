---
id: TASK-179
title: 'External-ID dedup follow-ups: checksum check, blank ids, base schema index'
status: To Do
assignee: []
created_date: '2026-11-10 02:07'
labels:
  - importer
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Three small gaps from the external-ID dedup review:

- `import_parsed` skips the duplicate-checksum check that `import_file` makes, so a caller importing the same parsed set twice gets a second, empty `imports` row, despite the doc comment on `ParsedImport.checksum` saying the checksum prevents that.
- `Some("")` counts as a real `external_id`; an OFX file with a blank FITID would dedup every blank row against each other.
- The partial unique index on (account_id, external_id) is created by migration v18 but not mirrored in the base schema in db.rs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `import_parsed` refuses or answers a repeated checksum the way `import_file` does, or the doc comment says it does not
- [ ] #2 A blank or whitespace external id is treated as none
- [ ] #3 db.rs creates the index too, matching the migration
<!-- AC:END -->
