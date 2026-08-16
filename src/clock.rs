//! The app's one clock read.
//!
//! Nothing under `src/invoicing/` reads the clock — every derived status takes
//! its reference day as a parameter, which is what makes them deterministic in
//! tests and correct against the wall clock in production. This is where that
//! day comes from.

/// Today's local date as `YYYY-MM-DD` — the reference day every date-less
/// command ages, derives and reports against.
pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}
