use rusqlite::Connection;

use crate::error::Result;
use crate::invoicing::document::{company_block, parse_logo, CompanyBlock, Logo, MoneySummary};
use crate::invoicing::invoices::{is_void, line_items, paid_amount};
use crate::invoicing::render_html::{render_invoice_html, Branding, PayButton};
use crate::models::{Client, Invoice, InvoiceLineItem, InvoiceStatus};

/// Which pay element an invoice renders, wherever it is rendered.
///
/// Void and paid-in-full both omit it: an invoice that is settled or cancelled
/// must not offer a working payment link, and a page republished after the last
/// payment is exactly the moment that becomes reachable. It lives beside the
/// seam rather than in the CLI layer because `preview`, `send` and a republish
/// must all reach the same answer for the same invoice.
pub fn pay_button_for(invoice: &Invoice) -> PayButton<'_> {
    // A voided or settled invoice can still carry a live Stripe URL. Rendering
    // a working Pay button on either is the one way rendering could cost
    // someone money.
    if is_void(invoice) || invoice.status == InvoiceStatus::Paid.as_str() {
        return PayButton::Omitted;
    }
    match invoice.stripe_payment_link_url.as_deref() {
        Some(url) => PayButton::Link(url),
        None => PayButton::Placeholder,
    }
}

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

    // Whether there is a usable logo is decided **here**, once, and both
    // documents are then rendered from that one answer.
    //
    // `parse_logo` is everything a check can establish without a decoder, and it
    // holds in every build. `logo_is_embeddable` is the rest, and it only exists
    // where the `pdf` feature brought a decoder in. A value that fails either is
    // erased from the branding the page is rendered from, so the page is handed
    // no logo rather than asked to reach the same verdict a second time — which
    // is what stops a file the page would display and the PDF would refuse.
    // Neither refusal is an error: a logo never fails an invoice.
    let logo = parse_logo(branding.logo).ok().flatten();
    #[cfg(feature = "pdf")]
    let logo = logo.filter(crate::pdf::logo_is_embeddable);
    let branding = &Branding {
        logo: if logo.is_some() { branding.logo } else { "" },
        template: branding.template,
        company: branding.company,
        company_address: branding.company_address,
        company_phone: branding.company_phone,
        payment_instructions: branding.payment_instructions,
        contact_email: branding.contact_email,
    };

    let html = render_invoice_html(branding, invoice, client, &items, &money, pay);

    // The From block is decided once, above both renderers, for the same
    // reason: the page and the attachment agree by construction, not by review.
    let company = company_block(
        branding.company,
        branding.company_address,
        branding.company_phone,
    );
    let pdf = render_pdf(
        invoice,
        client,
        &company,
        logo.as_ref(),
        &items,
        &money,
        branding.payment_instructions,
    )?;
    Ok(RenderedInvoice { html, pdf })
}

#[cfg(feature = "pdf")]
#[allow(clippy::too_many_arguments)]
fn render_pdf(
    invoice: &Invoice,
    client: &Client,
    company: &CompanyBlock<'_>,
    logo: Option<&Logo>,
    items: &[InvoiceLineItem],
    money: &MoneySummary,
    payment_instructions: &str,
) -> Result<Option<Vec<u8>>> {
    crate::pdf::render_invoice_pdf(
        invoice,
        client,
        company,
        logo,
        items,
        money,
        payment_instructions,
    )
    .map(Some)
}

#[cfg(not(feature = "pdf"))]
fn render_pdf(
    _invoice: &Invoice,
    _client: &Client,
    _company: &CompanyBlock<'_>,
    _logo: Option<&Logo>,
    _items: &[InvoiceLineItem],
    _money: &MoneySummary,
    _payment_instructions: &str,
) -> Result<Option<Vec<u8>>> {
    Ok(None)
}

#[cfg(test)]
mod tests {
    use crate::db::{get_connection, init_db};
    use crate::invoicing::clients::{add_client, get_client};
    use crate::invoicing::invoices::{
        create_invoice, get_invoice, record_payment, set_payment_link, NewLineItem,
    };
    use crate::invoicing::render::render_invoice;
    use crate::invoicing::render_html::{Branding, PayButton, DEFAULT_TEMPLATE};
    use crate::migrations::run_migrations;

    fn brand(contact_email: &str) -> Branding<'_> {
        Branding {
            company: "",
            contact_email,
            ..Branding::with_template(DEFAULT_TEMPLATE)
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

        // The number is in the tab title and in the metadata band; there is no
        // heading line on the document any more.
        assert!(
            out.html.contains("<title>Invoice 1248</title>"),
            "got: {}",
            out.html
        );
        assert!(out.html.contains("Invoice ID"), "got: {}", out.html);
        assert!(out.html.contains("Acme"), "got: {}", out.html);
        assert!(out.html.contains("100.00"));
    }

    #[test]
    fn the_supplied_template_is_what_the_seam_renders() {
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        let branding = Branding {
            company: "",
            contact_email: "b@e.test",
            ..Branding::with_template("<p>CUSTOM {{NUMBER}} {{CLIENT}} {{ROWS}} {{TOTAL}}</p>")
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
    fn the_pdf_and_the_page_render_every_figure_identically() {
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

        // The money block and the line items both, in one style: the page used
        // to print a bare `100.00` in its table where this document printed
        // `$100.00`, and the block mixed `$` rows with `USD` rows.
        for figure in ["$40.00", "$60.00", "$100.00"] {
            assert!(out.html.contains(figure), "{figure} missing from the page");
            assert!(
                text.contains(figure),
                "{figure} missing from the pdf: {text}"
            );
        }
        assert!(!out.html.contains("USD"), "no code on a dollar page");
        assert!(!text.contains("USD"), "nor on the attachment: {text}");
    }

    /// The non-USD case both documents have to get right together.
    #[cfg(feature = "pdf")]
    #[test]
    fn a_non_usd_invoice_names_its_currency_on_both_documents() {
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        conn.execute("UPDATE invoices SET currency = 'EUR' WHERE id = ?1", [id])
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

        assert!(out.html.contains("EUR 100.00"), "got: {}", out.html);
        assert!(text.contains("EUR 100.00"), "got: {text}");
        assert!(!out.html.contains('$'), "no dollar sign on the page");
        assert!(!text.contains('$'), "nor on the attachment: {text}");
    }

    #[test]
    fn a_settled_invoice_never_renders_a_pay_button() {
        use crate::invoicing::render::pay_button_for;
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        set_payment_link(&conn, id, "pl_1", "https://pay/x").unwrap();
        record_payment(&conn, id, 100.0, "2026-08-05", "other", None).unwrap();
        let invoice = get_invoice(&conn, id).unwrap();

        assert_eq!(invoice.status, "paid");
        assert!(
            matches!(pay_button_for(&invoice), PayButton::Omitted),
            "a settled invoice must not offer a working payment link"
        );
    }

    #[test]
    fn a_partly_paid_invoice_keeps_its_link() {
        use crate::invoicing::render::pay_button_for;
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        set_payment_link(&conn, id, "pl_1", "https://pay/x").unwrap();
        record_payment(&conn, id, 40.0, "2026-08-05", "other", None).unwrap();
        let invoice = get_invoice(&conn, id).unwrap();

        assert!(matches!(
            pay_button_for(&invoice),
            PayButton::Link("https://pay/x")
        ));
    }

    #[test]
    fn a_void_invoice_never_renders_a_pay_button_even_with_a_live_link() {
        use crate::invoicing::render::pay_button_for;
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        set_payment_link(&conn, id, "pl_1", "https://pay/x").unwrap();
        crate::invoicing::invoices::void_invoice(&conn, id, "2026-08-06").unwrap();
        let invoice = get_invoice(&conn, id).unwrap();

        assert!(matches!(pay_button_for(&invoice), PayButton::Omitted));
    }

    #[test]
    fn a_draft_with_no_link_gets_the_placeholder() {
        use crate::invoicing::render::pay_button_for;
        let (_d, conn) = test_conn();
        let invoice = get_invoice(&conn, seed(&conn, &one_item())).unwrap();
        assert!(matches!(pay_button_for(&invoice), PayButton::Placeholder));
    }

    #[test]
    fn a_sent_unpaid_invoice_renders_its_real_link() {
        use crate::invoicing::render::pay_button_for;
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        set_payment_link(&conn, id, "pl_1", "https://pay/x").unwrap();
        let invoice = get_invoice(&conn, id).unwrap();
        assert!(matches!(
            pay_button_for(&invoice),
            PayButton::Link("https://pay/x")
        ));
    }

    /// A void whose status write did not land: the timestamp is the fact, the
    /// same reading `ensure_not_void` takes.
    #[test]
    fn a_stale_void_status_still_omits_the_pay_button() {
        use crate::invoicing::render::pay_button_for;
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        conn.execute(
            "UPDATE invoices SET voided_at='2026-08-06', status='draft',
                                 stripe_payment_link_url='https://pay/x' WHERE id=?1",
            [id],
        )
        .unwrap();
        let invoice = get_invoice(&conn, id).unwrap();
        assert!(matches!(pay_button_for(&invoice), PayButton::Omitted));
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
            company: "Bluepeak LLC",
            contact_email: "b@e.test",
            ..Branding::with_template("<h1>{{COMPANY}}</h1>{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}")
        };
        let out = render_invoice(&conn, &invoice, &client, PayButton::Omitted, &branding).unwrap();

        assert!(out.html.contains("<h1>Bluepeak LLC</h1>"));
        let text = crate::pdf::extract_text(&out.pdf.unwrap());
        assert!(text.contains("Bluepeak LLC"), "got: {text}");
    }

    fn letterhead<'a>(logo: &'a str, payment_instructions: &'a str) -> Branding<'a> {
        Branding {
            template: DEFAULT_TEMPLATE,
            company: "Bluepeak LLC",
            company_address: "P.O. Box 1234\nSpringfield, CA 90001",
            company_phone: "619.555.0123",
            logo,
            payment_instructions,
            contact_email: "b@e.test",
        }
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn the_pdf_and_the_page_carry_the_same_company_block() {
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        let out = render_invoice(
            &conn,
            &invoice,
            &client,
            PayButton::Omitted,
            &letterhead("", ""),
        )
        .unwrap();
        let text = crate::pdf::extract_text(&out.pdf.expect("a pdf"));

        for line in [
            "Bluepeak LLC",
            "P.O. Box 1234",
            "Springfield, CA 90001",
            "619.555.0123",
        ] {
            assert!(out.html.contains(line), "{line} missing from the page");
            assert!(text.contains(line), "{line} missing from the pdf: {text}");
        }
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn the_pdf_and_the_page_carry_the_same_metadata_rows() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", Some("ap@acme.test"), None, None).unwrap();
        let id = create_invoice(
            &conn,
            cid,
            "2026-08-04",
            Some("2026-09-03"),
            "USD",
            &one_item(),
            None,
            Some("Net 30"),
        )
        .unwrap();
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        let out = render_invoice(
            &conn,
            &invoice,
            &client,
            PayButton::Omitted,
            &letterhead("", ""),
        )
        .unwrap();
        let text = crate::pdf::extract_text(&out.pdf.expect("a pdf"));

        for row in crate::invoicing::document::meta_rows(&invoice) {
            assert!(
                out.html.contains(&row.value),
                "{} missing from the page",
                row.value
            );
            assert!(
                text.contains(&row.value),
                "{} missing from the pdf: {text}",
                row.value
            );
        }
        assert!(out.html.contains("2026-09-03 (Net 30)"));
        assert!(text.contains("2026-09-03 (Net 30)"), "got: {text}");
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn a_sparse_invoice_omits_the_same_blocks_in_both_documents() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let id = create_invoice(
            &conn,
            cid,
            "2026-08-04",
            None,
            "USD",
            &one_item(),
            None,
            None,
        )
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

        for absence in ["From", "ph.", "Due Date", "Notes", "Terms", "Payment"] {
            assert!(
                !out.html.contains(absence),
                "{absence} survived on the page"
            );
            assert!(!text.contains(absence), "{absence} survived on the pdf");
        }
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn the_same_logo_reaches_both_documents() {
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();
        let uri = crate::pdf::logo_uri(400, 60);

        let out = render_invoice(
            &conn,
            &invoice,
            &client,
            PayButton::Omitted,
            &letterhead(&uri, ""),
        )
        .unwrap();

        assert!(out.html.contains(&uri), "the page carries the data URI");
        assert_eq!(
            crate::pdf::image_xobjects(&out.pdf.expect("a pdf")).len(),
            1,
            "the attachment carries the image"
        );
    }

    /// A logo may never cost an invoice. A stored value that cannot be used
    /// degrades both documents to the company name and fails neither.
    #[cfg(feature = "pdf")]
    #[test]
    fn an_unusable_logo_degrades_on_both_documents_and_fails_neither() {
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        let out = render_invoice(
            &conn,
            &invoice,
            &client,
            PayButton::Omitted,
            &letterhead("data:image/png;base64,bm90IGEgcG5n", ""),
        )
        .expect("a bad logo is not a failed render");

        assert!(!out.html.contains("<img"), "no broken image on the page");
        let text = crate::pdf::extract_text(&out.pdf.expect("a pdf"));
        assert!(
            text.contains("Bluepeak LLC"),
            "the wordmark stands in: {text}"
        );
    }

    /// A logo that passes every structural check and still will not decode is
    /// the case where the two documents could disagree: the page has no decoder
    /// to consult, so it would show a broken `<img>` beside a PDF drawing the
    /// wordmark. The seam decides once and neither document uses it.
    #[cfg(feature = "pdf")]
    #[test]
    fn a_logo_that_will_not_decode_is_dropped_from_both_documents() {
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        // A well-formed PNG wrapper — signature, `IHDR` declaring a size, and
        // the `IEND` that says the file is whole — around no image data at all.
        let mut hollow = vec![
            0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H',
            b'D', b'R', 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x08, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        hollow.extend_from_slice(&[
            0x00, 0x00, 0x00, 0x00, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82,
        ]);
        let uri = {
            use base64::Engine as _;
            format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(&hollow)
            )
        };
        assert!(
            crate::invoicing::document::parse_logo(&uri)
                .unwrap()
                .is_some(),
            "the premise: it passes every check that costs no decoder"
        );

        let out = render_invoice(
            &conn,
            &invoice,
            &client,
            PayButton::Omitted,
            &letterhead(&uri, ""),
        )
        .expect("an undecodable logo is not a failed render");

        assert!(!out.html.contains("<img"), "no broken image on the page");
        let pdf = out.pdf.expect("a pdf");
        assert!(
            crate::pdf::image_xobjects(&pdf).is_empty(),
            "and nothing embedded in the attachment"
        );
        assert!(
            crate::pdf::extract_text(&pdf).contains("Bluepeak LLC"),
            "the wordmark stands in on both"
        );
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn the_payment_instructions_reach_both_documents_or_neither() {
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        let out = render_invoice(
            &conn,
            &invoice,
            &client,
            PayButton::Omitted,
            &letterhead("", "Wells Fargo\nRouting 121000248"),
        )
        .unwrap();
        let text = crate::pdf::extract_text(&out.pdf.expect("a pdf"));
        for line in ["Payment", "Wells Fargo", "Routing 121000248"] {
            assert!(out.html.contains(line), "{line} missing from the page");
            assert!(text.contains(line), "{line} missing from the pdf: {text}");
        }

        let out = render_invoice(
            &conn,
            &invoice,
            &client,
            PayButton::Omitted,
            &letterhead("", ""),
        )
        .unwrap();
        let text = crate::pdf::extract_text(&out.pdf.expect("a pdf"));
        assert!(!out.html.contains("Payment"), "got: {}", out.html);
        assert!(!text.contains("Payment"), "got: {text}");
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
        // The number is the tab title and the metadata band; the document
        // carries no heading line.
        assert!(out.html.contains("<title>Invoice 1248</title>"));
        assert!(out.html.contains("Invoice ID"));
    }
}
