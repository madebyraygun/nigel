//! Who may sign in to a shared `nigel serve`: users, their sessions, their
//! passwords and their second factor.
//!
//! Not under `server/`, because the CLI creates and manages users too. The
//! books' own encryption password is `crate::password`, a different thing.
//!
//! Every function that writes or judges a time takes `now` as a parameter;
//! `clock::now_utc()` is the one read, at the caller.

/// Ordered `ReadOnly < Bookkeeper < Admin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    ReadOnly,
    Bookkeeper,
    Admin,
}

impl Role {
    pub const ALL: [Role; 3] = [Role::ReadOnly, Role::Bookkeeper, Role::Admin];

    pub fn as_str(self) -> &'static str {
        match self {
            Role::ReadOnly => "read_only",
            Role::Bookkeeper => "bookkeeper",
            Role::Admin => "admin",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "read_only" => Some(Role::ReadOnly),
            "bookkeeper" => Some(Role::Bookkeeper),
            "admin" => Some(Role::Admin),
            _ => None,
        }
    }

    /// The word a person reads: Admin, Bookkeeper, Read-only.
    pub fn label(self) -> &'static str {
        match self {
            Role::ReadOnly => "Read-only",
            Role::Bookkeeper => "Bookkeeper",
            Role::Admin => "Admin",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_order_and_round_trip() {
        assert!(Role::ReadOnly < Role::Bookkeeper && Role::Bookkeeper < Role::Admin);
        for role in Role::ALL {
            assert_eq!(Role::parse(role.as_str()), Some(role));
        }
        assert_eq!(Role::parse("owner"), None);
        assert_eq!(Role::ReadOnly.label(), "Read-only");
    }
}
