---
id: TASK-167
title: Remote MCP endpoint so Claude can draft and send documents
status: To Do
assignee: []
created_date: '2026-10-27T13:54:00Z'
labels:
  - documents
  - multiuser
  - backend
  - security
  - enhancement
dependencies:
  - TASK-32.2
  - TASK-32.9
  - TASK-109.9
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Let an operator reach Nigel from Claude (claude.ai or Claude Code) as a custom connector, so Claude can draft a document for a client and send it once the operator approves. Nigel has no MCP surface today. Skills drive the CLI on the same machine, which stops working once Nigel runs as a shared server on another host behind a tunnel.

Approach: a remote MCP server at /mcp on the host that runs nigel serve in multiuser mode, using the streamable HTTP transport. Build it into the server as a thin layer over the data-layer verbs the JSON API already calls, rather than as a separate process that proxies the API: one binary, one auth model, and every call is attributed and role-checked like any other write. Auth is OAuth as the MCP authorization spec and claude.ai custom connectors expect it (protected-resource metadata, dynamic client registration, PKCE), with each token bound to a Nigel user. No pasted API keys. Off by default, enabled by a setting.

First tools:
- list_clients: active clients with id, name and email
- list_document_kinds: the operator's active kinds
- create_document_draft: client, kind, title and a Markdown body, producing a draft (needs TASK-109.9)
- get_document: status, current version, the can* flags and the preview link
- send_document: its own tool so the client can require explicit approval; mirrors POST /api/documents/:id/send

Out of scope for v1: accepting, countersigning, withdrawing or deleting over MCP, invoices, and a local stdio server.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 With the setting on, the server answers MCP over streamable HTTP at /mcp; with it off, /mcp is 404
- [ ] #2 /mcp requires an OAuth access token; the server publishes protected-resource and authorization-server metadata, supports dynamic client registration and PKCE, the consent step signs in as an existing Nigel user, and an admin can revoke a token
- [ ] #3 list_clients, list_document_kinds, create_document_draft, get_document and send_document call the same data-layer code as the JSON API, are attributed to the token's user, and respect that user's role
- [ ] #4 send_document is the only tool that sends anything: it carries the destructiveHint annotation and requires confirm: true, and no tool accepts, countersigns, withdraws or deletes
- [ ] #5 The Host/Origin checks and the deployment guide (TASK-32.9) cover /mcp behind a tunnel, with the forwarded Host pinned to the public name
- [ ] #6 On demo data with the fictional cast, a test drives the flow end to end: register a client, get a token, list clients, draft a document for Acme, send it, and read back status sent
- [ ] #7 Setup is documented, including adding Nigel in claude.ai as a custom connector, and README Features mentions it
<!-- AC:END -->
