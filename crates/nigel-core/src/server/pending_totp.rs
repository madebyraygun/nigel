//! Two-step enrollments in progress: a fresh secret held in memory for ten
//! minutes, one per session, until a code from the user's app confirms it.
//! Nothing is written to the books until then.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rand::RngCore;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use crate::identity::totp::SECRET_LEN;

pub const SETUP_LIFETIME: Duration = Duration::from_secs(10 * 60);

struct Pending {
    setup_id: String,
    user_id: i64,
    secret: Zeroizing<[u8; SECRET_LEN]>,
    expires: Instant,
}

#[derive(Default)]
pub struct PendingTotpSetups {
    by_session: Mutex<HashMap<i64, Pending>>,
}

impl std::fmt::Debug for PendingTotpSetups {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PendingTotpSetups(<redacted>)")
    }
}

impl PendingTotpSetups {
    /// Hold `secret` for `session_id`, replacing any setup that session had
    /// started. Answers the setup id the client sends back.
    pub fn start(
        &self,
        session_id: i64,
        user_id: i64,
        secret: Zeroizing<[u8; SECRET_LEN]>,
        now: Instant,
    ) -> String {
        let mut raw = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut raw);
        let setup_id = hex::encode(raw);
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        let mut pending = self.by_session.lock().unwrap();
        pending.retain(|_, p| p.expires > now);
        pending.insert(
            session_id,
            Pending {
                setup_id: setup_id.clone(),
                user_id,
                secret,
                expires: now + SETUP_LIFETIME,
            },
        );
        setup_id
    }

    /// The user and secret this session's unexpired setup `setup_id` holds.
    pub fn secret_for(
        &self,
        session_id: i64,
        setup_id: &str,
        now: Instant,
    ) -> Option<(i64, Zeroizing<[u8; SECRET_LEN]>)> {
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        let pending = self.by_session.lock().unwrap();
        let held = pending.get(&session_id)?;
        let same_id: bool = held.setup_id.as_bytes().ct_eq(setup_id.as_bytes()).into();
        (same_id && held.expires > now).then(|| (held.user_id, held.secret.clone()))
    }

    pub fn finish(&self, session_id: i64) {
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        self.by_session.lock().unwrap().remove(&session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret() -> Zeroizing<[u8; SECRET_LEN]> {
        Zeroizing::new([5u8; SECRET_LEN])
    }

    #[test]
    fn a_setup_is_held_per_session_for_ten_minutes() {
        let setups = PendingTotpSetups::default();
        let t0 = Instant::now();
        let id = setups.start(7, 2, secret(), t0);
        assert_eq!(id.len(), 32);
        let (user, held) = setups.secret_for(7, &id, t0).unwrap();
        assert_eq!((user, *held), (2, [5u8; SECRET_LEN]));
        assert!(setups.secret_for(8, &id, t0).is_none(), "another session");
        assert!(setups.secret_for(7, "00", t0).is_none(), "another id");
        assert!(
            setups.secret_for(7, &id, t0 + SETUP_LIFETIME).is_none(),
            "expired"
        );
    }

    #[test]
    fn a_new_setup_replaces_the_last_and_finishing_forgets_it() {
        let setups = PendingTotpSetups::default();
        let t0 = Instant::now();
        let first = setups.start(7, 2, secret(), t0);
        let second = setups.start(7, 2, secret(), t0);
        assert!(setups.secret_for(7, &first, t0).is_none());
        assert!(setups.secret_for(7, &second, t0).is_some());
        setups.finish(7);
        assert!(setups.secret_for(7, &second, t0).is_none());
    }

    #[test]
    fn debug_shows_no_secret() {
        let setups = PendingTotpSetups::default();
        setups.start(7, 2, secret(), Instant::now());
        let printed = format!("{setups:?}");
        assert!(!printed.contains('5'), "{printed}");
    }
}
