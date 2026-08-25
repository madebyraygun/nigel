//! The plain-text body of the invoice email.
//!
//! The email deliberately carries no HTML: mail clients render a full web page
//! badly, and the page itself is one click away. What a client reads in their
//! mail client is this text — the same figures the page and the PDF draw,
//! through the same [`MoneySummary`] and [`meta_rows`] — plus the link that
//! opens the published page and the PDF riding as the attachment.

use crate::invoicing::document::{meta_rows, money, payment_lines, terms_block_text, MoneySummary};
use crate::models::{Invoice, InvoiceLineItem};

/// Render the email body for one invoice.
///
/// `subject` is the message's own subject line, repeated as the opening line so
/// a client whose reader hides headers still sees who is billing them.
/// `page_url` is the published page — the one artifact that can take a payment.
pub fn render_email_text(
    invoice: &Invoice,
    items: &[InvoiceLineItem],
    money_summary: &MoneySummary,
    payment_instructions: &str,
    subject: &str,
    page_url: &str,
) -> String {
    let mut blocks: Vec<String> = vec![subject.to_string()];

    blocks.push(
        meta_rows(invoice)
            .iter()
            .map(|row| format!("{}: {}", row.label, row.value))
            .collect::<Vec<_>>()
            .join("\n"),
    );

    blocks.push(
        items
            .iter()
            .map(|i| {
                format!(
                    "{}  {} x {}  {}",
                    i.description,
                    i.quantity,
                    money(i.unit_amount, &invoice.currency),
                    money(i.line_total, &invoice.currency)
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    );

    blocks.push(
        money_summary
            .lines()
            .iter()
            .map(|line| format!("{}: {}", line.label, money(line.amount, &invoice.currency)))
            .collect::<Vec<_>>()
            .join("\n"),
    );

    blocks.push(format!("Pay now: {page_url}"));

    if let Some(notes) = invoice
        .notes
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
    {
        blocks.push(format!("Notes:\n{notes}"));
    }
    // Single-line terms already rode beside the due date in the metadata rows;
    // this block exists only when they did not — the same rule as the page and
    // the PDF, decided by the same function.
    if let Some(terms) = terms_block_text(invoice) {
        blocks.push(format!("Terms:\n{terms}"));
    }
    let payment = payment_lines(payment_instructions);
    if !payment.is_empty() {
        blocks.push(format!("Payment:\n{}", payment.join("\n")));
    }

    blocks.push("The invoice is also attached as a PDF.".into());
    blocks.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::render_email_text;
    use crate::invoicing::document::MoneySummary;
    use crate::models::{Invoice, InvoiceLineItem};

    fn item(description: &str, quantity: f64, unit_amount: f64) -> InvoiceLineItem {
        InvoiceLineItem {
            id: None,
            invoice_id: None,
            description: description.into(),
            quantity,
            unit_amount,
            line_total: quantity * unit_amount,
            position: 0,
        }
    }

    fn invoice() -> Invoice {
        Invoice {
            id: 1,
            number: 1248,
            client_id: 1,
            issue_date: "2026-08-04".into(),
            due_date: Some("2026-09-03".into()),
            status: "draft".into(),
            currency: "USD".into(),
            subtotal: 190.0,
            tax: 0.0,
            total: 190.0,
            notes: Some("Thanks for your business".into()),
            terms: Some("Net 30".into()),
            token: "tok".into(),
            stripe_payment_link_id: None,
            stripe_payment_link_url: None,
            published_at: None,
            voided_at: None,
        }
    }

    fn summary(total: f64, paid: f64) -> MoneySummary {
        MoneySummary {
            subtotal: total,
            tax: 0.0,
            total,
            paid,
            balance: total - paid,
            credit: 0.0,
        }
    }

    fn body() -> String {
        render_email_text(
            &invoice(),
            &[item("Consulting", 1.0, 100.0), item("Hosting", 2.0, 45.0)],
            &summary(190.0, 0.0),
            "",
            "Invoice #1248 from Bluepeak LLC",
            "https://billing.example.test/i/tok/index.html",
        )
    }

    #[test]
    fn the_body_opens_with_the_subject_line() {
        assert!(
            body().starts_with("Invoice #1248 from Bluepeak LLC"),
            "got: {}",
            body()
        );
    }

    #[test]
    fn the_metadata_rows_are_the_shared_ones() {
        let text = body();
        assert!(text.contains("Invoice ID: 1248"), "got: {text}");
        assert!(text.contains("Issue Date: 2026-08-04"), "got: {text}");
        // Single-line terms fold into the due date, exactly as on the page and
        // the PDF.
        assert!(
            text.contains("Due Date: 2026-09-03 (Net 30)"),
            "got: {text}"
        );
    }

    #[test]
    fn every_line_item_appears_with_quantity_unit_and_amount() {
        let text = body();
        assert!(
            text.contains("Consulting  1 x $100.00  $100.00"),
            "got: {text}"
        );
        assert!(text.contains("Hosting  2 x $45.00  $90.00"), "got: {text}");
    }

    #[test]
    fn the_money_lines_are_the_summary_lines() {
        let text = render_email_text(
            &invoice(),
            &[item("Consulting", 1.0, 100.0)],
            &summary(100.0, 40.0),
            "",
            "Invoice #1248",
            "https://x.test/i/t/index.html",
        );
        assert!(text.contains("Total: $100.00"), "got: {text}");
        assert!(text.contains("Paid: $40.00"), "got: {text}");
        assert!(text.contains("Balance due: $60.00"), "got: {text}");
    }

    #[test]
    fn the_pay_line_carries_the_page_url() {
        assert!(
            body().contains("Pay now: https://billing.example.test/i/tok/index.html"),
            "got: {}",
            body()
        );
    }

    #[test]
    fn the_attachment_sentence_closes_the_body() {
        assert!(
            body()
                .trim_end()
                .ends_with("The invoice is also attached as a PDF."),
            "got: {}",
            body()
        );
    }

    #[test]
    fn notes_and_payment_instructions_appear_when_set() {
        let text = render_email_text(
            &invoice(),
            &[item("Consulting", 1.0, 100.0)],
            &summary(100.0, 0.0),
            "Wells Fargo\nRouting 121000248",
            "Invoice #1248",
            "https://x.test/i/t/index.html",
        );
        assert!(
            text.contains("Notes:\nThanks for your business"),
            "got: {text}"
        );
        assert!(
            text.contains("Payment:\nWells Fargo\nRouting 121000248"),
            "got: {text}"
        );
    }

    #[test]
    fn multi_line_terms_get_their_own_block() {
        let mut inv = invoice();
        inv.terms = Some("Net 30.\nLate fee 1.5%/mo.".into());
        let text = render_email_text(
            &inv,
            &[item("Consulting", 1.0, 100.0)],
            &summary(100.0, 0.0),
            "",
            "Invoice #1248",
            "https://x.test/i/t/index.html",
        );
        assert!(text.contains("Due Date: 2026-09-03\n"), "got: {text}");
        assert!(
            text.contains("Terms:\nNet 30.\nLate fee 1.5%/mo."),
            "got: {text}"
        );
    }

    #[test]
    fn absent_values_omit_their_blocks() {
        let inv = Invoice {
            due_date: None,
            notes: None,
            terms: None,
            ..invoice()
        };
        let text = render_email_text(
            &inv,
            &[item("Consulting", 1.0, 100.0)],
            &summary(100.0, 0.0),
            "",
            "Invoice #1248",
            "https://x.test/i/t/index.html",
        );
        assert!(text.contains("Consulting"), "not vacuous: {text}");
        for absence in ["Due Date", "Notes", "Terms", "Payment:", "Paid"] {
            assert!(!text.contains(absence), "{absence} survived: {text}");
        }
    }

    #[test]
    fn a_non_usd_invoice_names_its_currency() {
        let mut inv = invoice();
        inv.currency = "EUR".into();
        let text = render_email_text(
            &inv,
            &[item("Consulting", 1.0, 100.0)],
            &summary(100.0, 0.0),
            "",
            "Invoice #1248",
            "https://x.test/i/t/index.html",
        );
        assert!(
            text.contains("Consulting  1 x EUR 100.00  EUR 100.00"),
            "got: {text}"
        );
        assert!(text.contains("Total: EUR 100.00"), "got: {text}");
        assert!(!text.contains('$'), "got: {text}");
    }

    #[test]
    fn the_body_is_plain_text_with_no_markup() {
        let text = render_email_text(
            &invoice(),
            &[item("A <fancy> item & co", 1.0, 100.0)],
            &summary(100.0, 0.0),
            "",
            "Invoice #1248",
            "https://x.test/i/t/index.html",
        );
        // No escaping and no tags of our own: the text goes into a text/plain
        // part, where `&amp;` would read literally.
        assert!(text.contains("A <fancy> item & co"), "got: {text}");
        assert!(!text.contains("&amp;"), "got: {text}");
        assert!(!text.contains("<p>"), "got: {text}");
    }
}
