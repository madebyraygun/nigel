//! Who a request runs as. The session layer (or the desktop router) inserts
//! one on every `/api` request; handlers take it as an extractor.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use super::error::ApiError;
use crate::identity::sessions::{ActiveSession, SessionKind};
use crate::identity::Role;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionUser {
    pub id: i64,
    pub uid: String,
    pub email: String,
    pub role: Role,
    pub session_id: i64,
    pub kind: SessionKind,
    pub must_change_password: bool,
}

impl From<&ActiveSession> for SessionUser {
    fn from(session: &ActiveSession) -> Self {
        Self {
            id: session.user_id,
            uid: session.uid.clone(),
            email: session.email.clone(),
            role: session.role,
            session_id: session.id,
            kind: session.kind,
            must_change_password: session.must_change_password,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Principal {
    /// CLI-equivalent authority: a local-mode token session, or the desktop
    /// transport.
    LocalOwner,
    User(SessionUser),
}

impl Principal {
    /// True for the local owner, and for a user whose role is at least `role`.
    pub fn satisfies(&self, role: Role) -> bool {
        match self {
            Principal::LocalOwner => true,
            Principal::User(user) => user.role >= role,
        }
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Principal {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Principal>()
            .cloned()
            .ok_or_else(|| ApiError::internal("This request reached a handler with no principal."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(role: Role) -> Principal {
        Principal::User(SessionUser {
            id: 1,
            uid: "0".repeat(32),
            email: "pat@example.com".into(),
            role,
            session_id: 1,
            kind: SessionKind::Cookie,
            must_change_password: false,
        })
    }

    #[test]
    fn the_local_owner_satisfies_every_role_and_a_user_their_own_and_below() {
        for role in Role::ALL {
            assert!(Principal::LocalOwner.satisfies(role));
        }
        assert!(user(Role::Bookkeeper).satisfies(Role::ReadOnly));
        assert!(user(Role::Bookkeeper).satisfies(Role::Bookkeeper));
        assert!(!user(Role::Bookkeeper).satisfies(Role::Admin));
        assert!(!user(Role::ReadOnly).satisfies(Role::Bookkeeper));
    }
}
