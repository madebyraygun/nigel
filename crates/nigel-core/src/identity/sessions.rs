//! Sessions: a random 32-byte token handed to the client once, stored only as
//! its SHA-256. Idle for 30 days, or 90 days after creation, ends a session.

use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use super::{stamp, Role};
use crate::error::{NigelError, Result};

pub const IDLE_DAYS: i64 = 30;
pub const ABSOLUTE_DAYS: i64 = 90;
pub const RENEW_AFTER_SECONDS: i64 = 3600;
pub const USER_AGENT_MAX_CHARS: usize = 256;
const TOKEN_BYTES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    Cookie,
    Bearer,
}

impl SessionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SessionKind::Cookie => "cookie",
            SessionKind::Bearer => "bearer",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "cookie" => Some(SessionKind::Cookie),
            "bearer" => Some(SessionKind::Bearer),
            _ => None,
        }
    }
}

/// A session just created. `token` is the only copy of the credential: it is
/// returned to the client once and never stored.
pub struct IssuedSession {
    pub id: i64,
    pub token: Zeroizing<String>,
    pub expires_at: String,
}

impl std::fmt::Debug for IssuedSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssuedSession")
            .field("id", &self.id)
            .field("token", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// A live session joined to its enabled user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSession {
    pub id: i64,
    pub user_id: i64,
    pub uid: String,
    pub role: Role,
    pub must_change_password: bool,
    pub kind: SessionKind,
    pub created_at: String,
    pub last_seen_at: String,
    pub expires_at: String,
}

/// The stored hash of a presented token: hex-decoded to exactly 32 bytes, then
/// SHA-256. Anything that is not 64 hex characters is `None`.
pub fn token_hash(token_hex: &str) -> Option<[u8; 32]> {
    let raw = Zeroizing::new(hex::decode(token_hex).ok()?);
    if raw.len() != TOKEN_BYTES {
        return None;
    }
    Some(Sha256::digest(raw.as_slice()).into())
}

fn truncate_chars(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

fn parse_stamp(value: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| NigelError::Other(format!("unreadable session timestamp: {value}")))
}

/// Issue a session for a user. It does not check the user is enabled: callers
/// create sessions only after `verify_credentials` or `verify_totp` succeeds.
pub fn create_session(
    conn: &Connection,
    user_id: i64,
    kind: SessionKind,
    user_agent: Option<&str>,
    ip: Option<&str>,
    now: DateTime<Utc>,
) -> Result<IssuedSession> {
    let mut raw = Zeroizing::new([0u8; TOKEN_BYTES]);
    rand::rngs::OsRng.fill_bytes(raw.as_mut());
    let token = Zeroizing::new(hex::encode(raw.as_ref()));
    let hash: [u8; 32] = Sha256::digest(raw.as_ref()).into();
    let expires_at = stamp(now + Duration::days(IDLE_DAYS));
    conn.execute(
        "INSERT INTO sessions
             (token_hash, user_id, kind, created_at, last_seen_at, expires_at, user_agent, ip)
         VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6, ?7)",
        params![
            hash.as_slice(),
            user_id,
            kind.as_str(),
            stamp(now),
            expires_at,
            user_agent.map(|ua| truncate_chars(ua, USER_AGENT_MAX_CHARS)),
            ip,
        ],
    )?;
    Ok(IssuedSession {
        id: conn.last_insert_rowid(),
        token,
        expires_at,
    })
}

/// The live session a token names: unexpired at `now`, its user enabled.
pub fn find_session(
    conn: &Connection,
    token_hex: &str,
    now: DateTime<Utc>,
) -> Result<Option<ActiveSession>> {
    let Some(hash) = token_hash(token_hex) else {
        return Ok(None);
    };
    let row = conn
        .query_row(
            "SELECT s.id, s.user_id, u.uid, u.role, u.must_change_password, s.kind,
                    s.created_at, s.last_seen_at, s.expires_at
               FROM sessions s JOIN users u ON u.id = s.user_id
              WHERE s.token_hash = ?1 AND s.expires_at > ?2 AND u.disabled_at IS NULL",
            params![hash.as_slice(), stamp(now)],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, bool>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, String>(8)?,
                ))
            },
        )
        .optional()?;
    let Some((id, user_id, uid, role, must_change, kind, created, seen, expires)) = row else {
        return Ok(None);
    };
    let role = Role::parse(&role)
        .ok_or_else(|| NigelError::Other(format!("unknown role in users table: {role}")))?;
    let kind = SessionKind::parse(&kind)
        .ok_or_else(|| NigelError::Other(format!("unknown session kind: {kind}")))?;
    Ok(Some(ActiveSession {
        id,
        user_id,
        uid,
        role,
        must_change_password: must_change,
        kind,
        created_at: created,
        last_seen_at: seen,
        expires_at: expires,
    }))
}

/// Slide a session's expiry forward once its last use is over an hour old:
/// `expires_at = min(now + 30 days, created_at + 90 days)`. Answers the new
/// expiry when it wrote one, `None` when the session was used within the hour
/// or has since ended or expired.
pub fn renew_session(
    conn: &Connection,
    session: &ActiveSession,
    now: DateTime<Utc>,
) -> Result<Option<String>> {
    let last_seen = parse_stamp(&session.last_seen_at)?;
    if now - last_seen <= Duration::seconds(RENEW_AFTER_SECONDS) {
        return Ok(None);
    }
    let absolute = parse_stamp(&session.created_at)? + Duration::days(ABSOLUTE_DAYS);
    let expires_at = stamp((now + Duration::days(IDLE_DAYS)).min(absolute));
    let renewed = conn.execute(
        "UPDATE sessions SET last_seen_at = ?1, expires_at = ?2
          WHERE id = ?3 AND expires_at > ?1",
        params![stamp(now), expires_at, session.id],
    )?;
    Ok((renewed > 0).then_some(expires_at))
}

pub fn end_session(conn: &Connection, id: i64) -> Result<bool> {
    Ok(conn.execute("DELETE FROM sessions WHERE id = ?1", [id])? > 0)
}

/// End every session of a user, or every one but `except`. Answers how many.
pub fn end_user_sessions(conn: &Connection, user_id: i64, except: Option<i64>) -> Result<usize> {
    Ok(conn.execute(
        "DELETE FROM sessions WHERE user_id = ?1 AND (?2 IS NULL OR id != ?2)",
        params![user_id, except],
    )?)
}

pub fn sweep_expired(conn: &Connection, now: DateTime<Utc>) -> Result<usize> {
    Ok(conn.execute("DELETE FROM sessions WHERE expires_at <= ?1", [stamp(now)])?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::testing::{at, db, insert_user};

    fn admin(conn: &Connection) -> i64 {
        insert_user(conn, "pat@example.com", "admin")
    }

    #[test]
    fn the_token_is_64_hex_and_only_its_hash_is_stored() {
        let (_dir, conn) = db();
        let user = admin(&conn);
        let issued = create_session(
            &conn,
            user,
            SessionKind::Cookie,
            None,
            None,
            at("2026-01-01T00:00:00Z"),
        )
        .unwrap();
        assert_eq!(issued.token.len(), 64);
        assert!(issued.token.bytes().all(|b| b.is_ascii_hexdigit()));
        let stored: Vec<u8> = conn
            .query_row("SELECT token_hash FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            stored.as_slice(),
            token_hash(&issued.token).unwrap().as_slice()
        );
        assert_ne!(stored, hex::decode(issued.token.as_str()).unwrap());
        assert_eq!(issued.expires_at, "2026-01-31T00:00:00Z");
    }

    #[test]
    fn a_token_finds_its_session_and_nothing_else_does() {
        let (_dir, conn) = db();
        let user = admin(&conn);
        let now = at("2026-01-01T00:00:00Z");
        let issued = create_session(&conn, user, SessionKind::Bearer, None, None, now).unwrap();
        let found = find_session(&conn, &issued.token, now).unwrap().unwrap();
        assert_eq!(found.user_id, user);
        assert_eq!(found.role, Role::Admin);
        assert_eq!(found.kind, SessionKind::Bearer);
        assert!(found.must_change_password);
        assert!(find_session(&conn, &"0".repeat(64), now).unwrap().is_none());
        assert!(find_session(&conn, "not hex", now).unwrap().is_none());
        assert!(find_session(&conn, &issued.token[..62], now)
            .unwrap()
            .is_none());
    }

    #[test]
    fn an_expired_session_or_a_disabled_user_finds_nothing() {
        let (_dir, conn) = db();
        let user = admin(&conn);
        let sam = insert_user(&conn, "sam@example.com", "bookkeeper");
        let start = at("2026-01-01T00:00:00Z");
        let mine = create_session(&conn, user, SessionKind::Cookie, None, None, start).unwrap();
        assert!(find_session(&conn, &mine.token, at("2026-01-31T00:00:00Z"))
            .unwrap()
            .is_none());
        let theirs = create_session(&conn, sam, SessionKind::Cookie, None, None, start).unwrap();
        conn.execute("UPDATE users SET disabled_at = 'x' WHERE id = ?1", [sam])
            .unwrap();
        assert!(find_session(&conn, &theirs.token, start).unwrap().is_none());
    }

    fn row_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn renewing_a_session_that_was_ended_answers_none_and_brings_no_row_back() {
        let (_dir, conn) = db();
        let user = admin(&conn);
        let start = at("2026-01-01T00:00:00Z");
        let issued = create_session(&conn, user, SessionKind::Cookie, None, None, start).unwrap();
        let later = at("2026-01-02T00:00:00Z");
        let found = find_session(&conn, &issued.token, later).unwrap().unwrap();
        end_session(&conn, issued.id).unwrap();
        assert_eq!(renew_session(&conn, &found, later).unwrap(), None);
        assert_eq!(row_count(&conn), 0);
    }

    #[test]
    fn renewing_an_expired_unswept_session_answers_none_and_leaves_it_expired() {
        let (_dir, conn) = db();
        let user = admin(&conn);
        let start = at("2026-01-01T00:00:00Z");
        let issued = create_session(&conn, user, SessionKind::Cookie, None, None, start).unwrap();
        let found = find_session(&conn, &issued.token, start).unwrap().unwrap();
        let after_expiry = at("2026-02-01T00:00:00Z");
        assert_eq!(renew_session(&conn, &found, after_expiry).unwrap(), None);
        let (seen, expires): (String, String) = conn
            .query_row("SELECT last_seen_at, expires_at FROM sessions", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(seen, "2026-01-01T00:00:00Z");
        assert_eq!(expires, "2026-01-31T00:00:00Z");
    }

    #[test]
    fn renewal_starts_just_past_an_hour_after_the_last_one() {
        let (_dir, conn) = db();
        let user = admin(&conn);
        let start = at("2026-01-01T00:00:00Z");
        let issued = create_session(&conn, user, SessionKind::Cookie, None, None, start).unwrap();
        let found = find_session(&conn, &issued.token, start).unwrap().unwrap();
        assert_eq!(
            renew_session(&conn, &found, at("2026-01-01T00:59:59Z")).unwrap(),
            None
        );
        assert!(renew_session(&conn, &found, at("2026-01-01T01:00:01Z"))
            .unwrap()
            .is_some());
    }

    #[test]
    fn renewal_waits_an_hour_slides_thirty_days_and_never_passes_ninety() {
        let (_dir, conn) = db();
        let user = admin(&conn);
        let start = at("2026-01-01T00:00:00Z");
        let issued = create_session(&conn, user, SessionKind::Cookie, None, None, start).unwrap();
        let session = |now| find_session(&conn, &issued.token, now).unwrap().unwrap();

        let soon = at("2026-01-01T00:59:00Z");
        assert_eq!(renew_session(&conn, &session(soon), soon).unwrap(), None);

        let later = at("2026-01-20T00:00:00Z");
        assert_eq!(
            renew_session(&conn, &session(later), later)
                .unwrap()
                .as_deref(),
            Some("2026-02-19T00:00:00Z")
        );
        let renewed = session(at("2026-02-01T00:00:00Z"));
        assert_eq!(renewed.expires_at, "2026-02-19T00:00:00Z");
        assert_eq!(renewed.last_seen_at, "2026-01-20T00:00:00Z");

        let near_the_cap = at("2026-03-15T00:00:00Z");
        conn.execute(
            "UPDATE sessions SET last_seen_at = '2026-03-01T00:00:00Z', expires_at = '2026-03-31T00:00:00Z'",
            [],
        )
        .unwrap();
        assert_eq!(
            renew_session(&conn, &session(near_the_cap), near_the_cap)
                .unwrap()
                .as_deref(),
            Some("2026-04-01T00:00:00Z"),
            "created_at + 90 days caps the renewal"
        );
        assert!(
            find_session(&conn, &issued.token, at("2026-04-01T00:00:00Z"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn the_user_agent_is_cut_to_256_characters() {
        let (_dir, conn) = db();
        let user = admin(&conn);
        let long = "é".repeat(300);
        create_session(
            &conn,
            user,
            SessionKind::Cookie,
            Some(&long),
            Some("203.0.113.9"),
            at("2026-01-01T00:00:00Z"),
        )
        .unwrap();
        let (ua, ip): (String, String) = conn
            .query_row("SELECT user_agent, ip FROM sessions", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(ua.chars().count(), 256);
        assert_eq!(ip, "203.0.113.9");
    }

    #[test]
    fn ending_one_all_or_all_but_one_and_sweeping() {
        let (_dir, conn) = db();
        let user = admin(&conn);
        let start = at("2026-01-01T00:00:00Z");
        let a = create_session(&conn, user, SessionKind::Cookie, None, None, start).unwrap();
        let b = create_session(&conn, user, SessionKind::Cookie, None, None, start).unwrap();
        let c = create_session(&conn, user, SessionKind::Bearer, None, None, start).unwrap();
        let sam = insert_user(&conn, "sam@example.com", "bookkeeper");
        let theirs = create_session(&conn, sam, SessionKind::Cookie, None, None, start).unwrap();
        let live = |token: &str| find_session(&conn, token, start).unwrap().is_some();
        assert!(end_session(&conn, a.id).unwrap());
        assert!(!end_session(&conn, a.id).unwrap());
        assert_eq!(end_user_sessions(&conn, user, Some(b.id)).unwrap(), 1);
        assert!(live(&b.token));
        assert!(!live(&c.token));
        assert!(live(&theirs.token));
        assert_eq!(end_user_sessions(&conn, user, None).unwrap(), 1);
        assert!(!live(&b.token));
        assert!(live(&theirs.token));
        assert_eq!(end_user_sessions(&conn, sam, None).unwrap(), 1);

        create_session(&conn, user, SessionKind::Cookie, None, None, start).unwrap();
        assert_eq!(sweep_expired(&conn, at("2026-01-30T23:59:59Z")).unwrap(), 0);
        assert_eq!(sweep_expired(&conn, at("2026-01-31T00:00:00Z")).unwrap(), 1);
    }
}
