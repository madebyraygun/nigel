---
id: TASK-109.19
title: 'Documents: give an executed document a value and bill it on a schedule'
status: To Do
assignee: []
created_date: '2026-10-07 14:24'
updated_date: '2026-10-07 14:26'
labels:
  - documents
  - invoicing
dependencies:
  - TASK-109.4
parent_task_id: TASK-109
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
An accepted agreement is where billing starts, but today nothing connects the two: the operator re-keys the amount into an invoice or a schedule by hand. Let a document carry a contract value, then turn an executed document into invoices — one invoice, or a billing schedule — linked back to the document so either side shows the other.

Existing schedules (nigel invoice schedule add) bill a fixed amount every month, quarter or year from a start date, with an optional end. A contract value usually wants one of: a single invoice; a deposit then the balance; N equal installments; or a recurring fee for a term. Decide in the design which of these the first cut supports, and whether installments are a new schedule shape or a set of dated draft invoices.

Open questions for the design: is the value part of what is signed (frozen per sent version, shown on the page) or Nigel-only metadata; does conversion require executed or also accepted; currency handling when the client's invoices use another; what revise or withdraw does to a schedule already created from the document.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A document carries an optional value (amount in minor units plus currency), editable while draft, shown in CLI show, the API detail and the web detail
- [ ] #2 An executed document can be converted to billing from the CLI, the API and the web detail: at least a single invoice and an installment plan (count, cadence, start), producing draft invoices or a schedule whose total equals the document's value
- [ ] #3 Rounding: installments that do not divide evenly put the remainder on one stated installment, and the totals are tested to equal the value exactly
- [ ] #4 The created invoices or schedule record the document they came from; the document detail lists them and the invoice/schedule detail links back
- [ ] #5 Converting twice is refused unless the earlier billing is voided or ended, so a contract is never billed twice by accident
- [ ] #6 Nothing is sent automatically by the conversion unless the operator chooses autosend, matching existing schedule behaviour
- [ ] #7 The conversion offers an option to attach the executed document to every invoice it produces: the PDF of the version that was signed is attached to each invoice email alongside invoice.pdf, including invoices a schedule generates later; the choice is recorded on the schedule or invoices and can be turned off afterwards
- [ ] #8 Mail supports more than one attachment (OutgoingMail carries a list), invoice emails without the option are unchanged, and the combined attachments stay under the mail provider's size limit or the send refuses with a clear message
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Attachment option: the invoice email today carries exactly one attachment (Mailer::send / OutgoingMail.attachment is an Option), so this needs a list. Attach the executed version's PDF as filed (the checksum the signatures bind to), named from the document title via render::attachment_name.
<!-- SECTION:NOTES:END -->
