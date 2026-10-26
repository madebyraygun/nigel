//! Sign-in rate limits, per account and per client IP: in memory and
//! process-wide like `UnlockGate`, so a restart clears them.
//!
//! An attempt is charged as a failure when it is admitted, so parallel
//! guesses cannot all pass the check before any is counted; the caller then
//! settles it as a success, as not a failure, or leaves the charge standing.

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::http::HeaderMap;
use sha2::{Digest, Sha256};

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
}

/// What eviction needs to know about an entry in either bucket.
trait Entry {
    /// Nothing in it still counts: no lockout running, no failure in the
    /// window.
    fn idle(&mut self, now: Instant) -> bool;
    fn locked_until(&self) -> Option<Instant>;
    /// Eviction order among unlocked entries: fewest failures, then the
    /// oldest last failure.
    fn rank(&self) -> (usize, Option<Instant>);
}

impl Entry for Account {
    fn idle(&mut self, now: Instant) -> bool {
        self.locked_until.is_none_or(|until| now >= until)
            && self
                .last_failure
                .is_none_or(|at| now.duration_since(at) >= WINDOW)
    }

    fn locked_until(&self) -> Option<Instant> {
        self.locked_until
    }

    fn rank(&self) -> (usize, Option<Instant>) {
        (self.failures as usize, self.last_failure)
    }
}

impl Entry for Client {
    fn idle(&mut self, now: Instant) -> bool {
        self.prune(now);
        self.failures.is_empty() && self.locked_until.is_none_or(|until| now >= until)
    }

    fn locked_until(&self) -> Option<Instant> {
        self.locked_until
    }

    fn rank(&self) -> (usize, Option<Instant>) {
        (self.failures.len(), self.failures.back().copied())
    }
}

/// Make room for `key` in `map` if it is new and the map is full: drop idle
/// entries, then the lowest-ranked unlocked one. A running lockout is never
/// evicted; when every entry is locked the new key is refused until the
/// first lockout ends.
fn make_room<E: Entry>(
    map: &mut HashMap<String, E>,
    key: &str,
    now: Instant,
) -> Result<(), Duration> {
    if map.len() < MAX_KEYS || map.contains_key(key) {
        return Ok(());
    }
    map.retain(|_, entry| !entry.idle(now));
    while map.len() >= MAX_KEYS {
        let victim = map
            .iter()
            .filter(|(_, entry)| entry.locked_until().is_none_or(|until| now >= until))
            .min_by_key(|(_, entry)| entry.rank())
            .map(|(key, _)| key.clone());
        match victim {
            Some(victim) => {
                map.remove(&victim);
            }
            None => {
                let first_unlock = map
                    .values()
                    .filter_map(|entry| entry.locked_until())
                    .min()
                    .map_or(Duration::ZERO, |until| until - now);
                return Err(first_unlock);
            }
        }
    }
    Ok(())
}

#[derive(Debug, Default)]
struct Buckets {
    accounts: HashMap<String, Account>,
    clients: HashMap<String, Client>,
}

/// One admitted attempt, already counted as a failure.
#[must_use = "settle the attempt with succeeded, not_a_failure or failed"]
#[derive(Debug)]
pub struct Attempt {
    account: String,
    ip: String,
    at: Instant,
    /// The account's last failure before this attempt was charged.
    previous_last_failure: Option<Instant>,
    /// The lockouts this attempt started, as their end instants.
    account_lock: Option<Instant>,
    ip_lock: Option<Instant>,
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
        make_room(&mut buckets.accounts, account, now)?;
        make_room(&mut buckets.clients, ip, now)?;

        let entry = buckets.accounts.entry(account.to_string()).or_default();
        let previous_last_failure = entry.last_failure;
        entry.failures += 1;
        entry.last_failure = Some(now);
        let account_lock = (entry.failures >= ACCOUNT_FAILURES).then(|| now + LOCKOUT);
        if account_lock.is_some() {
            entry.locked_until = account_lock;
        }

        let client = buckets.clients.entry(ip.to_string()).or_default();
        client.prune(now);
        client.failures.push_back(now);
        let ip_lock = (client.failures.len() >= IP_FAILURES).then(|| now + LOCKOUT);
        if ip_lock.is_some() {
            client.locked_until = ip_lock;
        }

        Ok(Attempt {
            account: account.to_string(),
            ip: ip.to_string(),
            at: now,
            previous_last_failure,
            account_lock,
            ip_lock,
        })
    }

    /// The attempt signed in: the account starts again and the attempt no
    /// longer counts against the IP. A lockout another attempt started
    /// stands.
    pub fn succeeded(&self, attempt: Attempt) {
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        let mut buckets = self.buckets.lock().unwrap();
        let foreign_lock = buckets
            .accounts
            .get(&attempt.account)
            .and_then(|a| a.locked_until)
            .is_some_and(|until| Some(until) != attempt.account_lock);
        if !foreign_lock {
            buckets.accounts.remove(&attempt.account);
        }
        refund_ip(&mut buckets, &attempt);
    }

    /// The attempt was not a failure (`totp_required`, or the server could
    /// not judge it): take back what `begin` charged, including the lockout
    /// it started, if one did.
    pub fn not_a_failure(&self, attempt: Attempt) {
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        let mut buckets = self.buckets.lock().unwrap();
        if let Some(account) = buckets.accounts.get_mut(&attempt.account) {
            account.failures = account.failures.saturating_sub(1);
            if account.last_failure == Some(attempt.at) {
                account.last_failure = attempt.previous_last_failure;
            }
            if attempt.account_lock.is_some() && account.locked_until == attempt.account_lock {
                account.locked_until = None;
            }
            if account.failures == 0 && account.locked_until.is_none() {
                buckets.accounts.remove(&attempt.account);
            }
        }
        refund_ip(&mut buckets, &attempt);
    }

    /// The attempt failed and its charge stands. Answers the buckets it
    /// locked, for the event line.
    pub fn failed(&self, attempt: Attempt) -> Vec<Bucket> {
        let mut locked = Vec::new();
        if attempt.account_lock.is_some() {
            locked.push(Bucket::Account);
        }
        if attempt.ip_lock.is_some() {
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
        if attempt.ip_lock.is_some() && client.locked_until == attempt.ip_lock {
            client.locked_until = None;
        }
        if client.failures.is_empty() && client.locked_until.is_none() {
            buckets.clients.remove(&attempt.ip);
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

/// Who an attempt came from, for the IP bucket and the event line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientIp {
    /// The IP bucket's key: an IPv4 address, or an IPv6 address's /64 (an
    /// IPv4-mapped address counts as IPv4). A forwarded header that holds no
    /// address is keyed `fwd:` plus a hash of its value.
    pub bucket: String,
    /// For the event line: the full address, `invalid-forwarded`, or
    /// `unknown`. Never the raw header value.
    pub log: String,
}

impl ClientIp {
    fn of(ip: IpAddr) -> Self {
        let bucket = match ip.to_canonical() {
            IpAddr::V4(v4) => v4.to_string(),
            IpAddr::V6(v6) => format!("{}/64", Ipv6Addr::from(u128::from(v6) & (!0u128 << 64))),
        };
        Self {
            bucket,
            log: ip.to_string(),
        }
    }

    fn invalid_forwarded(raw: &[u8]) -> Self {
        let digest = Sha256::digest(raw);
        Self {
            bucket: format!("fwd:{}", hex::encode(&digest[..8])),
            log: "invalid-forwarded".to_string(),
        }
    }

    fn unknown() -> Self {
        Self {
            bucket: "unknown".to_string(),
            log: "unknown".to_string(),
        }
    }
}

/// The client behind a request: the TCP peer, unless the peer is loopback
/// and `header` is configured, when it is the address that header carries
/// (`CF-Connecting-IP` as given, the rightmost `X-Forwarded-For` entry), with
/// any port or brackets stripped. A configured header that is absent falls
/// back to the peer; one that is present but holds no address gets its own
/// opaque bucket rather than the proxy's. No peer at all is `unknown`.
pub fn client_ip(
    peer: Option<SocketAddr>,
    headers: &HeaderMap,
    header: Option<ClientIpHeader>,
) -> ClientIp {
    let Some(peer) = peer else {
        return ClientIp::unknown();
    };
    let trusted = header.filter(|_| peer.ip().to_canonical().is_loopback());
    let Some(header) = trusted else {
        return ClientIp::of(peer.ip());
    };
    let raw: Vec<&[u8]> = headers
        .get_all(header.header_name())
        .iter()
        .map(|v| v.as_bytes())
        .collect();
    if raw.is_empty() {
        return ClientIp::of(peer.ip());
    }
    let lines: Option<Vec<&str>> = raw.iter().map(|b| std::str::from_utf8(b).ok()).collect();
    let value = lines.and_then(|lines| match header {
        ClientIpHeader::CfConnectingIp => lines.first().map(|v| v.trim()),
        ClientIpHeader::XForwardedFor => lines
            .into_iter()
            .flat_map(|v| v.split(','))
            .map(str::trim)
            .rfind(|v| !v.is_empty()),
    });
    match value.and_then(parse_forwarded) {
        Some(ip) => ClientIp::of(ip),
        None => ClientIp::invalid_forwarded(&raw.join(&b'\n')),
    }
}

/// An address as a proxy may write it: bare, `ip:port`, `[v6]` or
/// `[v6]:port`.
fn parse_forwarded(value: &str) -> Option<IpAddr> {
    value
        .parse::<IpAddr>()
        .ok()
        .or_else(|| value.parse::<SocketAddr>().ok().map(|s| s.ip()))
        .or_else(|| {
            let inner = value.strip_prefix('[')?.strip_suffix(']')?;
            inner.parse::<Ipv6Addr>().ok().map(IpAddr::V6)
        })
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

    fn flood(limiter: &LoginLimiter, count: usize, at: Instant) {
        for i in 0..count {
            let ip = format!("10.{}.{}.{}", i / 65536, (i / 256) % 256, i % 256);
            fail(limiter, &format!("flood{i}@example.com"), &ip, at);
        }
    }

    #[test]
    fn a_flood_of_new_keys_cannot_evict_a_locked_account() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for _ in 0..ACCOUNT_FAILURES {
            fail(&limiter, PAT, IP, t0);
        }
        flood(&limiter, MAX_KEYS, t0 + Duration::from_secs(1));
        let later = t0 + Duration::from_secs(2);
        assert!(limiter.begin(PAT, "198.51.100.7", later).is_err());
    }

    #[test]
    fn a_flood_of_new_keys_cannot_evict_a_locked_ip() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for i in 0..IP_FAILURES {
            fail(&limiter, &format!("user{i}@example.com"), IP, t0);
        }
        flood(&limiter, MAX_KEYS, t0 + Duration::from_secs(1));
        let later = t0 + Duration::from_secs(2);
        assert!(limiter.begin("fresh@example.com", IP, later).is_err());
    }

    #[test]
    fn an_existing_key_at_the_cap_evicts_no_one() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        flood(&limiter, MAX_KEYS, t0);
        fail(&limiter, "flood0@example.com", "10.0.0.0", t0);
        assert_eq!(limiter.sizes(), (MAX_KEYS, MAX_KEYS));
    }

    #[test]
    fn eviction_takes_the_fewest_failures_before_the_oldest() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for _ in 0..(ACCOUNT_FAILURES - 1) {
            fail(&limiter, PAT, IP, t0);
        }
        flood(&limiter, MAX_KEYS - 1, t0 + Duration::from_secs(1));
        fail(
            &limiter,
            "new@example.com",
            "198.51.100.7",
            t0 + Duration::from_secs(2),
        );
        let locks = fail(&limiter, PAT, IP, t0 + Duration::from_secs(3));
        assert_eq!(locks, [Bucket::Account], "the older, busier count survived");
    }

    #[test]
    fn a_table_full_of_locked_accounts_refuses_a_new_one() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for i in 0..MAX_KEYS {
            let ip = format!("10.{}.{}.{}", i / 65536, (i / 256) % 256, i % 256);
            for _ in 0..ACCOUNT_FAILURES {
                fail(&limiter, &format!("flood{i}@example.com"), &ip, t0);
            }
        }
        let later = t0 + Duration::from_secs(60);
        let wait = limiter.begin(PAT, IP, later).unwrap_err();
        assert_eq!(wait, LOCKOUT - Duration::from_secs(60));
        assert_eq!(limiter.sizes().0, MAX_KEYS);
    }

    #[test]
    fn a_totp_required_answer_does_not_hold_a_slot() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        limiter.not_a_failure(limiter.begin(PAT, IP, t0).unwrap());
        assert_eq!(limiter.sizes(), (0, 0));
    }

    #[test]
    fn a_totp_required_answer_does_not_delay_the_lapse() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for _ in 0..4 {
            fail(&limiter, PAT, IP, t0);
        }
        let mid = t0 + Duration::from_secs(10 * 60);
        limiter.not_a_failure(limiter.begin(PAT, IP, mid).unwrap());
        let lapsed = t0 + WINDOW;
        assert!(
            fail(&limiter, PAT, IP, lapsed).is_empty(),
            "the count lapsed"
        );
    }

    #[test]
    fn settling_an_attempt_clears_only_the_account_lock_it_set() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for _ in 0..4 {
            fail(&limiter, PAT, IP, t0);
        }
        let stale = limiter.begin(PAT, IP, t0).unwrap();
        let later = t0 + LOCKOUT;
        for i in 0..ACCOUNT_FAILURES {
            fail(&limiter, PAT, &format!("198.51.100.{i}"), later);
        }
        limiter.not_a_failure(stale);
        assert!(limiter.begin(PAT, "198.51.100.99", later).is_err());
    }

    #[test]
    fn settling_an_attempt_clears_only_the_ip_lock_it_set() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for i in 0..(IP_FAILURES - 1) {
            fail(&limiter, &format!("user{i}@example.com"), IP, t0);
        }
        let stale = limiter.begin("last@example.com", IP, t0).unwrap();
        let later = t0 + LOCKOUT;
        for i in 0..IP_FAILURES {
            fail(&limiter, &format!("again{i}@example.com"), IP, later);
        }
        limiter.not_a_failure(stale);
        assert!(limiter.begin("fresh@example.com", IP, later).is_err());
    }

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.append(*name, HeaderValue::from_str(value).unwrap());
        }
        map
    }

    const CF: Option<ClientIpHeader> = Some(ClientIpHeader::CfConnectingIp);
    const XFF: Option<ClientIpHeader> = Some(ClientIpHeader::XForwardedFor);

    fn resolve(
        peer: &str,
        pairs: &[(&'static str, &str)],
        header: Option<ClientIpHeader>,
    ) -> ClientIp {
        client_ip(Some(peer.parse().unwrap()), &headers(pairs), header)
    }

    fn plain(address: &str) -> ClientIp {
        ClientIp {
            bucket: address.to_string(),
            log: address.to_string(),
        }
    }

    #[test]
    fn client_ip_trusts_the_header_only_from_loopback() {
        let forged = [("cf-connecting-ip", "203.0.113.9")];
        assert_eq!(
            resolve("127.0.0.1:50000", &forged, CF),
            plain("203.0.113.9")
        );
        assert_eq!(resolve("[::1]:50000", &forged, CF), plain("203.0.113.9"));
        assert_eq!(
            resolve("[::ffff:127.0.0.1]:50000", &forged, CF),
            plain("203.0.113.9")
        );
        assert_eq!(
            resolve("198.51.100.7:50000", &forged, CF),
            plain("198.51.100.7")
        );
        assert_eq!(
            resolve("127.0.0.1:50000", &forged, None),
            plain("127.0.0.1")
        );
        assert_eq!(client_ip(None, &headers(&forged), CF), plain("unknown"));
    }

    #[test]
    fn client_ip_takes_the_rightmost_forwarded_for_entry() {
        let one_line = [("x-forwarded-for", "203.0.113.9, 198.51.100.20")];
        assert_eq!(
            resolve("127.0.0.1:1", &one_line, XFF),
            plain("198.51.100.20")
        );
        let two_lines = [
            ("x-forwarded-for", "203.0.113.9"),
            ("x-forwarded-for", "198.51.100.30"),
        ];
        assert_eq!(
            resolve("127.0.0.1:1", &two_lines, XFF),
            plain("198.51.100.30")
        );
    }

    #[test]
    fn client_ip_strips_a_port_and_brackets_from_the_header() {
        let v4_port = [("cf-connecting-ip", "203.0.113.9:4711")];
        assert_eq!(resolve("127.0.0.1:1", &v4_port, CF), plain("203.0.113.9"));
        let v6_port = [("x-forwarded-for", "198.51.100.1, [2001:db8::5]:443")];
        let v6_bracketed = [("cf-connecting-ip", "[2001:db8::5]")];
        let expected = ClientIp {
            bucket: "2001:db8::/64".to_string(),
            log: "2001:db8::5".to_string(),
        };
        assert_eq!(resolve("127.0.0.1:1", &v6_port, XFF), expected);
        assert_eq!(resolve("127.0.0.1:1", &v6_bracketed, CF), expected);
    }

    #[test]
    fn an_unparseable_header_gets_its_own_opaque_bucket() {
        let garbage = resolve("127.0.0.1:1", &[("cf-connecting-ip", "not-an-ip")], CF);
        assert_eq!(garbage.log, "invalid-forwarded");
        assert!(garbage.bucket.starts_with("fwd:"), "{}", garbage.bucket);
        assert!(!garbage.bucket.contains("not-an-ip"));
        assert_ne!(garbage.bucket, "127.0.0.1");
        let again = resolve("127.0.0.1:1", &[("cf-connecting-ip", "not-an-ip")], CF);
        assert_eq!(again.bucket, garbage.bucket);
        let other = resolve("127.0.0.1:1", &[("cf-connecting-ip", "other-junk")], CF);
        assert_ne!(other.bucket, garbage.bucket);
        let bad_xff = resolve(
            "127.0.0.1:1",
            &[("x-forwarded-for", "203.0.113.9, junk")],
            XFF,
        );
        assert_eq!(bad_xff.log, "invalid-forwarded");
    }

    #[test]
    fn an_absent_header_falls_back_to_the_peer() {
        assert_eq!(resolve("127.0.0.1:1", &[], CF), plain("127.0.0.1"));
        assert_eq!(
            resolve("127.0.0.1:1", &[("cf-connecting-ip", "203.0.113.9")], XFF),
            plain("127.0.0.1")
        );
    }

    #[test]
    fn ipv6_clients_share_a_bucket_per_64_prefix() {
        let a = resolve("[2001:db8:1:2:aaaa::1]:1", &[], None);
        let b = resolve("[2001:db8:1:2:bbbb::9]:1", &[], None);
        let c = resolve("[2001:db8:1:3::1]:1", &[], None);
        assert_eq!(a.bucket, "2001:db8:1:2::/64");
        assert_eq!(a.log, "2001:db8:1:2:aaaa::1");
        assert_eq!(b.bucket, a.bucket);
        assert_ne!(c.bucket, a.bucket);
    }

    #[test]
    fn an_ipv4_mapped_address_shares_the_ipv4_bucket() {
        let mapped = resolve("[::ffff:198.51.100.7]:1", &[], None);
        assert_eq!(mapped.bucket, "198.51.100.7");
        assert_eq!(mapped.log, "::ffff:198.51.100.7");
        let forwarded = resolve(
            "127.0.0.1:1",
            &[("cf-connecting-ip", "::ffff:203.0.113.9")],
            CF,
        );
        assert_eq!(forwarded.bucket, "203.0.113.9");
        assert_eq!(forwarded.log, "::ffff:203.0.113.9");
    }

    #[test]
    fn the_account_key_is_the_normalized_email_or_the_trimmed_input() {
        assert_eq!(account_key(" Pat@Example.com "), "pat@example.com");
        assert_eq!(account_key("  Not An Email "), "not an email");
        assert_eq!(account_key(&"a".repeat(1000)).chars().count(), 320);
    }
}
