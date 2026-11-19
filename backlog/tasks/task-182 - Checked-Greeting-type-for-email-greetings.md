---
id: TASK-182
title: Checked Greeting type for email greetings
status: To Do
assignee: []
created_date: '2026-11-19 00:33'
labels: []
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Today a resolved email greeting is a plain Option<&str>, the same type as raw text, so the checked state lives in doc comments and caller discipline. Every send path calls resolve_greeting first, but the sinks (send_invoice, send_invoice_traced, preview_invoice_email, documents SendContext.greeting, preview_document_email, render_document_email_text, mark_sent, set_email_greeting) accept unchecked text. Two failures the compiler cannot catch: an unchecked greeting reaching a send delivers a literal ${frstName}, because fill_placeholders leaves unknown names in place; and a greeting checked for one kind used for the other fills ${invoiceNumber} with an empty string, because GreetingValues answers every name. Follows TASK-181 (PR #91).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A Greeting type carrying its GreetingKind and the trimmed, checked text can only be built by resolve_greeting (private constructor); resolve_greeting returns Result<Option<Greeting>>
- [ ] #2 Every send, preview, render and record sink takes Option<&Greeting> instead of Option<&str>, so passing unchecked text does not compile
- [ ] #3 Filling placeholders is a method on Greeting that only fills names offered by its own kind; using an invoice greeting on a document send does not compile or is refused
- [ ] #4 clean_greeting, check_placeholders and fill_placeholders are no longer pub outside the greeting module
- [ ] #5 No behaviour change: existing greeting, parity, route, schedule and CLI tests pass unchanged apart from call-site types
<!-- AC:END -->
