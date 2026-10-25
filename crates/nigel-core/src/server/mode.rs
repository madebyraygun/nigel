//! Local or shared: derived from the books on every reading, never stored or
//! configured. Within one server run the mode only ever moves local → shared,
//! so a stale reading is always the one that accepts less.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};

use super::secret::SessionToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Local,
    Shared,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Local => "local",
            Mode::Shared => "shared",
        }
    }
}

/// Whether a server may leave local mode. Every `AppState::new` caller — the
/// desktop shells and the tests — is pinned: its requests run as the local
/// owner whatever the books hold. Only `nigel serve` derives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModePolicy {
    PinnedLocal,
    Derived,
}

/// A public-host request against a local-mode server re-derives the mode, at
/// most this often, so traffic through a tunnel costs one users query per
/// interval rather than one per request.
pub const REFRESH_INTERVAL: Duration = Duration::from_secs(2);

enum Current {
    Local(SessionToken),
    Shared,
}

pub(crate) struct ModeCell {
    policy: ModePolicy,
    current: RwLock<Current>,
    last_refresh: Mutex<Option<Instant>>,
    refresh_failed: AtomicBool,
}

impl std::fmt::Debug for ModeCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModeCell")
            .field("policy", &self.policy)
            .field("mode", &self.mode())
            .field("token", &self.local_token())
            .finish()
    }
}

impl ModeCell {
    pub(crate) fn local(policy: ModePolicy, token: SessionToken) -> Self {
        Self {
            policy,
            current: RwLock::new(Current::Local(token)),
            last_refresh: Mutex::new(None),
            refresh_failed: AtomicBool::new(false),
        }
    }

    pub(crate) fn shared() -> Self {
        Self {
            policy: ModePolicy::Derived,
            current: RwLock::new(Current::Shared),
            last_refresh: Mutex::new(None),
            refresh_failed: AtomicBool::new(false),
        }
    }

    pub(crate) fn policy(&self) -> ModePolicy {
        self.policy
    }

    pub(crate) fn mode(&self) -> Mode {
        // unwrap: poisoned lock means a thread panicked — unrecoverable
        match *self.current.read().unwrap() {
            Current::Local(_) => Mode::Local,
            Current::Shared => Mode::Shared,
        }
    }

    pub(crate) fn local_token(&self) -> Option<SessionToken> {
        // unwrap: poisoned lock means a thread panicked — unrecoverable
        match &*self.current.read().unwrap() {
            Current::Local(token) => Some(token.clone()),
            Current::Shared => None,
        }
    }

    /// Move to shared mode and drop the token. True when this call changed
    /// the mode; a pinned cell never changes.
    pub(crate) fn enter_shared(&self) -> bool {
        if self.policy == ModePolicy::PinnedLocal {
            return false;
        }
        // unwrap: poisoned lock means a thread panicked — unrecoverable
        let mut current = self.current.write().unwrap();
        if matches!(*current, Current::Shared) {
            return false;
        }
        *current = Current::Shared;
        true
    }

    /// Claim the right to re-derive at `now`: false when a refresh was
    /// claimed less than `REFRESH_INTERVAL` before it.
    pub(crate) fn claim_refresh(&self, now: Instant) -> bool {
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        let mut last = self.last_refresh.lock().unwrap();
        if last.is_some_and(|at| now.duration_since(at) < REFRESH_INTERVAL) {
            return false;
        }
        *last = Some(now);
        true
    }

    /// True for the first failed re-derivation of this server's run, so a
    /// failure that repeats on every refresh is reported once.
    pub(crate) fn first_refresh_failure(&self) -> bool {
        !self.refresh_failed.swap(true, Ordering::SeqCst)
    }
}

/// The mode the books at `db_path` call for: shared when they exist, this
/// process can read them, and they have at least one user. Absent, empty and
/// locked books are local. An error is the caller's to read as local.
pub fn derive(db_path: &Path) -> crate::error::Result<Mode> {
    if !super::routes::status::initialized(db_path) {
        return Ok(Mode::Local);
    }
    if crate::db::get_db_password().is_none() && crate::db::is_encrypted(db_path)? {
        return Ok(Mode::Local);
    }
    let conn = crate::db::get_connection(db_path)?;
    Ok(if crate::identity::users::has_users(&conn)? {
        Mode::Shared
    } else {
        Mode::Local
    })
}

/// [`derive`], reading a failure as local after handing `report` a notice
/// that says why.
pub(crate) fn derive_or_local(db_path: &Path, report: impl FnOnce(String)) -> Mode {
    derive(db_path).unwrap_or_else(|err| {
        report(derive_failure_notice(db_path, &err));
        Mode::Local
    })
}

/// One line for a failed [`derive`]. A refused password is named as the
/// cause; the password itself is never part of the line.
pub(crate) fn derive_failure_notice(db_path: &Path, err: &dyn std::fmt::Display) -> String {
    let cause = if crate::db::get_db_password().is_some() {
        format!("they didn't open with the database password this server was given: {err}")
    } else {
        err.to_string()
    };
    format!(
        "notice: couldn't read the books at {} to choose local or shared mode ({cause}); serving in local mode.",
        db_path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::users::{create_user, NewUser};
    use crate::identity::Role;
    use crate::server::testutil::{encrypt, temp_db, PASSWORD};

    /// Clears the process-global password when the test ends, failed
    /// assertion or not.
    struct ClearPassword;

    impl Drop for ClearPassword {
        fn drop(&mut self) {
            crate::db::set_db_password_for_tests(None);
        }
    }

    fn set_password() -> ClearPassword {
        crate::db::set_db_password_for_tests(Some(zeroize::Zeroizing::new(PASSWORD.to_string())));
        ClearPassword
    }

    fn add_admin(db_path: &Path) {
        let conn = crate::db::get_connection(db_path).unwrap();
        create_user(
            &conn,
            &NewUser {
                email: "pat@example.com",
                display_name: "Pat Example",
                role: Role::Admin,
            },
            crate::clock::now_utc(),
        )
        .unwrap();
    }

    #[test]
    fn books_without_users_are_local_and_with_users_shared() {
        let (_dir, db_path) = temp_db();
        assert_eq!(derive(&db_path).unwrap(), Mode::Local);
        add_admin(&db_path);
        assert_eq!(derive(&db_path).unwrap(), Mode::Shared);
    }

    #[test]
    fn absent_or_empty_books_are_local() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("nigel.db");
        assert_eq!(derive(&db_path).unwrap(), Mode::Local);
        std::fs::write(&db_path, b"").unwrap();
        assert_eq!(derive(&db_path).unwrap(), Mode::Local);
    }

    #[test]
    fn locked_books_with_users_are_local_until_unlocked() {
        let (_dir, db_path) = temp_db();
        add_admin(&db_path);
        encrypt(&db_path);
        crate::db::set_db_password_for_tests(None);
        assert_eq!(derive(&db_path).unwrap(), Mode::Local);
        let _clear = set_password();
        assert_eq!(derive(&db_path).unwrap(), Mode::Shared);
    }

    #[test]
    fn unreadable_books_are_reported_once_and_read_as_local() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("nigel.db");
        std::fs::create_dir(&db_path).unwrap();
        assert!(derive(&db_path).is_err());

        let mut notices = Vec::new();
        assert_eq!(derive_or_local(&db_path, |n| notices.push(n)), Mode::Local);
        assert_eq!(notices.len(), 1, "{notices:?}");
        assert!(
            notices[0].contains(&db_path.display().to_string()),
            "{}",
            notices[0]
        );
        assert!(notices[0].contains("local mode"), "{}", notices[0]);
    }

    #[test]
    fn a_password_the_books_refuse_is_named_and_never_printed() {
        let (_dir, db_path) = temp_db();
        let _clear = set_password();
        assert!(derive(&db_path).is_err());

        let mut notices = Vec::new();
        assert_eq!(derive_or_local(&db_path, |n| notices.push(n)), Mode::Local);
        assert_eq!(notices.len(), 1, "{notices:?}");
        assert!(
            notices[0].contains("didn't open with the database password"),
            "{}",
            notices[0]
        );
        assert!(!notices[0].contains(PASSWORD), "{}", notices[0]);
    }

    #[test]
    fn readable_books_report_nothing() {
        let (_dir, db_path) = temp_db();
        let mut notices = Vec::new();
        assert_eq!(derive_or_local(&db_path, |n| notices.push(n)), Mode::Local);
        assert!(notices.is_empty(), "{notices:?}");
    }

    #[test]
    fn only_the_first_refresh_failure_is_reported() {
        let cell = ModeCell::local(ModePolicy::Derived, SessionToken::from("ab".repeat(32)));
        assert!(cell.first_refresh_failure());
        assert!(!cell.first_refresh_failure());
        assert!(!cell.first_refresh_failure());
    }

    #[test]
    fn the_cell_moves_local_to_shared_once_and_drops_the_token() {
        let cell = ModeCell::local(ModePolicy::Derived, SessionToken::from("ab".repeat(32)));
        assert_eq!(cell.mode(), Mode::Local);
        assert!(cell.local_token().is_some());
        assert!(cell.enter_shared());
        assert_eq!(cell.mode(), Mode::Shared);
        assert!(cell.local_token().is_none());
        assert!(!cell.enter_shared(), "already shared");
    }

    #[test]
    fn a_pinned_cell_never_leaves_local() {
        let cell = ModeCell::local(ModePolicy::PinnedLocal, SessionToken::from("ab".repeat(32)));
        assert!(!cell.enter_shared());
        assert_eq!(cell.mode(), Mode::Local);
        assert!(cell.local_token().is_some());
    }

    #[test]
    fn refreshes_are_claimed_at_most_once_per_interval() {
        let cell = ModeCell::local(ModePolicy::Derived, SessionToken::from("ab".repeat(32)));
        let t0 = std::time::Instant::now();
        assert!(cell.claim_refresh(t0));
        assert!(!cell.claim_refresh(t0 + REFRESH_INTERVAL / 2));
        assert!(cell.claim_refresh(t0 + REFRESH_INTERVAL));
    }
}
