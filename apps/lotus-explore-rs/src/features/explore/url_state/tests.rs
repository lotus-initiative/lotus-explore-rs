// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `url_state`, in their own file.

use super::{deployment_base_path, theme_icon_href};

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

/// The icon the app pins for each theme, and the file it has to resolve to.
///
/// Asserted against the files themselves, so renaming one fails here
/// instead of producing a 404 that silently falls back to the `.ico` and
/// leaves the tab icon on the light mark again.
#[test]
fn each_theme_pins_an_icon_that_exists() {
    const LIGHT: &str = include_str!("../../../../public/favicon-light.svg");
    const DARK: &str = include_str!("../../../../public/favicon-dark.svg");

    for (dark_mode, file, artwork) in [
        (false, "favicon-light.svg", LIGHT),
        (true, "favicon-dark.svg", DARK),
    ] {
        let href = theme_icon_href(dark_mode);
        assert_eq!(
            href, file,
            "dark_mode={dark_mode} must pin {file}, not {href}"
        );
        // The artwork is compiled in, so a missing file is a build error
        // rather than a 404 at runtime; assert it is the real mark anyway,
        // so an empty or truncated placeholder cannot pass.
        assert!(
            artwork.contains("viewBox=") && artwork.len() > 1000,
            "{file} is not a real icon"
        );
    }
}

#[test]
fn the_pinned_icons_are_not_the_os_driven_one() {
    // `favicon.svg` asks the OS. Pinning that file would answer the wrong
    // question on exactly the machine the pin exists for.
    for dark_mode in [false, true] {
        assert_ne!(theme_icon_href(dark_mode), "favicon.svg");
    }
}
