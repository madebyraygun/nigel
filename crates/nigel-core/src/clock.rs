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

/// The calendar day an instant falls on in a given zone, `YYYY-MM-DD`.
///
/// Generic over the zone so the conversion can be tested against a fixed
/// offset: reading it through the machine's own zone would make the answer a
/// property of the test host.
fn day_in<Tz: chrono::TimeZone>(epoch_seconds: i64, zone: &Tz) -> Option<String> {
    chrono::DateTime::from_timestamp(epoch_seconds, 0).map(|instant| {
        instant
            .with_timezone(zone)
            .date_naive()
            .format("%Y-%m-%d")
            .to_string()
    })
}

/// The local calendar day an epoch-second instant falls on.
///
/// The books are kept in local days, so this is where an instant handed over by
/// a gateway becomes one. `None` for a timestamp outside the representable
/// range, which is what a gateway sending nonsense looks like from here.
///
/// Converting a given instant is not a clock read: the reference day every
/// derivation ages against still arrives as a parameter.
pub fn local_day(epoch_seconds: i64) -> Option<String> {
    day_in(epoch_seconds, &chrono::Local)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    /// 2025-12-31T22:30:00Z is still December where the offset is zero or west
    /// of it, and already January five hours east. The conversion is pinned
    /// against explicit zones rather than the machine's, so the test says the
    /// same thing everywhere.
    #[test]
    fn an_instant_becomes_the_day_it_falls_on_in_the_zone_it_is_read_in() {
        let new_years_eve = 1_767_220_200;
        let utc = FixedOffset::east_opt(0).unwrap();
        let east = FixedOffset::east_opt(5 * 3600).unwrap();
        let west = FixedOffset::west_opt(5 * 3600).unwrap();

        assert_eq!(day_in(new_years_eve, &utc).unwrap(), "2025-12-31");
        assert_eq!(day_in(new_years_eve, &east).unwrap(), "2026-01-01");
        assert_eq!(day_in(new_years_eve, &west).unwrap(), "2025-12-31");
    }

    /// The local reading is a real calendar day, and an instant no calendar can
    /// hold is `None` rather than a panic or a guess.
    #[test]
    fn the_local_day_is_a_real_date_and_an_impossible_instant_is_none() {
        let day = local_day(1_767_220_200).expect("a representable instant");
        assert!(
            chrono::NaiveDate::parse_from_str(&day, "%Y-%m-%d").is_ok(),
            "not a calendar day: {day}"
        );
        assert!(local_day(i64::MAX).is_none());
    }
}
