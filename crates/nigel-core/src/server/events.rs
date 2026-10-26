//! One stderr line per sign-in event — the journal record under systemd
//! until the audit log replaces these lines with rows carrying the same
//! fields. `reason` is a `&'static str`, so no event can carry a secret.

/// The longest email a line quotes, in characters.
const EMAIL_MAX_CHARS: usize = 254;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    LoginSucceeded,
    LoginFailed,
    LoginRefused,
    Lockout,
    Logout,
    PasswordChanged,
    TotpEnabled,
    TotpRemoved,
    TotpUnavailable,
    SharedModeBegan,
    LimiterFull,
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LoginSucceeded => "login_succeeded",
            Self::LoginFailed => "login_failed",
            Self::LoginRefused => "login_refused",
            Self::Lockout => "lockout",
            Self::Logout => "logout",
            Self::PasswordChanged => "password_changed",
            Self::TotpEnabled => "totp_enabled",
            Self::TotpRemoved => "totp_removed",
            Self::TotpUnavailable => "totp_unavailable",
            Self::SharedModeBegan => "shared_mode_began",
            Self::LimiterFull => "limiter_full",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthEvent<'a> {
    kind: EventKind,
    email: Option<&'a str>,
    user_id: Option<i64>,
    ip: Option<&'a str>,
    session_id: Option<i64>,
    reason: Option<&'static str>,
}

impl<'a> AuthEvent<'a> {
    pub fn new(kind: EventKind) -> Self {
        Self {
            kind,
            email: None,
            user_id: None,
            ip: None,
            session_id: None,
            reason: None,
        }
    }

    pub fn email(mut self, email: &'a str) -> Self {
        self.email = Some(email);
        self
    }

    pub fn user(mut self, id: i64) -> Self {
        self.user_id = Some(id);
        self
    }

    pub fn ip(mut self, ip: &'a str) -> Self {
        self.ip = Some(ip);
        self
    }

    pub fn session(mut self, id: i64) -> Self {
        self.session_id = Some(id);
        self
    }

    pub fn reason(mut self, reason: &'static str) -> Self {
        self.reason = Some(reason);
        self
    }

    /// `auth event=… email="…" user=… ip=… session=… reason=…`. The email is
    /// whatever was typed at a login, so it is cut short and `{:?}`-escaped:
    /// a newline in it cannot start a second line.
    pub fn line(&self) -> String {
        let dash = |v: Option<String>| v.unwrap_or_else(|| "-".to_string());
        let email = self.email.map(|e| {
            let cut: String = e.chars().take(EMAIL_MAX_CHARS).collect();
            format!("{cut:?}")
        });
        let ip = self.ip.map(|ip| {
            ip.chars()
                .take(64)
                .collect::<String>()
                .escape_debug()
                .to_string()
        });
        format!(
            "auth event={} email={} user={} ip={} session={} reason={}",
            self.kind.as_str(),
            dash(email),
            dash(self.user_id.map(|id| id.to_string())),
            dash(ip),
            dash(self.session_id.map(|id| id.to_string())),
            self.reason.unwrap_or("-"),
        )
    }
}

/// Write the event's line to stderr.
pub fn emit(event: &AuthEvent<'_>) {
    eprintln!("{}", event.line());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_names_every_field_and_dashes_the_absent_ones() {
        let line = AuthEvent::new(EventKind::LoginSucceeded)
            .email("pat@example.com")
            .user(2)
            .ip("203.0.113.9")
            .session(14)
            .line();
        assert_eq!(
            line,
            "auth event=login_succeeded email=\"pat@example.com\" user=2 ip=203.0.113.9 session=14 reason=-"
        );
        let line = AuthEvent::new(EventKind::LoginFailed)
            .email("nobody@example.com")
            .ip("203.0.113.9")
            .reason("credentials")
            .line();
        assert_eq!(
            line,
            "auth event=login_failed email=\"nobody@example.com\" user=- ip=203.0.113.9 session=- reason=credentials"
        );
    }

    #[test]
    fn an_email_cannot_forge_a_second_line() {
        let typed = "pat@example.com\nauth event=login_succeeded \"x\"";
        let line = AuthEvent::new(EventKind::LoginFailed).email(typed).line();
        assert!(!line.contains('\n'), "{line}");
        assert!(line.contains("\\n"), "{line}");
        assert!(line.contains("\\\"x\\\""), "{line}");

        let long = "a".repeat(10_000);
        let line = AuthEvent::new(EventKind::LoginFailed).email(&long).line();
        assert!(line.len() < 400, "{}", line.len());
    }
}
