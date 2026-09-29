// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Consolidated app-level state for download orchestration and render telemetry.

use lotus_query::ExportFormat as DownloadFormat;

/// App-level state.  One signal of this type lives at the root of `App`.
/// Scope is deliberately narrow:
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct AppState {
    /// Download orchestration (format pending, direct-execute mode).
    pub download: DownloadState,

    /// One-shot logging guards used by the download-dispatch hook.
    pub metrics: MetricsState,

    /// Dark mode preference.
    pub dark_mode: bool,
}

// ── Download State: Orchestration ─────────────────────────────────────────────

/// Download action orchestration and pending-format queue.
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct DownloadState {
    /// Parsed pending programmatic download format.
    pub pending_format: Option<DownloadFormat>,

    /// Raw invalid format from URL (`?download=true&format=...`) preserved so
    /// startup validation can report the exact unsupported value once.
    pub pending_invalid_format: Option<String>,

    /// `true` when the URL included `?execute=true` (direct search + preview).
    pub direct_execute: bool,
}

// ── Metrics State: One-shot telemetry guards ──────────────────────────────────

/// Guards that prevent duplicate log events during the download-wait sequence.
/// These are reset to `false` once the awaited condition resolves.
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct MetricsState {
    /// We already logged "waiting for loading to finish" this dispatch cycle.
    pub waiting_loading_logged: bool,
    /// We already logged "waiting for SPARQL query to materialize" this cycle.
    pub waiting_query_logged: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_state_default_is_inactive() {
        let state = AppState::default();
        assert_eq!(state.download, DownloadState::default());
    }

    #[test]
    fn download_state_default_is_inactive() {
        let state = DownloadState::default();
        assert!(state.pending_format.is_none());
        assert!(state.pending_invalid_format.is_none());
        assert!(!state.direct_execute);
    }

    #[test]
    fn metrics_state_default_has_no_logged_guards() {
        let state = MetricsState::default();
        assert!(!state.waiting_loading_logged);
        assert!(!state.waiting_query_logged);
    }

    #[test]
    fn app_state_clone_is_equal_to_original() {
        let a = AppState::default();
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn download_state_with_pending_format_is_not_default() {
        let s = DownloadState {
            pending_format: Some(DownloadFormat::Csv),
            pending_invalid_format: None,
            direct_execute: false,
        };
        assert_ne!(s, DownloadState::default());
    }

    #[test]
    fn metrics_state_loading_guard_can_be_set_and_cleared() {
        let m = MetricsState {
            waiting_loading_logged: true,
            ..MetricsState::default()
        };
        assert!(m.waiting_loading_logged);
        let m2 = MetricsState {
            waiting_loading_logged: false,
            ..MetricsState::default()
        };
        assert!(!m2.waiting_loading_logged);
    }

    #[test]
    fn download_state_direct_execute_flag_round_trips() {
        let s = DownloadState {
            pending_format: None,
            pending_invalid_format: None,
            direct_execute: true,
        };
        assert!(s.direct_execute);
        assert_ne!(s, DownloadState::default());
    }

    #[test]
    fn download_state_can_hold_invalid_startup_format() {
        let s = DownloadState {
            pending_format: None,
            pending_invalid_format: Some("ttl".into()),
            direct_execute: false,
        };
        assert_eq!(s.pending_invalid_format.as_deref(), Some("ttl"));
    }
}
