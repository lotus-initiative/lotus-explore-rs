// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `chemical`, in their own file.

use super::extract_exact_mass_from_json;

#[test]
fn a_mass_is_found_at_any_depth() {
    let nested = serde_json::json!({
        "results": [{ "compound": { "exact_molecular_weight": "46.04186" } }]
    });
    assert_eq!(extract_exact_mass_from_json(&nested), Some(46.04186));
}
