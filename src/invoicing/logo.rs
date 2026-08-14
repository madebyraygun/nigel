//! The letterhead logo as an object beside the page.
//!
//! The stored value is a `data:` URI and stays one — [`crate::db`]'s
//! `company_logo` metadata key is the single source of truth, and
//! [`crate::invoicing::document::parse_logo`] is the single validation path.
//! What changes at publish time is only where the **page** points: a published
//! page is an email body, Gmail strips `data:` URIs out of one, and a recipient
//! who should see a letterhead sees the business name instead.
//!
//! So a send puts the same bytes up as their own object at a stable key and
//! renders the page against that address. **Once**, not once per invoice: the
//! fingerprint of what is up there and the URL it went to are recorded in the
//! `published_logo` metadata key, and a send whose logo is unchanged uploads
//! nothing.
//!
//! Nothing here may fail a send or a republish. A logo that cannot be published
//! degrades to the `data:` URI in that render — the page stays self-contained,
//! it just stops rendering in Gmail — and the operator gets a sentence.

use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::db::{get_metadata, set_metadata};
use crate::invoicing::document::Logo;
use crate::invoicing::gateway::AssetPublisher;

/// What is published right now, as `"<fingerprint> <url>"`.
///
/// Both halves, because either can go stale on its own: the operator can
/// replace the image, and they can repoint `public_base_url` at a different
/// bucket without touching it. A pair that does not match what this send would
/// publish is a re-upload.
pub const PUBLISHED_LOGO_KEY: &str = "published_logo";

/// The bytes' identity. Content, not filename or timestamp: two sends of the
/// same image must agree that it is the same image.
pub fn fingerprint(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Where a page should point its `<img>`, and anything worth saying about how
/// it got there.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HostedLogo {
    /// The absolute URL, or `None` to leave the render carrying the `data:`
    /// URI — no logo configured, or an upload that did not happen.
    pub url: Option<String>,
    /// The sentence a failed upload earns, in one place so a terminal, the TUI
    /// and a browser describe it identically.
    pub warning: Option<String>,
    /// Whether this call actually put bytes there. `false` for an unchanged
    /// logo, which is the ordinary case after the first send.
    pub uploaded: bool,
}

impl HostedLogo {
    fn hosted(url: String, uploaded: bool) -> Self {
        Self {
            url: Some(url),
            warning: None,
            uploaded,
        }
    }
}

/// Make sure the letterhead logo is an object beside the pages, and answer the
/// address a page should carry.
///
/// `logo` is the render seam's verdict — [`crate::invoicing::render::usable_logo`]
/// — so a value neither document would draw is never uploaded either.
pub fn publish_letterhead_logo<P: AssetPublisher>(
    conn: &Connection,
    logo: Option<&Logo>,
    publisher: &P,
) -> HostedLogo {
    let Some(logo) = logo else {
        return HostedLogo::default();
    };
    let url = publisher.logo_url(logo.mime);
    let published = format!("{} {url}", fingerprint(&logo.bytes));
    if get_metadata(conn, PUBLISHED_LOGO_KEY).as_deref() == Some(published.as_str()) {
        return HostedLogo::hosted(url, false);
    }

    match publisher.publish_logo(&logo.bytes, logo.mime) {
        Ok(url) => {
            // Best-effort: a metadata write that fails costs one redundant
            // upload on the next send, which is not worth failing a send over.
            let _ = set_metadata(conn, PUBLISHED_LOGO_KEY, &published);
            HostedLogo::hosted(url, true)
        }
        // The recorded pair is left alone: it still describes what is actually
        // in the bucket, which is what a voided page will point at.
        Err(e) => HostedLogo {
            url: None,
            warning: Some(format!(
                "Warning: the letterhead logo could not be published beside the page ({e}), so \
                 this page carries it inline. The invoice is unaffected; a mail client that \
                 refuses data: URIs will show the business name where the logo goes."
            )),
            uploaded: false,
        },
    }
}

/// The logo object this installation has actually published, if any.
///
/// Read from the recorded pair rather than derived from configuration, so a
/// caller with no publisher and no settings — the voided page — can point at an
/// object that is known to exist instead of guessing at one that might not.
pub fn published_logo_url(conn: &Connection) -> Option<String> {
    let recorded = get_metadata(conn, PUBLISHED_LOGO_KEY)?;
    let url = recorded.split_whitespace().nth(1)?;
    (!url.is_empty()).then(|| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::error::{NigelError, Result};
    use crate::invoicing::document::parse_logo;
    use crate::migrations::run_migrations;
    use std::cell::RefCell;

    fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    /// A 2×1 PNG, the smallest thing `parse_logo` will accept.
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

    /// A second image with the same shape and different bytes: the trailing
    /// `IEND` CRC is not checked, so flipping it makes distinct content.
    fn other_png() -> Logo {
        let mut bytes = PNG_2X1.to_vec();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        png(&bytes)
    }

    #[derive(Default)]
    struct CapturePub {
        logos: RefCell<Vec<(Vec<u8>, String)>>,
    }
    impl AssetPublisher for CapturePub {
        fn publish(&self, token: &str, _h: &[u8], _p: &[u8]) -> Result<String> {
            Ok(format!("https://billing.example.test/i/{token}/index.html"))
        }
        fn publish_page(&self, token: &str, _h: &[u8]) -> Result<String> {
            Ok(format!("https://billing.example.test/i/{token}/index.html"))
        }
        fn logo_url(&self, mime: &str) -> String {
            format!(
                "https://billing.example.test/i/{}",
                crate::invoicing::r2::logo_object(mime)
            )
        }
        fn publish_logo(&self, bytes: &[u8], mime: &str) -> Result<String> {
            self.logos
                .borrow_mut()
                .push((bytes.to_vec(), mime.to_string()));
            Ok(self.logo_url(mime))
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
        fn logo_url(&self, _mime: &str) -> String {
            "https://billing.example.test/i/logo.png".to_string()
        }
        fn publish_logo(&self, _bytes: &[u8], _mime: &str) -> Result<String> {
            Err(NigelError::Other(
                "r2 403: <Error><Code>SignatureDoesNotMatch</Code></Error>".into(),
            ))
        }
    }

    #[test]
    fn no_logo_publishes_nothing_and_leaves_the_page_alone() {
        let (_d, conn) = test_conn();
        let publisher = CapturePub::default();

        let hosted = publish_letterhead_logo(&conn, None, &publisher);

        assert_eq!(hosted, HostedLogo::default());
        assert!(publisher.logos.borrow().is_empty());
        assert!(published_logo_url(&conn).is_none());
    }

    #[test]
    fn the_first_send_uploads_the_logo_and_answers_its_address() {
        let (_d, conn) = test_conn();
        let publisher = CapturePub::default();

        let hosted = publish_letterhead_logo(&conn, Some(&png(PNG_2X1)), &publisher);

        assert!(hosted.uploaded);
        assert_eq!(
            hosted.url.as_deref(),
            Some("https://billing.example.test/i/logo.png")
        );
        assert!(hosted.warning.is_none());
        let logos = publisher.logos.borrow();
        assert_eq!(logos.len(), 1);
        assert_eq!(logos[0].0, PNG_2X1);
        assert_eq!(logos[0].1, "image/png");
    }

    /// The whole point of the content hash: one object per image, not one per
    /// send and certainly not one per invoice.
    #[test]
    fn an_unchanged_logo_is_not_uploaded_again() {
        let (_d, conn) = test_conn();
        let publisher = CapturePub::default();
        let logo = png(PNG_2X1);

        publish_letterhead_logo(&conn, Some(&logo), &publisher);
        let second = publish_letterhead_logo(&conn, Some(&logo), &publisher);
        let third = publish_letterhead_logo(&conn, Some(&logo), &publisher);

        assert!(!second.uploaded && !third.uploaded);
        assert_eq!(second.url, third.url);
        assert_eq!(
            publisher.logos.borrow().len(),
            1,
            "the same bytes go up once"
        );
    }

    #[test]
    fn a_replaced_logo_is_uploaded_over_the_old_one() {
        let (_d, conn) = test_conn();
        let publisher = CapturePub::default();

        publish_letterhead_logo(&conn, Some(&png(PNG_2X1)), &publisher);
        let after = publish_letterhead_logo(&conn, Some(&other_png()), &publisher);

        assert!(after.uploaded, "different bytes, same key");
        assert_eq!(publisher.logos.borrow().len(), 2);
    }

    /// The address is half the identity. Repointing `public_base_url` at another
    /// bucket leaves the recorded URL naming an object this installation no
    /// longer serves, and an unchanged image is not a reason to keep it.
    #[test]
    fn moving_the_bucket_republishes_the_same_image() {
        struct Moved;
        impl AssetPublisher for Moved {
            fn publish(&self, _t: &str, _h: &[u8], _p: &[u8]) -> Result<String> {
                Ok(String::new())
            }
            fn publish_page(&self, _t: &str, _h: &[u8]) -> Result<String> {
                Ok(String::new())
            }
            fn logo_url(&self, _mime: &str) -> String {
                "https://invoices.example.test/i/logo.png".to_string()
            }
            fn publish_logo(&self, _bytes: &[u8], mime: &str) -> Result<String> {
                Ok(self.logo_url(mime))
            }
        }
        let (_d, conn) = test_conn();
        let logo = png(PNG_2X1);
        publish_letterhead_logo(&conn, Some(&logo), &CapturePub::default());

        let moved = publish_letterhead_logo(&conn, Some(&logo), &Moved);

        assert!(moved.uploaded, "same bytes, different address");
        assert_eq!(
            published_logo_url(&conn).as_deref(),
            Some("https://invoices.example.test/i/logo.png")
        );
    }

    #[test]
    fn a_failed_upload_falls_back_to_the_data_uri_and_says_so() {
        let (_d, conn) = test_conn();

        let hosted = publish_letterhead_logo(&conn, Some(&png(PNG_2X1)), &FailPub);

        assert!(hosted.url.is_none(), "the render keeps the data: URI");
        assert!(!hosted.uploaded);
        let warning = hosted.warning.expect("a sentence");
        assert!(warning.contains("SignatureDoesNotMatch"), "{warning}");
        assert!(
            warning.contains("data:"),
            "it has to say what the page fell back to: {warning}"
        );
        assert!(
            published_logo_url(&conn).is_none(),
            "nothing was published, so nothing is recorded"
        );
    }

    /// A void has no publisher and no configuration; it reads what a send left.
    #[test]
    fn the_published_address_is_readable_without_a_publisher() {
        let (_d, conn) = test_conn();
        publish_letterhead_logo(&conn, Some(&png(PNG_2X1)), &CapturePub::default());

        assert_eq!(
            published_logo_url(&conn).as_deref(),
            Some("https://billing.example.test/i/logo.png")
        );
    }

    #[test]
    fn a_fingerprint_follows_the_bytes() {
        assert_eq!(fingerprint(b"a"), fingerprint(b"a"));
        assert_ne!(fingerprint(b"a"), fingerprint(b"b"));
        assert_eq!(fingerprint(b"a").len(), 64);
    }
}
