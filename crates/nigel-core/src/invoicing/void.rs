//! Void, and the teardown that follows it.
//!
//! [`crate::invoicing::invoices::void_invoice`] cancels an invoice in Nigel's
//! books. Two things it cannot reach outlive it: the Stripe payment link a
//! client was emailed, which stays chargeable, and the page published on R2,
//! which keeps offering the button that charges it. A payment taken through
//! either lands against an invoice `sync` no longer polls (`run_sync` selects
//! `sent`/`partial`/`overdue`), so it goes unrecorded — which is what this
//! module exists to prevent.
//!
//! **The teardown is best-effort, always.** The void is committed before either
//! call is made, and no failure out here rolls it back: Stripe being down is not
//! a reason for an invoice to stay open in the books. What a failure buys is a
//! sentence — the link's own URL, so an operator can kill it by hand.
//!
//! The whole matrix, per invoice:
//!
//! | Payment link on the row | `stripe_secret_key` | What void does |
//! |---|---|---|
//! | none | either | nothing, silently — there is no link to deactivate |
//! | present | configured | deactivates it; a failure warns and prints the URL |
//! | present | unset | warns and prints the URL — the link is live and chargeable whether or not this installation can reach Stripe |
//!
//! | Published | R2 + `public_base_url` | What void does |
//! |---|---|---|
//! | never published | either | nothing, silently — no page exists |
//! | published | configured | replaces index.html with the voided page; a failure warns. The PDF is left alone and the address keeps resolving |
//! | published | unset | warns: the page stays live saying the invoice is payable |
//!
//! So a void on an unpublished draft with no link — the ordinary case, and the
//! only one an unconfigured installation ever has — is silent, and every
//! sentence printed names something that is still live.

use rusqlite::Connection;

use crate::error::Result;
use crate::invoicing::gateway::{AssetPublisher, PaymentGateway};
use crate::invoicing::invoices::{get_invoice, void_invoice};
use crate::invoicing::render_html::voided_page_html;
use crate::models::Invoice;

/// What voiding a published invoice will do, said before it is done — the
/// confirmation dialog's caution and the note on a voided invoice's detail.
/// Deliberately conditional: which half runs depends on what is configured, and
/// the row itself does not record which of them did.
pub const PUBLISHED_VOID_NOTICE: &str =
    "This invoice was already published. Voiding it replaces the published page with a voided \
     notice and deactivates its Stripe payment link, wherever each of those is configured; \
     anything that is not configured stays live.";

/// What is still live after a void that could not replace the page.
pub const PUBLISHED_VOID_WARNING: &str =
    "Warning: this invoice was already published and the R2 publisher is not configured, so its \
     page stays live and still offers to take payment — take it down yourself.";

/// How one half of the teardown turned out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeardownStep {
    /// Nothing to do: no payment link, or nothing was ever published.
    NotApplicable,
    /// There is something live, and the credentials to reach it are not
    /// configured.
    Skipped,
    Done,
    /// The upstream's own message, kept verbatim — `r2 403: SignatureDoesNotMatch`
    /// is the only information anyone has about why it refused.
    Failed(String),
}

impl TeardownStep {
    /// Is there something out there that a person still has to deal with?
    fn needs_a_human(&self) -> bool {
        matches!(self, Self::Skipped | Self::Failed(_))
    }
}

/// A void that happened, and what it managed to tear down.
#[derive(Debug, Clone)]
pub struct VoidOutcome {
    pub number: i64,
    pub link: TeardownStep,
    pub page: TeardownStep,
    /// The payment link that is still live, when one is. `None` when the link
    /// was deactivated, when there never was one, or when the row carries an id
    /// without a URL.
    pub payment_link_url: Option<String>,
}

impl VoidOutcome {
    /// The sentences every front end prints, in one place so the CLI, the TUI
    /// and the browser cannot say different things about the same void. Empty
    /// when nothing is left live, which is the ordinary case.
    pub fn warnings(&self) -> Vec<String> {
        let mut out = Vec::new();
        let at = match &self.payment_link_url {
            Some(url) => format!(": {url}"),
            None => String::new(),
        };
        match &self.link {
            TeardownStep::Skipped => out.push(format!(
                "Warning: stripe_secret_key is not configured, so the Stripe payment link is still \
                 live{at} — deactivate it in Stripe yourself."
            )),
            TeardownStep::Failed(message) => out.push(format!(
                "Warning: could not deactivate the Stripe payment link ({message}). It is still \
                 live{at} — deactivate it in Stripe yourself."
            )),
            TeardownStep::NotApplicable | TeardownStep::Done => {}
        }
        match &self.page {
            TeardownStep::Skipped => out.push(PUBLISHED_VOID_WARNING.to_string()),
            TeardownStep::Failed(message) => out.push(format!(
                "Warning: could not replace the published invoice page ({message}). It stays live \
                 and still offers to take payment — take it down yourself."
            )),
            TeardownStep::NotApplicable | TeardownStep::Done => {}
        }
        out
    }
}

/// Has this invoice put anything out in the world?
///
/// `false` means the teardown is a no-op whatever is configured — no link to
/// deactivate, no page to replace — and therefore that voiding it reaches no
/// network. A front end that has to freeze itself around a call it is going to
/// make asks this first, so an ordinary draft void stays the database write it
/// always was.
pub fn has_teardown_work(invoice: &Invoice) -> bool {
    invoice.stripe_payment_link_id.is_some() || invoice.published_at.is_some()
}

/// Void an invoice and take down what it published.
///
/// The order is the safe one: `void_invoice` runs first and commits, so a
/// refusal (already void, payments recorded) costs no network call, and neither
/// Stripe nor R2 can leave the books half-cancelled. `gateway` and `publisher`
/// are `Option` because void is the one invoicing command that works on an
/// installation with nothing configured — see the module's matrix.
pub fn void_invoice_with_teardown<G: PaymentGateway, P: AssetPublisher>(
    conn: &Connection,
    invoice_id: i64,
    today: &str,
    gateway: Option<&G>,
    publisher: Option<&P>,
) -> Result<VoidOutcome> {
    // Read before the write: `void_invoice` moves the status, and the teardown
    // works from the link and the token, which it does not touch.
    let invoice = get_invoice(conn, invoice_id)?;
    // The letterhead a send published, read back rather than rebuilt: void needs
    // no configuration and uploads nothing of its own, so the notice can only
    // point at an object that is known to be there — and only while this
    // installation still serves the address it was published to.
    let logo = publisher
        .map(|publisher| publisher.public_base())
        .and_then(|base| crate::invoicing::logo::published_logo_url(conn, base));
    void_invoice(conn, invoice_id, today)?;
    Ok(teardown(&invoice, logo.as_deref(), gateway, publisher))
}

/// Everything after the commit. Infallible by construction: each half reports
/// how it went, and neither can return an error that would read as a failed
/// void.
fn teardown<G: PaymentGateway, P: AssetPublisher>(
    invoice: &Invoice,
    logo_url: Option<&str>,
    gateway: Option<&G>,
    publisher: Option<&P>,
) -> VoidOutcome {
    let link = match (invoice.stripe_payment_link_id.as_deref(), gateway) {
        (None, _) => TeardownStep::NotApplicable,
        (Some(_), None) => TeardownStep::Skipped,
        (Some(id), Some(gateway)) => match gateway.deactivate_payment_link(id) {
            Ok(()) => TeardownStep::Done,
            Err(e) => TeardownStep::Failed(e.to_string()),
        },
    };

    let page = match (invoice.published_at.as_deref(), publisher) {
        (None, _) => TeardownStep::NotApplicable,
        (Some(_), None) => TeardownStep::Skipped,
        (Some(_), Some(publisher)) => {
            let html = voided_page_html(invoice.number, logo_url);
            match publisher.publish_page(&invoice.token, html.as_bytes()) {
                Ok(_) => TeardownStep::Done,
                Err(e) => TeardownStep::Failed(e.to_string()),
            }
        }
    };

    VoidOutcome {
        number: invoice.number,
        payment_link_url: match link.needs_a_human() {
            true => invoice.stripe_payment_link_url.clone(),
            false => None,
        },
        link,
        page,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::error::NigelError;
    use crate::invoicing::clients::add_client;
    use crate::invoicing::gateway::{PaidSession, PaymentLink};
    use crate::invoicing::invoices::{
        create_invoice, mark_published, record_payment, set_payment_link, NewLineItem,
    };
    use crate::migrations::run_migrations;
    use crate::models::Client;
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

    /// A sent invoice: a payment link a client can still use, and a page.
    fn seed_sent(conn: &Connection) -> i64 {
        let id = seed(conn);
        set_payment_link(conn, id, "plink_1", "https://buy.stripe.com/x").unwrap();
        mark_published(conn, id, "2026-08-04").unwrap();
        id
    }

    #[derive(Default)]
    struct FakeGw {
        deactivated: RefCell<Vec<String>>,
    }
    impl PaymentGateway for FakeGw {
        fn create_payment_link(&self, _i: &Invoice, _c: &Client) -> Result<PaymentLink> {
            unreachable!("void creates nothing")
        }
        fn paid_sessions(&self, _id: &str) -> Result<Vec<PaidSession>> {
            Ok(vec![])
        }
        fn deactivate_payment_link(&self, id: &str) -> Result<()> {
            self.deactivated.borrow_mut().push(id.to_string());
            Ok(())
        }
    }

    /// Stripe answering the way it does when the key is wrong.
    struct FailGw;
    impl PaymentGateway for FailGw {
        fn create_payment_link(&self, _i: &Invoice, _c: &Client) -> Result<PaymentLink> {
            unreachable!("void creates nothing")
        }
        fn paid_sessions(&self, _id: &str) -> Result<Vec<PaidSession>> {
            Ok(vec![])
        }
        fn deactivate_payment_link(&self, _id: &str) -> Result<()> {
            Err(NigelError::Other(
                "stripe 401: Invalid API Key provided".into(),
            ))
        }
    }

    #[derive(Default)]
    struct FakePub {
        pages: RefCell<Vec<(String, String)>>,
        full_publishes: RefCell<u32>,
    }
    impl AssetPublisher for FakePub {
        fn publish(&self, token: &str, _h: &[u8], _p: &[u8]) -> Result<String> {
            *self.full_publishes.borrow_mut() += 1;
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
        fn publish_logo(&self, _bytes: &[u8], _mime: &str) -> Result<String> {
            unreachable!("void publishes no logo; it points at the one a send left")
        }
    }

    struct ForbiddenPub;
    impl AssetPublisher for ForbiddenPub {
        fn publish(&self, _t: &str, _h: &[u8], _p: &[u8]) -> Result<String> {
            unreachable!("void publishes no PDF")
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
            unreachable!("void publishes no logo; it points at the one a send left")
        }
    }

    /// A fully configured installation: the link goes inactive and the page is
    /// replaced with one that says so.
    #[test]
    fn voiding_a_sent_invoice_deactivates_the_link_and_republishes_the_page() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        let token = get_invoice(&conn, id).unwrap().token;
        let gateway = FakeGw::default();
        let publisher = FakePub::default();

        let outcome =
            void_invoice_with_teardown(&conn, id, "2026-08-06", Some(&gateway), Some(&publisher))
                .expect("voids");

        assert_eq!(outcome.link, TeardownStep::Done);
        assert_eq!(outcome.page, TeardownStep::Done);
        assert_eq!(outcome.payment_link_url, None);
        assert!(outcome.warnings().is_empty(), "{:?}", outcome.warnings());
        assert_eq!(*gateway.deactivated.borrow(), vec!["plink_1".to_string()]);

        let pages = publisher.pages.borrow();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].0, token);
        assert!(pages[0].1.contains("voided"), "got: {}", pages[0].1);
        assert!(pages[0].1.contains("#1248"), "got: {}", pages[0].1);
        // The PDF the client was sent is left where it is.
        assert_eq!(*publisher.full_publishes.borrow(), 0);

        assert_eq!(get_invoice(&conn, id).unwrap().status, "void");
    }

    /// TASK-105 AC-5. A void replaces the page; the letterhead survives it. The
    /// notice is still the operator's page, and the logo object it points at is
    /// the one their last send put there — nothing here uploads or deletes it.
    #[test]
    fn a_voids_republished_page_keeps_the_letterhead_logo() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        record_published_logo(&conn, "https://billing.example.test/i/logo-1a2b3c4d.png");
        let gateway = FakeGw::default();
        let publisher = FakePub::default();

        void_invoice_with_teardown(&conn, id, "2026-08-06", Some(&gateway), Some(&publisher))
            .expect("voids");

        let pages = publisher.pages.borrow();
        assert!(
            pages[0]
                .1
                .contains("<img src=\"https://billing.example.test/i/logo-1a2b3c4d.png\""),
            "got: {}",
            pages[0].1
        );
        assert!(pages[0].1.contains("voided"), "got: {}", pages[0].1);
    }

    /// The record a send leaves behind: the fingerprint and the address.
    fn record_published_logo(conn: &Connection, url: &str) {
        crate::db::set_metadata(
            conn,
            crate::invoicing::logo::PUBLISHED_LOGO_KEY,
            &format!("deadbeef {url}"),
        )
        .unwrap();
    }

    /// **A bucket move must not put a decommissioned domain on a page.** The
    /// recorded address belongs to a host this installation no longer serves, so
    /// the notice omits the image rather than pointing at it.
    #[test]
    fn a_void_after_a_bucket_move_omits_the_logo() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        record_published_logo(&conn, "https://old-bucket.example.test/i/logo-1a2b3c4d.png");
        let publisher = FakePub::default();

        void_invoice_with_teardown(
            &conn,
            id,
            "2026-08-06",
            Some(&FakeGw::default()),
            Some(&publisher),
        )
        .expect("voids");

        let pages = publisher.pages.borrow();
        assert!(
            !pages[0].1.contains("<img"),
            "the old bucket is not this installation's to link to: {}",
            pages[0].1
        );
        assert!(pages[0].1.contains("voided"), "the notice still stands");
    }

    /// **Clearing the logo clears the record**, so the next document published
    /// carries none — while the objects behind already-delivered pages stay put.
    #[test]
    fn a_void_after_the_logo_was_cleared_carries_none() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        record_published_logo(&conn, "https://billing.example.test/i/logo-1a2b3c4d.png");
        crate::invoicing::logo::set_company_logo(&conn, "").unwrap();
        let publisher = FakePub::default();

        void_invoice_with_teardown(
            &conn,
            id,
            "2026-08-06",
            Some(&FakeGw::default()),
            Some(&publisher),
        )
        .expect("voids");

        assert!(!publisher.pages.borrow()[0].1.contains("<img"));
    }

    /// An installation that never published a logo gets a notice with no image
    /// rather than one pointing at an object that is not there.
    #[test]
    fn a_void_with_no_published_logo_writes_a_page_with_no_image() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        let publisher = FakePub::default();

        void_invoice_with_teardown(
            &conn,
            id,
            "2026-08-06",
            Some(&FakeGw::default()),
            Some(&publisher),
        )
        .expect("voids");

        assert!(!publisher.pages.borrow()[0].1.contains("<img"));
    }

    /// AC #2: the void stands, and the URL is handed over for manual cleanup.
    #[test]
    fn a_stripe_failure_leaves_the_invoice_voided_and_names_the_link() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        let publisher = FakePub::default();

        let outcome =
            void_invoice_with_teardown(&conn, id, "2026-08-06", Some(&FailGw), Some(&publisher))
                .expect("the void is not rolled back");

        assert_eq!(get_invoice(&conn, id).unwrap().status, "void");
        assert!(matches!(outcome.link, TeardownStep::Failed(_)));
        assert_eq!(
            outcome.payment_link_url.as_deref(),
            Some("https://buy.stripe.com/x")
        );
        let warnings = outcome.warnings().join("\n");
        assert!(warnings.contains("https://buy.stripe.com/x"), "{warnings}");
        assert!(warnings.contains("Invalid API Key provided"), "{warnings}");
        // The other half still ran: one failure does not cancel the other.
        assert_eq!(outcome.page, TeardownStep::Done);
    }

    #[test]
    fn a_publish_failure_leaves_the_invoice_voided_and_says_the_page_is_live() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        let gateway = FakeGw::default();

        let outcome = void_invoice_with_teardown(
            &conn,
            id,
            "2026-08-06",
            Some(&gateway),
            Some(&ForbiddenPub),
        )
        .expect("the void is not rolled back");

        assert_eq!(get_invoice(&conn, id).unwrap().status, "void");
        assert_eq!(outcome.link, TeardownStep::Done);
        assert!(matches!(outcome.page, TeardownStep::Failed(_)));
        let warnings = outcome.warnings().join("\n");
        assert!(warnings.contains("SignatureDoesNotMatch"), "{warnings}");
        // Nothing about Stripe: that link is off.
        assert!(!warnings.contains("Stripe"), "{warnings}");
    }

    /// The predicate a front end freezes itself on has to agree with what the
    /// teardown actually does, or a screen promises calls that never happen.
    #[test]
    fn nothing_to_tear_down_means_both_halves_are_not_applicable() {
        let (_d, conn) = test_conn();
        let draft = get_invoice(&conn, seed(&conn)).unwrap();
        assert!(!has_teardown_work(&draft));
        let outcome = teardown(
            &draft,
            None,
            Some(&FakeGw::default()),
            Some(&FakePub::default()),
        );
        assert_eq!(outcome.link, TeardownStep::NotApplicable);
        assert_eq!(outcome.page, TeardownStep::NotApplicable);

        let (_d2, other) = test_conn();
        let sent = get_invoice(&other, seed_sent(&other)).unwrap();
        assert!(has_teardown_work(&sent));
    }

    /// A link with no page, and a page with no link, are each work.
    #[test]
    fn either_half_alone_is_teardown_work() {
        let (_d, conn) = test_conn();
        let linked = seed(&conn);
        set_payment_link(&conn, linked, "plink_1", "https://buy.stripe.com/x").unwrap();
        assert!(has_teardown_work(&get_invoice(&conn, linked).unwrap()));

        let (_d2, other) = test_conn();
        let published = seed(&other);
        mark_published(&other, published, "2026-08-04").unwrap();
        assert!(has_teardown_work(&get_invoice(&other, published).unwrap()));
    }

    /// The ordinary void on an installation with nothing configured: an
    /// unpublished draft has nothing live, so it says nothing.
    #[test]
    fn voiding_an_unpublished_draft_with_no_config_is_silent() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);

        let outcome =
            void_invoice_with_teardown(&conn, id, "2026-08-06", None::<&FakeGw>, None::<&FakePub>)
                .expect("voids");

        assert_eq!(outcome.link, TeardownStep::NotApplicable);
        assert_eq!(outcome.page, TeardownStep::NotApplicable);
        assert!(outcome.warnings().is_empty(), "{:?}", outcome.warnings());
        assert_eq!(get_invoice(&conn, id).unwrap().status, "void");
    }

    /// Unconfigured, but something is live: both halves say what is still out
    /// there, and the link warning carries the URL.
    #[test]
    fn voiding_a_published_invoice_with_no_config_warns_about_both() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);

        let outcome =
            void_invoice_with_teardown(&conn, id, "2026-08-06", None::<&FakeGw>, None::<&FakePub>)
                .expect("voids");

        assert_eq!(outcome.link, TeardownStep::Skipped);
        assert_eq!(outcome.page, TeardownStep::Skipped);
        assert_eq!(
            outcome.payment_link_url.as_deref(),
            Some("https://buy.stripe.com/x")
        );
        let warnings = outcome.warnings();
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0].contains("stripe_secret_key"), "{warnings:?}");
        assert!(
            warnings[0].contains("https://buy.stripe.com/x"),
            "{warnings:?}"
        );
        assert_eq!(warnings[1], PUBLISHED_VOID_WARNING);
    }

    /// A send that failed after the link was created leaves a draft carrying a
    /// live link. Nothing was published, so only the link half has anything to
    /// say.
    #[test]
    fn an_unpublished_invoice_that_has_a_link_still_gets_it_deactivated() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        set_payment_link(&conn, id, "plink_1", "https://buy.stripe.com/x").unwrap();
        let gateway = FakeGw::default();
        let publisher = FakePub::default();

        let outcome =
            void_invoice_with_teardown(&conn, id, "2026-08-06", Some(&gateway), Some(&publisher))
                .expect("voids");

        assert_eq!(outcome.link, TeardownStep::Done);
        assert_eq!(outcome.page, TeardownStep::NotApplicable);
        assert!(publisher.pages.borrow().is_empty(), "nothing to replace");
        assert!(outcome.warnings().is_empty());
    }

    /// The guard runs before anything leaves the machine, so a refused void
    /// cannot deactivate a link on an invoice that is still open.
    #[test]
    fn a_refused_void_makes_no_call_and_leaves_the_invoice_alone() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        record_payment(&conn, id, 100.0, "2026-08-05", "stripe", None).unwrap();
        let gateway = FakeGw::default();
        let publisher = FakePub::default();

        let err =
            void_invoice_with_teardown(&conn, id, "2026-08-06", Some(&gateway), Some(&publisher))
                .unwrap_err();

        assert!(
            matches!(
                err,
                NigelError::Conflict {
                    code: "has_payments",
                    ..
                }
            ),
            "got: {err:?}"
        );
        assert!(gateway.deactivated.borrow().is_empty());
        assert!(publisher.pages.borrow().is_empty());
        assert_ne!(get_invoice(&conn, id).unwrap().status, "void");
    }

    #[test]
    fn voiding_an_already_void_invoice_is_still_refused() {
        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        let gateway = FakeGw::default();
        let publisher = FakePub::default();
        void_invoice_with_teardown(&conn, id, "2026-08-06", Some(&gateway), Some(&publisher))
            .expect("voids once");

        let err =
            void_invoice_with_teardown(&conn, id, "2026-08-07", Some(&gateway), Some(&publisher))
                .unwrap_err();

        assert!(
            matches!(
                err,
                NigelError::Conflict {
                    code: "already_void",
                    ..
                }
            ),
            "got: {err:?}"
        );
        // And the second attempt did not deactivate the link a second time.
        assert_eq!(gateway.deactivated.borrow().len(), 1);
    }

    /// The reason the teardown exists, from the other end: an invoice that was
    /// sent and never paid, then voided, is no longer polled — so a payment
    /// taken through a link that was *not* deactivated would go unrecorded.
    /// `sync` skips it on status alone, and the row keeps its link id, so the
    /// teardown is the only thing standing between the two.
    #[test]
    fn sync_records_nothing_against_an_invoice_voided_after_it_was_sent() {
        use crate::invoicing::invoices::paid_amount;
        use crate::invoicing::sync::sync_all_report;

        struct PayingGw;
        impl PaymentGateway for PayingGw {
            fn create_payment_link(&self, _i: &Invoice, _c: &Client) -> Result<PaymentLink> {
                unreachable!()
            }
            fn paid_sessions(&self, id: &str) -> Result<Vec<PaidSession>> {
                Ok(vec![PaidSession {
                    session_id: format!("cs_{id}"),
                    amount: 100.0,
                    paid_at: None,
                }])
            }
            fn deactivate_payment_link(&self, _id: &str) -> Result<()> {
                Ok(())
            }
        }

        let (_d, conn) = test_conn();
        let id = seed_sent(&conn);
        assert_eq!(get_invoice(&conn, id).unwrap().status, "sent");

        let publisher = FakePub::default();
        void_invoice_with_teardown(&conn, id, "2026-08-06", Some(&PayingGw), Some(&publisher))
            .expect("voids");

        let report =
            sync_all_report(&conn, "2026-08-07", &PayingGw).expect("a run with nothing in it");
        assert_eq!(report.invoices_checked, 0, "a void invoice is not polled");
        assert_eq!(report.recorded, 0);
        assert_eq!(paid_amount(&conn, id).unwrap(), 0.0);
        // The id is still on the row — being skipped is a fact about the status,
        // not about the link having been forgotten.
        assert_eq!(
            get_invoice(&conn, id)
                .unwrap()
                .stripe_payment_link_id
                .as_deref(),
            Some("plink_1")
        );
    }
}
