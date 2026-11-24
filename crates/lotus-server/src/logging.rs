// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Minimal stderr logger for the native HTTP API server, replacing
//! `tracing-subscriber`.

use std::io::Write as _;

struct StderrLogger;

impl log::Log for StderrLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(
            stderr,
            "[{} {}] {}",
            record.level(),
            record.target(),
            record.args()
        );
    }

    fn flush(&self) {}
}

/// Installs the global `log` stderr logger using `RUST_LOG` (defaults to `info`).
pub fn init() {
    static LOGGER: StderrLogger = StderrLogger;
    let env_val = std::env::var("RUST_LOG").ok();
    let filter = parse_log_level(env_val.as_deref());
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(filter);
    }
}

/// Parses a `RUST_LOG`-style directive string into a `log::LevelFilter`.
pub fn parse_log_level(spec: Option<&str>) -> log::LevelFilter {
    let Some(raw) = spec else {
        return log::LevelFilter::Info;
    };
    let mut best: Option<log::LevelFilter> = None;
    for part in raw.split(',') {
        let token = part
            .trim()
            .rsplit_once('=')
            .map_or_else(|| part.trim(), |(_, level)| level.trim());
        let parsed = match token.to_ascii_lowercase().as_str() {
            "off" => Some(log::LevelFilter::Off),
            "error" => Some(log::LevelFilter::Error),
            "warn" => Some(log::LevelFilter::Warn),
            "info" => Some(log::LevelFilter::Info),
            "debug" => Some(log::LevelFilter::Debug),
            "trace" => Some(log::LevelFilter::Trace),
            _ => None,
        };
        if let Some(level) = parsed {
            best = Some(best.map_or(level, |prev| prev.max(level)));
        }
    }
    best.unwrap_or(log::LevelFilter::Info)
}

#[cfg(test)]
#[path = "logging/tests.rs"]
mod tests;
