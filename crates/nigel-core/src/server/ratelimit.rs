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
    /// The key is a real user's: exempt from `MAX_KEYS` and never evicted,
    /// since there are only as many such entries as users.
    known: bool,
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
    /// Outside the cap and never evicted.
    fn exempt(&self) -> bool {
        false
    }
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

    fn exempt(&self) -> bool {
        self.known
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

/// Whether a bucket can track a key.
enum Room {
    /// The key is held already, or is exempt from the cap.
    Held,
    /// A new key fits.
    Free,
    /// A new key fits once this unlocked entry is evicted.
    Evict(String),
    /// Every capped entry is a running lockout: the key goes untracked.
    Full,
}

/// Decide whether `map` can take `key`, leaving the eviction to the caller
/// so no bucket changes until every bucket has been asked. Idle entries,
/// which hold no count or lockout, may be dropped on the way. The victim is
/// the unlocked capped entry with the fewest failures, then the oldest; a
/// running lockout is never chosen.
fn plan_room<E: Entry>(
    map: &mut HashMap<String, E>,
    key: &str,
    exempt_key: bool,
    capacity: usize,
    now: Instant,
) -> Room {
    if exempt_key || map.contains_key(key) {
        return Room::Held;
    }
    let capped = |map: &HashMap<String, E>| map.values().filter(|e| !e.exempt()).count();
    if map.len() < capacity || capped(map) < capacity {
        return Room::Free;
    }
    map.retain(|_, entry| !entry.idle(now));
    if capped(map) < capacity {
        return Room::Free;
    }
    map.iter()
        .filter(|(_, entry)| !entry.exempt())
        .filter(|(_, entry)| entry.locked_until().is_none_or(|until| now >= until))
        .min_by_key(|(_, entry)| entry.rank())
        .map_or(Room::Full, |(key, _)| Room::Evict(key.clone()))
}

/// Act on a plan: evict if it says to, and answer whether the key is
/// tracked and whether this is the first untracked key since the bucket
/// last had room.
fn take_room<E>(
    map: &mut HashMap<String, E>,
    room: Room,
    reported_full: &mut bool,
) -> (bool, bool) {
    match room {
        Room::Held => (true, false),
        Room::Free => {
            *reported_full = false;
            (true, false)
        }
        Room::Evict(victim) => {
            map.remove(&victim);
            *reported_full = false;
            (true, false)
        }
        Room::Full => (false, !std::mem::replace(reported_full, true)),
    }
}

#[derive(Debug, Default)]
struct Buckets {
    accounts: HashMap<String, Account>,
    clients: HashMap<String, Client>,
    accounts_full_reported: bool,
    clients_full_reported: bool,
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
    /// Whether each bucket holds an entry for this attempt; one that was
    /// full of lockouts admits it untracked.
    account_tracked: bool,
    ip_tracked: bool,
    newly_full: Vec<Bucket>,
}

impl Attempt {
    /// The buckets this attempt found full of lockouts when none had been
    /// reported since they last had room: once per flood, for the operator's
    /// log.
    pub fn newly_full(&self) -> &[Bucket] {
        &self.newly_full
    }
}

#[derive(Debug)]
pub struct LoginLimiter {
    buckets: Mutex<Buckets>,
    capacity: usize,
}

impl Default for LoginLimiter {
    fn default() -> Self {
        Self {
            buckets: Mutex::default(),
            capacity: MAX_KEYS,
        }
    }
}

impl LoginLimiter {
    /// Admit an attempt and charge it, or answer how long until one would be
    /// admitted. Checked before any password is, so a locked-out guess costs
    /// no Argon2 time.
    ///
    /// `known_user` says whether `account` is a real user's key; the caller
    /// looks the email up before verifying any password. Real users' entries
    /// sit outside `MAX_KEYS` and are never evicted. When a bucket is full of
    /// lockouts a new key is admitted untracked rather than refused: an
    /// unknown address has no account to protect, and a real account keeps
    /// its own bucket whatever the IP table holds.
    pub fn begin(
        &self,
        account: &str,
        known_user: bool,
        ip: &str,
        now: Instant,
    ) -> Result<Attempt, Duration> {
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        let mut guard = self.buckets.lock().unwrap();
        let buckets = &mut *guard;
        let account_wait = buckets.accounts.get_mut(account).and_then(|a| a.wait(now));
        let ip_wait = buckets.clients.get_mut(ip).and_then(|c| c.wait(now));
        if let Some(wait) = account_wait.into_iter().chain(ip_wait).max() {
            return Err(wait);
        }
        let account_room = plan_room(
            &mut buckets.accounts,
            account,
            known_user,
            self.capacity,
            now,
        );
        let ip_room = plan_room(&mut buckets.clients, ip, false, self.capacity, now);
        let (account_tracked, accounts_newly_full) = take_room(
            &mut buckets.accounts,
            account_room,
            &mut buckets.accounts_full_reported,
        );
        let (ip_tracked, clients_newly_full) = take_room(
            &mut buckets.clients,
            ip_room,
            &mut buckets.clients_full_reported,
        );
        let mut newly_full = Vec::new();
        if accounts_newly_full {
            newly_full.push(Bucket::Account);
        }
        if clients_newly_full {
            newly_full.push(Bucket::Ip);
        }

        let mut previous_last_failure = None;
        let mut account_lock = None;
        if account_tracked {
            let entry = buckets.accounts.entry(account.to_string()).or_default();
            entry.known |= known_user;
            previous_last_failure = entry.last_failure;
            entry.failures += 1;
            entry.last_failure = Some(now);
            account_lock = (entry.failures >= ACCOUNT_FAILURES).then(|| now + LOCKOUT);
            if account_lock.is_some() {
                entry.locked_until = account_lock;
            }
        }

        let mut ip_lock = None;
        if ip_tracked {
            let client = buckets.clients.entry(ip.to_string()).or_default();
            client.prune(now);
            client.failures.push_back(now);
            ip_lock = (client.failures.len() >= IP_FAILURES).then(|| now + LOCKOUT);
            if ip_lock.is_some() {
                client.locked_until = ip_lock;
            }
        }

        Ok(Attempt {
            account: account.to_string(),
            ip: ip.to_string(),
            at: now,
            previous_last_failure,
            account_lock,
            ip_lock,
            account_tracked,
            ip_tracked,
            newly_full,
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
        if attempt.account_tracked && !foreign_lock {
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
        let tracked = buckets
            .accounts
            .get_mut(&attempt.account)
            .filter(|_| attempt.account_tracked);
        if let Some(account) = tracked {
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
    fn with_capacity(capacity: usize) -> Self {
        Self {
            buckets: Mutex::default(),
            capacity,
        }
    }

    #[cfg(test)]
    fn account_keys(&self) -> Vec<String> {
        let buckets = self.buckets.lock().unwrap();
        buckets.accounts.keys().cloned().collect()
    }

    #[cfg(test)]
    fn sizes(&self) -> (usize, usize) {
        let buckets = self.buckets.lock().unwrap();
        (buckets.accounts.len(), buckets.clients.len())
    }
}

fn refund_ip(buckets: &mut Buckets, attempt: &Attempt) {
    let tracked = buckets
        .clients
        .get_mut(&attempt.ip)
        .filter(|_| attempt.ip_tracked);
    if let Some(client) = tracked {
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

    const NOBODY: &str = "nobody@example.com";

    /// PAT is the one real user in these tests; every other address is
    /// unknown.
    fn begin(
        limiter: &LoginLimiter,
        account: &str,
        ip: &str,
        at: Instant,
    ) -> Result<Attempt, Duration> {
        limiter.begin(account, account == PAT, ip, at)
    }

    fn fail(limiter: &LoginLimiter, account: &str, ip: &str, at: Instant) -> Vec<Bucket> {
        limiter.failed(begin(limiter, account, ip, at).expect("admitted"))
    }

    #[test]
    fn five_failures_lock_an_account_for_fifteen_minutes() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for i in 0..4 {
            assert!(fail(&limiter, PAT, IP, t0).is_empty(), "failure {i}");
        }
        assert_eq!(fail(&limiter, PAT, IP, t0), [Bucket::Account]);
        let wait = begin(&limiter, PAT, "198.51.100.7", t0).unwrap_err();
        assert_eq!(wait, LOCKOUT);
        let later = t0 + Duration::from_secs(60);
        assert_eq!(
            begin(&limiter, PAT, IP, later).unwrap_err(),
            LOCKOUT - Duration::from_secs(60)
        );
        let after = t0 + LOCKOUT;
        let attempt = begin(&limiter, PAT, IP, after).expect("the lockout has passed");
        limiter.failed(attempt);
        assert!(
            begin(&limiter, PAT, IP, after).is_ok(),
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
            limiter.succeeded(begin(&limiter, PAT, IP, t0).unwrap());
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
        assert!(begin(&limiter, "nobody@example.com", "198.51.100.7", t0).is_err());
    }

    #[test]
    fn a_totp_required_answer_is_not_a_failure() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for _ in 0..3 {
            fail(&limiter, PAT, IP, t0);
        }
        for _ in 0..IP_FAILURES {
            limiter.not_a_failure(begin(&limiter, PAT, IP, t0).unwrap());
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
        assert!(begin(
            &limiter,
            "fresh@example.com",
            IP,
            t0 + Duration::from_secs(20)
        )
        .is_err());
        assert!(begin(&limiter, "fresh@example.com", "198.51.100.7", t0).is_ok());
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
            .map(|_| begin(&limiter, PAT, IP, t0).unwrap())
            .collect();
        assert!(begin(&limiter, PAT, IP, t0).is_err(), "the sixth waits");
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
    fn a_flood_of_new_keys_cannot_evict_a_locked_unknown_account() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for _ in 0..ACCOUNT_FAILURES {
            fail(&limiter, NOBODY, IP, t0);
        }
        flood(&limiter, MAX_KEYS, t0 + Duration::from_secs(1));
        let later = t0 + Duration::from_secs(2);
        assert!(begin(&limiter, NOBODY, "198.51.100.7", later).is_err());
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
        assert!(begin(&limiter, "fresh@example.com", IP, later).is_err());
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
            fail(&limiter, NOBODY, IP, t0);
        }
        flood(&limiter, MAX_KEYS - 1, t0 + Duration::from_secs(1));
        fail(
            &limiter,
            "new@example.com",
            "198.51.100.7",
            t0 + Duration::from_secs(2),
        );
        let locks = fail(&limiter, NOBODY, IP, t0 + Duration::from_secs(3));
        assert_eq!(locks, [Bucket::Account], "the older, busier count survived");
    }

    fn lock_unknown_accounts(limiter: &LoginLimiter, count: usize, at: Instant) {
        for i in 0..count {
            for _ in 0..ACCOUNT_FAILURES {
                fail(
                    limiter,
                    &format!("locked{i}@example.com"),
                    &format!("198.51.100.{i}"),
                    at,
                );
            }
        }
    }

    #[test]
    fn a_flood_of_locked_unknown_keys_does_not_stop_a_real_user() {
        let limiter = LoginLimiter::with_capacity(3);
        let t0 = Instant::now();
        lock_unknown_accounts(&limiter, 3, t0);
        for _ in 0..(ACCOUNT_FAILURES - 1) {
            assert!(fail(&limiter, PAT, IP, t0).is_empty());
        }
        assert_eq!(
            fail(&limiter, PAT, IP, t0),
            [Bucket::Account],
            "PAT is tracked"
        );
    }

    #[test]
    fn a_real_users_lock_survives_any_flood() {
        let limiter = LoginLimiter::with_capacity(3);
        let t0 = Instant::now();
        for _ in 0..ACCOUNT_FAILURES {
            fail(&limiter, PAT, IP, t0);
        }
        flood(&limiter, 50, t0 + Duration::from_secs(1));
        assert!(begin(&limiter, PAT, "198.51.100.200", t0 + Duration::from_secs(2)).is_err());
    }

    #[test]
    fn an_unknown_key_at_a_table_full_of_locks_is_admitted_untracked() {
        let limiter = LoginLimiter::with_capacity(3);
        let t0 = Instant::now();
        lock_unknown_accounts(&limiter, 3, t0);
        for i in 0..(2 * ACCOUNT_FAILURES) {
            let attempt = begin(&limiter, NOBODY, &format!("203.0.113.{i}"), t0)
                .expect("admitted while the table is full");
            assert!(limiter.failed(attempt).is_empty(), "untracked never locks");
        }
        assert_eq!(limiter.sizes().0, 3);
    }

    #[test]
    fn a_full_table_is_reported_once_per_flood() {
        let limiter = LoginLimiter::with_capacity(3);
        let t0 = Instant::now();
        lock_unknown_accounts(&limiter, 3, t0);
        let first = begin(&limiter, NOBODY, "203.0.113.1", t0).unwrap();
        assert_eq!(first.newly_full(), [Bucket::Account]);
        limiter.failed(first);
        let second = begin(&limiter, NOBODY, "203.0.113.2", t0).unwrap();
        assert!(second.newly_full().is_empty());
        limiter.failed(second);
        let after = t0 + LOCKOUT;
        let room = begin(&limiter, "late@example.com", "203.0.113.3", after).unwrap();
        assert!(room.newly_full().is_empty());
        limiter.failed(room);
        lock_unknown_accounts(&limiter, 3, after);
        let again = begin(&limiter, NOBODY, "203.0.113.5", after).unwrap();
        assert_eq!(
            again.newly_full(),
            [Bucket::Account],
            "a new flood is reported"
        );
        limiter.failed(again);
    }

    #[test]
    fn a_new_ip_at_a_table_full_of_locked_ips_is_admitted() {
        let limiter = LoginLimiter::with_capacity(1);
        let t0 = Instant::now();
        for i in 0..IP_FAILURES {
            fail(&limiter, &format!("user{i}@example.com"), IP, t0);
        }
        let attempt = begin(&limiter, PAT, "198.51.100.7", t0).expect("admitted");
        assert_eq!(attempt.newly_full(), [Bucket::Ip]);
        limiter.failed(attempt);
        assert_eq!(limiter.sizes().1, 1);
        assert!(
            begin(&limiter, "fresh@example.com", IP, t0).is_err(),
            "the locked IP stays locked"
        );
    }

    #[test]
    fn no_eviction_happens_for_an_attempt_another_bucket_turns_away() {
        let limiter = LoginLimiter::with_capacity(2);
        let t0 = Instant::now();
        fail(&limiter, "a@example.com", "198.51.100.1", t0);
        fail(&limiter, "b@example.com", "198.51.100.1", t0);
        for i in 0..IP_FAILURES {
            let user = format!("user{i}@example.com");
            limiter.failed(limiter.begin(&user, true, IP, t0).unwrap());
        }
        assert!(begin(&limiter, "c@example.com", IP, t0).is_err());
        let held = limiter.account_keys();
        assert!(
            held.contains(&"a@example.com".to_string())
                && held.contains(&"b@example.com".to_string()),
            "{held:?}"
        );
    }

    #[test]
    fn a_real_users_count_is_neither_evicted_nor_counted_against_the_cap() {
        let limiter = LoginLimiter::with_capacity(3);
        let t0 = Instant::now();
        fail(&limiter, PAT, IP, t0);
        flood(&limiter, 3, t0 + Duration::from_secs(1));
        assert_eq!(limiter.sizes().0, 4, "three unknown keys beside PAT");
        flood(&limiter, 50, t0 + Duration::from_secs(2));
        for _ in 0..(ACCOUNT_FAILURES - 2) {
            assert!(fail(&limiter, PAT, IP, t0 + Duration::from_secs(3)).is_empty());
        }
        let locks = fail(&limiter, PAT, IP, t0 + Duration::from_secs(3));
        assert_eq!(locks, [Bucket::Account], "PAT's first failure was kept");
    }

    #[test]
    fn an_untracked_attempt_settles_nothing() {
        let limiter = LoginLimiter::with_capacity(3);
        let t0 = Instant::now();
        lock_unknown_accounts(&limiter, 3, t0);
        let refunded = begin(&limiter, NOBODY, "203.0.113.1", t0).unwrap();
        let signed_in = begin(&limiter, NOBODY, "203.0.113.2", t0).unwrap();
        let after = t0 + LOCKOUT;
        for _ in 0..(ACCOUNT_FAILURES - 1) {
            fail(&limiter, NOBODY, "203.0.113.3", after);
        }
        limiter.not_a_failure(refunded);
        limiter.succeeded(signed_in);
        let locks = fail(&limiter, NOBODY, "203.0.113.3", after);
        assert_eq!(locks, [Bucket::Account], "the tracked count was untouched");
    }

    #[test]
    fn a_totp_required_answer_does_not_hold_a_slot() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        limiter.not_a_failure(begin(&limiter, PAT, IP, t0).unwrap());
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
        limiter.not_a_failure(begin(&limiter, PAT, IP, mid).unwrap());
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
        let stale = begin(&limiter, PAT, IP, t0).unwrap();
        let later = t0 + LOCKOUT;
        for i in 0..ACCOUNT_FAILURES {
            fail(&limiter, PAT, &format!("198.51.100.{i}"), later);
        }
        limiter.not_a_failure(stale);
        assert!(begin(&limiter, PAT, "198.51.100.99", later).is_err());
    }

    #[test]
    fn settling_an_attempt_clears_only_the_ip_lock_it_set() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for i in 0..(IP_FAILURES - 1) {
            fail(&limiter, &format!("user{i}@example.com"), IP, t0);
        }
        let stale = begin(&limiter, "last@example.com", IP, t0).unwrap();
        let later = t0 + LOCKOUT;
        for i in 0..IP_FAILURES {
            fail(&limiter, &format!("again{i}@example.com"), IP, later);
        }
        limiter.not_a_failure(stale);
        assert!(begin(&limiter, "fresh@example.com", IP, later).is_err());
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
