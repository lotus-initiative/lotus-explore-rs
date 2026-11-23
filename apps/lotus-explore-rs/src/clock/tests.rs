// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `clock`, in their own file.

use super::*;

#[test]
fn the_year_is_this_century() {
    // A wide band: the point is to catch an arithmetic slip, not to fail in
    // 2099. On a host with no clock this is the fallback, which is also
    // checked below.
    let year = current_year();
    assert!((2020..=2100).contains(&year), "{year}");
}

#[test]
fn native_iso_stamp_is_well_formed() {
    let stamp = now_iso8601();
    assert!(
        lotus_model::utc::UtcDateTime::parse_rfc3339_utc(&stamp).is_ok(),
        "now_iso8601 produced invalid RFC 3339: {stamp:?}"
    );
}
