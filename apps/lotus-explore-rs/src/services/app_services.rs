// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Application-level services and dependency container.

use crate::repositories::HybridRepository;

/// Application-wide services container.
/// Holds references to all singleton dependencies needed throughout the app.
/// Designed to be provided via Dioxus context and used by hooks/components.
#[derive(Clone, Copy)]
pub struct AppServices {
    /// Data repository (API/SPARQL hybrid adapter).
    repo: HybridRepository,
}

impl AppServices {
    /// Create a new services container with all dependencies initialized.
    pub const fn new() -> Self {
        Self {
            repo: HybridRepository,
        }
    }

    /// Get the data repository.
    pub const fn repository(self) -> HybridRepository {
        self.repo
    }
}

#[cfg(test)]
mod tests {
    // The double-bind is the point: it is a compile-time assertion that
    // `AppServices` is Copy (the second bind would fail to compile otherwise).
    #![allow(clippy::no_effect_underscore_binding)]

    use super::*;

    #[test]
    fn app_services_is_copy() {
        let services = AppServices::new();
        let _copy = services;
        let _another_copy = services;
    }

    #[test]
    fn app_services_repository_is_consistent() {
        let services = AppServices::new();
        let repo1 = services.repository();
        let repo2 = services.repository();
        assert_eq!(repo1, repo2);
    }
}
