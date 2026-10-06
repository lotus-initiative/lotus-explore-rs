// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `url_state`, in their own file.

use super::deployment_base_path;

/// Every client-side route, which is the same list the server router, the
/// Dockerfile export stage, `nginx.conf`, and `index.html` each keep.
///
/// Looped because the per-route version read like coverage while being a
/// list that fell out of date: adding a route never touched it.
const ROUTES: [&str; 4] = ["/search", "/curation", "/draw", "/faq"];

#[test]
fn deployment_base_path_preserves_repository_prefix() {
    assert_eq!(deployment_base_path("/"), "");
    assert_eq!(
        deployment_base_path("/lotus-explore-rs/"),
        "/lotus-explore-rs"
    );
}

#[test]
fn every_client_route_resolves_back_to_the_same_base() {
    for route in ROUTES {
        assert_eq!(
            deployment_base_path(&format!("/lotus-explore-rs{route}")),
            "/lotus-explore-rs",
            "{route} should be recognised as a route, not as part of the base"
        );
    }
}
