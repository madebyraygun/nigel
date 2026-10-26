//! Shared server state.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use tokio::sync::RwLock as TokioRwLock;

use super::error::ApiError;
use super::secret::SessionToken;

/// The failure count at which answers start being held back. The first two
/// cost nothing but a message; the third and every one after it is delayed.
const MAX_FREE_ATTEMPTS: u32 = 3;
const BACKOFF_BASE: Duration = Duration::from_secs(1);
const BACKOFF_CAP: Duration = Duration::from_secs(30);

/// How long the server holds back the answer to the `failures`-th failed
/// unlock: nothing for the first two, then 1s, 2s, 4s… capped at 30s.
pub fn backoff_delay(failures: u32) -> Duration {
    let Some(step) = failures.checked_sub(MAX_FREE_ATTEMPTS) else {
        return Duration::ZERO;
    };
    let Some(factor) = 1u32.checked_shl(step) else {
        return BACKOFF_CAP;
    };
    (BACKOFF_BASE * factor).min(BACKOFF_CAP)
}

/// Failed-unlock bookkeeping: in memory, process-wide, never persisted. There
/// is no hard lockout — whoever is at the keyboard can always restart the
/// process — so repeated failures only get slower.
#[derive(Debug, Default)]
pub struct UnlockGate {
    failures: Mutex<u32>,
}

impl UnlockGate {
    /// How many failures have been recorded.
    #[cfg(test)]
    pub fn failures(&self) -> u32 {
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        *self.failures.lock().unwrap()
    }

    /// Count a failed attempt and report what to tell the caller: how many free
    /// attempts are left, and how long this answer should be held back.
    pub fn record_failure(&self) -> (u32, Duration) {
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        let mut failures = self.failures.lock().unwrap();
        *failures = failures.saturating_add(1);
        let count = *failures;
        drop(failures);

        (
            MAX_FREE_ATTEMPTS.saturating_sub(count),
            backoff_delay(count),
        )
    }

    pub fn reset(&self) {
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        *self.failures.lock().unwrap() = 0;
    }
}

/// Build-time capabilities the API advertises to the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Features {
    pub pdf: bool,
    pub gusto: bool,
}

impl Features {
    pub fn detect() -> Self {
        Self {
            pdf: cfg!(feature = "pdf"),
            gusto: cfg!(feature = "gusto"),
        }
    }
}

type ClockFn = dyn Fn() -> chrono::DateTime<chrono::Utc> + Send + Sync;

#[derive(Clone)]
struct StateClock(Arc<ClockFn>);

impl std::fmt::Debug for StateClock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StateClock")
    }
}

/// State cloned into every request. Handlers open their own `rusqlite`
/// connection from `db_path()` inside `spawn_blocking`; there is no pool.
///
/// The database password is deliberately absent: it lives in the process-global
/// `db::set_db_password` mutex that every CLI subcommand already uses, so
/// `nigel serve` inherits the CLI's one-database-per-process assumption.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Which database this server is serving. Behind a lock because the web
    /// settings screen can switch data directories while the server runs: a
    /// path fixed at startup would leave every later request reading the old
    /// books under the new directory's name.
    db_path: Arc<RwLock<PathBuf>>,
    /// Local mode with its token, or shared mode; see `mode.rs`.
    mode: Arc<super::mode::ModeCell>,
    /// How this server is reached: its public address and client-IP header.
    pub config: Arc<super::config::ServerConfig>,
    /// The instant every session write stamps and every expiry is judged
    /// against. The wall clock in production; tests pin it.
    clock: StateClock,
    pub features: Features,
    pub unlock: Arc<UnlockGate>,
    /// Sign-in rate limits; see `ratelimit.rs`.
    pub limiter: Arc<super::ratelimit::LoginLimiter>,
    /// Bounds how many Argon2 password verifications run at once; see
    /// [`password_check_slots`].
    pub password_checks: Arc<tokio::sync::Semaphore>,
    /// Two-step enrollments started but not yet confirmed.
    pub totp_setups: Arc<super::pending_totp::PendingTotpSetups>,
    /// Guards the database *file* rather than its contents.
    ///
    /// `cli::password::encrypt_database` and `decrypt_database` rewrite the
    /// file by rename and delete the `-wal`/`-shm` sidecars. A connection
    /// another request holds across that moment is reading a file that is no
    /// longer the database. Readers take the read side (`routes::with_conn`,
    /// and any handler that opens a connection itself); those two operations
    /// and the data-directory switch take the write side.
    pub db_gate: Arc<TokioRwLock<()>>,
    /// The version of a newer release, once the startup check has found one.
    ///
    /// A slot rather than a value because the check runs in the background:
    /// requests are answered immediately and this fills in when GitHub replies,
    /// so an early `/api/status` reports nothing rather than waiting on the
    /// network.
    update_available: Arc<RwLock<Option<String>>>,
}

/// How many password verifications may run at once: one per core, at most
/// four, so sign-in bursts cannot take every blocking thread and core.
pub fn password_check_slots() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
        .clamp(1, 4)
}

impl AppState {
    /// A state pinned to local mode, with `session_token` as its token: the
    /// desktop shells' state, and every test that does not need shared mode.
    pub fn new(db_path: PathBuf, session_token: String) -> Self {
        Self::with_mode(
            db_path,
            super::mode::ModeCell::local(
                super::mode::ModePolicy::PinnedLocal,
                SessionToken::from(session_token),
            ),
            super::config::ServerConfig::default(),
        )
    }

    /// The state `nigel serve` runs: its mode derived (and re-derived) from
    /// the books. A token is minted only for local mode.
    pub fn for_serve(
        db_path: PathBuf,
        mode: super::mode::Mode,
        config: super::config::ServerConfig,
    ) -> Self {
        let cell = match mode {
            super::mode::Mode::Local => super::mode::ModeCell::local(
                super::mode::ModePolicy::Derived,
                SessionToken::from(super::auth::generate_token()),
            ),
            super::mode::Mode::Shared => super::mode::ModeCell::shared(),
        };
        Self::with_mode(db_path, cell, config)
    }

    fn with_mode(
        db_path: PathBuf,
        mode: super::mode::ModeCell,
        config: super::config::ServerConfig,
    ) -> Self {
        Self {
            db_path: Arc::new(RwLock::new(db_path)),
            mode: Arc::new(mode),
            config: Arc::new(config),
            clock: StateClock(Arc::new(crate::clock::now_utc)),
            features: Features::detect(),
            unlock: Arc::new(UnlockGate::default()),
            limiter: Arc::new(super::ratelimit::LoginLimiter::default()),
            password_checks: Arc::new(tokio::sync::Semaphore::new(password_check_slots())),
            totp_setups: Arc::new(Default::default()),
            db_gate: Arc::new(TokioRwLock::new(())),
            update_available: Arc::new(RwLock::new(None)),
        }
    }

    /// Pin the clock, for tests of expiry and renewal.
    #[cfg(any(test, feature = "testutil"))]
    pub fn with_clock(
        mut self,
        clock: impl Fn() -> chrono::DateTime<chrono::Utc> + Send + Sync + 'static,
    ) -> Self {
        self.clock = StateClock(Arc::new(clock));
        self
    }

    /// A password-verification slot, or `None` when every one is in use.
    pub fn try_password_check(&self) -> Option<tokio::sync::OwnedSemaphorePermit> {
        self.password_checks.clone().try_acquire_owned().ok()
    }

    pub fn now(&self) -> chrono::DateTime<chrono::Utc> {
        (self.clock.0)()
    }

    pub fn mode(&self) -> super::mode::Mode {
        self.mode.mode()
    }

    /// The local-mode token, or `None` once the server is shared.
    pub fn local_token(&self) -> Option<SessionToken> {
        self.mode.local_token()
    }

    /// Re-derive the mode from the books and move to shared if they call for
    /// it. Never moves back. Pinned states answer local without reading.
    pub async fn refresh_mode(&self) -> super::mode::Mode {
        if self.mode.policy() == super::mode::ModePolicy::PinnedLocal
            || self.mode() == super::mode::Mode::Shared
        {
            return self.mode();
        }
        let gate = self.db_gate.clone().read_owned().await;
        let db_path = self.db_path();
        let reading = db_path.clone();
        let derived =
            super::routes::spawn_blocking_holding(gate, move || super::mode::derive(&reading))
                .await;
        let failure = match derived {
            Ok((_gate, Ok(super::mode::Mode::Shared))) => {
                if self.mode.enter_shared() {
                    super::events::emit(&super::events::AuthEvent::new(
                        super::events::EventKind::SharedModeBegan,
                    ));
                }
                None
            }
            Ok((_gate, Ok(super::mode::Mode::Local))) => None,
            Ok((_gate, Err(err))) => Some(err.to_string()),
            Err(err) => Some(err.message().to_string()),
        };
        if let Some(failure) = failure {
            if self.mode.first_refresh_failure() {
                eprintln!("{}", super::mode::derive_failure_notice(&db_path, &failure));
            }
        }
        self.mode()
    }

    /// [`Self::refresh_mode`], at most once per `mode::REFRESH_INTERVAL`.
    pub async fn refresh_mode_throttled(&self) -> super::mode::Mode {
        if self.mode() == super::mode::Mode::Shared || !self.mode.claim_refresh(self.now()) {
            return self.mode();
        }
        self.refresh_mode().await
    }

    /// The version of the newer release the startup check found, if any.
    pub fn update_available(&self) -> Option<String> {
        // unwrap: poisoned lock means a thread panicked — unrecoverable
        self.update_available.read().unwrap().clone()
    }

    /// Record what the startup update check found.
    pub fn set_update_available(&self, version: Option<String>) {
        // unwrap: poisoned lock means a thread panicked — unrecoverable
        *self.update_available.write().unwrap() = version;
    }

    /// The database this server is currently serving.
    ///
    /// Hands back an owned path rather than a guard, so no lock is ever held
    /// across an await point.
    pub fn db_path(&self) -> PathBuf {
        // unwrap: poisoned lock means a thread panicked — unrecoverable
        self.db_path.read().unwrap().clone()
    }

    /// The directory the database lives in — where snapshots, backups and
    /// upload spools go.
    pub fn data_dir(&self) -> PathBuf {
        self.db_path()
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf()
    }

    /// Point this server at a different database. Only the data-directory
    /// switch calls this, and only while holding `db_gate` for writing.
    pub fn set_db_path(&self, path: PathBuf) {
        // unwrap: poisoned lock means a thread panicked — unrecoverable
        *self.db_path.write().unwrap() = path;
    }

    /// True when the database is encrypted and this process has not been given
    /// the key yet.
    ///
    /// The encryption state is probed rather than cached at startup: the web
    /// settings screen can encrypt or decrypt the database while the server
    /// runs, and a stale cache would either lock out a valid session or serve a
    /// database whose key was just removed. The password check comes first, so
    /// an unlocked process never touches the filesystem for this.
    pub fn is_locked(&self) -> Result<bool, ApiError> {
        if crate::db::get_db_password().is_some() {
            return Ok(false);
        }
        crate::db::is_encrypted(&self.db_path()).map_err(ApiError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The token is the server's whole authentication; a `dbg!` of the state,
    /// or of anything that derives `Debug` around it, must not print it.
    #[test]
    fn a_derived_debug_of_the_state_carries_no_token() {
        #[derive(Debug)]
        #[allow(dead_code)]
        struct Holder {
            state: AppState,
        }

        let token = super::super::auth::generate_token();
        let state = AppState::new(PathBuf::from("/books/acme/nigel.db"), token.clone());
        for rendered in [
            format!("{state:?}"),
            format!("{state:#?}"),
            format!(
                "{:?}",
                Holder {
                    state: state.clone()
                }
            ),
        ] {
            assert!(!rendered.contains(&token), "token in {rendered}");
            assert!(rendered.contains("<redacted>"), "got {rendered}");
        }
        assert_eq!(state.local_token().unwrap().expose(), token);
    }

    #[tokio::test]
    async fn a_new_state_stays_local_over_books_with_users() {
        let (_dir, db_path) = super::super::testutil::temp_db();
        let conn = crate::db::get_connection(&db_path).unwrap();
        crate::identity::users::create_user(
            &conn,
            &crate::identity::users::NewUser {
                email: "pat@example.com",
                display_name: "Pat Example",
                role: crate::identity::Role::Admin,
            },
            crate::clock::now_utc(),
        )
        .unwrap();
        drop(conn);
        assert_eq!(
            super::super::mode::derive(&db_path).unwrap(),
            super::super::mode::Mode::Shared
        );

        let token = "ab".repeat(32);
        let state = AppState::new(db_path, token.clone());
        assert_eq!(state.refresh_mode().await, super::super::mode::Mode::Local);
        assert_eq!(state.mode(), super::super::mode::Mode::Local);
        assert_eq!(state.local_token().unwrap().expose(), token);
    }

    #[tokio::test]
    async fn a_serve_state_stays_local_when_the_books_cannot_be_read() {
        crate::db::set_db_password_for_tests(None);
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("nigel.db");
        std::fs::create_dir(&db_path).unwrap();
        assert!(super::super::mode::derive(&db_path).is_err());

        let state =
            AppState::for_serve(db_path, super::super::mode::Mode::Local, Default::default());
        assert_eq!(state.refresh_mode().await, super::super::mode::Mode::Local);
        assert_eq!(state.refresh_mode().await, super::super::mode::Mode::Local);
        assert!(state.local_token().is_some());
    }

    #[test]
    fn a_new_state_is_pinned_local_and_a_shared_serve_state_has_no_token() {
        let state = AppState::new(PathBuf::from("/books/acme/nigel.db"), "ab".repeat(32));
        assert_eq!(state.mode(), super::super::mode::Mode::Local);
        assert!(state.local_token().is_some());

        let shared = AppState::for_serve(
            PathBuf::from("/books/acme/nigel.db"),
            super::super::mode::Mode::Shared,
            Default::default(),
        );
        assert_eq!(shared.mode(), super::super::mode::Mode::Shared);
        assert!(shared.local_token().is_none());

        let local = AppState::for_serve(
            PathBuf::from("/books/acme/nigel.db"),
            super::super::mode::Mode::Local,
            Default::default(),
        );
        assert_eq!(local.local_token().unwrap().expose().len(), 64);
    }

    #[test]
    fn backoff_is_free_then_doubles_up_to_the_cap() {
        let expected_ms = [0, 0, 1_000, 2_000, 4_000, 8_000, 16_000, 30_000, 30_000];
        for (i, ms) in expected_ms.iter().enumerate() {
            let failures = i as u32 + 1;
            assert_eq!(
                backoff_delay(failures),
                Duration::from_millis(*ms),
                "delay after {failures} failures"
            );
        }
    }

    #[test]
    fn backoff_does_not_overflow_at_absurd_failure_counts() {
        assert_eq!(backoff_delay(u32::MAX), BACKOFF_CAP);
        assert_eq!(backoff_delay(64), BACKOFF_CAP);
        assert_eq!(backoff_delay(0), Duration::ZERO);
    }

    #[test]
    fn attempts_remaining_counts_down_then_saturates() {
        let gate = UnlockGate::default();
        assert_eq!(gate.record_failure(), (2, Duration::ZERO));
        assert_eq!(gate.record_failure(), (1, Duration::ZERO));
        assert_eq!(gate.record_failure(), (0, Duration::from_secs(1)));
        assert_eq!(gate.record_failure(), (0, Duration::from_secs(2)));
    }

    #[test]
    fn reset_restores_the_full_budget() {
        let gate = UnlockGate::default();
        gate.record_failure();
        gate.record_failure();
        gate.reset();
        assert_eq!(gate.record_failure(), (2, Duration::ZERO));
    }
}
