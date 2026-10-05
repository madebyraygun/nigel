//! Documents: a filed PDF, sent to one signer and any collaborators, answered
//! online or recorded by hand, revised and countersigned. Recorded assent, not
//! a legal e-signature: a typed name, an explicit consent, a time, an IP and a
//! user agent, bound to a SHA-256 checksum of the exact PDF.
pub mod guards;
pub mod kinds;
pub mod lifecycle;
pub mod model;
pub mod record;
pub mod render;
pub mod send;
pub mod status;
pub mod store;
#[cfg(any(test, feature = "testutil"))]
pub mod testing;
pub mod wire;
