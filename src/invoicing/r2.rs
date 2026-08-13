use std::time::Duration;

use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};

use crate::error::{NigelError, Result};
use crate::invoicing::gateway::AssetPublisher;

pub fn object_key(token: &str, filename: &str) -> String {
    format!("i/{token}/{filename}")
}

/// The object every published page is written to. The address Nigel hands out
/// names this file rather than its directory, because a static host is not
/// required to have an opinion about directories.
pub const PAGE_OBJECT: &str = "index.html";

/// The PDF beside it, kept next to `PAGE_OBJECT` so the two keys are named once.
pub const PDF_OBJECT: &str = "invoice.pdf";

pub fn public_url(public_base_url: &str, token: &str) -> String {
    format!(
        "{}/{}/{PAGE_OBJECT}",
        public_base_url.trim_end_matches('/'),
        token
    )
}

/// The sentence a `public_base_url` that misses the `i/` prefix earns.
///
/// It quotes no value: this travels on `settings::invoicing_status`, which
/// carries key names only.
const I_PREFIX_WARNING: &str = "public_base_url does not end in /i — Nigel writes objects under the i/ prefix, so published links will 404 unless that prefix is what this address serves.";

/// What is wrong with a `public_base_url` that cannot produce a working link,
/// and how to fix it — **without quoting the value**.
///
/// This is what crosses the wire. No API response carries a configured setting's
/// value, so the sentence names the key and the defect and stops there; a
/// terminal, answering the operator who just typed the command, prefixes the
/// value itself.
pub const PUBLIC_BASE_URL_DEFECT: &str = "public_base_url is not an absolute http(s) address. \
     Set it to the address your bucket is served at, including the scheme — for example \
     https://billing.example.com/i. A scheme-relative address such as //billing.example.com/i is not \
     enough: put https: in front of it, because a link in an email has no page to inherit a \
     scheme from.";

/// A `public_base_url` that cannot produce a working link at all. An absolute
/// http(s) address with a host is the whole requirement — the path is the
/// operator's business, and the `/i` question is a warning, not this.
///
/// Hand-rolled rather than parsed: `url` is not a dependency, and pulling one in
/// to answer "does this start with https:// and have a host" would be the
/// largest thing in this change. It is a real authority parse all the same —
/// `https://user@/i` and `https://:8787/i` have a non-empty first segment and no
/// host at all.
pub fn validate_public_base_url(value: &str) -> Result<()> {
    if has_absolute_http_host(value) {
        return Ok(());
    }
    Err(NigelError::Invalid(format!(
        "public_base_url \"{value}\" is wrong. {PUBLIC_BASE_URL_DEFECT}"
    )))
}

fn has_absolute_http_host(value: &str) -> bool {
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return false;
    }
    let lowered = value.to_ascii_lowercase();
    let Some(rest) = lowered
        .strip_prefix("http://")
        .or_else(|| lowered.strip_prefix("https://"))
    else {
        return false;
    };

    // The authority runs to the path, the query or the fragment.
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    // Userinfo is everything before the last `@`, and it is not the host.
    let host_port = match authority.rsplit_once('@') {
        Some((_userinfo, after)) => after,
        None => authority,
    };
    // A bracketed IPv6 literal keeps its colons; everything else ends at the port.
    let host = match host_port.strip_prefix('[') {
        Some(inside) => match inside.split_once(']') {
            Some((literal, _port)) => literal,
            None => return false,
        },
        None => host_port.split(':').next().unwrap_or_default(),
    };
    !host.is_empty()
}

/// Nigel writes every object under `i/`, so a base URL that does not end there
/// is usually pointing at the bucket root. Usually, not always: a rewrite can
/// map the prefix onto the domain root, which is why this is a sentence and not
/// a refusal.
pub fn public_base_url_warning(value: &str) -> Option<&'static str> {
    let trimmed = value.trim_end_matches('/');
    if trimmed.ends_with("/i") {
        return None;
    }
    Some(I_PREFIX_WARNING)
}

fn ensure_success(status: reqwest::StatusCode, body: &str) -> Result<()> {
    if status.is_success() {
        return Ok(());
    }
    Err(NigelError::Other(format!("r2 {status}: {body}")))
}

pub struct R2Publisher {
    pub account_id: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub public_base_url: String,
}

impl R2Publisher {
    fn put(&self, key: &str, body: &[u8], content_type: &str) -> Result<()> {
        let endpoint = format!("https://{}.r2.cloudflarestorage.com", self.account_id)
            .parse()
            .map_err(|e| NigelError::Other(format!("r2 endpoint: {e}")))?;
        let bucket = Bucket::new(endpoint, UrlStyle::Path, self.bucket.clone(), "auto")
            .map_err(|e| NigelError::Other(format!("r2 bucket: {e}")))?;
        let creds = Credentials::new(self.access_key.clone(), self.secret_key.clone());

        let action = bucket.put_object(Some(&creds), key);
        let signed = action.sign(Duration::from_secs(300));

        let resp = crate::invoicing::http_client()
            .put(signed)
            .header("content-type", content_type)
            .body(body.to_vec())
            .send()
            .map_err(|e| NigelError::Other(format!("r2 put: {e}")))?;
        let status = resp.status();
        let text = resp.text().map_err(|e| NigelError::Other(e.to_string()))?;
        ensure_success(status, &text)
    }
}

impl AssetPublisher for R2Publisher {
    fn publish(&self, token: &str, html: &[u8], pdf: &[u8]) -> Result<String> {
        self.put(
            &object_key(token, PAGE_OBJECT),
            html,
            "text/html; charset=utf-8",
        )?;
        self.put(&object_key(token, PDF_OBJECT), pdf, "application/pdf")?;
        Ok(public_url(&self.public_base_url, token))
    }

    fn publish_page(&self, token: &str, html: &[u8]) -> Result<String> {
        self.put(
            &object_key(token, PAGE_OBJECT),
            html,
            "text/html; charset=utf-8",
        )?;
        Ok(public_url(&self.public_base_url, token))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_key_layout() {
        assert_eq!(object_key("abc", "index.html"), "i/abc/index.html");
        assert_eq!(object_key("abc", "invoice.pdf"), "i/abc/invoice.pdf");
    }

    #[test]
    fn public_url_names_the_index_document_not_its_directory() {
        // A plain R2 custom domain serves objects by key and has no directory index.
        assert_eq!(
            public_url("https://billing.example.com/i", "abc"),
            "https://billing.example.com/i/abc/index.html"
        );
        assert_eq!(
            public_url("https://billing.example.com/i/", "abc"),
            "https://billing.example.com/i/abc/index.html"
        );
    }

    #[test]
    fn the_address_and_the_key_name_the_same_object() {
        let url = public_url("https://billing.example.com/i", "abc");
        assert!(
            url.ends_with(&object_key("abc", PAGE_OBJECT)),
            "the address and the key must not drift: {url}"
        );
    }

    #[test]
    fn a_base_url_without_a_scheme_is_refused_naming_the_setting() {
        let err = validate_public_base_url("billing.example.com")
            .unwrap_err()
            .to_string();
        assert!(err.contains("public_base_url"), "got: {err}");
        assert!(
            err.contains("billing.example.com"),
            "the offending value: {err}"
        );
        assert!(err.contains("https://"), "the expected shape: {err}");
    }

    #[test]
    fn the_shapes_that_can_produce_a_link_are_accepted() {
        for ok in [
            "https://billing.example.com/i",
            "https://billing.example.com/i/",
            "HTTP://billing.example.com/i",
            "http://localhost:8787/i",
            "http://user@localhost:8787/i",
            "http://[::1]:8787/i",
        ] {
            assert!(validate_public_base_url(ok).is_ok(), "refused: {ok}");
        }
    }

    #[test]
    fn the_shapes_that_cannot_are_refused() {
        for bad in [
            "",
            "   ",
            "billing.example.com",
            "//billing.example.com/i",
            "ftp://billing.example.com/i",
            "https://",
            "https:// billing.example.com",
            "https:///i",
        ] {
            assert!(validate_public_base_url(bad).is_err(), "accepted: {bad:?}");
        }
    }

    /// An authority is not its first path segment: these all have something
    /// before the `/` and no host in it.
    #[test]
    fn an_authority_with_no_host_in_it_is_refused() {
        for bad in [
            "https://user@/i",
            "https://:8787/i",
            "https://user:pw@/i",
            "https://@/i",
            "https://[::1/i",
        ] {
            assert!(validate_public_base_url(bad).is_err(), "accepted: {bad:?}");
        }
    }

    /// A scheme-relative base is the one refusal whose fix is not obvious, so
    /// the sentence spells it out.
    #[test]
    fn the_refusal_says_how_to_fix_a_scheme_relative_address() {
        let err = validate_public_base_url("//billing.example.com/i")
            .unwrap_err()
            .to_string();
        assert!(err.contains("https:"), "got: {err}");
        assert!(err.contains("in front"), "the fix has to be stated: {err}");
    }

    /// The wire-safe half of the sentence carries no value at all: it is what an
    /// API response says, and no response carries a configured setting.
    #[test]
    fn the_defect_sentence_quotes_nothing() {
        assert!(!PUBLIC_BASE_URL_DEFECT.contains('"'));
        assert!(PUBLIC_BASE_URL_DEFECT.contains("public_base_url"));
    }

    #[test]
    fn a_base_url_that_does_not_end_in_the_i_prefix_warns_without_quoting_it() {
        let warning = public_base_url_warning("https://billing.example.com").expect("warns");
        assert!(warning.contains("/i"), "got: {warning}");
        assert!(
            !warning.contains("billing.example.com"),
            "status carries no values: {warning}"
        );
        assert_eq!(
            public_base_url_warning("https://billing.example.com/i"),
            None
        );
        assert_eq!(
            public_base_url_warning("https://billing.example.com/i/"),
            None
        );
        assert!(public_base_url_warning("https://billing.example.com/invoices").is_some());
    }

    #[test]
    fn ensure_success_rejects_non_2xx_and_keeps_r2_message() {
        let err = ensure_success(
            reqwest::StatusCode::FORBIDDEN,
            "<Error><Code>SignatureDoesNotMatch</Code></Error>",
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("403"), "status missing from {msg:?}");
        assert!(
            msg.contains("SignatureDoesNotMatch"),
            "r2 message missing from {msg:?}"
        );
    }

    #[test]
    fn ensure_success_accepts_2xx() {
        assert!(ensure_success(reqwest::StatusCode::OK, "").is_ok());
    }
}
