// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! The one place in the app that reads a clock.
//!
//! `lotus-model` is pure and takes the year as an argument, which is what lets
//! its tests pin a year instead of freezing time. Something has to supply the
//! real one, and a binary is that something.

/// The current calendar year.
///
/// Used for the default upper year bound on a publication date, so a reference
/// dated next year is not rejected before it is published.
#[must_use]
pub fn current_year() -> u16 {
    use std::time::{SystemTime, UNIX_EPOCH};

    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0_i64, |d| {
            i64::try_from(d.as_secs()).unwrap_or(i64::MAX) / 86_400
        });
    // Days since the epoch to a civil year, by Howard Hinnant's algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    u16::try_from(yoe + era * 400).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_year_is_this_century() {
        // A wide band: the point is to catch an arithmetic slip, not to fail
        // in 2099.
        let year = current_year();
        assert!((2020..=2100).contains(&year), "{year}");
    }
}
