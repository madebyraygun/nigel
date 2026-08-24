use crate::error::Result;
use crate::models::{Client, Invoice};

#[derive(Debug, Clone)]
pub struct PaymentLink {
    pub id: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct PaidSession {
    pub session_id: String,
    pub amount: f64,
    /// When the gateway recorded the checkout session, in Unix seconds — the
    /// day the client paid, which is not the day a sync happens to run.
    ///
    /// `Option` because a gateway that answers without one is a fact `sync`
    /// handles, and because every fake in the test modules builds this by hand.
    pub paid_at: Option<i64>,
}

pub trait PaymentGateway {
    fn create_payment_link(&self, invoice: &Invoice, client: &Client) -> Result<PaymentLink>;
    fn paid_sessions(&self, payment_link_id: &str) -> Result<Vec<PaidSession>>;
    /// Stop a payment link taking money.
    ///
    /// A link outlives the invoice it was made for: voiding an invoice in Nigel
    /// leaves the URL a client was emailed as chargeable as it ever was, and a
    /// payment taken through it would land against an invoice `sync` no longer
    /// polls. The gateway's own word for this is what a deactivation must be —
    /// a link cannot be un-deactivated, which is why nothing calls it except
    /// void.
    fn deactivate_payment_link(&self, payment_link_id: &str) -> Result<()>;
}

pub trait AssetPublisher {
    fn publish(&self, token: &str, html: &[u8], pdf: &[u8]) -> Result<String>;
    /// Replace the published page, leaving every other object under the token
    /// alone.
    ///
    /// Void republishes a page saying so; the PDF beside it is the document the
    /// client was actually sent, and deleting it would break a link someone may
    /// have filed rather than answer it honestly.
    fn publish_page(&self, token: &str, html: &[u8]) -> Result<String>;

    /// The address every published object is served under — `public_base_url`.
    ///
    /// On the trait because a *recorded* address is only usable while this
    /// installation still serves it. After a bucket move the URL a page was
    /// published with names a host nobody is answering, and a page being written
    /// now must omit the image rather than point at it.
    fn public_base(&self) -> &str;

    /// Where these bytes are addressed once published.
    ///
    /// Content-addressed, so the same image always answers the same URL and a
    /// different one never collides with it — which is what makes a published
    /// object safe to treat as immutable. Pure, so a page can be rendered
    /// against this address before the object exists; that is what lets the
    /// upload wait until the render has succeeded.
    fn logo_url(&self, bytes: &[u8], mime: &str) -> String {
        crate::invoicing::r2::logo_public_url(self.public_base(), bytes, mime)
    }

    /// Put the letterhead logo beside the pages as its own object, answering
    /// [`AssetPublisher::logo_url`].
    ///
    /// **One object per distinct image**, not one per invoice: it is the
    /// operator's own mark, it carries no client data, and a mail client that
    /// refuses `data:` URIs — Gmail — fetches it once and caches it across every
    /// invoice. Whether to call it at all is the caller's decision:
    /// [`crate::invoicing::logo`] skips it when the object is already up there.
    ///
    /// Never a delete and never an overwrite of *different* bytes. Pages that
    /// have already been delivered point at these objects, and a delivered
    /// document may not change after delivery.
    fn publish_logo(&self, bytes: &[u8], mime: &str) -> Result<String>;
}

/// The two logo methods a fake publisher that simply succeeds needs, so the
/// dozen fakes across this crate do not each spell out the object layout — and
/// so a change to it lands in one place rather than fourteen.
#[cfg(test)]
macro_rules! fake_logo_publishing {
    ($base:expr) => {
        fn public_base(&self) -> &str {
            $base
        }
        fn publish_logo(&self, bytes: &[u8], mime: &str) -> $crate::error::Result<String> {
            Ok(self.logo_url(bytes, mime))
        }
    };
}
#[cfg(test)]
pub(crate) use fake_logo_publishing;

pub trait Mailer {
    /// One message to the billing contact with every other contact copied.
    ///
    /// Each entry is already a formatted header value, so a name carrying a
    /// comma is quoted before it gets here — `mailgun::format_address` is the
    /// one place that happens, for a recipient as much as for the sender.
    fn send_invoice(
        &self,
        to: &str,
        cc: &[String],
        subject: &str,
        text: &str,
        pdf: &[u8],
    ) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Ok1;
    impl AssetPublisher for Ok1 {
        fn publish(&self, token: &str, _h: &[u8], _p: &[u8]) -> crate::error::Result<String> {
            Ok(format!("https://billing.example.com/i/{token}/index.html"))
        }
        fn publish_page(&self, token: &str, _h: &[u8]) -> crate::error::Result<String> {
            Ok(format!("https://billing.example.com/i/{token}/index.html"))
        }
        fake_logo_publishing!("https://billing.example.com/i");
    }

    #[test]
    fn publisher_trait_returns_url() {
        let url = Ok1.publish("tok", b"<html>", b"%PDF").unwrap();
        assert_eq!(url, "https://billing.example.com/i/tok/index.html");
    }

    #[test]
    fn publishing_a_page_answers_the_same_address_as_a_full_publish() {
        assert_eq!(
            Ok1.publish_page("tok", b"<html>").unwrap(),
            Ok1.publish("tok", b"<html>", b"%PDF").unwrap()
        );
    }
}
