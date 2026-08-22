//! Putting a corrected page back where the client is looking.
//!
//! A published invoice is a static snapshot on R2, rendered once at send. When a
//! payment lands — `nigel invoice pay`, the launch sync, `nigel invoice sync`,
//! `POST /api/invoices/{n}/pay`, the TUI's `p` — the page a client bookmarked
//! still shows the balance they have already settled, and still offers the
//! button that charges it.
//!
//! This is [`crate::invoicing::void`]'s problem with a different verb, so it has
//! void's shape: **the write commits first**, nothing out here can undo it, and
//! every way it can go wrong is a variant and a sentence rather than an error. A
//! payment is recorded money; nothing about an upload may read as its failure.

use rusqlite::Connection;

use crate::invoicing::gateway::AssetPublisher;
use crate::invoicing::invoices::is_void;
use crate::invoicing::render::{pay_button_for, render_with_logo, usable_logo};
use crate::invoicing::render_html::Branding;
use crate::models::{Client, Invoice};

/// How a republish went. `NotApplicable` is the ordinary case: most payments
/// land on invoices that were never published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Republished {
    /// Never published, or void — there is no live page to correct.
    NotApplicable,
    /// There is a live page and no publisher configured to reach it.
    Skipped,
    /// The page was rewritten. `pdf` says whether the attachment beside it was
    /// rewritten too — a build without the `pdf` feature can only do the page.
    Done { pdf: bool },
    /// The upstream's own message, kept verbatim.
    Failed(String),
}

/// A payment that was recorded, and what it managed to correct.
#[derive(Debug, Clone)]
pub struct RepublishOutcome {
    pub number: i64,
    pub page: Republished,
    /// The letterhead logo's own sentence, when publishing it beside the page
    /// did not work. Separate from `page` because it is a different fact: the
    /// corrected page went up, it just carries the image inline.
    pub logo: Option<String>,
}

impl RepublishOutcome {
    /// The sentences every front end prints, in one place so a terminal and a
    /// browser describe the same republish identically. Empty when nothing needs
    /// saying, which is the ordinary case.
    pub fn warnings(&self) -> Vec<String> {
        let page = match &self.page {
            Republished::Skipped => vec![format!(
                "Warning: invoice #{} was paid but the R2 publisher is not configured, so its \
                 published page still shows the old balance.",
                self.number
            )],
            Republished::Failed(message) => vec![format!(
                "Warning: could not republish invoice #{}'s page ({message}). It still shows the \
                 old balance.",
                self.number
            )],
            Republished::NotApplicable | Republished::Done { .. } => Vec::new(),
        };
        page.into_iter().chain(self.logo.clone()).collect()
    }
}

/// Re-render a published invoice and put it back where it was.
///
/// Infallible by construction: the payment is already recorded and committed,
/// and nothing out here may read as a failed payment. Every way this can go
/// wrong — a broken custom template, an R2 outage, a database read — becomes a
/// [`Republished`] variant and a sentence.
///
/// A void invoice is `NotApplicable`: void has already replaced the page with
/// its own notice, and `record_payment` refuses a void invoice anyway, so this
/// branch is defence rather than a live path.
pub fn republish_invoice<P: AssetPublisher>(
    conn: &Connection,
    invoice: &Invoice,
    client: &Client,
    branding: &Branding<'_>,
    publisher: Option<&P>,
) -> RepublishOutcome {
    // Dispatched before anything is rendered, so the ordinary case — a payment
    // against an invoice that was never published — costs nothing at all.
    let (page, logo) = match (invoice.published_at.as_deref(), is_void(invoice), publisher) {
        (None, _, _) | (_, true, _) => (Republished::NotApplicable, None),
        (Some(_), false, None) => (Republished::Skipped, None),
        (Some(_), false, Some(publisher)) => publish(conn, invoice, client, branding, publisher),
    };
    RepublishOutcome {
        number: invoice.number,
        page,
        logo,
    }
}

fn publish<P: AssetPublisher>(
    conn: &Connection,
    invoice: &Invoice,
    client: &Client,
    branding: &Branding<'_>,
    publisher: &P,
) -> (Republished, Option<String>) {
    // Reached once for this document, as in `send`: the address the page points
    // at is derived from the bytes, and reaching the verdict decodes the image.
    let logo = usable_logo(branding.logo);
    let pending = crate::invoicing::logo::pending(logo.as_ref(), publisher);
    // Rendered through the same seam `send` publishes through, with the same
    // pay-button rule, so the corrected page is the page a re-send would make —
    // including no Pay button once the invoice is settled.
    let render = |branding: &Branding<'_>| {
        render_with_logo(
            conn,
            invoice,
            client,
            pay_button_for(invoice),
            branding,
            logo.as_ref(),
        )
    };

    let mut rendered = match render(&branding.with_logo_url(pending.url())) {
        Ok(rendered) => rendered,
        // Nothing has been uploaded: a republish that could not render writes
        // nothing at all, which is what makes a broken template cost the bucket
        // no object.
        Err(e) => return (Republished::Failed(e.to_string()), None),
    };

    // Only now, and in the ordinary case a no-op: the send already put this
    // image up, and the record says so.
    let logo_warning = crate::invoicing::logo::publish(conn, &pending, publisher);
    if logo_warning.is_some() {
        // The corrected page may only point at an object that is there.
        if let Ok(inline) = render(branding) {
            rendered = inline;
        }
    }

    // With the `pdf` feature both artifacts go back, so the attachment a client
    // saved and the page they bookmarked agree. Without it the page is corrected
    // and the PDF is left as the document that was actually emailed — the rule
    // void already follows.
    let (result, pdf) = match &rendered.pdf {
        Some(pdf) => (
            publisher.publish(&invoice.token, rendered.html.as_bytes(), pdf),
            true,
        ),
        None => (
            publisher.publish_page(&invoice.token, rendered.html.as_bytes()),
            false,
        ),
    };
    let page = match result {
        Ok(_) => Republished::Done { pdf },
        Err(e) => Republished::Failed(e.to_string()),
    };
    (page, logo_warning)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::error::{NigelError, Result};
    use crate::invoicing::clients::{add_client, get_client};
    use crate::invoicing::invoices::{
        create_invoice, get_invoice, mark_published, record_payment, set_payment_link, NewLineItem,
    };
    use crate::invoicing::render_html::DEFAULT_TEMPLATE;
    use crate::migrations::run_migrations;
    use std::cell::RefCell;

    fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    fn seed(conn: &Connection) -> i64 {
        let cid = add_client(conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem {
            description: "W".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        create_invoice(conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap()
    }

    /// A sent invoice: a payment link and a page a client can reach.
    fn seed_sent(conn: &Connection) -> i64 {
        let id = seed(conn);
        set_payment_link(conn, id, "plink_1", "https://buy.stripe.com/x").unwrap();
        mark_published(conn, id, "2026-08-04").unwrap();
        id
    }

    fn brand() -> Branding<'static> {
        Branding {
            company: "Bluepeak LLC",
            contact_email: "billing@example.test",
            ..Branding::with_template(DEFAULT_TEMPLATE)
        }
    }

    fn republish<P: AssetPublisher>(
        conn: &Connection,
        id: i64,
        publisher: Option<&P>,
    ) -> RepublishOutcome {
        let invoice = get_invoice(conn, id).unwrap();
        let client = get_client(conn, invoice.client_id).unwrap();
        republish_invoice(conn, &invoice, &client, &brand(), publisher)
    }

    #[derive(Default)]
    struct CapturePub {
        pages: RefCell<Vec<(String, String)>>,
        pairs: RefCell<Vec<String>>,
        logos: RefCell<Vec<Vec<u8>>>,
    }
    impl AssetPublisher for CapturePub {
        fn publish(&self, token: &str, html: &[u8], _pdf: &[u8]) -> Result<String> {
            self.pairs.borrow_mut().push(token.to_string());
            self.pages
                .borrow_mut()
                .push((token.to_string(), String::from_utf8(html.to_vec()).unwrap()));
            Ok(format!("https://billing.example.test/i/{token}/index.html"))
        }
        fn publish_page(&self, token: &str, html: &[u8]) -> Result<String> {
            self.pages
                .borrow_mut()
                .push((token.to_string(), String::from_utf8(html.to_vec()).unwrap()));
            Ok(format!("https://billing.example.test/i/{token}/index.html"))
        }
        fn public_base(&self) -> &str {
            "https://billing.example.test/i"
        }
        fn publish_logo(&self, bytes: &[u8], mime: &str) -> Result<String> {
            self.logos.borrow_mut().push(bytes.to_vec());
            Ok(self.logo_url(bytes, mime))
        }
    }

    struct FailPub;
    impl AssetPublisher for FailPub {
        fn publish(&self, _t: &str, _h: &[u8], _p: &[u8]) -> Result<String> {
            Err(NigelError::Other(
                "r2 403: <Error><Code>SignatureDoesNotMatch</Code></Error>".into(),
            ))
        }
        fn publish_page(&self, _t: &str, _h: &[u8]) -> Result<String> {
            Err(NigelError::Other(
                "r2 403: <Error><Code>SignatureDoesNotMatch</Code></Error>".into(),
            ))
        }
        fn public_base(&self) -> &str {
            "https://billing.example.test/i"
        }
        fn publish_logo(&self, _bytes: &[u8], _mime: &str) -> Result<String> {
            Err(NigelError::Other(
                "r2 403: <Error><Code>SignatureDoesNotMatch</Code></Error>".into(),
            ))
        }
    }

    #[test]
    fn an_unpublished_invoice_is_not_applicable_and_uploads_nothing() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        record_payment(&conn, id, 40.0, "2026-08-05", "other", None, "2026-08-05").unwrap();
        let publisher = CapturePub::default();

        let outcome = republish(&conn, id, Some(&publisher));

        assert_eq!(outcome.page, Republished::NotApplicable);
        assert!(outcome.warnings().is_empty());
        assert!(publisher.pages.borrow().is_empty(), "nothing was uploaded");
    }

    #[test]
    fn a_published_invoice_is_re_rendered_and_re_uploaded() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        record_payment(&conn, id, 40.0, "2026-08-05", "other", None, "2026-08-05").unwrap();
        let token = get_invoice(&conn, id).unwrap().token;
        let publisher = CapturePub::default();

        let outcome = republish(&conn, id, Some(&publisher));

        assert!(outcome.warnings().is_empty(), "{:?}", outcome.warnings());
        let pages = publisher.pages.borrow();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].0, token);
        assert!(pages[0].1.contains("Balance due"), "got: {}", pages[0].1);
        assert!(pages[0].1.contains("60.00"), "got: {}", pages[0].1);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn the_pdf_beside_the_page_is_replaced_too() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        record_payment(&conn, id, 40.0, "2026-08-05", "other", None, "2026-08-05").unwrap();
        let publisher = CapturePub::default();

        let outcome = republish(&conn, id, Some(&publisher));

        assert_eq!(outcome.page, Republished::Done { pdf: true });
        assert_eq!(
            publisher.pairs.borrow().len(),
            1,
            "publish(), not just the page"
        );
    }

    /// A page republished after the final payment offers nothing to pay, which
    /// is the whole reason the pay-button rule moved below the seam.
    #[test]
    fn a_settled_invoice_is_republished_without_its_pay_button() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        record_payment(&conn, id, 100.0, "2026-08-05", "other", None, "2026-08-05").unwrap();
        let publisher = CapturePub::default();

        republish(&conn, id, Some(&publisher));

        let pages = publisher.pages.borrow();
        assert!(!pages[0].1.contains("Pay online"), "got: {}", pages[0].1);
        assert!(
            !pages[0].1.contains("buy.stripe.com"),
            "got: {}",
            pages[0].1
        );
    }

    #[test]
    fn a_void_invoice_is_not_applicable() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        crate::invoicing::invoices::void_invoice(&conn, id, "2026-08-06").unwrap();
        let publisher = CapturePub::default();

        let outcome = republish(&conn, id, Some(&publisher));

        assert_eq!(outcome.page, Republished::NotApplicable);
        assert!(
            publisher.pages.borrow().is_empty(),
            "void already replaced the page with its own notice"
        );
    }

    #[test]
    fn no_publisher_is_skipped_and_warns_that_the_page_is_stale() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        record_payment(&conn, id, 40.0, "2026-08-05", "other", None, "2026-08-05").unwrap();

        let outcome = republish(&conn, id, None::<&CapturePub>);

        assert_eq!(outcome.page, Republished::Skipped);
        let warnings = outcome.warnings();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("#1248"), "{warnings:?}");
        assert!(warnings[0].contains("old balance"), "{warnings:?}");
    }

    #[test]
    fn a_failed_upload_keeps_the_upstreams_own_words() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        record_payment(&conn, id, 40.0, "2026-08-05", "other", None, "2026-08-05").unwrap();

        let outcome = republish(&conn, id, Some(&FailPub));

        assert!(matches!(outcome.page, Republished::Failed(_)));
        let warnings = outcome.warnings().join("\n");
        assert!(warnings.contains("SignatureDoesNotMatch"), "{warnings}");
        assert!(warnings.contains("#1248"), "{warnings}");
        // The payment is money that was received; nothing here undoes it.
        assert_eq!(
            crate::invoicing::invoices::paid_amount(&conn, id).unwrap(),
            40.0
        );
    }

    #[test]
    fn republishing_writes_nothing_to_the_invoice() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        record_payment(&conn, id, 40.0, "2026-08-05", "other", None, "2026-08-05").unwrap();
        let before = get_invoice(&conn, id).unwrap();

        republish(&conn, id, Some(&CapturePub::default()));

        let after = get_invoice(&conn, id).unwrap();
        assert_eq!(after.status, before.status);
        assert_eq!(after.published_at, before.published_at);
        assert_eq!(
            after.stripe_payment_link_url,
            before.stripe_payment_link_url
        );
    }

    /// The fallback path, reachable only in the build that has no PDF to make.
    #[cfg(not(feature = "pdf"))]
    #[test]
    fn without_the_pdf_feature_only_the_page_is_replaced_and_nothing_warns() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        record_payment(&conn, id, 40.0, "2026-08-05", "other", None, "2026-08-05").unwrap();
        let publisher = CapturePub::default();

        let outcome = republish(&conn, id, Some(&publisher));

        assert_eq!(outcome.page, Republished::Done { pdf: false });
        assert_eq!(publisher.pages.borrow().len(), 1);
        assert!(
            publisher.pairs.borrow().is_empty(),
            "publish_page, leaving the attachment the client was actually sent"
        );
        assert!(
            outcome.warnings().is_empty(),
            "the page is corrected; nothing is wrong"
        );
    }

    // ---------------------------------------------------------------------
    // The letterhead logo
    // ---------------------------------------------------------------------

    /// A logo both documents can draw. Only such a logo is ever published.
    #[cfg(feature = "pdf")]
    fn logo_uri() -> String {
        crate::pdf::logo_uri(400, 60)
    }

    /// The corrected page is the page a re-send would make, logo included — so
    /// a client who follows their bookmark after paying sees the same document
    /// they were emailed, not one that lost its letterhead.
    #[cfg(feature = "pdf")]
    #[test]
    fn a_republished_page_points_at_the_hosted_logo() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        record_payment(&conn, id, 40.0, "2026-08-05", "other", None, "2026-08-05").unwrap();
        let publisher = CapturePub::default();
        let uri = logo_uri();
        let branding = Branding {
            logo: &uri,
            ..brand()
        };
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        let outcome = republish_invoice(&conn, &invoice, &client, &branding, Some(&publisher));

        assert!(outcome.warnings().is_empty(), "{:?}", outcome.warnings());
        let uploaded = publisher.logos.borrow().clone();
        assert_eq!(uploaded.len(), 1);
        let hosted = publisher.logo_url(&uploaded[0], "image/png");
        let pages = publisher.pages.borrow();
        assert!(
            pages[0].1.contains(&format!("src=\"{hosted}\"")),
            "got: {}",
            pages[0].1
        );
    }

    /// A republish that cannot put the logo up still corrects the page, and the
    /// page still shows the mark — the payment is money already received, and
    /// nothing about an image may read as its failure.
    #[cfg(feature = "pdf")]
    #[test]
    fn a_failed_logo_upload_still_republishes_the_corrected_page() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        record_payment(&conn, id, 40.0, "2026-08-05", "other", None, "2026-08-05").unwrap();
        let publisher = LogoRefusingPub::default();
        let uri = logo_uri();
        let branding = Branding {
            logo: &uri,
            ..brand()
        };
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        let outcome = republish_invoice(&conn, &invoice, &client, &branding, Some(&publisher));

        assert!(matches!(outcome.page, Republished::Done { .. }));
        let warnings = outcome.warnings();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].contains("logo bucket is on fire"),
            "{warnings:?}"
        );
        let pages = publisher.pages.borrow();
        assert!(
            pages[0].1.contains("data:image/png;base64"),
            "the page carries the bytes instead: {}",
            pages[0].1
        );
    }

    /// **A republish that cannot render writes nothing at all.** The upload waits
    /// for the render, so a broken custom template costs the bucket no object —
    /// and the page a client is looking at is left exactly as it was.
    #[cfg(feature = "pdf")]
    #[test]
    fn a_republish_that_cannot_render_uploads_nothing() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        record_payment(&conn, id, 40.0, "2026-08-05", "other", None, "2026-08-05").unwrap();
        let publisher = CapturePub::default();
        let uri = logo_uri();
        let branding = Branding {
            logo: &uri,
            ..brand()
        };
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();
        // Any render failure will do; this is the one a test can cause without
        // a decoder or a filesystem. `render_with_logo` reads the line items
        // before it draws anything.
        conn.execute("DROP TABLE invoice_line_items", []).unwrap();

        let outcome = republish_invoice(&conn, &invoice, &client, &branding, Some(&publisher));

        assert!(matches!(outcome.page, Republished::Failed(_)));
        assert!(
            publisher.logos.borrow().is_empty(),
            "the logo waits for a render that never succeeded"
        );
        assert!(
            publisher.pages.borrow().is_empty(),
            "and so does the page itself"
        );
        assert!(
            crate::db::get_metadata(&conn, crate::invoicing::logo::PUBLISHED_LOGO_KEY).is_none(),
            "nothing was published, so nothing is recorded"
        );
    }

    #[derive(Default)]
    struct LogoRefusingPub {
        pages: RefCell<Vec<(String, String)>>,
    }
    impl AssetPublisher for LogoRefusingPub {
        fn publish(&self, token: &str, html: &[u8], _pdf: &[u8]) -> Result<String> {
            self.publish_page(token, html)
        }
        fn publish_page(&self, token: &str, html: &[u8]) -> Result<String> {
            self.pages
                .borrow_mut()
                .push((token.to_string(), String::from_utf8(html.to_vec()).unwrap()));
            Ok(format!("https://billing.example.test/i/{token}/index.html"))
        }
        fn public_base(&self) -> &str {
            "https://billing.example.test/i"
        }
        fn publish_logo(&self, _bytes: &[u8], _mime: &str) -> Result<String> {
            Err(NigelError::Other("r2 503: logo bucket is on fire".into()))
        }
    }
}
