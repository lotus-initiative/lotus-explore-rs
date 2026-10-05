// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Application-level services and dependency container.

use crate::repositories::HybridRepository;

/// Application-wide singleton dependencies, provided via Dioxus context.
///
/// `Copy` is load-bearing: read by many components through context, and a `Clone`
/// that deep-copied a repository would hand each its own cache. The `const`
/// assertion below checks that at build time, since a non-`Copy` type is a
/// compile error at every call site anyway.
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

/// `AppServices` must stay `Copy`: it is shared through Dioxus context, and a
/// `Clone` that owned its own repository would give each component a separate
/// cache. Binding it twice below would not compile otherwise, and this is where
/// that is checked -- a test would pass whether or not the type were `Copy`,
/// because the assertion is decided at compile time.
const _: () = {
    fn assert_copy<T: Copy>(_: T, _: T) {}
    fn check(services: AppServices) {
        assert_copy(services, services);
    }
    let _ = check;
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_services_repository_is_consistent() {
        let services = AppServices::new();
        let repo1 = services.repository();
        let repo2 = services.repository();
        assert_eq!(repo1, repo2);
    }
}
