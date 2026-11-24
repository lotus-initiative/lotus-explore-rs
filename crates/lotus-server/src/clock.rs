// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Host wall clock for publication-year validation bounds.
#![expect(
    clippy::redundant_pub_crate,
    reason = "private crate-root module uses pub(crate) because workspace denies unreachable_pub"
)]

/// Current UTC calendar year read from the system clock.
pub(crate) fn current_year() -> u16 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs().cast_signed());
    u16::try_from(lotus_model::utc::UtcDateTime::from_unix_seconds(secs).year).unwrap_or(u16::MAX)
}
