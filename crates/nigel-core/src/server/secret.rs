//! A string that will not print itself.
//!
//! Passwords arrive over HTTP as ordinary JSON strings, and everything they
//! land in — request structs, error paths, `#[derive(Debug)]` output — is one
//! stray format call away from a log line. [`Secret`] makes that impossible by
//! construction: its `Debug` is redacted and it zeroizes on drop, matching what
//! `splash.rs` and `password_manager.rs` already do for TUI input buffers.

use std::fmt;

use serde::Deserialize;
use zeroize::Zeroize;

#[derive(Deserialize, Default, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    /// Read the wrapped value. Every call site is a place a password can escape,
    /// so the name is deliberately loud.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl From<String> for Secret {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// The session token: the whole of this server's authentication, shared by
/// every clone of `AppState`.
///
/// `Debug` is redacted for [`Secret`]'s reason, so a derived `Debug` on
/// anything holding the state cannot print it. Cheap to clone, which is why it
/// is not a [`Secret`]: the token lives as long as the server does.
#[derive(Clone)]
pub struct SessionToken(std::sync::Arc<str>);

impl SessionToken {
    /// Read the token. As loud as [`Secret::expose`], for the same reason.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl From<String> for SessionToken {
    fn from(mut value: String) -> Self {
        let token = Self(std::sync::Arc::from(value.as_str()));
        value.zeroize();
        token
    }
}

impl fmt::Debug for SessionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionToken(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_token_never_shows_itself() {
        let token = SessionToken::from("0123456789abcdef".to_string());
        let rendered = format!("{token:?}");
        assert_eq!(rendered, "SessionToken(<redacted>)");
        assert_eq!(token.clone().expose(), "0123456789abcdef");
    }

    #[derive(Debug, Deserialize)]
    struct Holder {
        password: Secret,
    }

    #[test]
    fn debug_never_shows_the_value() {
        let secret = Secret::from("hunter2".to_string());
        let rendered = format!("{secret:?}");
        assert_eq!(rendered, "Secret(<redacted>)");
        assert!(!rendered.contains("hunter2"));
    }

    #[test]
    fn redaction_survives_a_derived_debug() {
        let holder: Holder = serde_json::from_str(r#"{"password": "hunter2"}"#).expect("json");
        let rendered = format!("{holder:?}");
        assert!(!rendered.contains("hunter2"), "leaked in {rendered}");
        assert!(rendered.contains("<redacted>"), "got {rendered}");
    }

    #[test]
    fn deserializes_transparently() {
        let holder: Holder = serde_json::from_str(r#"{"password": "hunter2"}"#).expect("json");
        assert_eq!(holder.password.expose(), "hunter2");
    }
}
