// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Browser yield-cadence and wall-clock deadline scheduling state (host-testable).
#![cfg(any(target_arch = "wasm32", test))]

pub(super) const VISIBLE_YIELD_INTERVAL_MS: u64 = 16;
pub(super) const VISIBLE_YIELD_BYTES: usize = 262_144;
pub(super) const HIDDEN_YIELD_INTERVAL_MS: u64 = 100;
pub(super) const HIDDEN_YIELD_BYTES: usize = 1_048_576;
pub(super) const RESUME_GRACE_MS: u32 = 2_000;

/// Cadence tracker deciding when browser chunk reading should yield a macrotask
/// so paint/input stay responsive when visible without throttling to timer rates
/// when hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct YieldSchedule {
    last_yield_ms: u64,
    bytes_since_yield: usize,
}

impl YieldSchedule {
    pub(super) const fn new(now_ms: u64) -> Self {
        Self {
            last_yield_ms: now_ms,
            bytes_since_yield: 0,
        }
    }

    pub(super) const fn record_bytes(&mut self, bytes: usize) {
        self.bytes_since_yield = self.bytes_since_yield.saturating_add(bytes);
    }

    pub(super) const fn should_yield(&self, now_ms: u64, hidden: bool) -> bool {
        if self.bytes_since_yield == 0 {
            return false;
        }
        let (interval_ms, byte_cap) = if hidden {
            (HIDDEN_YIELD_INTERVAL_MS, HIDDEN_YIELD_BYTES)
        } else {
            (VISIBLE_YIELD_INTERVAL_MS, VISIBLE_YIELD_BYTES)
        };
        now_ms.saturating_sub(self.last_yield_ms) >= interval_ms
            || self.bytes_since_yield >= byte_cap
    }

    pub(super) const fn mark_yielded(&mut self, now_ms: u64) {
        self.bytes_since_yield = 0;
        self.last_yield_ms = now_ms;
    }
}

/// Convert a [`Duration`](std::time::Duration) into whole milliseconds,
/// saturating at `u32::MAX` for browser `setTimeout`.
pub(super) fn duration_millis_u32(duration: std::time::Duration) -> u32 {
    u32::try_from(duration.as_millis()).unwrap_or(u32::MAX)
}

/// Wall-clock deadline tracker across browser background throttling or sleep,
/// with an optional one-shot foreground grace window on resume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DeadlineTracker {
    deadline_ms: u64,
    resume_grace_ms: u32,
    grace_armed: bool,
}

impl DeadlineTracker {
    pub(super) fn new(now_ms: u64, duration: std::time::Duration, resume_grace_ms: u32) -> Self {
        Self {
            deadline_ms: now_ms.saturating_add(u64::from(duration_millis_u32(duration))),
            resume_grace_ms,
            grace_armed: false,
        }
    }

    pub(super) fn remaining_on_resume(&mut self, now_ms: u64) -> u32 {
        let remaining = self.deadline_ms.saturating_sub(now_ms);
        if remaining > 0 {
            return u32::try_from(remaining).unwrap_or(u32::MAX);
        }
        if self.grace_armed || self.resume_grace_ms == 0 {
            return 0;
        }
        self.grace_armed = true;
        self.deadline_ms = now_ms.saturating_add(u64::from(self.resume_grace_ms));
        self.resume_grace_ms
    }
}
