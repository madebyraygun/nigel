//! The SimpleFIN protocol: claiming a setup token, and reading accounts.
//!
//! The access URL is a credential — it embeds the Basic Auth user and
//! password. It never reaches a log, an error message or `Debug` output:
//! requests are sent to the URL with its userinfo stripped and the
//! credentials passed as a header, and every `reqwest` error is reported
//! without its URL.

use std::time::Duration;

use base64::Engine;
use reqwest::{StatusCode, Url};
use serde::Deserialize;

use crate::error::{NigelError, Result};

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// A bridge assembles the response from the banks while the request waits,
/// so it is allowed longer than the invoicing gateways.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

fn http_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .expect("build the SimpleFIN HTTP client")
}

/// A SimpleFIN access URL. Opaque on purpose: there is no `Display`, and
/// `Debug` prints a placeholder.
#[derive(Clone, PartialEq, Eq)]
pub struct AccessUrl(String);

impl std::fmt::Debug for AccessUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AccessUrl(<redacted>)")
    }
}

impl AccessUrl {
    /// Accept a stored or freshly claimed access URL. It must carry
    /// credentials and use HTTPS; plain HTTP is allowed only to a loopback
    /// host, which is what a local test bridge is.
    pub fn parse(raw: &str) -> Result<Self> {
        let url = Url::parse(raw.trim())
            .map_err(|_| NigelError::Invalid("the SimpleFIN access URL is not a URL".into()))?;
        require_secure(&url, "access URL")?;
        if url.username().is_empty() || url.password().is_none() {
            return Err(NigelError::Invalid(
                "the SimpleFIN access URL carries no credentials".into(),
            ));
        }
        Ok(Self(raw.trim().to_string()))
    }

    /// The stored form, for settings.json and nowhere else.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// The URL without its userinfo, and the credentials it carried.
    fn split(&self) -> (Url, String, String) {
        let mut url = Url::parse(&self.0).expect("validated in parse");
        let user = percent_decode(url.username());
        let pass = percent_decode(url.password().unwrap_or(""));
        let _ = url.set_username("");
        let _ = url.set_password(None);
        (url, user, pass)
    }
}

fn require_secure(url: &Url, what: &str) -> Result<()> {
    let host = url
        .host_str()
        .unwrap_or("")
        .trim_start_matches('[')
        .trim_end_matches(']');
    let loopback = host == "localhost"
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback());
    match url.scheme() {
        "https" => Ok(()),
        "http" if loopback => Ok(()),
        _ => Err(NigelError::Invalid(format!(
            "the SimpleFIN {what} must use HTTPS"
        ))),
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Some(b) = std::str::from_utf8(&bytes[i + 1..i + 3])
                .ok()
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
            {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn transport_error(err: reqwest::Error) -> NigelError {
    NigelError::Other(format!("could not reach SimpleFIN: {}", err.without_url()))
}

/// Exchange a one-time setup token for an access URL.
pub fn claim(setup_token: &str) -> Result<AccessUrl> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(setup_token.trim())
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .ok_or_else(|| {
            NigelError::Invalid("that is not a SimpleFIN setup token (it is not base64)".into())
        })?;
    let claim_url = Url::parse(decoded.trim()).map_err(|_| {
        NigelError::Invalid("that is not a SimpleFIN setup token (it holds no URL)".into())
    })?;
    require_secure(&claim_url, "claim URL")?;

    let response = http_client()
        .post(claim_url)
        .header(reqwest::header::CONTENT_LENGTH, "0")
        .send()
        .map_err(transport_error)?;
    match response.status() {
        StatusCode::OK => {}
        StatusCode::FORBIDDEN => {
            return Err(NigelError::Invalid(
                "SimpleFIN refused the setup token: it has already been claimed or does not \
                 exist. If you did not claim it yourself, disable it at the bridge — someone \
                 else may have."
                    .into(),
            ))
        }
        status => {
            return Err(NigelError::Other(format!(
                "SimpleFIN answered the claim with HTTP {}",
                status.as_u16()
            )))
        }
    }
    let body = response.text().map_err(transport_error)?;
    AccessUrl::parse(&body)
}

/// What to ask `/accounts` for.
#[derive(Debug, Default, Clone)]
pub struct AccountsQuery {
    /// Unix seconds; transactions on or after.
    pub start: Option<i64>,
    /// Unix seconds; transactions strictly before.
    pub end: Option<i64>,
    pub balances_only: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountSet {
    #[serde(default)]
    pub errors: Vec<String>,
    pub accounts: Vec<Account>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Organization {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "sfin-url", default)]
    pub sfin_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Account {
    pub org: Organization,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub currency: String,
    pub balance: String,
    #[serde(rename = "balance-date")]
    pub balance_date: i64,
    #[serde(default)]
    pub transactions: Vec<Transaction>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub posted: i64,
    pub amount: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub pending: bool,
}

impl Organization {
    /// The organization's name for people: its name, else its domain.
    pub fn label(&self) -> String {
        self.name
            .clone()
            .or_else(|| self.domain.clone())
            .unwrap_or_else(|| "SimpleFIN".into())
    }

    /// What scopes an account id. Account ids are unique only within an
    /// organization, so the organization comes first in every key.
    fn key(&self) -> String {
        self.id
            .clone()
            .or_else(|| self.domain.clone())
            .or_else(|| self.sfin_url.clone())
            .or_else(|| self.name.clone())
            .unwrap_or_default()
    }
}

impl Account {
    /// A stable key for this account across syncs: organization, then id.
    pub fn key(&self) -> String {
        format!("{}/{}", self.org.key(), self.id)
    }
}

/// Read accounts, balances and (unless `balances_only`) transactions.
pub fn fetch_accounts(access: &AccessUrl, query: &AccountsQuery) -> Result<AccountSet> {
    let (base, user, pass) = access.split();
    let mut url = base;
    {
        let path = format!("{}/accounts", url.path().trim_end_matches('/'));
        url.set_path(&path);
        let mut pairs = url.query_pairs_mut();
        if let Some(start) = query.start {
            pairs.append_pair("start-date", &start.to_string());
        }
        if let Some(end) = query.end {
            pairs.append_pair("end-date", &end.to_string());
        }
        if query.balances_only {
            pairs.append_pair("balances-only", "1");
        }
    }
    if url.query() == Some("") {
        url.set_query(None);
    }

    let response = http_client()
        .get(url)
        .basic_auth(user, Some(pass))
        .send()
        .map_err(transport_error)?;
    match response.status() {
        StatusCode::OK => {}
        StatusCode::FORBIDDEN => {
            return Err(NigelError::Invalid(
                "SimpleFIN refused the stored credentials: access was revoked or has expired. \
                 Run `nigel sync setup` with a new setup token."
                    .into(),
            ))
        }
        StatusCode::PAYMENT_REQUIRED => {
            return Err(NigelError::Invalid(
                "SimpleFIN says payment is required: renew the subscription at the bridge.".into(),
            ))
        }
        status => {
            return Err(NigelError::Other(format!(
                "SimpleFIN answered with HTTP {}",
                status.as_u16()
            )))
        }
    }
    response.json::<AccountSet>().map_err(|e| {
        NigelError::Other(format!(
            "SimpleFIN sent a response Nigel cannot read: {}",
            e.without_url()
        ))
    })
}

/// A server-supplied message, made safe to print: control characters
/// removed and the length capped. The protocol requires clients to sanitize
/// these before display.
pub fn sanitize_message(raw: &str) -> String {
    const LIMIT: usize = 300;
    let clean: String = raw.chars().filter(|c| !c.is_control()).collect();
    let clean = clean.trim();
    if clean.chars().count() > LIMIT {
        let cut: String = clean.chars().take(LIMIT).collect();
        format!("{cut}…")
    } else {
        clean.to_string()
    }
}
