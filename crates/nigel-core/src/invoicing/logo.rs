//! The letterhead logo as an object beside the page.
//!
//! The stored value is a `data:` URI and stays one — the `company_logo`
//! metadata key is the single source of truth and
//! [`crate::invoicing::document::parse_logo`] the single validation path. What
//! changes at publish time is only where the **page** points: a published page
//! is an email body, Gmail strips `data:` URIs out of one, and a recipient who
//! should see a letterhead sees the business name instead.
//!
//! So a send puts the same bytes up as their own object and renders the page
//! against that address.
//!
//! **Published objects are immutable.** The key is the content
//! ([`crate::invoicing::r2::logo_object`]), so a rebrand writes a *different*
//! object and never touches the one an already-delivered invoice points at. A
//! document cannot change after it has been delivered; an old page keeping the
//! mark it was sent with is the correct outcome, not a stale cache. The price is
//! that old objects accumulate — one per rebrand, each capped at
//! [`crate::invoicing::document::MAX_LOGO_BYTES`] — and they are load-bearing
//! for pages already in clients' hands, so nothing here deletes them.
//!
//! The upload happens **after** the render succeeds, so a broken custom template
//! costs no object at all. The address is pure, which is what makes that
//! ordering possible: the page can be rendered against a URL before the bytes
//! are there.
//!
//! Nothing here may fail a send or a republish. A logo that cannot be published
//! degrades to the `data:` URI in that render — the page stays self-contained,
//! it just stops rendering in Gmail — and the operator gets a sentence.

use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::db::{get_metadata, set_metadata};
use crate::error::Result;
use crate::invoicing::document::Logo;
use crate::invoicing::gateway::AssetPublisher;

/// The stored letterhead logo, as a `data:` URI. The one source of truth.
pub const COMPANY_LOGO_KEY: &str = "company_logo";

/// What is published right now, as `"<fingerprint> <url>"`.
///
/// The fingerprint answers "is this image already up there"; the URL answers
/// "and is it still where we would put it". Either can go stale on its own — the
/// operator can replace the image, and they can repoint `public_base_url` at a
/// different bucket without touching it.
///
/// It is a record of what this installation published, not of what any
/// particular page carries: a page carries the URL it was rendered with, which
/// is why the objects behind those URLs are never disturbed.
pub const PUBLISHED_LOGO_KEY: &str = "published_logo";

/// The bytes' identity. Content, not filename or timestamp: two sends of the
/// same image must agree that it is the same image, and a different image must
/// never be handed the same address.
pub fn fingerprint(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// The logo a document is being published with: where it will be addressed, and
/// the bytes to put there once the render has succeeded.
///
/// Holding the two together is what keeps them in step. The page is rendered
/// against [`PendingLogo::url`] and the object is written by [`publish`], so a
/// page can never point at an address the upload was not aiming for.
pub struct PendingLogo<'a> {
    /// `None` when there is no usable logo: the document draws no image at all
    /// and nothing is ever uploaded.
    inner: Option<(&'a Logo, String)>,
}

impl PendingLogo<'_> {
    /// What the page should render its `<img src>` against.
    ///
    /// `None` leaves the document carrying the bytes inline, which is what a
    /// preview does and what a failed upload falls back to.
    pub fn url(&self) -> Option<&str> {
        self.inner.as_ref().map(|(_, url)| url.as_str())
    }

    /// The pair [`PUBLISHED_LOGO_KEY`] holds once this is published.
    fn record(&self) -> Option<String> {
        self.inner
            .as_ref()
            .map(|(logo, url)| format!("{} {url}", fingerprint(&logo.bytes)))
    }
}

/// Where this logo will live, without uploading anything.
///
/// `logo` is the render seam's verdict
/// ([`crate::invoicing::render::usable_logo`]), so a value neither document
/// would draw is never given an address and never uploaded.
pub fn pending<'a, P: AssetPublisher>(logo: Option<&'a Logo>, publisher: &P) -> PendingLogo<'a> {
    PendingLogo {
        inner: logo.map(|logo| (logo, publisher.logo_url(&logo.bytes, logo.mime))),
    }
}

/// Put the object there, now that the render has succeeded.
///
/// Answers the sentence a failure earns, or `None` — including for the two
/// ordinary silences: no logo to publish, and a logo already up at that address.
///
/// Never an error. The caller has a rendered document in hand and a client
/// waiting for it; a logo is decoration on a document about money.
pub fn publish<P: AssetPublisher>(
    conn: &Connection,
    pending: &PendingLogo<'_>,
    publisher: &P,
) -> Option<String> {
    let (logo, _) = pending.inner.as_ref()?;
    let record = pending.record()?;
    // Content-addressed, so "already recorded" really does mean the object is
    // there with these exact bytes.
    if get_metadata(conn, PUBLISHED_LOGO_KEY).as_deref() == Some(record.as_str()) {
        return None;
    }

    match publisher.publish_logo(&logo.bytes, logo.mime) {
        Ok(_) => {
            // Best-effort: a metadata write that fails costs one redundant
            // upload next time, which is not worth failing a send over.
            let _ = set_metadata(conn, PUBLISHED_LOGO_KEY, &record);
            None
        }
        // The record is left alone. It still names an object that is really
        // there — the previous one — and the pages published with it still
        // resolve, because nothing here overwrites or deletes.
        Err(e) => Some(format!(
            "Warning: the letterhead logo could not be published beside the page ({e}), so this \
             page carries it inline. The invoice is unaffected; a mail client that refuses data: \
             URIs will show the business name where the logo goes."
        )),
    }
}

/// The published logo's address, if **this installation still serves it**.
///
/// The base is a required argument rather than something read here, so no caller
/// can take the recorded URL unguarded: after a bucket move it names a host
/// nobody is answering, and a page being written now must omit the image rather
/// than point at a decommissioned domain.
///
/// Read by the voided-invoice notice, which has no publisher to ask and no
/// configuration of its own — and which must therefore only ever name an object
/// a send actually put there.
pub fn published_logo_url(conn: &Connection, public_base: &str) -> Option<String> {
    let recorded = get_metadata(conn, PUBLISHED_LOGO_KEY)?;
    let url = recorded.split_whitespace().nth(1)?;
    let base = public_base.trim_end_matches('/');
    (!base.is_empty() && url.starts_with(base)).then(|| url.to_string())
}

/// Write the stored letterhead logo, forgetting what is published when it is
/// cleared.
///
/// The one writer both settings screens go through, because clearing the logo
/// has to clear the record with it. The objects themselves stay — pages already
/// delivered point at them — but a document published *after* the operator
/// removed their logo must not still carry one, and the voided-invoice notice
/// reads that record.
pub fn set_company_logo(conn: &Connection, value: &str) -> Result<()> {
    set_metadata(conn, COMPANY_LOGO_KEY, value)?;
    if value.trim().is_empty() {
        set_metadata(conn, PUBLISHED_LOGO_KEY, "")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::error::NigelError;
    use crate::invoicing::document::parse_logo;
    use crate::invoicing::gateway::fake_logo_publishing;
    use crate::migrations::run_migrations;
    use std::cell::RefCell;
    use std::collections::HashMap;

    fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    /// A 2×1 PNG, the smallest thing `parse_logo` will accept. Nothing here
    /// decodes it.
    const PNG_2X1: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H', b'D',
        b'R', 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82,
    ];

    fn png(payload: &[u8]) -> Logo {
        use base64::Engine as _;
        let uri = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(payload)
        );
        parse_logo(&uri).unwrap().expect("a logo")
    }

    /// A second image: the trailing `IEND` CRC is not checked, so flipping it
    /// makes distinct content of the same shape.
    fn other_png() -> Logo {
        let mut bytes = PNG_2X1.to_vec();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        png(&bytes)
    }

    /// A bucket, so a test can ask what is actually stored at an address rather
    /// than only how many uploads happened.
    #[derive(Default)]
    struct Bucket {
        objects: RefCell<HashMap<String, Vec<u8>>>,
        uploads: RefCell<usize>,
    }
    impl AssetPublisher for Bucket {
        fn publish(&self, token: &str, _h: &[u8], _p: &[u8]) -> Result<String> {
            Ok(format!("https://billing.example.test/i/{token}/index.html"))
        }
        fn publish_page(&self, token: &str, _h: &[u8]) -> Result<String> {
            Ok(format!("https://billing.example.test/i/{token}/index.html"))
        }
        fn public_base(&self) -> &str {
            "https://billing.example.test/i"
        }
        fn publish_logo(&self, bytes: &[u8], mime: &str) -> Result<String> {
            let url = self.logo_url(bytes, mime);
            *self.uploads.borrow_mut() += 1;
            self.objects
                .borrow_mut()
                .insert(url.clone(), bytes.to_vec());
            Ok(url)
        }
    }

    struct FailPub;
    impl AssetPublisher for FailPub {
        fn publish(&self, _t: &str, _h: &[u8], _p: &[u8]) -> Result<String> {
            Ok(String::new())
        }
        fn publish_page(&self, _t: &str, _h: &[u8]) -> Result<String> {
            Ok(String::new())
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

    /// The same installation served from somewhere else.
    struct Moved;
    impl AssetPublisher for Moved {
        fn publish(&self, _t: &str, _h: &[u8], _p: &[u8]) -> Result<String> {
            Ok(String::new())
        }
        fn publish_page(&self, _t: &str, _h: &[u8]) -> Result<String> {
            Ok(String::new())
        }
        fake_logo_publishing!("https://invoices.example.test/i");
    }

    fn publish_now<P: AssetPublisher>(
        conn: &Connection,
        logo: Option<&Logo>,
        publisher: &P,
    ) -> (Option<String>, Option<String>) {
        let pending = pending(logo, publisher);
        let url = pending.url().map(str::to_string);
        (url, publish(conn, &pending, publisher))
    }

    #[test]
    fn no_logo_publishes_nothing_and_leaves_the_page_alone() {
        let (_d, conn) = test_conn();
        let bucket = Bucket::default();

        let (url, warning) = publish_now(&conn, None, &bucket);

        assert!(url.is_none() && warning.is_none());
        assert_eq!(*bucket.uploads.borrow(), 0);
        assert!(published_logo_url(&conn, bucket.public_base()).is_none());
    }

    #[test]
    fn the_first_send_uploads_the_logo_and_answers_its_address() {
        let (_d, conn) = test_conn();
        let bucket = Bucket::default();
        let logo = png(PNG_2X1);

        let (url, warning) = publish_now(&conn, Some(&logo), &bucket);

        assert!(warning.is_none());
        let url = url.expect("an address");
        assert!(url.starts_with("https://billing.example.test/i/logo-"));
        assert!(url.ends_with(".png"), "the type is part of the name: {url}");
        assert_eq!(bucket.objects.borrow()[&url], PNG_2X1);
    }

    /// The whole point of the content hash: the operator's mark is the same
    /// bytes on every invoice, so it is uploaded once and not once per send.
    #[test]
    fn an_unchanged_logo_is_not_uploaded_again() {
        let (_d, conn) = test_conn();
        let bucket = Bucket::default();
        let logo = png(PNG_2X1);

        let (first, _) = publish_now(&conn, Some(&logo), &bucket);
        let (second, _) = publish_now(&conn, Some(&logo), &bucket);
        let (third, _) = publish_now(&conn, Some(&logo), &bucket);

        assert_eq!(first, second);
        assert_eq!(second, third);
        assert_eq!(*bucket.uploads.borrow(), 1, "the same bytes go up once");
    }

    /// **A rebrand may not change a document that has already been delivered.**
    /// The new mark is a new object at a new address; the invoice a client is
    /// looking at still resolves to the mark it was sent with.
    #[test]
    fn a_rebrand_leaves_an_already_published_pages_logo_resolving_to_the_old_bytes() {
        let (_d, conn) = test_conn();
        let bucket = Bucket::default();
        let old = png(PNG_2X1);
        let new = other_png();

        let (delivered_with, _) = publish_now(&conn, Some(&old), &bucket);
        let (rebranded_to, _) = publish_now(&conn, Some(&new), &bucket);

        let delivered_with = delivered_with.expect("an address");
        let rebranded_to = rebranded_to.expect("an address");
        assert_ne!(
            delivered_with, rebranded_to,
            "a different image must get a different address"
        );
        let objects = bucket.objects.borrow();
        assert_eq!(
            objects[&delivered_with], PNG_2X1,
            "the delivered page's URL still resolves to the mark it was sent with"
        );
        assert_eq!(objects[&rebranded_to], new.bytes);
        assert_eq!(objects.len(), 2, "nothing was overwritten");
    }

    /// The record follows the newest publish, which is what the *next* document
    /// and the voided-invoice notice read.
    #[test]
    fn the_record_names_the_most_recently_published_logo() {
        let (_d, conn) = test_conn();
        let bucket = Bucket::default();
        publish_now(&conn, Some(&png(PNG_2X1)), &bucket);
        let (newest, _) = publish_now(&conn, Some(&other_png()), &bucket);

        assert_eq!(published_logo_url(&conn, bucket.public_base()), newest);
    }

    /// A `public_base_url` repointed at another bucket leaves the recorded
    /// address naming a host this installation no longer serves. A page written
    /// now omits the image rather than pointing at a decommissioned domain.
    #[test]
    fn a_recorded_address_under_a_different_base_is_not_offered() {
        let (_d, conn) = test_conn();
        publish_now(&conn, Some(&png(PNG_2X1)), &Bucket::default());

        assert!(
            published_logo_url(&conn, "https://billing.example.test/i").is_some(),
            "the premise: it is offered under the base it was published to"
        );
        assert!(published_logo_url(&conn, Moved.public_base()).is_none());
        assert!(published_logo_url(&conn, "").is_none());
    }

    /// The same image after a bucket move is a new upload: the address is half
    /// the identity, and a stale address is as wrong as a stale picture.
    #[test]
    fn moving_the_bucket_republishes_the_same_image() {
        let (_d, conn) = test_conn();
        let logo = png(PNG_2X1);
        publish_now(&conn, Some(&logo), &Bucket::default());

        let (moved, warning) = publish_now(&conn, Some(&logo), &Moved);

        assert!(warning.is_none());
        let moved = moved.expect("an address");
        assert!(
            moved.starts_with("https://invoices.example.test/i/logo-"),
            "the same image, at the address this installation now serves: {moved}"
        );
        assert_eq!(
            published_logo_url(&conn, Moved.public_base()).as_deref(),
            Some(moved.as_str())
        );
    }

    /// **Clearing the logo clears the record.** The objects stay where delivered
    /// pages point; what must not survive is the claim that this installation
    /// currently publishes a mark.
    #[test]
    fn clearing_the_stored_logo_forgets_what_was_published() {
        let (_d, conn) = test_conn();
        let bucket = Bucket::default();
        set_company_logo(&conn, "data:image/png;base64,AAAA").unwrap();
        publish_now(&conn, Some(&png(PNG_2X1)), &bucket);
        assert!(published_logo_url(&conn, bucket.public_base()).is_some());

        set_company_logo(&conn, "").unwrap();

        assert!(
            published_logo_url(&conn, bucket.public_base()).is_none(),
            "a document published after the logo was removed must not carry one"
        );
        assert_eq!(
            get_metadata(&conn, COMPANY_LOGO_KEY).unwrap_or_default(),
            ""
        );
    }

    /// Whitespace is not a logo. The TUI trims before it writes, and the web
    /// route trims too, but the rule belongs to the writer.
    #[test]
    fn a_blank_logo_counts_as_cleared() {
        let (_d, conn) = test_conn();
        let bucket = Bucket::default();
        publish_now(&conn, Some(&png(PNG_2X1)), &bucket);

        set_company_logo(&conn, "   \n ").unwrap();

        assert!(published_logo_url(&conn, bucket.public_base()).is_none());
    }

    /// Replacing the logo leaves the record for the *new* one, so nothing has to
    /// be forgotten on that path — the fingerprint simply stops matching.
    #[test]
    fn replacing_the_stored_logo_keeps_a_record_to_supersede() {
        let (_d, conn) = test_conn();
        let bucket = Bucket::default();
        publish_now(&conn, Some(&png(PNG_2X1)), &bucket);

        set_company_logo(&conn, "data:image/png;base64,BBBB").unwrap();

        assert!(
            published_logo_url(&conn, bucket.public_base()).is_some(),
            "the old object is still up and still correct for the pages that carry it"
        );
    }

    #[test]
    fn a_failed_upload_falls_back_to_the_data_uri_and_says_so() {
        let (_d, conn) = test_conn();

        let (url, warning) = publish_now(&conn, Some(&png(PNG_2X1)), &FailPub);

        assert!(url.is_some(), "the address is known before the upload");
        let warning = warning.expect("a sentence");
        assert!(warning.contains("SignatureDoesNotMatch"), "{warning}");
        assert!(
            warning.contains("data:"),
            "it has to say what the page fell back to: {warning}"
        );
        assert!(
            published_logo_url(&conn, FailPub.public_base()).is_none(),
            "nothing was published, so nothing is recorded"
        );
    }

    #[test]
    fn a_fingerprint_follows_the_bytes() {
        assert_eq!(fingerprint(b"a"), fingerprint(b"a"));
        assert_ne!(fingerprint(b"a"), fingerprint(b"b"));
        assert_eq!(fingerprint(b"a").len(), 64);
    }
}
