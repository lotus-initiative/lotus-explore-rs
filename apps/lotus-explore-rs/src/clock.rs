// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! The one place in the app that reads a clock.
//!
//! `lotus-model` is pure and takes the year as an argument, which is what lets
//! its tests pin a year instead of freezing time. Something has to supply the
//! real one, and the app is that something.
//!
//! Two implementations, because the two hosts do not have the same clock.
//! `std::time::SystemTime` compiles for `wasm32-unknown-unknown` and panics
//! when it is called — `time not implemented on this platform` — which is not
//! visible from a build and takes the whole app down on first render. The
//! browser has `Date`, and that is what the wasm build reads.
//!
//! The build succeeded and the app was broken. That is what the wasm job in CI
//! cannot catch and a browser can, and it is why the two are separate functions
//! rather than one behind a runtime check: a runtime check would still compile
//! the branch that panics.

/// The current calendar year.
///
/// Used for the default upper year bound on a publication date, so a reference
/// dated next year is not rejected before it is published, and as the upper
/// bound when deciding whether a year filter is doing anything.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn current_year() -> u16 {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0_i64, |d| {
            i64::try_from(d.as_secs()).unwrap_or(i64::MAX) / 86_400
        });
    year_from_days_since_epoch(days)
}

/// The current calendar year, from the browser's clock.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn current_year() -> u16 {
    // `Date::now` is milliseconds since the epoch as an `f64`. It returns
    // `NaN` rather than throwing when there is no working clock, and `NaN`
    // would flow through the arithmetic below into a cast, which is defined to
    // yield 0 -- and a day count of 0 reads as 1970, which rejects every
    // reference as predating 1800 and returns an empty result set. That looks
    // like correct behaviour, so the guard is here rather than at the call
    // site.
    let millis = js_sys::Date::now();
    if !millis.is_finite() || !(0.0..=MAX_MILLIS).contains(&millis) {
        return FALLBACK_YEAR;
    }
    // Truncating is what we want, not rounding: the year is the year the current
    // instant falls in, and an instant one second before midnight UTC is still
    // the earlier year. The cast is guarded by the range check above, which puts
    // the value inside the range a `f64` represents exactly at this magnitude.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "guarded by the range check above; the fractional part is seconds"
    )]
    let days = (millis / 86_400_000.0) as i64;
    year_from_days_since_epoch(days)
}

/// The largest millisecond timestamp the conversion above accepts.
///
/// Two bounds, and both matter. `f64` represents whole numbers exactly only
/// below 2^53, so a larger timestamp is not an exact count of milliseconds. And
/// the largest `i64` is about 9.2e18, which in milliseconds is 2.9e13 years --
///
/// far past any date a browser reports, so this is a guard against a tampered
/// or polyfilled `Date` rather than a real limit. Exceeding it is handled by the
/// fallback, which widens the search instead of emptying it.
#[cfg(target_arch = "wasm32")]
const MAX_MILLIS: f64 = 4_503_599_627_370_496.0; // 2^52

/// Used only when the host has no usable clock at all.
#[cfg(target_arch = "wasm32")]
const FALLBACK_YEAR: u16 = 9999;

/// Days since the Unix epoch to the calendar year that day falls in.
///
/// Howard Hinnant's `civil_from_days`, which is exact for every day in the
/// proleptic Gregorian calendar and needs no lookup table. The full function is
/// used rather than a year-only variant: the year-only version answers "which
/// year contains the day *before* this one", which is right for 364 days a year
/// and wrong on 1 January. That is the kind of bug that reads as correct for a
/// long time.
fn year_from_days_since_epoch(days: i64) -> u16 {
    // Shift the epoch to 0000-03-01 so that a leap day lands at the end of the
    // 400-year era rather than in the middle of it.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;

    // The year so far is the year the *March* falls in. January and February
    // have not happened yet in that year, so they belong to the next one, and
    // the only way to tell is to work out the month -- which is what this step
    // is for. Skipping it and returning `year` is right for 364 days a year.
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let year = if month_prime >= 10 { year + 1 } else { year };

    u16::try_from(year).unwrap_or(u16::MAX)
}

#[cfg(test)]
#[path = "clock/tests.rs"]
mod tests;
