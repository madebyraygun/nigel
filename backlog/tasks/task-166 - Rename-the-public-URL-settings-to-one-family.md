---
id: TASK-166
title: Rename the public URL settings to one family
status: To Do
assignee: []
created_date: '2026-10-24 21:40'
labels:
  - settings
  - invoicing
  - documents
milestone: m-3
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Nigel's public URL settings are named inconsistently: public_base_url (where clients open invoice pages) and documents_base_url (where they open documents to sign), with multiuser adding the server's own address. They become one family: server_public_url, invoice_public_url, document_public_url, with matching env vars (NIGEL_SERVER_PUBLIC_URL, NIGEL_INVOICE_PUBLIC_URL, NIGEL_DOCUMENT_PUBLIC_URL).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 public_base_url is renamed invoice_public_url and documents_base_url is renamed document_public_url in settings.json, the settings API, the web Settings screen, the CLI and every error code and message that names them (invalid_public_base_url becomes invalid_invoice_public_url or invalid_document_public_url as appropriate)
- [ ] #2 The old settings.json keys are still read and are rewritten to the new keys on the next save; a file carrying both an old and a new key uses the new one
- [ ] #3 The old env vars NIGEL_PUBLIC_BASE_URL and NIGEL_DOCUMENTS_BASE_URL are still read when the new ones are unset, and the CLI's status/doctor output says the old name is deprecated
- [ ] #4 docs (api.md, invoicing.md, design-constraints.md, README.md) and the CHANGELOG describe the new names; tests cover old-key and old-env-var compatibility
<!-- AC:END -->
