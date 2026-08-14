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

    /// Where the letterhead logo is addressed, for an image of this type.
    ///
    /// Pure, and on the trait rather than derived by a caller, because only the
    /// publisher knows the base its objects are served under — and deciding
    /// whether the object up there is still the right one means comparing the
    /// address as well as the bytes. An operator who repoints `public_base_url`
    /// at a different bucket has a stale URL, not a stale image.
    fn logo_url(&self, mime: &str) -> String;

    /// Put the letterhead logo beside the pages as its own object, answering
    /// [`AssetPublisher::logo_url`].
    ///
    /// **One** object for the whole installation at a stable key, not one per
    /// invoice: it is the operator's own mark, it carries no client data, and a
    /// mail client that refuses `data:` URIs — Gmail — fetches it once and
    /// caches it across every invoice. Whether to call it at all is the caller's
    /// decision: [`crate::invoicing::logo`] skips it when the bytes already up
    /// there are these, so a send that changes nothing uploads nothing.
    fn publish_logo(&self, bytes: &[u8], mime: &str) -> Result<String>;
}

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
        html: &str,
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
        fn logo_url(&self, mime: &str) -> String {
            format!(
                "https://billing.example.com/i/{}",
                crate::invoicing::r2::logo_object(mime)
            )
        }
        fn publish_logo(&self, _bytes: &[u8], mime: &str) -> crate::error::Result<String> {
            Ok(self.logo_url(mime))
        }
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
