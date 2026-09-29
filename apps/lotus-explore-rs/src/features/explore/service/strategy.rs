// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Query execution strategy — selects between the three search paths.

/// Controls which I/O path the search pipeline takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionStrategy {
    /// Try the REST API first; fall back to direct SPARQL if the API is
    /// unconfigured or returns an error.
    ApiFirst,
    /// Skip the REST API and go straight to direct SPARQL queries.
    Direct,
    /// Skip all queries; build the query string and return it for download.
    DownloadOnly,
}

impl ExecutionStrategy {
    /// Choose a strategy from the `direct_download` flag and whether the REST
    /// API is explicitly enabled.
    pub const fn resolve(direct_download: bool, api_enabled: bool) -> Self {
        if direct_download {
            Self::DownloadOnly
        } else if api_enabled {
            Self::ApiFirst
        } else {
            Self::Direct
        }
    }

    pub fn is_download_only(self) -> bool {
        self == Self::DownloadOnly
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_flag_selects_download_only() {
        assert_eq!(
            ExecutionStrategy::resolve(true, /* api_enabled */ false),
            ExecutionStrategy::DownloadOnly
        );
    }

    #[test]
    fn normal_flag_selects_direct_by_default() {
        // The API fast-path is opt-in: with no API configured, interactive
        // searches go direct to SPARQL rather than hitting the REST API.
        assert_eq!(
            ExecutionStrategy::resolve(false, /* api_enabled */ false),
            ExecutionStrategy::Direct
        );
    }

    #[test]
    fn api_enabled_flag_selects_api_first() {
        assert_eq!(
            ExecutionStrategy::resolve(false, /* api_enabled */ true),
            ExecutionStrategy::ApiFirst
        );
    }

    #[test]
    fn download_only_is_download_only() {
        assert!(ExecutionStrategy::DownloadOnly.is_download_only());
        assert!(!ExecutionStrategy::ApiFirst.is_download_only());
        assert!(!ExecutionStrategy::Direct.is_download_only());
    }
}
