use rusqlite::Connection;

use crate::error::Result;
use crate::invoicing::document::MoneySummary;
use crate::invoicing::invoices::{line_items, paid_amount};
use crate::invoicing::render_html::{render_invoice_html, Branding, PayButton};
use crate::models::{Client, Invoice, InvoiceLineItem};

/// Everything `invoice send` publishes for one invoice.
pub struct RenderedInvoice {
    pub html: String,
    /// `None` only in a build without the `pdf` feature; each caller decides
    /// whether that is fatal (`send`) or a notice (`preview`).
    pub pdf: Option<Vec<u8>>,
}

/// Render an invoice exactly as `send` publishes it. Reads the database, makes
/// no network call, and writes nothing — the one seam `send` and `preview`
/// share, so a preview cannot disagree with what a client receives.
pub fn render_invoice(
    conn: &Connection,
    invoice: &Invoice,
    client: &Client,
    pay: PayButton<'_>,
    branding: &Branding<'_>,
) -> Result<RenderedInvoice> {
    // Loaded here rather than passed in, so both callers get the same rows in
    // the same order — and, for the same reason, the money block is built here
    // rather than by whoever is rendering: every caller above the seam shows
    // the same figures without asking for them.
    let items = line_items(conn, invoice.id)?;
    let money = MoneySummary::of(invoice, paid_amount(conn, invoice.id)?);
    let html = render_invoice_html(branding, invoice, client, &items, &money, pay);
    let pdf = render_pdf(invoice, client, &items, branding.company, &money)?;
    Ok(RenderedInvoice { html, pdf })
}

#[cfg(feature = "pdf")]
fn render_pdf(
    invoice: &Invoice,
    client: &Client,
    items: &[InvoiceLineItem],
    company: &str,
    money: &MoneySummary,
) -> Result<Option<Vec<u8>>> {
    crate::pdf::render_invoice_pdf(invoice, client, items, company, money).map(Some)
}

#[cfg(not(feature = "pdf"))]
fn render_pdf(
    _invoice: &Invoice,
    _client: &Client,
    _items: &[InvoiceLineItem],
    _company: &str,
    _money: &MoneySummary,
) -> Result<Option<Vec<u8>>> {
    Ok(None)
}

#[cfg(test)]
mod tests {
    use crate::db::{get_connection, init_db};
    use crate::invoicing::clients::{add_client, get_client};
    use crate::invoicing::invoices::{create_invoice, get_invoice, NewLineItem};
    use crate::invoicing::render::render_invoice;
    use crate::invoicing::render_html::{Branding, PayButton, DEFAULT_TEMPLATE};
    use crate::migrations::run_migrations;

    fn brand(contact_email: &str) -> Branding<'_> {
        Branding {
            template: DEFAULT_TEMPLATE,
            company: "",
            contact_email,
        }
    }

    fn test_conn() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    fn seed(conn: &rusqlite::Connection, items: &[NewLineItem]) -> i64 {
        let cid = add_client(conn, "Acme", Some("ap@acme.test"), None, None).unwrap();
        create_invoice(conn, cid, "2026-08-04", None, "USD", items, None, None).unwrap()
    }

    fn one_item() -> Vec<NewLineItem> {
        vec![NewLineItem {
            description: "Consulting".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }]
    }

    #[test]
    fn renders_the_html_send_would_publish() {
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        let out = render_invoice(
            &conn,
            &invoice,
            &client,
            PayButton::Placeholder,
            &brand("ap@acme.test"),
        )
        .unwrap();

        assert!(out.html.contains("Invoice #1248"), "got: {}", out.html);
        assert!(out.html.contains("Contact ap@acme.test"));
        assert!(out.html.contains("100.00"));
    }

    #[test]
    fn the_supplied_template_is_what_the_seam_renders() {
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        let branding = Branding {
            template: "<p>CUSTOM {{NUMBER}} {{CLIENT}} {{ROWS}} {{TOTAL}}</p>",
            company: "",
            contact_email: "b@e.test",
        };
        let out = render_invoice(&conn, &invoice, &client, PayButton::Omitted, &branding).unwrap();

        assert!(out.html.starts_with("<p>CUSTOM 1248"), "got: {}", out.html);
        assert!(!out.html.contains("Direct deposit"));
    }

    #[test]
    fn line_items_come_from_the_database_in_position_order() {
        let (_d, conn) = test_conn();
        let items = vec![
            NewLineItem {
                description: "First".into(),
                quantity: 1.0,
                unit_amount: 10.0,
            },
            NewLineItem {
                description: "Second".into(),
                quantity: 1.0,
                unit_amount: 20.0,
            },
            NewLineItem {
                description: "Third".into(),
                quantity: 1.0,
                unit_amount: 30.0,
            },
        ];
        let id = seed(&conn, &items);
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        let out = render_invoice(
            &conn,
            &invoice,
            &client,
            PayButton::Omitted,
            &brand("b@e.test"),
        )
        .unwrap();

        let at = |needle: &str| out.html.find(needle).expect("line item missing from html");
        assert!(at("First") < at("Second"));
        assert!(at("Second") < at("Third"));
    }

    /// The figures come from below the seam, so preview, send, the API preview
    /// routes and a republish all show them without any of them asking.
    #[test]
    fn the_seam_reads_the_payments_and_the_page_shows_them() {
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        crate::invoicing::invoices::record_payment(&conn, id, 40.0, "2026-08-05", "other", None)
            .unwrap();
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        let out = render_invoice(
            &conn,
            &invoice,
            &client,
            PayButton::Omitted,
            &brand("b@e.test"),
        )
        .unwrap();

        assert!(out.html.contains("Paid"), "got: {}", out.html);
        assert!(out.html.contains("Balance due"), "got: {}", out.html);
        assert!(out.html.contains("60.00"), "got: {}", out.html);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn the_pdf_and_the_page_carry_the_same_money_labels() {
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        crate::invoicing::invoices::record_payment(&conn, id, 40.0, "2026-08-05", "other", None)
            .unwrap();
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        let out = render_invoice(
            &conn,
            &invoice,
            &client,
            PayButton::Omitted,
            &brand("b@e.test"),
        )
        .unwrap();
        let text = crate::pdf::extract_text(&out.pdf.expect("a pdf"));

        for label in ["Total", "Paid", "Balance due"] {
            assert!(out.html.contains(label), "{label} missing from the page");
            assert!(text.contains(label), "{label} missing from the pdf: {text}");
        }
    }

    /// Not just the same labels — the same strings. These rows exist on both
    /// documents for the first time in this change, so nothing forces them
    /// apart and a client comparing the two sees one figure.
    #[cfg(feature = "pdf")]
    #[test]
    fn the_pdf_and_the_page_render_the_payment_rows_identically() {
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        crate::invoicing::invoices::record_payment(&conn, id, 40.0, "2026-08-05", "other", None)
            .unwrap();
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        let out = render_invoice(
            &conn,
            &invoice,
            &client,
            PayButton::Omitted,
            &brand("b@e.test"),
        )
        .unwrap();
        let text = crate::pdf::extract_text(&out.pdf.expect("a pdf"));

        for figure in ["USD 40.00", "USD 60.00"] {
            assert!(out.html.contains(figure), "{figure} missing from the page");
            assert!(
                text.contains(figure),
                "{figure} missing from the pdf: {text}"
            );
        }
    }

    #[test]
    fn rendering_writes_nothing_to_the_invoice() {
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        render_invoice(
            &conn,
            &invoice,
            &client,
            PayButton::Placeholder,
            &brand("b@e.test"),
        )
        .unwrap();

        let after = get_invoice(&conn, id).unwrap();
        assert_eq!(after.status, "draft");
        assert!(after.published_at.is_none());
        assert!(after.stripe_payment_link_url.is_none());
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn pdf_is_rendered_when_the_feature_is_on() {
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        let out = render_invoice(
            &conn,
            &invoice,
            &client,
            PayButton::Omitted,
            &brand("b@e.test"),
        )
        .unwrap();
        assert!(out.pdf.unwrap().starts_with(b"%PDF"));
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn the_pdf_carries_the_same_company_the_html_does() {
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        let branding = Branding {
            template: "<h1>{{COMPANY}}</h1>{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}",
            company: "Bluepeak LLC",
            contact_email: "b@e.test",
        };
        let out = render_invoice(&conn, &invoice, &client, PayButton::Omitted, &branding).unwrap();

        assert!(out.html.contains("<h1>Bluepeak LLC</h1>"));
        let text = crate::pdf::extract_text(&out.pdf.unwrap());
        assert!(text.contains("Bluepeak LLC"), "got: {text}");
    }

    #[cfg(not(feature = "pdf"))]
    #[test]
    fn pdf_is_none_without_the_feature_and_html_still_renders() {
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        let out = render_invoice(
            &conn,
            &invoice,
            &client,
            PayButton::Omitted,
            &brand("b@e.test"),
        )
        .unwrap();
        assert!(out.pdf.is_none());
        assert!(out.html.contains("Invoice #1248"));
    }
}
