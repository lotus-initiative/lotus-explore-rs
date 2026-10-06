// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `numeric`, in their own file.

use super::*;

#[test]
fn formats_mass_to_four_decimals() {
    assert_eq!(format_mass_value(194.0797), "194.0797");
    assert_eq!(format_mass_value(12.0), "12.0000");
}
