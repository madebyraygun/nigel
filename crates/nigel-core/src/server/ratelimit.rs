//! Sign-in rate limits, per account and per client IP: in memory and
//! process-wide like `UnlockGate`, so a restart clears them.
//!
//! An attempt is charged as a failure when it is admitted, so parallel
//! guesses cannot all pass the check before any is counted; the caller then
//! settles it as a success, as not a failure, or leaves the charge standing.

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::http::HeaderMap;

use super::config::ClientIpHeader;

pub const ACCOUNT_FAILURES: u32 = 5;
pub const IP_FAILURES: usize = 20;
pub const WINDOW: Duration = Duration::from_secs(15 * 60);
pub const LOCKOUT: Duration = Duration::from_secs(15 * 60);
/// Keys kept per bucket. Past it, idle entries go first, then the one whose
/// last failure is oldest.
pub const MAX_KEYS: usize = 10_000;
const ACCOUNT_KEY_MAX_CHARS: usize = 320;

/// Which limit an attempt tripped, for the lockout event line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bucket {
    Account,
    Ip,
}

impl Bucket {
    pub fn as_str(self) -> &'static str {
        match self {
            Bucket::Account => "account",
            Bucket::Ip => "ip",
        }
    }
}

#[derive(Debug, Default)]
struct Account {
    failures: u32,
    last_failure: Option<Instant>,
    locked_until: Option<Instant>,
}

impl Account {
    /// How long until this account admits an attempt; `None` when it does
    /// now. An expired lockout, or a count with no failure in the window,
    /// starts again from zero.
    fn wait(&mut self, now: Instant) -> Option<Duration> {
        if let Some(until) = self.locked_until {
            if now < until {
                return Some(until - now);
            }
            *self = Self::default();
        }
        if self
            .last_failure
            .is_some_and(|at| now.duration_since(at) >= WINDOW)
        {
            *self = Self::default();
        }
        None
    }

    fn idle(&self, now: Instant) -> bool {
        self.locked_until.is_none_or(|until| now >= until)
            && self
                .last_failure
                .is_none_or(|at| now.duration_since(at) >= WINDOW)
    }
}

#[derive(Debug, Default)]
struct Client {
    failures: VecDeque<Instant>,
    locked_until: Option<Instant>,
}

impl Client {
    fn prune(&mut self, now: Instant) {
        while self
            .failures
            .front()
            .is_some_and(|at| now.duration_since(*at) >= WINDOW)
        {
            self.failures.pop_front();
        }
    }

    fn wait(&mut self, now: Instant) -> Option<Duration> {
        if let Some(until) = self.locked_until {
            if now < until {
                return Some(until - now);
            }
            *self = Self::default();
        }
        None
    }

    fn idle(&mut self, now: Instant) -> bool {
        self.prune(now);
        self.failures.is_empty() && self.locked_until.is_none_or(|until| now >= until)
    }

    fn last_failure(&self) -> Option<Instant> {
        self.failures.back().copied()
    }
}

#[derive(Debug, Default)]
struct Buckets {
    accounts: HashMap<String, Account>,
    clients: HashMap<String, Client>,
}

impl Buckets {
    fn make_room(&mut self, now: Instant) {
        if self.accounts.len() >= MAX_KEYS {
            self.accounts.retain(|_, a| !a.idle(now));
        }
        while self.accounts.len() >= MAX_KEYS {
            let oldest = self
                .accounts
                .iter()
                .min_by_key(|(_, a)| a.last_failure)
                .map(|(k, _)| k.clone());
            match oldest {
                Some(key) => self.accounts.remove(&key),
                None => break,
            };
        }
        if self.clients.len() >= MAX_KEYS {
            self.clients.retain(|_, c| !c.idle(now));
        }
        while self.clients.len() >= MAX_KEYS {
            let oldest = self
                .clients
                .iter()
                .min_by_key(|(_, c)| c.last_failure())
                .map(|(k, _)| k.clone());
            match oldest {
                Some(key) => self.clients.remove(&key),
                None => break,
            };
        }
    }
}

/// One admitted attempt, already counted as a failure.
#[must_use = "settle the attempt with succeeded, not_a_failure or failed"]
#[derive(Debug)]
pub struct Attempt {
    account: String,
    ip: String,
    at: Instant,
    locked_account: bool,
    locked_ip: bool,
}

#[derive(Debug, Default)]
pub struct LoginLimiter {
    buckets: Mutex<Buckets>,
}

impl LoginLimiter {
    /// Admit an attempt and charge it, or answer how long until one would be
    /// admitted. Checked before any password is, so a locked-out guess costs
    /// no Argon2 time.
    pub fn begin(&self, account: &str, ip: &str, now: Instant) -> Result<Attempt, Duration> {
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        let mut buckets = self.buckets.lock().unwrap();
        let account_wait = buckets.accounts.get_mut(account).and_then(|a| a.wait(now));
        let ip_wait = buckets.clients.get_mut(ip).and_then(|c| c.wait(now));
        if let Some(wait) = account_wait.into_iter().chain(ip_wait).max() {
            return Err(wait);
        }
        buckets.make_room(now);

        let entry = buckets.accounts.entry(account.to_string()).or_default();
        entry.failures += 1;
        entry.last_failure = Some(now);
        let locked_account = entry.failures >= ACCOUNT_FAILURES;
        if locked_account {
            entry.locked_until = Some(now + LOCKOUT);
        }

        let client = buckets.clients.entry(ip.to_string()).or_default();
        client.prune(now);
        client.failures.push_back(now);
        let locked_ip = client.failures.len() >= IP_FAILURES;
        if locked_ip {
            client.locked_until = Some(now + LOCKOUT);
        }

        Ok(Attempt {
            account: account.to_string(),
            ip: ip.to_string(),
            at: now,
            locked_account,
            locked_ip,
        })
    }

    /// The attempt signed in: the account starts again and the attempt no
    /// longer counts against the IP.
    pub fn succeeded(&self, attempt: Attempt) {
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        let mut buckets = self.buckets.lock().unwrap();
        buckets.accounts.remove(&attempt.account);
        refund_ip(&mut buckets, &attempt);
    }

    /// The attempt was not a failure (`totp_required`, or the server could
    /// not judge it): take back what `begin` charged.
    pub fn not_a_failure(&self, attempt: Attempt) {
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        let mut buckets = self.buckets.lock().unwrap();
        if let Some(account) = buckets.accounts.get_mut(&attempt.account) {
            account.failures = account.failures.saturating_sub(1);
            if attempt.locked_account {
                account.locked_until = None;
            }
        }
        refund_ip(&mut buckets, &attempt);
    }

    /// The attempt failed and its charge stands. Answers the buckets it
    /// locked, for the event line.
    pub fn failed(&self, attempt: Attempt) -> Vec<Bucket> {
        let mut locked = Vec::new();
        if attempt.locked_account {
            locked.push(Bucket::Account);
        }
        if attempt.locked_ip {
            locked.push(Bucket::Ip);
        }
        locked
    }

    #[cfg(test)]
    fn sizes(&self) -> (usize, usize) {
        let buckets = self.buckets.lock().unwrap();
        (buckets.accounts.len(), buckets.clients.len())
    }
}

fn refund_ip(buckets: &mut Buckets, attempt: &Attempt) {
    if let Some(client) = buckets.clients.get_mut(&attempt.ip) {
        if let Some(position) = client.failures.iter().position(|at| *at == attempt.at) {
            client.failures.remove(position);
        }
        if attempt.locked_ip {
            client.locked_until = None;
        }
    }
}

/// The account bucket's key: the normalized email, or for anything that is
/// not an address the trimmed, lower-cased input — so unknown and malformed
/// addresses lock out exactly as real ones do. Cut to 320 characters.
pub fn account_key(email: &str) -> String {
    let key = crate::identity::users::normalize_email(email)
        .unwrap_or_else(|_| email.trim().to_lowercase());
    key.chars().take(ACCOUNT_KEY_MAX_CHARS).collect()
}

/// The client's address for the IP bucket: the TCP peer, unless the peer is
/// loopback and `header` is configured, when it is the address that header
/// carries (`CF-Connecting-IP` as given, the rightmost `X-Forwarded-For`
/// entry). A missing header, or one that does not hold an address, falls
/// back to the peer. No peer at all is `"unknown"`.
pub fn client_ip(
    peer: Option<SocketAddr>,
    headers: &HeaderMap,
    header: Option<ClientIpHeader>,
) -> String {
    let Some(peer) = peer else {
        return "unknown".to_string();
    };
    let forwarded = header
        .filter(|_| peer.ip().is_loopback())
        .and_then(|header| {
            let values = headers
                .get_all(header.header_name())
                .iter()
                .filter_map(|v| v.to_str().ok());
            match header {
                ClientIpHeader::CfConnectingIp => values.map(str::trim).next().map(str::to_string),
                ClientIpHeader::XForwardedFor => values
                    .flat_map(|v| v.split(','))
                    .map(str::trim)
                    .rfind(|v| !v.is_empty())
                    .map(str::to_string),
            }
        })
        .and_then(|value| value.parse::<IpAddr>().ok());
    forwarded.unwrap_or(peer.ip()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    const PAT: &str = "pat@example.com";
    const IP: &str = "203.0.113.9";

    fn fail(limiter: &LoginLimiter, account: &str, ip: &str, at: Instant) -> Vec<Bucket> {
        limiter.failed(limiter.begin(account, ip, at).expect("admitted"))
    }

    #[test]
    fn five_failures_lock_an_account_for_fifteen_minutes() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for i in 0..4 {
            assert!(fail(&limiter, PAT, IP, t0).is_empty(), "failure {i}");
        }
        assert_eq!(fail(&limiter, PAT, IP, t0), [Bucket::Account]);
        let wait = limiter.begin(PAT, "198.51.100.7", t0).unwrap_err();
        assert_eq!(wait, LOCKOUT);
        let later = t0 + Duration::from_secs(60);
        assert_eq!(
            limiter.begin(PAT, IP, later).unwrap_err(),
            LOCKOUT - Duration::from_secs(60)
        );
        let after = t0 + LOCKOUT;
        let attempt = limiter
            .begin(PAT, IP, after)
            .expect("the lockout has passed");
        limiter.failed(attempt);
        assert!(
            limiter.begin(PAT, IP, after).is_ok(),
            "the count started again"
        );
    }

    #[test]
    fn a_success_resets_the_account_count() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for _ in 0..4 {
            fail(&limiter, PAT, IP, t0);
        }
        for _ in 0..IP_FAILURES {
            limiter.succeeded(limiter.begin(PAT, IP, t0).unwrap());
        }
        for _ in 0..4 {
            assert!(fail(&limiter, PAT, IP, t0).is_empty());
        }
    }

    #[test]
    fn an_unknown_address_locks_like_a_known_one() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for _ in 0..5 {
            fail(&limiter, "nobody@example.com", IP, t0);
        }
        assert!(limiter
            .begin("nobody@example.com", "198.51.100.7", t0)
            .is_err());
    }

    #[test]
    fn a_totp_required_answer_is_not_a_failure() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for _ in 0..3 {
            fail(&limiter, PAT, IP, t0);
        }
        for _ in 0..IP_FAILURES {
            limiter.not_a_failure(limiter.begin(PAT, IP, t0).unwrap());
        }
        assert!(
            fail(&limiter, PAT, IP, t0).is_empty(),
            "still the fourth failure"
        );
        assert_eq!(fail(&limiter, PAT, IP, t0), [Bucket::Account], "the fifth");
    }

    #[test]
    fn an_account_count_lapses_after_fifteen_quiet_minutes() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for _ in 0..4 {
            fail(&limiter, PAT, IP, t0);
        }
        let later = t0 + WINDOW;
        for _ in 0..4 {
            assert!(fail(&limiter, PAT, IP, later).is_empty());
        }
    }

    #[test]
    fn twenty_failures_from_one_ip_in_the_window_lock_the_ip() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for i in 0..19 {
            let account = format!("user{i}@example.com");
            assert!(fail(&limiter, &account, IP, t0 + Duration::from_secs(i)).is_empty());
        }
        let locks = fail(
            &limiter,
            "user19@example.com",
            IP,
            t0 + Duration::from_secs(19),
        );
        assert_eq!(locks, [Bucket::Ip]);
        assert!(limiter
            .begin("fresh@example.com", IP, t0 + Duration::from_secs(20))
            .is_err());
        assert!(limiter
            .begin("fresh@example.com", "198.51.100.7", t0)
            .is_ok());
    }

    #[test]
    fn the_ip_window_slides() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for i in 0..19 {
            fail(&limiter, &format!("user{i}@example.com"), IP, t0);
        }
        let slid = t0 + WINDOW;
        assert!(
            fail(&limiter, "late@example.com", IP, slid).is_empty(),
            "the first 19 aged out"
        );
    }

    #[test]
    fn concurrent_attempts_are_charged_before_they_are_checked() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        let held: Vec<Attempt> = (0..5)
            .map(|_| limiter.begin(PAT, IP, t0).unwrap())
            .collect();
        assert!(limiter.begin(PAT, IP, t0).is_err(), "the sixth waits");
        drop(held);
    }

    #[test]
    fn memory_stays_bounded() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for i in 0..(MAX_KEYS + 50) {
            let ip = format!("10.0.{}.{}", i / 256, i % 256);
            fail(&limiter, &format!("user{i}@example.com"), &ip, t0);
        }
        let (accounts, clients) = limiter.sizes();
        assert!(
            accounts <= MAX_KEYS && clients <= MAX_KEYS,
            "{accounts} {clients}"
        );
    }

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.append(*name, HeaderValue::from_str(value).unwrap());
        }
        map
    }

    #[test]
    fn client_ip_trusts_the_header_only_from_loopback() {
        let loopback: SocketAddr = "127.0.0.1:50000".parse().unwrap();
        let remote: SocketAddr = "198.51.100.7:50000".parse().unwrap();
        let cf = Some(ClientIpHeader::CfConnectingIp);
        let xff = Some(ClientIpHeader::XForwardedFor);
        let forged = headers(&[("cf-connecting-ip", "203.0.113.9")]);

        assert_eq!(client_ip(Some(loopback), &forged, cf), "203.0.113.9");
        assert_eq!(client_ip(Some(remote), &forged, cf), "198.51.100.7");
        assert_eq!(client_ip(Some(loopback), &forged, None), "127.0.0.1");
        assert_eq!(
            client_ip(
                Some(loopback),
                &headers(&[("x-forwarded-for", "203.0.113.9, 198.51.100.20")]),
                xff
            ),
            "198.51.100.20"
        );
        assert_eq!(
            client_ip(
                Some(loopback),
                &headers(&[
                    ("x-forwarded-for", "203.0.113.9"),
                    ("x-forwarded-for", "198.51.100.30")
                ]),
                xff
            ),
            "198.51.100.30"
        );
        assert_eq!(
            client_ip(
                Some(loopback),
                &headers(&[("cf-connecting-ip", "not-an-ip")]),
                cf
            ),
            "127.0.0.1"
        );
        assert_eq!(client_ip(None, &forged, cf), "unknown");
    }

    #[test]
    fn the_account_key_is_the_normalized_email_or_the_trimmed_input() {
        assert_eq!(account_key(" Pat@Example.com "), "pat@example.com");
        assert_eq!(account_key("  Not An Email "), "not an email");
        assert_eq!(account_key(&"a".repeat(1000)).chars().count(), 320);
    }
}
