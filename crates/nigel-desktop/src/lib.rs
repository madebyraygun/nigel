//! The desktop shell's library surface: `main` links it to build the window,
//! and the integration tests under `tests/` link it to drive the same request
//! path with no window at all.

pub mod chrome;
pub mod db;
pub mod imports;
pub mod menu;
pub mod reveal;
pub mod save;
pub mod transport;
pub mod window_state;

/// The scheme's URL, origin and guard, shared with the Swift shell's
/// `nigel-ffi` through `nigel-core` so the origin cannot drift between them.
pub use nigel_core::server::scheme::{
    scheme_origin, scheme_url, trusted_host, trusted_origins, SCHEME,
};
