// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Pure proleptic Gregorian UTC calendar arithmetic and RFC 3339 formatting.
//!
//! No system clock and no I/O: callers pass Unix timestamps or strings in and
//! receive calendar values or formatted UTC stamps back.

use std::fmt;

use thiserror::Error;

const SECONDS_PER_DAY: i64 = 86_400;
const DAYS_TO_UNIX_EPOCH: i64 = 719_468;
const DAYS_PER_400_YEARS: i64 = 146_097;

/// A calendar date and time in UTC (whole seconds).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UtcDateTime {
    /// Proleptic Gregorian year (e.g. `2026`).
    pub year: i32,
    /// Month of the year (`1..=12`).
    pub month: u8,
    /// Day of the month (`1..=31`).
    pub day: u8,
    /// Hour of the day (`0..=23`).
    pub hour: u8,
    /// Minute of the hour (`0..=59`).
    pub minute: u8,
    /// Second of the minute (`0..=59`).
    pub second: u8,
}

/// Error returned when parsing an RFC 3339 UTC timestamp fails.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[error("invalid RFC 3339 timestamp: {reason}")]
pub struct UtcParseError {
    reason: &'static str,
}

impl UtcParseError {
    const fn new(reason: &'static str) -> Self {
        Self { reason }
    }
}

impl UtcDateTime {
    /// Converts a Unix timestamp (seconds since `1970-01-01T00:00:00Z`) to UTC.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "all narrowed fields are bounded by modulo arithmetic or Gregorian calendar limits"
    )]
    pub const fn from_unix_seconds(secs: i64) -> Self {
        let days = secs.div_euclid(SECONDS_PER_DAY);
        let rem = secs.rem_euclid(SECONDS_PER_DAY);
        let hour = (rem / 3_600) as u8;
        let minute = ((rem % 3_600) / 60) as u8;
        let second = (rem % 60) as u8;
        let (year, month, day) = civil_from_days(days);
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }

    /// Formats this timestamp as `YYYY-MM-DDTHH:MM:SSZ`.
    #[must_use]
    pub fn format_iso_utc(self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }

    /// Formats this timestamp as `YYYYMMDDTHHMMSSZ` (compact filename stamp).
    #[must_use]
    pub fn format_compact_utc(self) -> String {
        format!(
            "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }

    /// Formats the calendar date as `YYYY-MM-DD`.
    #[must_use]
    pub fn format_date(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// Parses an RFC 3339 timestamp (`YYYY-MM-DDTHH:MM:SS[.frac](Z|+HH:MM|-HH:MM)`)
    /// and normalizes it to whole-second UTC.
    ///
    /// # Errors
    /// Returns [`UtcParseError`] when the syntax, calendar date, time, or offset
    /// is invalid.
    pub fn parse_rfc3339_utc(input: &str) -> Result<Self, UtcParseError> {
        let bytes = input.as_bytes();
        // Minimum valid timestamp: `YYYY-MM-DDTHH:MM:SSZ` (20 bytes).
        if bytes.len() < 20 {
            return Err(UtcParseError::new("expected YYYY-MM-DDTHH:MM:SSZ"));
        }
        if bytes.get(4) != Some(&b'-')
            || bytes.get(7) != Some(&b'-')
            || !matches!(bytes.get(10), Some(b'T' | b't'))
            || bytes.get(13) != Some(&b':')
            || bytes.get(16) != Some(&b':')
        {
            return Err(UtcParseError::new(
                "expected YYYY-MM-DDTHH:MM:SS separators",
            ));
        }

        let year = i32::from(parse_u16_digits(bytes.get(0..4))?);
        let month = parse_u8_digits(bytes.get(5..7))?;
        let day = parse_u8_digits(bytes.get(8..10))?;
        let hour = parse_u8_digits(bytes.get(11..13))?;
        let minute = parse_u8_digits(bytes.get(14..16))?;
        let second = parse_u8_digits(bytes.get(17..19))?;

        if !(1..=12).contains(&month) {
            return Err(UtcParseError::new("month out of range"));
        }
        let max_day = days_in_month(year, month);
        if day < 1 || day > max_day {
            return Err(UtcParseError::new("day out of range"));
        }
        if hour > 23 || minute > 59 || second > 59 {
            return Err(UtcParseError::new("time component out of range"));
        }

        let mut pos = 19;
        if bytes.get(pos) == Some(&b'.') {
            let after_dot = pos + 1;
            let frac_len = bytes
                .get(after_dot..)
                .unwrap_or_default()
                .iter()
                .take_while(|b| b.is_ascii_digit())
                .count();
            if frac_len == 0 {
                return Err(UtcParseError::new("empty fractional seconds"));
            }
            pos = after_dot + frac_len;
        }

        let offset_secs = parse_utc_offset(bytes.get(pos..))?;
        let days = days_from_civil(i64::from(year), i64::from(month), i64::from(day));
        let local_secs = days * SECONDS_PER_DAY
            + i64::from(hour) * 3_600
            + i64::from(minute) * 60
            + i64::from(second);
        Ok(Self::from_unix_seconds(local_secs - offset_secs))
    }
}

impl fmt::Display for UtcDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format_iso_utc())
    }
}

/// Maps days since the Unix epoch (`1970-01-01`) to the UTC calendar year,
/// clamped to `0..=u16::MAX`.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "year is clamped to 0..=u16::MAX before narrowing"
)]
pub const fn year_from_days_since_epoch(days: i64) -> u16 {
    let (year, _, _) = civil_from_days(days);
    if year <= 0 {
        0
    } else if year >= u16::MAX as i32 {
        u16::MAX
    } else {
        year as u16
    }
}

/// Howard Hinnant's proleptic Gregorian `civil_from_days` algorithm (public domain).
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "month (1..=12) and day (1..=31) fit in u8; era shift guarantees non-negative doe"
)]
const fn civil_from_days(days: i64) -> (i32, u8, u8) {
    // Clamp before adding DAYS_TO_UNIX_EPOCH so extreme i64 inputs stay within
    // i32::MIN..=i32::MAX calendar years without overflow or sign wrap.
    const MIN_DAYS: i64 = -700_000_000_000;
    const MAX_DAYS: i64 = 700_000_000_000;
    let clamped = if days < MIN_DAYS {
        MIN_DAYS
    } else if days > MAX_DAYS {
        MAX_DAYS
    } else {
        days
    };
    let z = clamped + DAYS_TO_UNIX_EPOCH;
    let era = z.div_euclid(DAYS_PER_400_YEARS);
    let doe = (z - era * DAYS_PER_400_YEARS) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let month = if mp < 10 {
        (mp + 3) as u8
    } else {
        (mp - 9) as u8
    };
    let year = if month <= 2 { y + 1 } else { y };
    (year as i32, month, day)
}

/// Inverse of [`civil_from_days`]: days since `1970-01-01` for a valid proleptic
/// Gregorian date.
const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let shifted_month = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * shifted_month + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * DAYS_PER_400_YEARS + doe - DAYS_TO_UNIX_EPOCH
}

const fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

const fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn parse_u8_digits(slice: Option<&[u8]>) -> Result<u8, UtcParseError> {
    let Some([hi, lo]) = slice else {
        return Err(UtcParseError::new("expected two digits"));
    };
    if !hi.is_ascii_digit() || !lo.is_ascii_digit() {
        return Err(UtcParseError::new("expected ASCII digits"));
    }
    Ok((hi - b'0') * 10 + (lo - b'0'))
}

fn parse_u16_digits(slice: Option<&[u8]>) -> Result<u16, UtcParseError> {
    let Some([a, b, c, d]) = slice else {
        return Err(UtcParseError::new("expected four-digit year"));
    };
    let mut acc = 0_u16;
    for byte in [a, b, c, d] {
        if !byte.is_ascii_digit() {
            return Err(UtcParseError::new("expected ASCII year digits"));
        }
        acc = acc * 10 + u16::from(byte - b'0');
    }
    Ok(acc)
}

fn parse_utc_offset(slice: Option<&[u8]>) -> Result<i64, UtcParseError> {
    match slice {
        Some([b'Z' | b'z']) => Ok(0),
        Some([sign @ (b'+' | b'-'), h1, h2, b':', m1, m2]) => {
            let hours = parse_u8_digits(Some(&[*h1, *h2]))?;
            let minutes = parse_u8_digits(Some(&[*m1, *m2]))?;
            if hours > 23 || minutes > 59 {
                return Err(UtcParseError::new("timezone offset out of range"));
            }
            let total = i64::from(hours) * 3_600 + i64::from(minutes) * 60;
            Ok(if *sign == b'-' { -total } else { total })
        }
        _ => Err(UtcParseError::new("expected Z or +HH:MM/-HH:MM offset")),
    }
}

#[cfg(test)]
#[path = "utc/tests.rs"]
mod tests;
