// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

pub fn log_info_evt(event: &str, phase: &str, state: &str, details: Option<&str>) {
    if let Some(details) = details.filter(|details| !details.is_empty()) {
        log::info!("event={event} phase={phase} state={state} {details}");
    } else {
        log::info!("event={event} phase={phase} state={state}");
    }
}

pub fn log_debug_evt(event: &str, phase: &str, state: &str, details: Option<&str>) {
    if let Some(details) = details.filter(|details| !details.is_empty()) {
        log::debug!("event={event} phase={phase} state={state} {details}");
    } else {
        log::debug!("event={event} phase={phase} state={state}");
    }
}

pub fn log_warn_evt(event: &str, phase: &str, state: &str, details: Option<&str>) {
    if let Some(details) = details.filter(|details| !details.is_empty()) {
        log::warn!("event={event} phase={phase} state={state} {details}");
    } else {
        log::warn!("event={event} phase={phase} state={state}");
    }
}

pub fn log_timing_evt(
    event: &str,
    phase: &str,
    state: &str,
    duration: std::time::Duration,
    details: Option<&str>,
) {
    let elapsed_ms = duration.as_secs_f64() * 1000.0;
    if let Some(details) = details.filter(|details| !details.is_empty()) {
        log::info!(
            "event={event} phase={phase} state={state} elapsed_ms={elapsed_ms:.1} {details}"
        );
    } else {
        log::info!("event={event} phase={phase} state={state} elapsed_ms={elapsed_ms:.1}");
    }
}
