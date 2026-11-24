// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! System clock and UTC timestamp helpers for the CLI binary.

/// The current calendar year, for the default upper year bound.
///
/// `lotus-model` takes the year as an argument rather than reading a clock, so
/// that it stays pure; a binary is where a clock belongs.
#[must_use]
pub fn current_year() -> u16 {
    // A clock outside u16's years saturates rather than wrapping to a
    // plausible-looking year.
    u16::try_from(utc_now().year).unwrap_or(u16::MAX)
}

/// The current UTC time as an ISO-8601 stamp (`YYYY-MM-DDTHH:MM:SSZ`), for the
/// export metadata document.
#[must_use]
pub fn now_iso_utc() -> String {
    iso_utc(utc_now())
}

/// Reads the system clock via `std::time::SystemTime` and converts via
/// [`lotus_model::utc::UtcDateTime::from_unix_seconds`].
fn utc_now() -> lotus_model::utc::UtcDateTime {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs().cast_signed());
    lotus_model::utc::UtcDateTime::from_unix_seconds(secs)
}

/// An instant as an ISO-8601 UTC stamp (`YYYY-MM-DDTHH:MM:SSZ`): whole seconds,
/// and `Z` rather than `+00:00`, which is what the metadata documents carry.
#[must_use]
pub fn iso_utc(instant: lotus_model::utc::UtcDateTime) -> String {
    instant.format_iso_utc()
}

/// Parse an ISO-8601 UTC timestamp (`YYYY-MM-DDTHH:MM:SSZ`) into a `UtcDateTime`.
///
/// The archive needs a `UtcDateTime` for the filename, and `--now` arrives as text.
pub fn parse_iso_utc(s: &str) -> anyhow::Result<lotus_model::utc::UtcDateTime> {
    lotus_model::utc::UtcDateTime::parse_rfc3339_utc(s)
        .map_err(|e| anyhow::anyhow!("--now must be an ISO-8601 UTC timestamp: {e}"))
}
